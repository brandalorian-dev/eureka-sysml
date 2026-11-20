#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Generate and check the CycloneDX SBOMs of the shipped crates (PROJECT_PLAN §4.8, ADR 0008).

Usage:
  python scripts/sbom.py [--out DIR]   Generate into DIR (default target/sbom), then check.
  python scripts/sbom.py --check DIR   Only check the SBOMs already in DIR.

Needs Python 3.11+ and `cargo-cyclonedx` (CI pins the version). Writes one SBOM per shipped crate
and target, `<crate>_<target>.cdx.json` in CycloneDX 1.6 JSON, after removing any old SBOMs from
DIR.

What is shipped: workspace members that can be published. Crates with `publish = false`
(test infrastructure such as sysml-corpus, sysml-synth and sysml-bench) are not.

Each SBOM is checked against `cargo metadata` and Cargo.lock:
  - its components are exactly the crate's dependencies for that target (normal and build
    dependencies, not dev-dependencies), by name and version
  - dependencies needed only to build (build.rs, and what those need) have scope "excluded"; the
    rest have scope "required"
  - every component has a package URL and a license
  - every crates.io component carries the SHA-256 checksum recorded in Cargo.lock
  - no test-infrastructure crate appears, and no component is listed twice
  - no absolute file path appears, so the SBOM does not depend on the checkout location
  - DIR holds no other SBOM files

Reproducibility: the timestamp comes from SOURCE_DATE_EPOCH, or from the last commit when it is
not set. Workspace crates' bom-refs are Cargo package ids, which contain the checkout's absolute
path; they are rewritten relative to the workspace root. Every SBOM is generated twice and the two
must be byte-identical, so the same commit gives the same SBOM wherever it is checked out.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import urllib.parse
from pathlib import Path

if sys.version_info < (3, 11):
    sys.exit("scripts/sbom.py needs Python 3.11 or later (it reads Cargo.lock with tomllib).")
import tomllib  # noqa: E402  (after the version check, so older Pythons get a clear message)

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
                name = f"{key[0]} {key[1]}"
                if key in listed:
                    errors.append(f"{where}: {name} is listed more than once")
                listed[key] = c
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
            if "file:///" in path.read_text(encoding="utf-8"):
                errors.append(f"{where}: contains an absolute file path, so it depends on the checkout location")
            if len(errors) == before:
                print(f"{where}: {len(listed)} components")
    expected_files = {f"{c['name']}_{t}.cdx.json" for t in TARGETS for c in shipped_crates(metadata(t))}
    for stray in sorted(p.name for p in sbom_dir.glob("*.cdx.json") if p.name not in expected_files):
        errors.append(f"{stray}: not the SBOM of a shipped crate and target")
    return errors


def source_date_epoch() -> str:
    value = os.environ.get("SOURCE_DATE_EPOCH", "").strip()
    if not value:
        try:
            value = run(["git", "log", "-1", "--format=%ct"]).strip()
        except (OSError, subprocess.CalledProcessError):
            sys.exit("SOURCE_DATE_EPOCH is not set and git cannot give the last commit's time. "
                     "Set SOURCE_DATE_EPOCH (seconds since 1970) to generate a reproducible SBOM.")
    if not value.isdigit():
        sys.exit(f"SOURCE_DATE_EPOCH must be a number of seconds since 1970, not {value!r}.")
    return value


def normalize(path: Path) -> None:
    """Rewrite the checkout's absolute path in Cargo package ids (used as bom-refs) relative to
    the workspace root, for example `path+file:///…/eureka-sysml/crates/x#0.0.1` becomes
    `path+file:crates/x#0.0.1`."""
    text = path.read_text(encoding="utf-8")
    for root in {ROOT.as_posix(), urllib.parse.quote(ROOT.as_posix())}:
        text = text.replace(f"file://{root}/", "file:")
    path.write_text(json.dumps(json.loads(text), indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def generate_once(out: Path, epoch: str) -> list[str]:
    """Run cargo-cyclonedx for every target and move the shipped crates' SBOMs into `out`."""
    errors = []
    env = dict(os.environ, SOURCE_DATE_EPOCH=epoch)
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
            if p["name"] not in shipped:
                produced.unlink(missing_ok=True)  # test infrastructure: not shipped
            elif not produced.exists():
                found = sorted(str(f.relative_to(ROOT)) for f in ROOT.glob("crates/*/*.cdx.json"))
                errors.append(f"cargo-cyclonedx did not write {produced.relative_to(ROOT)} (found: {found})")
            else:
                shutil.move(produced, out / produced.name)
                normalize(out / produced.name)
    return errors


def generate(out: Path) -> list[str]:
    """Generate into `out`, a second time into a scratch folder, and require identical output."""
    epoch = source_date_epoch()
    out.mkdir(parents=True, exist_ok=True)
    for old in out.glob("*.cdx.json"):
        old.unlink()
    errors = generate_once(out, epoch)
    with tempfile.TemporaryDirectory() as scratch:
        errors += generate_once(Path(scratch), epoch)
        first = {f.name: f.read_bytes() for f in out.glob("*.cdx.json")}
        second = {f.name: f.read_bytes() for f in Path(scratch).glob("*.cdx.json")}
    for name in sorted(set(first) | set(second)):
        if first.get(name) != second.get(name):
            errors.append(f"{name}: two runs on the same commit differ, so the SBOM is not reproducible")
    return errors


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
        errors = generate(args.out)
    else:
        errors = []
    errors += check(args.check or args.out)
    for e in errors:
        print("ERROR:", e, file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
