# ADR 0006: Test corpus and errata in the OMG library archives

- Status: Accepted
- Date: 2026-11-20

## Context

PROJECT_PLAN §8 makes a conformance corpus the acceptance gate. The `sysml-corpus` crate loads it from the local `docs/specs/`: the 10 standard libraries as project interchange files (`.kpar`, KerML 10.3) and the example models (SimpleVehicleModel). The OMG 20250201 library archives do not follow KerML 10.3 exactly, so the harness has to interpret them.

## Decision

**What is in the corpus.** Every `.kpar` and every `.kerml`/`.sysml` file listed in `specs.lock.json`. The lock file is the single list; a test fails if a listed model file is missing from the corpus. Annex A of the API specification (API test cases, not models) belongs to the API conformance suite (Phase 2). Textual notation snippets from the specification text are a later addition, because the extracted text does not reliably separate code from prose.

**Project root.** KerML 10.3 places `.project.json` and `.meta.json` "at its top level", and Table 13 note 2 makes paths relative to the archive root. Every OMG archive instead wraps the project in a single folder (for example `Kernel Semantic Library/.meta.json`), with index paths relative to that folder. When every entry is inside one top-level folder, that folder is the project root.

**Operating-system metadata.** Archive entries named `.DS_Store`, `Thumbs.db` or `desktop.ini`, and anything under `__MACOSX/`, are ignored and listed in `Library::ignored`. The Systems Library contains two such entries. Any other entry that is neither project metadata nor a `.kerml`/`.sysml` file is an error.

**Errata.** These defects are corrected by the `ERRATA` table in `sysml-corpus`:

| Library | Defect | Correction |
|---|---|---|
| SysML Systems Library | Index entry `AnalysisCases` names `AnalysisCase.sysml` | `AnalysisCases.sysml` |
| SysML Requirement Derivation Library | Index entry `RequirementsDerivation` names `RequirementsDerivation.sysml`; the file is `RequirementDerivation.sysml` and declares `package RequirementDerivation` | Both the name and the file are corrected |
| SysML Analysis Library | Usage `…/KerML/20250201/Quantities-and-Units-Library.kpar` | `…/SysML/20250201/Quantities-and-Units-Domain-Library.kpar` |
| SysML Geometry Library | Usage `…/SysML/20250201/Quantities-and-Units-Library.kpar` | `…/SysML/20250201/Quantities-and-Units-Domain-Library.kpar` |

A library's usage IRI is its pinned download URL in `specs.lock.json`. Every other usage matches one exactly.

The tests assert that the set of defects found equals the errata table. When OMG publishes corrected archives, the test fails and the entry is removed.

**Version constraints.** Every `versionConstraint` in the libraries is a bare version such as `1.0.0`. KerML 10.3 recommends semantic-versioning ranges; under npm range syntax a bare version matches only itself, so the harness checks for equality. Range syntax is not needed yet.

**Expectations.** Every corpus source is `Expectation::Valid`: it must parse, resolve and validate without errors. Invalid cases (for example from spec snippets) can add other expectations later.

## Consequences

- Until the parser exists (Phase 1a), the corpus tests check completeness and the KerML 10.3 archive rules. The check that each index name occurs in its file is a textual approximation, to be replaced by real top-level name resolution.
- The harness reads files with `std::fs`, so it runs natively. Running the corpus against the WASM build (PROJECT_PLAN §8) needs the sources passed in, or embedded, and is left to Phase 1a.
- `sysml-corpus` is test infrastructure (`publish = false`). The `.kpar` reading here is minimal; the product's reader belongs in `sysml-interchange`.
