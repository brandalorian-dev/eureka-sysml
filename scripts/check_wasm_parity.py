#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Check that every test binary passes the same tests natively and as WebAssembly (step 1a.1).

Usage: python scripts/check_wasm_parity.py NATIVE_LOG WASM_LOG

The logs are the output of `cargo test --workspace --no-fail-fast` and of the same command with
`--target wasm32-unknown-unknown` and `wasm-bindgen-test-runner`. For each test binary (crate and
target), the number of passed tests must be equal.

This guards against the silent failure mode of WASM tests: `wasm-bindgen-test-runner` runs only
`#[wasm_bindgen_test]` functions and reports "no tests to run!" for plain `#[test]`, which looks
like success. Doc-tests do not run on WASM and are ignored.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

RUNNING = re.compile(r"^\s*Running (\S+(?: \S+)?) \((\S+)\)")
RESULT = re.compile(r"^test result: \w+\. (\d+) passed;")


def binary_key(target: str, path: str) -> tuple[str, str]:
    """(crate, target), with the crate taken from the build directory (target/…/build/<crate>/…)."""
    parts = path.replace("\\", "/").split("/")
    crate = parts[parts.index("build") + 1] if "build" in parts else Path(path).name.split("-")[0]
    return crate, target


def parse(log: str) -> dict[tuple[str, str], int]:
    """Passed-test counts per test binary. A binary with "no tests to run!" counts 0."""
    counts: dict[tuple[str, str], int] = {}
    current = None
    for line in log.splitlines():
        if m := RUNNING.match(line):
            current = binary_key(m.group(1), m.group(2))
            counts[current] = 0
        elif line.strip().startswith("Doc-tests "):
            current = None
        elif current is not None and (m := RESULT.match(line.strip())):
            counts[current] = int(m.group(1))
            current = None
    return counts


def compare(native: dict, wasm: dict) -> list[str]:
    if not native:
        return ["no test binaries found in the native log"]
    errors = []
    for key, n in sorted(native.items()):
        name = f"{key[0]} {key[1]}"
        if key not in wasm:
            errors.append(f"{name}: did not run on WASM ({n} natively)")
        elif wasm[key] != n:
            errors.append(f"{name}: {n} natively, {wasm[key]} on WASM")
    for key in sorted(set(wasm) - set(native)):
        errors.append(f"{key[0]} {key[1]}: ran on WASM only")
    return errors


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    native = parse(Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace"))
    wasm = parse(Path(sys.argv[2]).read_text(encoding="utf-8", errors="replace"))
    errors = compare(native, wasm)
    for e in errors:
        print("ERROR:", e, file=sys.stderr)
    if not errors:
        total = sum(native.values())
        print(f"{len(native)} test binaries, {total} tests: the same tests pass natively and on WASM")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
