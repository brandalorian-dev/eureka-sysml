# ADR 0006: Test corpus and errata in the OMG library archives

- Status: Accepted
- Date: 2026-11-20

## Context

PROJECT_PLAN §8 makes a conformance corpus the acceptance gate. The `sysml-corpus` crate loads it from the local `docs/specs/`: the 10 standard libraries as project interchange files (`.kpar`, KerML 10.3) and the example models (SimpleVehicleModel). The OMG 20250201 library archives do not follow KerML 10.3 exactly, so the harness has to interpret them.

## Decision

**What is in the corpus.** Every `.kpar` and every `.kerml`/`.sysml` file listed in `specs.lock.json`. The lock file is the single list. The completeness test classifies lock entries independently of the loader: documents, metamodel and schema files, and API pilot material are not models, and every other entry must be in the corpus. The "Annex A examples" of PROJECT_PLAN §6 and §8 are SysML Part 1 Annex A, "Example Model": the SimpleVehicleModel, whose complete machine-readable form (`informative/ptc-25-04-31.sysml`) is in the corpus. The excerpts printed in the annex are spec snippets. Textual notation snippets from the specification text, those excerpts included, are a later addition (Phase 1a), because the extracted text does not reliably separate code from prose. API Annex A (OCL test cases for the API, not models) belongs to the API conformance suite (Phase 2), not to this corpus.

*Amended 2026-11-20:* the first version of this ADR read "Annex A examples" as API Annex A and deferred them to Phase 2. They are SysML Annex A, and the corpus already holds its model.

**Integrity.** Every corpus file is checked against its SHA-256 in `specs.lock.json` when it is loaded, so a modified archive or model fails with a clear error. `EUREKA_SPECS_DIR` must therefore hold the files that the repository's lock pins. A lock entry without a `path`, a library without a `url` (its usage IRI) and a file without a `sha256` are errors.

**Project root.** KerML 10.3 places `.project.json` and `.meta.json` "at its top level", and Table 13 note 2 makes paths relative to the archive root. Every OMG archive instead wraps the project in a single folder (for example `Kernel Semantic Library/.meta.json`), with index paths relative to that folder. When every entry is inside one top-level folder, that folder is the project root.

**Operating-system metadata.** Archive entries named `.DS_Store`, `Thumbs.db` or `desktop.ini`, and anything under `__MACOSX/`, are ignored and listed in `Library::ignored`. The Systems Library contains two such entries. JSON (`.json`) and XMI (`.xmi`) model interchange files are allowed by KerML 10.2. They are recorded in `Library::interchange_files` but not loaded as sources; the OMG archives contain none. Any other entry is an error. An entry larger than 16 MiB is an error too, so a malformed or hostile archive cannot exhaust memory; the largest library file is about 150 KB.

**Checksums.** `.meta.json` checksums are verified when present. KerML 10.3 Table 13 describes a list of checksum objects per file, while the normative `KerML-Model-Interchange.json` defines a single object; both forms are read. Only `SHA256` is supported, and any other algorithm is an error rather than being skipped. The OMG archives have no checksums.

**Errata.** These defects are corrected by the `ERRATA` table in `sysml-corpus`:

| Library | Defect | Correction |
|---|---|---|
| SysML Systems Library | Index entry `AnalysisCases` names `AnalysisCase.sysml` | `AnalysisCases.sysml` |
| SysML Requirement Derivation Library | Index entry `RequirementsDerivation` names `RequirementsDerivation.sysml`; the file is `RequirementDerivation.sysml` and declares `package RequirementDerivation` | Both the name and the file are corrected |
| SysML Analysis Library | Usage `…/KerML/20250201/Quantities-and-Units-Library.kpar` | `…/SysML/20250201/Quantities-and-Units-Domain-Library.kpar` |
| SysML Geometry Library | Usage `…/SysML/20250201/Quantities-and-Units-Library.kpar` | `…/SysML/20250201/Quantities-and-Units-Domain-Library.kpar` |

A library's usage IRI is its pinned download URL in `specs.lock.json`. Every other usage matches one exactly. A usage correction applies only to the library it is recorded for, so the same broken IRI in another library is reported, not corrected.

The tests assert that the set of defects found equals the errata table. When OMG publishes corrected archives, the test fails and the entry is removed.

**Version constraints.** Every `versionConstraint` in the libraries is a bare version such as `1.0.0`. KerML 10.3 recommends semantic-versioning ranges; under npm range syntax a bare version matches only itself, so the harness checks for equality. Range syntax is not needed yet.

**Expectations.** Every corpus source is `Expectation::Valid`: it must parse, resolve and validate without errors. Invalid cases (for example from spec snippets) can add other expectations later.

## Consequences

- Until the parser exists (Phase 1a), the corpus tests check completeness and the KerML 10.3 archive rules. The check that each index name occurs in its file is a textual approximation, to be replaced by real top-level name resolution.
- The harness reads files with `std::fs`, so it runs natively. Running the corpus against the WASM build (PROJECT_PLAN §8) needs the sources passed in, or embedded, and is left to Phase 1a.
- `sysml-corpus` is test infrastructure (`publish = false`). The `.kpar` reading here is minimal; the product's reader belongs in `sysml-interchange`.
