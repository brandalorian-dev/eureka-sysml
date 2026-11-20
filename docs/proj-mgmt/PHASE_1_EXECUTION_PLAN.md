# Eureka SysML: Phase 1 execution plan

Status: Draft v1.0 · 2026-11-20 · Covers PROJECT_PLAN v0.7 §6, Phases 1a and 1b (Release 0.1 "Language")

This plan breaks Phase 1 into ordered steps, each small enough for one or a few pull requests. PROJECT_PLAN.md remains the source of truth for scope, architecture and budgets. Where this plan finds something the project plan does not settle, it is listed under [decisions to make before or during Phase 1](#24-decisions-needed).

## 1. Executive summary

Phase 1 builds the language engine: everything needed to read, check and write KerML and SysML models, without a server or a web UI. It ends with **Release 0.1 "Language"**: a command-line tool (`eureka`), a language server (`eureka-lsp`) with a VS Code extension, and `.kpar`/JSON conversion. It is the project's critical path, because every later phase builds on these crates.

**Phase 1a, KerML core (estimated 10–14 weeks).**
- An error-tolerant, lossless KerML parser (KerML 8.2).
- The arena element graph and the mapping from syntax to the abstract syntax (KerML 8.3).
- Name resolution, imports and visibility.
- Incremental recomputation with `salsa`.
- The KerML semantics: 80 implied-relationship constraints with the redundancy rules, and 88 validation constraints (KerML 8.4).
- Loading the three circular KerML libraries with zero errors and normative v5 UUIDs (KerML 9.1), and a pre-resolved snapshot.
- From the first step, the same tests also run on the WebAssembly build.

**Phase 1b, SysML language, CLI and language server (estimated 8–10 weeks).**
- The SysML textual notation (SysML 8.2.2) on top of the KerML engine, covering 93 more metaclasses.
- 128 more implied-relationship constraints and 92 more validation constraints.
- The Systems Library and the six domain libraries.
- A formatter that preserves comments, and JSON and `.kpar` interchange (KerML 10).
- The `eureka` CLI, `eureka-lsp` and a VS Code extension.
- The first three performance budgets, measured on the 1M-element synthetic model.

**What makes it hard.** The parser is routine. The schedule risk is in the semantics:
- **The implied relationships.** 208 constraints, each of which can add relationships, with rules to avoid redundant ones (KerML 8.4.2). The KerML libraries also use each other circularly, so they must resolve as one unit.
- **The 1M-element budgets.** These force incremental, memory-conscious design from the start rather than as a later optimization.

**How it is controlled.**
- **Test first, from the spec.** Every step starts from tests derived from the spec: the 95-source corpus, the KerML 8.4.3/8.4.4 tables, the reference UUIDs, the synthetic models.
- **One PR per item.** Every PR passes CI, including the WASM run and the minimum Rust version, and gets `/code-review` before merging.
- **Recalibration.** The estimates are recalibrated at the end of Phase 1a, before Phase 1b is committed to. This plan's bottom-up estimate for Phase 1b already exceeds the project plan's (see §3.3).

### 1.1 Scale of the work

These counts come from the normative files, and give the semantic steps a measurable denominator.

| | KerML (Phase 1a) | SysML additions (Phase 1b) |
|---|---|---|
| Metaclasses (`KerML.json` / `SysML.json`) | 82, plus 2 enumerations | 93, plus 5 enumerations (175 and 7 in total) |
| Implied-relationship constraints (`check…`) | 80: 59 specialization, 8 redefinition, 6 type-featuring, 7 binding-connector | 128: 116, 7, 1, 4 |
| Validation constraints (`validate…`) | 88 | 92 |
| Derived-property rules (`derive…`) | 90 | 145 |
| Operations | 84 | 19 |
| Library files in the corpus | 36 `.kerml` in 3 libraries | 58 `.sysml` in 7 libraries |
| Grammar clauses | KerML 8.2.2–8.2.5 | SysML 8.2.2.1–8.2.2.27 |

The constraint, derivation and operation counts are the `ownedRule` and `ownedOperation` entries in `KerML.xmi` and `SysML.xmi`. `SysML.xmi` holds only the 93 SysML metaclasses, so the two columns do not overlap.

## 2. Prerequisites and entry criteria

### 2.1 Entry criteria

Phase 1 starts when all of these hold.

| # | Criterion | State on 2026-11-20 |
|---|---|---|
| E1 | Phase 0 exit met (PROJECT_PLAN §6) | Met (v0.7) |
| E2 | CI green on `main` (fmt, clippy, tests, wasm32 build, MSRV, `cargo deny`, benchmarks run once, SBOM, hygiene) | Met |
| E3 | The corpus loads: the 10 libraries and SimpleVehicleModel, 95 sources, verified against `specs.lock.json` | Met (`sysml-corpus`) |
| E4 | The synthetic generator produces 1k, 50k and 1M models | Met (`sysml-synth`) |
| E5 | The §7 budgets are recorded as data, with the phase that measures each one | Met (`sysml-bench`) |
| E6 | Per-clause spec text available locally | Met (`scripts/extract_spec_text.py`) |
| E7 | A reference for the library element UUIDs has been chosen (§2.3) | **Open**: step 1a.0 |
| E8 | The decisions marked "before 1a" in §2.4 have ADRs | **Open**: step 1a.0 |

### 2.2 Tooling

Already in place: Rust 1.100 stable and the 1.89 minimum toolchain, the wasm32 target, Python 3.11+, `pdftotext`, `cargo-deny`, `cargo-cyclonedx`, `gh`, Node 26 and Docker.

To add:
- `wasm-pack` and a WASM test runner: `wasm-bindgen-test`, run under Node. Needed in step 1a.1.
- `poppler-utils` `pdftohtml` (font information), if step 1a.5 uses font-aware snippet extraction.
- The VS Code extension packaging tool (`@vscode/vsce`), in step 1b.8.

### 2.3 Reference data (test oracles)

Phase 1 is test-first, so each spec rule needs an oracle that is not our own code.

| Rule | Oracle | Status |
|---|---|---|
| Lexical structure and grammar (KerML 8.2, SysML 8.2.2) | The 94 library files and SimpleVehicleModel must parse with zero errors and round-trip byte for byte. Plus the spec snippets (step 1a.5) and the EBNF in the clause text. | Available |
| Name resolution (KerML 8.2.3.5) | The libraries must resolve with zero unresolved names. Plus hand-written cases from the clause's examples. | Available |
| Implied relationships (KerML 8.4.3.1, 8.4.4.1 tables; SysML 8.4) | One golden test per table row, written from the tables. The OMG pilot implementation is allowed as a behavioral cross-check only (CLAUDE.md: no reuse of its code). | Available; the pilot cross-check is optional |
| Library element UUIDs (KerML 9.1, SysML 9.1) | **Needs a source of published UUIDs.** The `.kpar` archives hold only text. A candidate is the library serializations in the EPL-2.0 SysML-v2-Release repository, which PROJECT_PLAN §13 already names as the source for shipped libraries. If they carry element ids, pin that commit in `specs.lock.json`. | **Open**: step 1a.0 |
| JSON serialization (KerML 10.4) | `SysML.json` and `KerML.json`, through the property descriptors `sysml-model` generates. Plus the element examples in the API `OpenAPI.json`. | Available |
| Project interchange (KerML 10.3) | The 10 library archives, with the errata of ADR 0006 | Available |
| Element-count rules of `sysml-synth` | The parser's element counts (step 1b.4) | Arrives in Phase 1b |

### 2.4 Decisions needed

| # | Decision | Needed before | Recommended default |
|---|---|---|---|
| D1 | Parser architecture: hand-written recursive descent producing a `rowan` lossless tree, or a parser generator | 1a.2 (ADR) | Hand-written recursive descent with `rowan`, the rust-analyzer approach PROJECT_PLAN §4.2 names. It gives the best error recovery and full control over trivia. |
| D2 | `salsa` version and the boundaries of the incremental query graph (file → tree → elements → resolution → semantics) | 1a.7 (ADR) | The current `salsa` release, if it builds for wasm32 and on Rust 1.89. A spike in 1a.0 confirms this. |
| D3 | Arena and ID design for the element graph: element ids as API `DataIdentity` ids (PROJECT_PLAN §4.3), and storage of implied versus owned relationships | 1a.6 (ADR) | Interned dense indices in an arena, mapped to UUIDs. Implied relationships are kept apart from owned ones, so serialization can include or omit them (KerML 10.3 `includesImplied`). |
| D4 | Diagnostics model: codes, severity, spans, the constraint name and the spec clause | 1a.2 (ADR) | One diagnostic type across the parser, resolver and validator. Every semantic diagnostic names its constraint, for example `validateFeatureMultiplicityDomain`. |
| D5 | Snapshot format for the pre-resolved libraries | 1a.11 (ADR) | A versioned binary format through a serde-compatible crate such as `postcard`, keyed by metamodel IRI, library version and crate version |
| D6 | How CI gates timings (ADR 0007): a dedicated runner or instruction counts | 1a.11 (ADR) | Instruction counts on GitHub-hosted runners (for example a Cachegrind-based harness), with wall-clock budgets measured but not gated until a dedicated runner exists |
| D7 | Library UUID oracle (§2.3) | 1a.0 | As in §2.3 |
| D8 | VS Code extension tooling. The extension is TypeScript, but PROJECT_PLAN v0.7 moved the pnpm/TypeScript workspace to Phase 3. | 1b.8 | A self-contained `editors/vscode` npm package: a thin `vscode-languageclient` wrapper, with its own lockfile and `npm audit` and an npm SBOM in its CI job. The pnpm workspace still waits for Phase 3. |
| D9 | Release 0.1: which crates go to crates.io, their license (Apache-2.0, or MIT OR Apache-2.0 per PROJECT_PLAN §13.1), and whether to sign the 0.1 artifacts | 1b.10 (owner decision) | Publish nothing to crates.io at 0.1; ship binaries through GitHub Releases. Attach the SBOMs (§4.8 requires an SBOM for every release) and sign with Sigstore now, since that is cheap to do from the first release. |

## 3. Detailed steps, in recommended sequence

### 3.1 Working rules for every step

- **Test first.** Write the failing tests from the spec rule and the oracle in §2.3, then implement (CLAUDE.md).
- **Cite.** Every spec behavior cites its clause in a comment, for example `// KerML 8.4.4.6.2`. Every interpretation of an ambiguous clause gets an ADR. Every defect in an OMG file goes into an errata table with a test that fails when OMG fixes it (ADRs 0005, 0006).
- **Small PRs.** One branch and PR per item, signed-off commits, CI green, `/code-review` findings fixed in a further commit, then a rebase merge.
- **WASM parity.** From step 1a.1 on, every test also runs on the WASM build.
- **Performance stays visible.** New benchmarks go into `sysml-bench`, and a budget's `benchmark` field is set once it is measured.
- **Keep records current.** CLAUDE.md, the crate table in PROJECT_PLAN §4.2 and the ADR index are updated as crates arrive.

Each step below gives its goal, the spec clauses, what it delivers, the tests to write first, when it is done, and an indicative effort in weeks of full-time work.

### 3.2 Phase 1a: KerML core

#### 1a.0 Start-up: decisions, spikes and the UUID oracle
- **Goal:** settle the "before 1a" decisions of §2.4 and remove the two unknowns that could change the design.
- **Work:**
  - Write the ADRs for D1, D3 and D4.
  - Spike the candidate dependencies (`rowan`, `salsa`, `uuid` with the `v5` feature, `wasm-bindgen`, `wasm-bindgen-test`). Each must build for wasm32, pass `cargo deny` and work on Rust 1.89.
  - Find and pin the library UUID oracle (D7).
- **Done when:** the ADRs are merged, the spike results are recorded in them, and the UUID oracle is in `specs.lock.json`, or the fallback is decided. The fallback is to test the top-level package UUIDs and the `path()` rules from the clause text alone, and treat full UUID conformance as unverified.
- **Effort:** 0.5

#### 1a.1 WASM test harness (moved from Phase 0)
- **Goal:** the existing tests run on WASM before there is much code to port.
- **Clauses:** none; this is PROJECT_PLAN §8 (corpus on native and WASM).
- **Delivers:**
  - the `sysml-wasm` crate skeleton
  - `wasm-pack test --node` for the workspace crates
  - a way to give WASM tests the corpus without file access: the sources embedded at build time from the local `docs/specs/` (nothing generated is committed), or passed in by the test runner
  - a CI job "Rust (wasm tests)"
- **Tests first:** the existing `sysml-model`, `sysml-api-types` and `sysml-corpus` tests, unchanged, run on WASM.
- **Done when:** CI runs every existing test natively and on WASM.
- **Effort:** 0.5

#### 1a.2 Lexer and lossless syntax tree
- **Goal:** turn any text into tokens and a lossless tree, never failing.
- **Clauses:** KerML 8.2.2 (8.2.2.1 line terminators and white space, 8.2.2.2 notes and comments, 8.2.2.3 names including unrestricted names, 8.2.2.4 numeric values, 8.2.2.5 strings, 8.2.2.6 reserved words, 8.2.2.7 symbols).
- **Delivers:**
  - the `sysml-syntax` crate: the token kinds, a hand-written lexer, and the `rowan` tree types
  - the parser framework (event-based, with error recovery)
  - the diagnostics type (D4)
- **Tests first:**
  - Lexing all 94 library files and SimpleVehicleModel produces no error tokens.
  - Concatenating the tokens reproduces every input byte for byte.
  - One unit test per lexical rule and per reserved word.
  - Property tests: any byte string lexes without panicking.
- **Done when:** the lexer tests pass natively and on WASM, and the framework can parse an empty root namespace.
- **Effort:** 0.75–1

#### 1a.3 KerML grammar: Root and Core layers
- **Goal:** parse the Root and Core concrete syntax into the tree.
- **Clauses:**
  - KerML 8.2.3 (8.2.3.1 elements and relationships, 8.2.3.2 dependencies, 8.2.3.3 annotations, comments, documentation and textual representation, 8.2.3.4 namespaces, imports and namespace elements)
  - KerML 8.2.4 (8.2.4.1 types: specialization, conjugation, disjoining, unioning/intersecting/differencing, feature membership; 8.2.4.2 classifiers; 8.2.4.3 features: typing, subsetting, redefinition, chaining, inverting, type featuring)
- **Tests first:**
  - One parse test per production, from the EBNF and the clause examples, checked against stored tree snapshots.
  - Error recovery: a missing `;`, `}` or name still yields a tree with one diagnostic and the rest of the file parsed.
- **Done when:** every Root and Core production has a test, and the parts of the library files that use only these layers parse with zero errors.
- **Effort:** 1–1.5

#### 1a.4 KerML grammar: Kernel layer and expressions
- **Goal:** complete the KerML grammar.
- **Clauses:**
  - KerML 8.2.5.1–8.2.5.7: data types, classes, structures, associations, connectors (including binding connectors and successions), behaviors and steps, functions, expressions, predicates, Boolean expressions and invariants
  - KerML 8.2.5.8, expressions: operator expressions with their precedence and associativity, primary, base and literal expressions
  - KerML 8.2.5.9–8.2.5.13: interactions and flows, feature values, multiplicities, metadata, packages
- **Tests first:**
  - One test per production.
  - An operator-precedence and associativity test table built from KerML Table 6 (8.2.5.8.1).
  - **All 36 KerML library files parse with zero errors and round-trip byte for byte.**
- **Done when:** the KerML library files pass, and fuzzing (`cargo fuzz` or property tests) finds no panics in an agreed run time.
- **Effort:** 1–1.5

#### 1a.5 Spec snippets in the corpus (moved from Phase 0)
- **Goal:** add the textual examples from the KerML and SysML specification text to the corpus.
- **Approach:** extract the code from the PDFs with font information (`pdftohtml -xml`; code is set in a monospaced font), or use the parser to accept only text that parses. Mark fragments that are not complete root namespaces, and the examples of deliberately invalid models, with their own `Expectation`.
- **Delivers:** snippet sources in `sysml-corpus`, with a stable id per clause and example, and an ADR on the extraction rules.
- **Tests first:** the corpus test counts the snippets per clause. Every KerML snippet marked valid parses with zero errors. The SysML snippets are only collected here, and must parse by step 1b.1.
- **Done when:** the snippets are in the corpus and the KerML ones pass.
- **Effort:** 0.25–0.5

#### 1a.6 Arena element graph and the mapping to the abstract syntax
- **Goal:** turn syntax trees into model elements and relationships.
- **Clauses:** KerML 8.3.2–8.3.4 (the abstract syntax), and the mapping notes on each production in 8.2.3–8.2.5.
- **Delivers:**
  - the element graph in `sysml-model` (D3), with typed access to the properties of all 82 KerML metaclasses through the generated descriptors
  - the lowering from tree to elements, with a source span for each element
- **Tests first:**
  - For each production, the elements and relationships it must create: memberships, owning memberships, relationships, owned features, values.
  - The element counts of the KerML library files, by metaclass, as recorded snapshots that are reviewed by hand against the clauses.
- **Done when:** every KerML library file lowers to elements without errors, and the model graph can be walked by ownership and by relationship.
- **Effort:** 1–1.5

#### 1a.7 Name resolution, imports and incremental queries
- **Goal:** resolve every qualified name, incrementally.
- **Clauses:**
  - KerML 8.2.3.5 (8.2.3.5.1–8.2.3.5.4: local and global namespaces, local and visible resolution, full resolution)
  - KerML 8.2.3.4.2 (imports, including recursive imports and filters)
  - membership and visibility rules (8.3.2.4)
- **Delivers:**
  - the `sysml-semantics` crate on `salsa` (D2), with queries for file → tree → elements → scopes → resolved references
  - loading the three KerML libraries as one circular unit
- **Tests first:**
  - The clause examples as resolution tests, covering visibility, aliases, imports, inheritance and redefinition-based hiding.
  - **The three KerML libraries resolve with zero unresolved names.**
  - Incrementality: editing one library file recomputes only the queries that depend on it, asserted from `salsa`'s event log.
- **Done when:** these pass natively and on WASM.
- **Effort:** 1.5–2

#### 1a.8 Derived properties and operations
- **Goal:** compute the derived properties and operations the semantics and the UUIDs need.
- **Clauses:**
  - KerML 8.3.2–8.3.4: the `derive…` rules (90) and the operations (84) of the KerML metaclasses
  - in particular `path()` (8.3.2.1.2, 8.3.2.1.3, 8.3.2.4.8), qualified names, effective names, inherited memberships, featuring types and feature directions
- **Delivers:** the derived properties as `salsa` queries, computed on read. Derived values sent by clients are ignored (PROJECT_PLAN §4.3, Derived Property Full Conformance).
- **Tests first:** one test per `derive…` rule and operation, from its OCL in the metamodel, prioritizing those used by the constraints in 1a.9 and 1a.10 and by `path()`.
- **Done when:** every rule and operation that the semantic and validation constraints use is covered, and the rest are listed with their status.
- **Effort:** 0.75–1

#### 1a.9 Implied relationships
- **Goal:** add the relationships the semantic constraints require (KerML 8.4.2).
- **Clauses:**
  - KerML 8.4.2 (the four kinds and the two redundancy rules)
  - the tables in 8.4.3.1 (Core) and 8.4.4.1 (Kernel)
  - the per-construct semantics in 8.4.3.2–8.4.3.4 and 8.4.4.2–8.4.4.14
- **Work:** implement table by table: specialization constraints (59), then redefinition (8), type-featuring (6), and binding-connector (7) constraints. Implied relationships are kept apart from owned ones (D3).
- **Tests first:**
  - One golden test per table row: a minimal model, and the implied relationships expected.
  - The redundancy rules: the clause's Structure/Class/Classifier example, and same-general duplicates.
  - **The KerML libraries satisfy every `check…` constraint after implication.**
  - Optionally, a behavioral comparison with the pilot on the libraries.
- **Done when:** all 80 KerML `check…` constraints are implemented and tested, and the libraries satisfy them.
- **Effort:** 1.5–2.5

#### 1a.10 Validation constraints
- **Goal:** report every KerML well-formedness violation as a diagnostic.
- **Clauses:** the 88 `validate…` constraints of KerML 8.3, from their OCL.
- **Tests first:** one positive and one negative test per constraint. **The KerML libraries produce zero diagnostics.**
- **Done when:** all 88 constraints are implemented, with the constraint name and clause in each diagnostic (D4).
- **Effort:** 0.75–1

#### 1a.11 Library UUIDs, the snapshot and the first budget
- **Goal:** normative library identity, and fast library loading.
- **Clauses:** KerML 9.1, the v5 UUID scheme: the `NameSpace_URL` namespace, the package URL `https://www.omg.org/spec/KerML/<package>`, and `path()` for the elements inside.
- **Delivers:**
  - UUID assignment for library elements
  - the `sysml-libraries` crate, which writes and loads the pre-resolved snapshot (D5)
  - the `library-load` benchmark in `sysml-bench`, and the CI timing gate (D6)
- **Tests first:**
  - The top-level package UUIDs, computed by hand from 9.1.
  - Every library element's UUID matches the oracle (D7).
  - A snapshot round trip: loading the snapshot gives the same model as resolving from text.
- **Done when:** these pass, `library-load` is measured against its budget (< 300 ms server, < 1 s browser), and `BUDGETS` records the benchmark.
- **Effort:** 0.75–1

#### 1a.12 Phase 1a close-out and recalibration
- **Work:**
  - Check the Phase 1a exit criteria (§5.1).
  - Measure throughput: PRs, constraints implemented per week, and review findings per PR.
  - Update the PROJECT_PLAN §6 estimates, and this plan's Phase 1b estimates.
- **Effort:** included in the steps above

**Phase 1a bottom-up total: about 10–14 weeks**, which matches the project plan.

### 3.3 Phase 1b: SysML language, CLI and language server

#### 1b.1 SysML grammar
- **Goal:** parse the SysML textual notation, reusing the KerML lexer, expressions and framework.
- **Clauses:** SysML 8.2.2, in clause order:
  - 8.2.2.1 overview and lexical structure
  - 8.2.2.2–8.2.2.5: elements and relationships, dependencies, annotations, namespaces and packages
  - 8.2.2.6: definition and usage, with definitions, usages, reference usages, body elements, specialization and multiplicity
  - 8.2.2.7–8.2.2.12: attributes, enumerations, occurrences, items, parts, ports
  - 8.2.2.13–8.2.2.16: connections, interfaces, allocations, flows
  - 8.2.2.17–8.2.2.18: actions (control nodes, send and accept, assignment, terminate, structured control) and states (transitions)
  - 8.2.2.19–8.2.2.27: calculations, constraints, requirements and concerns, cases, analysis cases, verification cases, use cases, views, viewpoints and renderings, metadata
- **Tests first:**
  - One test per production.
  - **All 58 SysML library files, SimpleVehicleModel, the SysML snippets and the 1k/50k synthetic models parse with zero errors and round-trip byte for byte.**
- **Done when:** these pass natively and on WASM.
- **Effort:** 2–2.5

#### 1b.2 SysML abstract syntax mapping
- **Goal:** lower SysML trees to the 93 SysML metaclasses.
- **Clauses:** SysML 8.3.2–8.3.27, and the mapping notes in 8.2.2.
- **Tests first:** the elements expected for each production, and per-metaclass element counts for the SysML libraries and SimpleVehicleModel, as reviewed snapshots.
- **Done when:** every SysML source in the corpus lowers to elements without errors.
- **Effort:** 1–1.5

#### 1b.3 SysML semantics and validation
- **Goal:** the SysML implicit specializations and the other implied relationships, and SysML validation.
- **Clauses:** SysML 8.4.1–8.4.23, and the 128 `check…` and 92 `validate…` constraints in `SysML.xmi`.
- **Tests first:**
  - Golden tests per constraint, as in 1a.9 and 1a.10. For example, every `part` specializes `Parts::parts` (PROJECT_PLAN §2.2).
  - **The Systems Library and the six domain libraries resolve with zero diagnostics.**
- **Done when:** all 128 + 92 constraints are implemented and tested.
- **Effort:** 1.5–2

#### 1b.4 SysML libraries, SimpleVehicleModel and the synthetic models
- **Goal:** the full corpus is clean, and the synthetic models are confirmed.
- **Clauses:** SysML 9.1 (UUIDs with the `https://www.omg.org/spec/SysML/` prefix), 9.2–9.8.
- **Work:**
  - Add the seven SysML libraries to the snapshot.
  - Resolve SimpleVehicleModel (SysML Annex A) and all the snippets.
  - Parse and resolve the 1k, 50k and 1M synthetic models, and compare their element counts with `sysml-synth`'s `Stats::elements` rules. Correct the rules or the generator, and update ADR 0007.
- **Tests first:** SysML library UUIDs against the oracle, zero diagnostics across the whole corpus, and the synthetic element counts within a stated tolerance.
- **Done when:** the whole corpus has zero diagnostics.
- **Effort:** 0.5–1

#### 1b.5 Serializer and formatter
- **Goal:** text from the model that keeps comments and formatting, and a deterministic formatter.
- **Clauses:** KerML 8.2 and SysML 8.2.2, for which notation is valid; KerML 10.2 (textual interchange files).
- **Delivers:** model → text serialization that reuses stored trivia where the model is unchanged, and a formatter with an idempotent output.
- **Tests first:**
  - **text → model → text is stable for the whole corpus.**
  - Formatting is idempotent: `format(format(x)) == format(x)`.
  - Comments and documentation are never lost.
- **Done when:** these pass for the whole corpus and the synthetic models.
- **Effort:** 1–1.5

#### 1b.6 JSON and `.kpar` interchange
- **Goal:** Model Interchange conformance (SysML 2, item 4).
- **Clauses:** KerML 10.3 (project interchange) and 10.4 (JSON serialization: primitive types, enumerations, element references, elements, models).
- **Delivers:**
  - the `sysml-interchange` crate: JSON import and export through the `sysml-model` descriptors
  - `.kpar` import and export, with `.project.json`, `.meta.json`, the index and checksums. The archive reading moves here from `sysml-corpus` (ADR 0006), and the corpus then uses this crate.
- **Tests first:**
  - The JSON output validates against `SysML.json`, and the API `OpenAPI.json` element examples import.
  - text → JSON → model gives the same model.
  - `.kpar` export → import round-trips the libraries.
  - The errata of ADR 0006 are applied on import and never written on export.
- **Done when:** these pass for the whole corpus.
- **Effort:** 0.75–1

#### 1b.7 `eureka` CLI
- **Goal:** the command-line tool of Release 0.1.
- **Delivers:**
  - `eureka validate`, `format` (with `--check`), `convert` (text, JSON and `.kpar`, in any direction) and `query`
  - `query` uses the API's `Query`/`Constraint` model from `sysml-api-types`, so it means the same as the server will
- **Tests first:** command-line tests on corpus files, covering the exit codes, machine-readable diagnostics, and convert round trips.
- **Done when:** all four commands work on the corpus.
- **Effort:** 0.5

#### 1b.8 `eureka-lsp` and the VS Code extension
- **Goal:** language features in an editor.
- **Delivers:**
  - an LSP server, with incremental text sync driving the `salsa` queries. Its features: diagnostics, completion, hover, go-to-definition, find references, document symbols, formatting, and rename where the IDs allow it
  - a thin VS Code extension (D8)
- **Tests first:** LSP protocol tests for each feature on corpus files, and the `language-features` benchmark (< 50 ms local).
- **Done when:** the features work in VS Code on SimpleVehicleModel and the libraries, and `language-features` is measured.
- **Effort:** 1–1.5

#### 1b.9 Performance budgets on the 1M-element model
- **Goal:** meet the Phase 1b budgets of PROJECT_PLAN §7.
- **Delivers:** the `re-resolve` benchmark (< 1 s after a commit of fewer than 100 changed elements) and the `find-references` benchmark (< 2 s over 1M elements), both on the 1M synthetic model, with gates (D6).
- **Done when:** both budgets are met and recorded in `BUDGETS`. If they are not met, the plan is revised before Release 0.1, not after.
- **Effort:** 0.5–1. This step is mostly measurement and tuning, so it can overlap with 1b.5–1b.8.

#### 1b.10 Release 0.1 "Language"
- **Delivers:**
  - a release workflow: versioned binaries of `eureka` and `eureka-lsp` for the supported platforms, with SBOMs attached (PROJECT_PLAN §4.8) and the signing scope decided in D9
  - the packaged VS Code extension
  - a changelog
  - release notes stating which SysML conformance points (SysML Clause 2) 0.1 claims: Textual Notation, Abstract Syntax (through the API and CLI), Model Interchange, and Semantic conformance for the libraries
- **Done when:** the exit criteria in §5.2 hold and the release is published.
- **Effort:** 0.25–0.5

**Phase 1b bottom-up total: about 9–13 weeks, against the project plan's 8–10.** The difference comes mainly from the SysML grammar and semantics (1b.1–1b.3), and from the formatter. The recalibration at the end of Phase 1a (1a.12) decides which estimate to plan by.

### 3.4 Sequence and parallel work

```
1a.0 ─ 1a.1 ─ 1a.2 ─ 1a.3 ─ 1a.4 ─┬─ 1a.6 ─ 1a.7 ─ 1a.8 ─ 1a.9 ─ 1a.10 ─ 1a.11 ─ 1a.12
                                  └─ 1a.5 (snippets, needs only the parser)

1b.1 ─ 1b.2 ─ 1b.3 ─ 1b.4 ─┬─ 1b.5 ─ 1b.6 ─ 1b.7 ─────┬─ 1b.10
                           ├─ 1b.8 (LSP, after 1b.4) ─┤
                           └─ 1b.9 (budgets) ─────────┘
```

PROJECT_PLAN §12 allows parallel Claude Code sessions in git worktrees for independent work. Good candidates:
- 1a.5 alongside 1a.6–1a.7
- the per-table implied relationships in 1a.9, one table per session
- 1b.6 alongside 1b.5
- 1b.8 alongside 1b.5–1b.7

## 4. Assumptions

| # | Assumption | If it does not hold |
|---|---|---|
| A1 | One developer full time with Claude Code, as in PROJECT_PLAN §6. The estimates are calendar weeks. | Scale the schedule; the sequence does not change. |
| A2 | The specifications stay at the versions pinned in `specs.lock.json` for the whole of Phase 1: KerML 1.0, SysML 2.0, metamodel `20250201`. | A revision goes through `check_schema_assumptions.py` and the errata tests, and its cost is estimated separately. |
| A3 | The normative machine-readable files take precedence over the documents (SysML Clause 2). Their defects go into errata tables with tests (ADRs 0005, 0006). | — |
| A4 | The OMG pilot implementation is used only as a behavioral reference. None of its code is reused (CLAUDE.md). | — |
| A5 | A UUID oracle can be found (§2.3, D7). | UUID conformance rests on the clause text and is reported as unverified in the 0.1 release notes. |
| A6 | `rowan`, `salsa`, `uuid` (v5) and the LSP and WASM crates build for wasm32, pass `cargo deny`, and work on Rust 1.89. Spiked in 1a.0. | Raise the minimum Rust version, or choose alternatives, before 1a.2. |
| A7 | The parser stays hand-written and shared between KerML and SysML (D1), so SysML adds productions, not a second parser. | 1b.1 grows. |
| A8 | Phase 1 does not need PostgreSQL, the server or a web UI. Interchange, the CLI and the LSP work on files. | — |
| A9 | The 1M-element budgets can be met with incremental `salsa` queries and the arena design. 1b.9 checks this before the release. | Design changes, or a budget renegotiated in PROJECT_PLAN §7, before 0.1 |
| A10 | Development continues in WSL2 Ubuntu, with GitHub-hosted CI runners (CLAUDE.md, Environment). | — |
| A11 | The VS Code extension is the only TypeScript in Phase 1 (D8). | — |
| A12 | Review stays mandatory. In Phase 0 it found an important defect in every PR it reviewed (PROJECT_PLAN §8). Its time is part of each step's estimate. | — |

## 5. Exit criteria

### 5.1 Phase 1a exit

All of these are checked by automated tests in CI, except X1.7.

| # | Criterion | Evidence |
|---|---|---|
| X1.1 | The three KerML libraries (36 files) parse, round-trip byte for byte, and resolve with **zero diagnostics** | Corpus tests |
| X1.2 | Every KerML spec snippet marked valid parses and resolves with zero diagnostics | Corpus tests |
| X1.3 | All 80 KerML `check…` constraints are implemented, with a golden test per KerML 8.4.3.1 and 8.4.4.1 table row, and the libraries satisfy them | Semantics tests |
| X1.4 | All 88 KerML `validate…` constraints are implemented, with positive and negative tests | Validation tests |
| X1.5 | Library element UUIDs follow KerML 9.1 and match the oracle, or the unverified part is recorded (A5) | UUID tests |
| X1.6 | **The WASM build passes the same tests** as the native build | The CI WASM job |
| X1.7 | The estimates are recalibrated: PROJECT_PLAN §6 and this plan's Phase 1b updated from measured throughput | A plan revision PR |
| X1.8 | The `library-load` budget is measured and gated: < 300 ms server, < 1 s browser | `sysml-bench` |
| X1.9 | Editing one file recomputes only what depends on it | Incrementality tests |

### 5.2 Phase 1b exit and Release 0.1

| # | Criterion | Evidence |
|---|---|---|
| X2.1 | **All 10 libraries, SimpleVehicleModel (SysML Annex A) and the spec snippets resolve with zero diagnostics** | Corpus tests |
| X2.2 | All 128 SysML `check…` and 92 `validate…` constraints are implemented and tested | Semantics and validation tests |
| X2.3 | **The round trip is stable**: text → model → text for the whole corpus, idempotent formatting, no comment lost | Serializer tests |
| X2.4 | JSON (KerML 10.4) and `.kpar` (KerML 10.3) export and import round-trip the corpus, and JSON output validates against `SysML.json` | Interchange tests |
| X2.5 | **The `re-resolve` and `find-references` budgets are met on the 1M-element synthetic model** (< 1 s and < 2 s), and `language-features` is measured (< 50 ms local) | `sysml-bench`, with the gates of D6 |
| X2.6 | The synthetic models parse with zero diagnostics, and `sysml-synth`'s element-count rules are confirmed or corrected (ADR 0007) | Synthetic model tests |
| X2.7 | `eureka validate`, `format`, `convert` and `query` work on the corpus | CLI tests |
| X2.8 | `eureka-lsp` provides diagnostics, completion, hover, go-to-definition, find references, symbols and formatting, with the VS Code extension | LSP tests, and a manual check |
| X2.9 | The WASM build passes the same tests | The CI WASM job |
| X2.10 | **Release 0.1 is published**: binaries, the VS Code extension, SBOMs (PROJECT_PLAN §4.8), signing per D9, and release notes stating the conformance points claimed (SysML Clause 2) | The GitHub Release |
| X2.11 | CLAUDE.md, PROJECT_PLAN §4.2 and the ADR index reflect the crates and decisions of Phase 1 | Review |
