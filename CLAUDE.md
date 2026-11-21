# CLAUDE.md: working on Eureka SysML

The project plan is `docs/proj-mgmt/PROJECT_PLAN.md`. Read §4 (architecture) and §6 (phases) before starting a feature.

## Hard rules

- **Never commit anything under `docs/specs/`.** It holds the OMG specifications, which are fetched locally and listed in `specs.lock.json`. CI fails if any file there is tracked.
- **Never commit generated code.** Code generated from the spec files is produced at build time (for example, `crates/sysml-model/build.rs`).
- **Spec behavior must cite its clause** in a comment, for example `// KerML 8.4.4.3` or `// API PIM 7.1.2`. If the spec is ambiguous, write down the interpretation in an ADR (`docs/adr/`).
- **Test first, from the spec.** Write failing tests from the spec rules (library corpus, implied-relationship tables, API Annex A cases) before implementing.
- **No reuse of OMG pilot implementation code.** The pilots may be used only as behavioral references.
- Commits are signed off (`git commit -s`). Keep changes small: one issue per branch and PR.

## Spec lookup

Run `python scripts/fetch_specs.py fetch`, then `python scripts/extract_spec_text.py` and `python scripts/extract_spec_snippets.py`. After that, the specs are available locally:

| Need | Look in |
|---|---|
| One clause of a spec | `docs/specs/text/<kerml\|sysml-part1\|sysml-part2\|api>/<clause>-<title>.md`. `INDEX.md` in each folder lists every clause. |
| Metamodel (JSON schema) | `docs/specs/normative/SysML.json` (a superset of `KerML/normative/KerML.json`) |
| Standard libraries | `docs/specs/KerML/normative/*.kpar` (KerML) and `docs/specs/normative/*.kpar` (SysML), which are ZIP files of `.kerml`/`.sysml` sources |
| API | `docs/specs/api/omg/normative/OpenAPI.json`, `Schema.json` |
| Example model | `docs/specs/informative/ptc-25-04-31.sysml` (SimpleVehicleModel) |
| Spec examples (snippets) | `docs/specs/snippets/<kerml\|sysml>/<clause>-<n>.<ext>` and `index.json`, from `extract_spec_snippets.py` (ADR 0016) |
| Library element UUIDs (test oracle) | `docs/specs/SysML-v2-Release/2025-02/sysml.library.xmi/` (`elementId` attributes; ADR 0009). Root namespace ids are not normative. |

Commonly needed clauses:
- KerML 8.2: textual grammar
- KerML 8.4.2–8.4.4: semantic constraints and implied relationships
- KerML 10: interchange (`.kpar`, JSON)
- KerML 9.1: library UUIDs
- SysML 8.2.2: textual notation
- SysML 8.2.3: graphical notation
- SysML 9.2.20: standard views
- API 7.1.2: versioning rules
- API Annex A: conformance tests

## Environment

Development happens in WSL2 (Ubuntu 24.04); CI runs on `ubuntu-24.04`, pinned for its poppler 24.02, which the snippet extraction checks (ADR 0016).
- Keep the repo on the Linux filesystem (`~/code/eureka-sysml`), not under `/mnt/c`. Builds there are much faster, and file modes and case sensitivity behave correctly.
- The git config must use Linux values: `core.filemode=true`, `core.symlinks=true`, `core.ignorecase=false`, `core.autocrlf=false`. `.gitattributes` enforces LF line endings, so never write CRLF.
- `python` is Python 3 (via `python-is-python3`); `python3` also works.
- Rust comes from `rustup`, and `rust-toolchain.toml` pins stable with rustfmt, clippy and `wasm32-unknown-unknown`.
- `pdftotext` and `pdftohtml` (`poppler-utils`) are needed for `extract_spec_text.py` and `extract_spec_snippets.py`.
- Installed: `cargo-deny` (run `cargo deny check` before a PR), `gh` (run `gh auth login` once before PR work), Node and pnpm (for `apps/web`), and Docker.
- `wasm-bindgen-cli` 0.2.130 (`wasm-bindgen-test-runner`) runs WASM tests under Node. It must match the `wasm-bindgen` crate's version exactly, and it does not accept `cargo test -q`: `CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner cargo test --target wasm32-unknown-unknown`.
- Not installed yet: `wasm-pack` (`cargo install --locked wasm-pack`, needed in Phase 1a step 1a.1) and PostgreSQL. For the `sysml-store` integration tests, run PostgreSQL in Docker rather than installing it in WSL, to match the CI service container.
- There is no `rustfmt.toml`, so rustfmt's default width of 100 applies.

## Commands

```sh
python scripts/fetch_specs.py fetch|verify|status   # spec files (lock maintainers: `lock`)
python scripts/check_schema_assumptions.py           # validate codegen assumptions
python scripts/extract_spec_text.py                  # per-clause spec text
python scripts/extract_spec_snippets.py              # spec examples into docs/specs/snippets (ADR 0016)
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --target wasm32-unknown-unknown
cargo test --workspace --bench '*'                   # run each benchmark once (as CI does)
cargo bench -p sysml-bench                           # measure the benchmarks
python scripts/check_uuid_oracle.py                  # the UUID oracle still follows KerML 9.1
python -m unittest discover -s scripts/tests         # tests of the Python scripts
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  cargo test --workspace --target wasm32-unknown-unknown --no-fail-fast   # every test as WebAssembly (ADR 0010)
python scripts/check_wasm_parity.py NATIVE_LOG WASM_LOG   # same tests pass on both (CI does this)
wasm-pack build crates/sysml-wasm --target web --out-dir ../../target/wasm-pkg/sysml-wasm && node scripts/smoke_wasm_pkg.mjs
python scripts/sbom.py                               # CycloneDX SBOMs into target/sbom (needs cargo-cyclonedx)
cargo deny check                                     # licenses, advisories, sources (as in CI)
```

`build.rs` reads the specs from `docs/specs/`. Set `EUREKA_SPECS_DIR` to use a copy somewhere else.

## Architecture in one paragraph

A Rust core in `crates/` compiles natively (for the server) and to WASM (for the browser):
- `sysml-syntax`: lossless, error-tolerant lexer and parser (ADR 0011; a leading byte-order mark is trivia, ADR 0013) and the `Diagnostic` type (ADR 0012). Since step 1a.4: the whole KerML grammar (8.2.3–8.2.5), with expressions as a Pratt parser over Table 6 (ADRs 0014, 0015). Grammar tests use inline `expect-test` snapshots: `UPDATE_EXPECT=1 cargo test -p sysml-syntax`, then review the diff. Long fuzz run: `cargo test -p sysml-syntax --test fuzz -- --ignored` (`FUZZ_SECONDS`, `FUZZ_SEED`)
- `sysml-model`: generated metaclasses, and the arena element graph (ADR 0017): dense `ElementIx`, a UUID map, the stored core, and typed views of the 76 stored KerML attributes generated from `KerML.xmi`
- `sysml-api-types`: API records (Project, Commit, DataVersion, Query, …) generated from the API `Schema.json`; spec corrections in ADR 0005
- `sysml-synth` (test infrastructure): deterministic synthetic SysML models at any size (`Config::with_target_elements`), for benchmarks and scale tests
- `sysml-wasm`: the `wasm-bindgen` interface the browser worker calls; a skeleton since step 1a.1 (ADR 0010)
- `sysml-bench` (test infrastructure): the PROJECT_PLAN §7 budgets as data (`BUDGETS`, kept equal to the plan by a test) and the `divan` benchmarks; ADR 0007
- `sysml-corpus` (test infrastructure): the standard libraries and example models from `docs/specs/`. Corpus tests use `sysml_corpus::Corpus::load()` and iterate `sources()`; library archive errata in ADR 0006. The spec snippets come from `sysml_corpus::snippets()`, with their expectations in `crates/sysml-corpus/snippets.json` (ADR 0016)
- `sysml-semantics`: since step 1a.6, the lowering from syntax trees to the graph (`lower`, ADR 0018), tested element by element against the pilot's library XMI (`tests/oracle.rs`). Since step 1a.7, name resolution on `salsa` queries (`resolve.rs`, ADR 0019), with the implied specializations and redefinitions it needs (`implied.rs`, ADR 0020), tested name by name against the pilot (`tests/resolve_oracle.rs`). Queries read `structure` (span-free), not `lowered`, so a layout edit stops early. The rest of the implied relationships follow in step 1a.9
- `sysml-versioning`: API PIM commit, diff and merge rules
- `sysml-store`: PostgreSQL
- `sysml-server`: axum, the REST PSM, and `/x/…` extensions

The TypeScript UI (`apps/web`, from Phase 3) talks to the server and to the WASM worker. PostgreSQL stores element versions as JSONB in the KerML 10.4 JSON format. Element IDs are API DataIdentity IDs.

## Conventions

- Rust edition 2021, `unsafe` forbidden, clippy warnings are errors in CI.
- Licenses: our code is Apache-2.0, and dependency licenses must pass `deny.toml`. Each new file starts with `SPDX-License-Identifier: Apache-2.0`.
- Python scripts use only the standard library unless there is a strong reason.
- **Every test also runs as WebAssembly (ADR 0010).** Each test module and test file starts with `#[cfg(target_arch = "wasm32")] use wasm_bindgen_test::wasm_bindgen_test as test;`, or CI's parity check fails. Tests read spec files with `sysml_corpus::read_spec(path)` and repository files with `include_str!`, never `std::fs`, because wasm32 has no file system. A spec file a test needs on WASM is embedded by `crates/sysml-corpus/build.rs`: the corpus files, the KerML library XMI (`is_kerml_library_xmi`), the extracted snippets, and any other file listed in `EXTRA`.
