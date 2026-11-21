#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Manage the local, never-committed specification folder `docs/specs/`.

The OMG specification documents and machine-readable files are NOT stored in git
(see docs/proj-mgmt/PROJECT_PLAN.md §13). Instead `specs.lock.json` at the repository
root records every file the build and tests rely on: its path, size, SHA-256 and,
where one is known, a pinned download URL.

Commands:
  status   Show which files are present, missing or modified.
  verify   Exit non-zero unless every file is present with the expected checksum.
  fetch    Download every missing file that has a URL, then verify. Files without a
           URL must be downloaded by hand from their landing page.
  lock     (Maintainers only) Rewrite the sizes and checksums in specs.lock.json from
           the files currently in docs/specs/. Existing URLs and landing pages are kept.
           New files are added without a URL.

Only the standard library is used, so the script runs before any toolchain is installed.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPECS = ROOT / "docs" / "specs"
LOCK = ROOT / "specs.lock.json"
# Files that are never tracked (OS/editor clutter, derived text).
IGNORED_NAMES = {".DS_Store", "Thumbs.db", "desktop.ini", "README-SOURCE.md"}
IGNORED_SUFFIXES = {".url"}  # Windows internet shortcuts
# Derived from the fetched files, never locked: docs/specs/text/ by extract_spec_text.py and
# docs/specs/snippets/ by extract_spec_snippets.py (ADR 0016).
IGNORED_DIRS = {"text", "snippets"}


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def load_lock() -> dict:
    if not LOCK.exists():
        return {"version": 1, "files": []}
    return json.loads(LOCK.read_text(encoding="utf-8"))


def save_lock(lock: dict) -> None:
    lock["files"].sort(key=lambda e: e["path"])
    LOCK.write_text(json.dumps(lock, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def safe_target(rel: str) -> Path:
    """Resolve a manifest path and refuse anything that escapes docs/specs/."""
    target = (SPECS / rel).resolve()
    if SPECS.resolve() not in target.parents:
        raise SystemExit(f"refusing path outside docs/specs/: {rel}")
    return target


def check(entry: dict) -> str:
    p = safe_target(entry["path"])
    if not p.exists():
        return "missing"
    if p.stat().st_size != entry["size"] or sha256(p) != entry["sha256"]:
        return "modified"
    return "ok"


def cmd_status(lock: dict) -> int:
    counts: dict[str, int] = {}
    for e in lock["files"]:
        s = check(e)
        counts[s] = counts.get(s, 0) + 1
        if s != "ok":
            hint = e.get("url") or f"manual download: {e.get('landing', '?')}"
            print(f"{s:9} {e['path']}  ({hint})")
    print(", ".join(f"{k}: {v}" for k, v in sorted(counts.items())) or "no files in lock")
    return 0


def cmd_verify(lock: dict) -> int:
    bad = [(e["path"], s) for e in lock["files"] if (s := check(e)) != "ok"]
    for path, s in bad:
        print(f"{s:9} {path}", file=sys.stderr)
    if bad:
        print(f"{len(bad)} spec file(s) not verified. Run `python scripts/fetch_specs.py fetch`.", file=sys.stderr)
        return 1
    print(f"all {len(lock['files'])} spec files verified")
    return 0


# Transient upstream failures are retried: HTTP 5xx (including Cloudflare's 52x, seen from omg.org)
# and 429, connection errors and timeouts. Other HTTP errors and checksum mismatches are not.
ATTEMPTS = 4
RETRY_DELAYS = (5, 15, 45)  # seconds before the 2nd, 3rd and 4th attempt


def is_transient(err: Exception) -> bool:
    if isinstance(err, urllib.error.HTTPError):
        return err.code >= 500 or err.code == 429
    return isinstance(err, (urllib.error.URLError, TimeoutError, ConnectionError))


def download_once(url: str, dest: Path) -> Path:
    """Download `url` into a temporary file next to `dest`, and return its path."""
    req = urllib.request.Request(url, headers={"User-Agent": "eureka-sysml-fetch-specs"})
    tmp = tempfile.NamedTemporaryFile(delete=False, dir=dest.parent)
    try:
        with tmp, urllib.request.urlopen(req, timeout=120) as resp:
            for chunk in iter(lambda: resp.read(1 << 20), b""):
                tmp.write(chunk)
    except BaseException:
        Path(tmp.name).unlink(missing_ok=True)  # never leave a partial download behind
        raise
    return Path(tmp.name)


def download(url: str, dest: Path, expected_sha: str) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    for attempt in range(1, ATTEMPTS + 1):
        try:
            tmp_path = download_once(url, dest)
            break
        except Exception as err:
            if attempt == ATTEMPTS or not is_transient(err):
                raise
            delay = RETRY_DELAYS[attempt - 1]
            print(f"  attempt {attempt} failed ({err}); retrying in {delay} s", file=sys.stderr)
            time.sleep(delay)
    got = sha256(tmp_path)
    if got != expected_sha:
        tmp_path.unlink()
        raise RuntimeError(f"checksum mismatch for {url}: got {got}")
    tmp_path.replace(dest)


def cmd_fetch(lock: dict) -> int:
    manual = []
    for e in lock["files"]:
        if check(e) == "ok":
            continue
        if not e.get("url"):
            manual.append(e)
            continue
        print(f"fetching {e['path']}")
        try:
            download(e["url"], safe_target(e["path"]), e["sha256"])
        except Exception as ex:  # report and continue with the other files
            print(f"  FAILED: {ex}", file=sys.stderr)
    if manual:
        print("\nThese files have no pinned URL. Download them from their landing page into docs/specs/:")
        for e in manual:
            print(f"  {e['path']}\n      from {e.get('landing', '?')}")
    return cmd_verify(lock)


def cmd_lock(lock: dict) -> int:
    if not SPECS.is_dir():
        raise SystemExit("docs/specs/ does not exist")
    by_path = {e["path"]: e for e in lock["files"]}
    seen = set()
    for p in sorted(SPECS.rglob("*")):
        if not p.is_file() or p.name in IGNORED_NAMES or p.suffix in IGNORED_SUFFIXES:
            continue
        rel = p.relative_to(SPECS).as_posix()
        if rel.split("/")[0] in IGNORED_DIRS:
            continue
        seen.add(rel)
        e = by_path.setdefault(rel, {"path": rel, "url": None})
        e["size"] = p.stat().st_size
        e["sha256"] = sha256(p)
    removed = set(by_path) - seen
    for rel in removed:
        print(f"dropping (no longer present): {rel}")
        del by_path[rel]
    lock["files"] = list(by_path.values())
    save_lock(lock)
    print(f"wrote {LOCK.name} with {len(lock['files'])} entries")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command", choices=["status", "verify", "fetch", "lock"])
    args = ap.parse_args()
    lock = load_lock()
    return {"status": cmd_status, "verify": cmd_verify, "fetch": cmd_fetch, "lock": cmd_lock}[args.command](lock)


if __name__ == "__main__":
    sys.exit(main())
