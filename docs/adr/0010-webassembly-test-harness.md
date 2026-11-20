# ADR 0010: WebAssembly test harness

- Status: Accepted
- Date: 2026-11-20
- Step: 1a.1 of the Phase 1 execution plan

## Context

PROJECT_PLAN §8 requires the conformance corpus and the tests to run on both the native and the WASM build. Phase 1a's exit requires "the WASM build passes the same tests" (execution plan X1.6). Before step 1a.1, CI only *built* every crate for `wasm32-unknown-unknown`. It never ran a test there.

Two problems surfaced when the existing tests were first run as WebAssembly under Node:
1. **Silent skips.** `wasm-bindgen-test-runner` runs only `#[wasm_bindgen_test]` functions. For plain `#[test]` it prints "no tests to run!" and exits successfully. The first run therefore reported success while running none of the workspace's tests.
2. **No file system.** In the browser target, `std::fs` fails at runtime. Every test that read a spec file or a repository file failed: the corpus, the OpenAPI examples and the §7 budget table.

## Decision

**Tests run on both targets from the same source.** Each test module and test file imports, for wasm32 only:

```rust
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;
```

so `#[test]` means `#[wasm_bindgen_test]` there. `wasm-bindgen-test` is a dev-dependency for the wasm32 target only.

**File access is portable.**
- **Repository files** (`specs.lock.json`, `PROJECT_PLAN.md`) are compiled in with `include_str!`. `sysml_corpus::LOCK` holds the lock.
- **Spec files** under `docs/specs/` are read with `sysml_corpus::read_spec(path)`, by their path in `specs.lock.json`. Natively it reads `docs/specs/` (or `EUREKA_SPECS_DIR`) as before. On wasm32, it serves copies that `sysml-corpus`'s build script embeds with `include_bytes!`: the corpus files, plus the few others listed in its `EXTRA`. The loader and the build script decide what a corpus file is through one shared module (`src/files.rs`), so they cannot disagree. The copies are generated into `OUT_DIR` from the local, never-committed specs (ADR 0004), so nothing is committed, and native builds embed nothing.
- **`Corpus::load()`** reads through `read_spec`, so it works on both targets. `Corpus::from_lock(lock, read)` replaces `Corpus::load_with(dir, lock)` and takes a reader instead of a folder.

**CI checks parity, not just success.** The `Rust (wasm tests)` job runs the workspace tests natively and as WebAssembly under Node, with `wasm-bindgen-test-runner` pinned to the `wasm-bindgen` version in `Cargo.lock`. `scripts/check_wasm_parity.py` then requires every test binary to pass the same number of tests on both targets. A test that compiles but is skipped on WASM therefore fails CI, with a message naming its file. The check strips the ANSI color codes cargo writes under `CARGO_TERM_COLOR=always`, takes each binary's crate from the last `build` folder in its path, and reports two binaries with the same key as an error. The piped test steps (`cargo test … | tee`) set `shell: bash`, which GitHub runs with `-eo pipefail`. A failing test therefore fails its step, and the parity check is not the only gate.

**The test runner, not `wasm-pack test`.** Execution plan step 1a.1 named `wasm-pack test --node`. `wasm-pack test` works one crate at a time and manages its own runner. `cargo test --workspace --target wasm32-unknown-unknown` with `wasm-bindgen-test-runner` runs the whole workspace in one command and was already proven in the 1a.0 spike. `wasm-pack` is still used, for what it is for: `wasm-pack build` packages `sysml-wasm` as an ES module (`--target web`), and a Node script calls its exports from JavaScript.

## Consequences

- A new test file needs the two-line import, or the parity check fails. A test that reads a file uses `read_spec` or `include_str!`. A spec file a test needs on WASM is added to `EXTRA` in `crates/sysml-corpus/build.rs`.
- The wasm32 build of `sysml-corpus` contains about 8 MB of embedded spec data. `sysml-corpus` is test infrastructure and is never shipped.
- Doc-tests run natively only.
- The WASM run takes about 18 seconds locally, and the CI job also builds the native tests for the parity comparison.
