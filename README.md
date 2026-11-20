# Eureka SysML

**Eureka SysML** is a free, open-source, multi-user web application for building **SysML v2** models. It supports both the textual and graphical notations and scales to very large models (1M+ elements). It is designed to be self-hosted, including in air-gapped and regulated environments.

> **Status: pre-alpha (Phase 0, foundations).** Nothing here is usable yet. See the [project plan](docs/proj-mgmt/PROJECT_PLAN.md) for the roadmap and staged releases.

## Standards

| Specification | Version |
|---|---|
| OMG Kernel Modeling Language (KerML) | 1.0 |
| OMG Systems Modeling Language (SysML) | 2.0 |
| OMG Systems Modeling API and Services | 1.0 (REST/HTTP PSM) |

## Architecture (planned)

- A **Rust core** (parser, semantics, validation, versioning), compiled natively for the server and to WebAssembly for the browser
- A **TypeScript web UI**: text editor, diagrams, tables and matrices
- **PostgreSQL** storage that follows the SysML v2 API & Services versioning model

The details are in the [project plan](docs/proj-mgmt/PROJECT_PLAN.md) §3–4.

## Getting started (developers)

Requirements:
- Rust (stable, installed through `rustup`; the toolchain is pinned in `rust-toolchain.toml`)
- Python 3.10+
- `pdftotext` (poppler-utils), optional, for spec text extraction

```sh
# 1. Fetch the specification files (not stored in git) and verify their checksums
python scripts/fetch_specs.py fetch

# 2. Optional: split the spec PDFs into per-clause text under docs/specs/text/
python scripts/extract_spec_text.py

# 3. Build and test
cargo test --workspace
```

## Repository layout

| Path | Contents |
|---|---|
| `crates/` | Rust crates (`sysml-model`: metaclasses generated from `SysML.json`; `sysml-api-types`: API records generated from `Schema.json`; …) |
| `scripts/` | Developer tools: spec fetching, spec text extraction, schema checks |
| `docs/proj-mgmt/` | Project plan and other project management documents |
| `docs/adr/` | Architecture decision records |
| `docs/specs/` | **Local only, never committed.** Fetched OMG specifications (see `specs.lock.json`). |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). All commits must be signed off (DCO).

## License

Apache License 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE). Bundled third-party material keeps its own license; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

OMG, SysML and KerML are trademarks of the Object Management Group. This project is not affiliated with or endorsed by OMG.
