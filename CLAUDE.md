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

Run `python scripts/fetch_specs.py fetch` and then `python scripts/extract_spec_text.py`. After that, the specs are available locally:

| Need | Look in |
|---|---|
| One clause of a spec | `docs/specs/text/<kerml\|sysml-part1\|sysml-part2\|api>/<clause>-<title>.md`. `INDEX.md` in each folder lists every clause. |
| Metamodel (JSON schema) | `docs/specs/normative/SysML.json` (a superset of `KerML/normative/KerML.json`) |
| Standard libraries | `docs/specs/KerML/normative/*.kpar` (KerML) and `docs/specs/normative/*.kpar` (SysML), which are ZIP files of `.kerml`/`.sysml` sources |
| API | `docs/specs/api/omg/normative/OpenAPI.json`, `Schema.json` |
| Example model | `docs/specs/informative/ptc-25-04-31.sysml` (SimpleVehicleModel) |

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

Development happens in WSL2 (Ubuntu); CI runs on `ubuntu-latest`.
- Keep the repo on the Linux filesystem (`~/code/eureka-sysml`), not under `/mnt/c`. Builds there are much faster, and file modes and case sensitivity behave correctly.
- The git config must use Linux values: `core.filemode=true`, `core.symlinks=true`, `core.ignorecase=false`, `core.autocrlf=false`. `.gitattributes` enforces LF line endings, so never write CRLF.
- `python` is Python 3 (via `python-is-python3`); `python3` also works.
- Rust comes from `rustup`, and `rust-toolchain.toml` pins stable with rustfmt, clippy and `wasm32-unknown-unknown`.
- `pdftotext` (`poppler-utils`) is needed for `extract_spec_text.py`.
- Installed: `cargo-deny` (run `cargo deny check` before a PR), `gh` (run `gh auth login` once before PR work), Node and pnpm (for `apps/web`), and Docker.
- Not installed yet: `wasm-pack` (`cargo install --locked wasm-pack`, needed for the Phase 0 wasm-pack CI build) and PostgreSQL. For the `sysml-store` integration tests, run PostgreSQL in Docker rather than installing it in WSL, to match the CI service container.
- There is no `rustfmt.toml`, so rustfmt's default width of 100 applies.

## Commands

```sh
python scripts/fetch_specs.py fetch|verify|status   # spec files (lock maintainers: `lock`)
python scripts/check_schema_assumptions.py           # validate codegen assumptions
python scripts/extract_spec_text.py                  # per-clause spec text
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --target wasm32-unknown-unknown
cargo deny check                                     # licenses, advisories, sources (as in CI)
```

`build.rs` reads the specs from `docs/specs/`. Set `EUREKA_SPECS_DIR` to use a copy somewhere else.

## Architecture in one paragraph

A Rust core in `crates/` compiles natively (for the server) and to WASM (for the browser):
- `sysml-syntax`: lossless, error-tolerant parser
- `sysml-model`: generated metaclasses plus an arena element graph
- `sysml-semantics`: salsa-based incremental resolution and implied relationships
- `sysml-versioning`: API PIM commit, diff and merge rules
- `sysml-store`: PostgreSQL
- `sysml-server`: axum, the REST PSM, and `/x/…` extensions

The TypeScript UI (`apps/web`, from Phase 3) talks to the server and to the WASM worker. PostgreSQL stores element versions as JSONB in the KerML 10.4 JSON format. Element IDs are API DataIdentity IDs.

## Conventions

- Rust edition 2021, `unsafe` forbidden, clippy warnings are errors in CI.
- Licenses: our code is Apache-2.0, and dependency licenses must pass `deny.toml`. Each new file starts with `SPDX-License-Identifier: Apache-2.0`.
- Python scripts use only the standard library unless there is a strong reason.
