# ADR 0008: CycloneDX SBOM of the shipped crates

- Status: Accepted
- Date: 2026-11-20

## Context

PROJECT_PLAN §4.8 requires "an SBOM (CycloneDX) for every release", alongside signed releases and reproducible builds where feasible. Phase 0 puts SBOM generation in CI. There are no releases or binaries yet, so the SBOM describes the shipped library crates and their dependencies. It grows with the product.

## Decision

**Tool.** `cargo-cyclonedx`, the CycloneDX project's own generator for Cargo. It is pinned to an exact version in CI and installed from a prebuilt binary with `taiki-e/install-action`. The output is CycloneDX 1.6 JSON.

**What is described.** One SBOM per shipped crate and per shipped target:
- Shipped crates are the workspace members that can be published. Crates with `publish = false` (`sysml-corpus`, `sysml-synth`, `sysml-bench`) are test infrastructure and are not described.
- The targets are `x86_64-unknown-linux-gnu` (the server) and `wasm32-unknown-unknown` (the browser's WASM core). Their dependencies differ: on wasm32, `uuid` brings in `wasm-bindgen` and its helper crates.
- Dependencies used only to build (`build.rs` code generation and what it needs) are listed with scope `excluded`. Dev-dependencies are left out.

**Checks.** `scripts/sbom.py` generates the SBOMs and then checks each one against `cargo metadata` and `Cargo.lock`:
- the components are exactly the crate's dependencies for that target, and each has the right scope
- every component has a package URL and a license
- every crates.io component carries the SHA-256 recorded in `Cargo.lock`
- no test-infrastructure crate appears, and no component is listed twice
- no absolute file path appears
- the output folder holds no other SBOM files; old ones are removed before generating

So an SBOM cannot silently drift from what is built. Licenses themselves are checked by `cargo deny` against `deny.toml`.

**Reproducibility.** The same commit produces byte-identical SBOMs wherever it is checked out:
- The timestamp comes from `SOURCE_DATE_EPOCH`, or from the last commit's time when it is not set. Without either (for example in a source tarball), the script stops with a message.
- `cargo-cyclonedx` uses Cargo package ids as `bom-ref` values. For workspace crates these contain the checkout's absolute path (`path+file:///…/crates/sysml-model#0.0.1`), which would also leak local paths. The script rewrites them relative to the workspace root (`path+file:crates/sysml-model#0.0.1`), and the check rejects any remaining absolute path.
- `cargo-cyclonedx` 0.5.10 writes no `serialNumber`. To catch a random serial number, or any other random value, in a future version, the script generates every SBOM twice and requires the two to be identical.

This was verified by generating from a second checkout of the same commit at another path (one containing a space): the SBOMs were identical.

The script needs Python 3.11 or later, for `tomllib`. Python 3.10 reached its end of life in October 2026, and the repository's documented minimum is now 3.11.

**CI.** The `sbom` job runs the script on every push and pull request and uploads the SBOMs as the `sbom-<commit>` build artifact.

## Consequences

- The release workflow (Phase 10) attaches the SBOMs of the release commit and signs them with Sigstore/cosign, together with the binaries and images.
- New shipped binaries (the `eureka` CLI and `eureka-server`, Phases 1b and 2) are described automatically when they become workspace members. Adding a server platform means adding its target triple to `TARGETS` in `scripts/sbom.py`.
- The web client (Phase 3) needs its own SBOM for the npm dependencies, for example from `@cyclonedx/cyclonedx-npm`.
- Bundled third-party material that is not a Cargo dependency, such as the EPL-2.0 standard libraries (PROJECT_PLAN §13), has to be added to the release SBOM when it is shipped.
