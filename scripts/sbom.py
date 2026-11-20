#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Generate and check the CycloneDX SBOMs of the shipped crates (PROJECT_PLAN §4.8, ADR 0008).

Usage:
  python scripts/sbom.py [--out DIR]   Generate into DIR (default target/sbom), then check.
  python scripts/sbom.py --check DIR   Only check the SBOMs already in DIR.

Needs `cargo-cyclonedx` (CI pins the version). Writes one SBOM per shipped crate and target:
`<crate>_<target>.cdx.json`, CycloneDX 1.6 JSON.

What is shipped: workspace members that can be published. Crates with `publish = false`
(test infrastructure such as sysml-corpus, sysml-synth and sysml-bench) are not.

Each SBOM is checked against `cargo metadata` and Cargo.lock:
  - its components are exactly the crate's dependencies for that target (normal and build
    dependencies, not dev-dependencies), by name and version
  - dependencies needed only to build (build.rs, and what those need) have scope "excluded"; the
    rest have scope "required"
  - every component has a package URL and a license
  - every crates.io component carries the SHA-256 checksum recorded in Cargo.lock
  - no test-infrastructure crate appears

The timestamp comes from SOURCE_DATE_EPOCH, or from the last commit when it is not set, so the
same commit always produces the same SBOM.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPEC_VERSION = "1.6"
# The platforms the product ships for: the server, and the browser (the WASM core).
TARGETS = ["x86_64-unknown-linux-gnu", "wasm32-unknown-unknown"]


def run(cmd: list[str], **kw) -> str:
    return subprocess.run(cmd, cwd=ROOT, check=True, capture_output=True, text=True, **kw).stdout


def metadata(target: str) -> dict:
    return json.loads(run(["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", target]))


def shipped_crates(meta: dict) -> list[dict]:
    """Workspace members that can be published. `publish = false` appears as an empty list."""
    members = set(meta["workspace_members"])
    return [p for p in meta["packages"] if p["id"] in members and p["publish"] != []]


def test_infrastructure(meta: dict) -> set[str]:
    members = set(meta["workspace_members"])
    return {p["name"] for p in meta["packages"] if p["id"] in members and p["publish"] == []}


def dependencies(meta: dict, root_id: str) -> dict[str, str]:
    """The crate's dependencies for this target: package id -> scope ("required"/"excluded").

    A package is required if it is reached through normal dependencies only. Anything reached
    through a build-dependency is needed only to build, so it is excluded (unless it is also
    required on another path). Dev-dependencies are not followed.
    """
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}

    def walk(include_build: bool) -> set[str]:
        seen, stack = set(), [root_id]
        while stack:
            for d in nodes[stack.pop()]["deps"]:
                kinds = {k["kind"] for k in d["dep_kinds"]}
                if None in kinds or (include_build and "build" in kinds):
                    if d["pkg"] not in seen:
                        seen.add(d["pkg"])
                        stack.append(d["pkg"])
        return seen - {root_id}

    required = walk(include_build=False)
    return {pkg: ("required" if pkg in required else "excluded") for pkg in walk(include_build=True)}


def lock_checksums() -> dict[tuple[str, str], str]:
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))
    return {(p["name"], p["version"]): p["checksum"] for p in lock.get("package", []) if "checksum" in p}


def check(sbom_dir: Path) -> list[str]:
    errors = []
    sums = lock_checksums()
    for target in TARGETS:
        meta = metadata(target)
        packages = {p["id"]: p for p in meta["packages"]}
        infra = test_infrastructure(meta)
        for crate in shipped_crates(meta):
            path = sbom_dir / f"{crate['name']}_{target}.cdx.json"
            where = path.name
            if not path.exists():
                errors.append(f"{where}: missing")
                continue
            bom = json.loads(path.read_text(encoding="utf-8"))
            before = len(errors)
            if bom.get("bomFormat") != "CycloneDX" or bom.get("specVersion") != SPEC_VERSION:
                errors.append(f"{where}: not a CycloneDX {SPEC_VERSION} BOM")
            top = bom.get("metadata", {}).get("component", {})
            if (top.get("name"), top.get("version")) != (crate["name"], crate["version"]):
                errors.append(f"{where}: describes {top.get('name')} {top.get('version')}")

            expected = {
                (packages[i]["name"], packages[i]["version"]): scope
                for i, scope in dependencies(meta, crate["id"]).items()
            }
            listed = {}
            for c in bom.get("components", []):
                key = (c.get("name"), c.get("version"))
                listed[key] = c
                name = f"{key[0]} {key[1]}"
                if not c.get("purl", "").startswith(f"pkg:cargo/{key[0]}@{key[1]}"):
                    errors.append(f"{where}: {name}: package URL {c.get('purl')!r}")
                if not c.get("licenses"):
                    errors.append(f"{where}: {name}: no license")
                if key[0] in infra:
                    errors.append(f"{where}: {name} is test infrastructure and is not shipped")
                if key in sums:
                    hashes = {h.get("alg"): h.get("content") for h in c.get("hashes", [])}
                    if hashes.get("SHA-256") != sums[key]:
                        errors.append(f"{where}: {name}: SHA-256 does not match Cargo.lock")
                if key in expected and c.get("scope") != expected[key]:
                    errors.append(f"{where}: {name}: scope {c.get('scope')!r}, expected {expected[key]!r}")
            for key in sorted(set(expected) - set(listed)):
                errors.append(f"{where}: dependency {key[0]} {key[1]} is missing")
            for key in sorted(set(listed) - set(expected)):
                errors.append(f"{where}: {key[0]} {key[1]} is not a dependency for {target}")
            if len(errors) == before:
                print(f"{where}: {len(listed)} components")
    return errors


def generate(out: Path) -> None:
    env = dict(os.environ)
    env.setdefault("SOURCE_DATE_EPOCH", run(["git", "log", "-1", "--format=%ct"]).strip())
    out.mkdir(parents=True, exist_ok=True)
    for target in TARGETS:
        meta = metadata(target)
        cmd = ["cargo", "cyclonedx", "--format", "json", "--spec-version", SPEC_VERSION,
               "--target", target, "--target-in-filename", "--quiet"]
        subprocess.run(cmd, cwd=ROOT, check=True, env=env)
        shipped = {p["name"] for p in shipped_crates(meta)}
        members = set(meta["workspace_members"])
        for p in meta["packages"]:
            if p["id"] not in members:
                continue
            produced = Path(p["manifest_path"]).parent / f"{p['name']}_{target}.cdx.json"
            if p["name"] in shipped:
                shutil.move(produced, out / produced.name)
            else:
                produced.unlink(missing_ok=True)  # test infrastructure: not shipped


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    group = ap.add_mutually_exclusive_group()
    group.add_argument("--out", type=Path, default=ROOT / "target" / "sbom")
    group.add_argument("--check", type=Path, metavar="DIR")
    args = ap.parse_args()
    if args.check is None:
        if shutil.which("cargo-cyclonedx") is None:
            print("cargo-cyclonedx not found. Install it with `cargo install --locked cargo-cyclonedx`.", file=sys.stderr)
            return 1
        generate(args.out)
    errors = check(args.check or args.out)
    for e in errors:
        print("ERROR:", e, file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
