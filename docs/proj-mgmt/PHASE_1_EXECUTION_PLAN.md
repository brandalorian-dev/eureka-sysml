# Eureka SysML: Phase 1 execution plan

Status: Draft v1.21 · 2026-11-23 · Covers PROJECT_PLAN v0.7 §6, Phases 1a and 1b (Release 0.1 "Language")

This plan breaks Phase 1 into ordered steps, each small enough for one or a few pull requests. PROJECT_PLAN.md remains the source of truth for scope, architecture and budgets. Where this plan finds something the project plan does not settle, it is listed under [decisions](#24-decisions-and-their-recommended-defaults), and settled by the [decision methodology](#25-decision-methodology): each recommended default is evaluated during the step that first depends on it.

Change history:
- v1.0: first version
- v1.1: the decision methodology (§2.5). Each decision is evaluated in its execution step, so the decisions are no longer entry criteria.
- v1.2: step 1a.0 done. D7 adopted (ADR 0009); the dependency spike recorded as evidence for D1 and D2 (`docs/proj-mgmt/evidence/1a.0-dependency-spike.md`).
- v1.3: step 1a.1 done (ADR 0010). All 77 tests run as WebAssembly with a parity check; `wasm-pack build` packages `sysml-wasm`.
- v1.4: D1 and D4 adopted (ADRs 0011, 0012), from the step 1a.2 parser spike.
- v1.5: step 1a.2 done: the `sysml-syntax` lexer, parser framework and diagnostics.
- v1.6: step 1a.3 done, with the owner-approved scope change: the Kernel parts the Root and Core layers need first (packages, multiplicities with literal and feature-reference bounds, prefix metadata) are pulled forward from 1a.4, and the library criterion is restated as measurable. ADR 0014 records two readings of the grammar.
- v1.7: step 1a.4 done: the whole KerML grammar, including the expressions. All 36 KerML library files parse with zero diagnostics, and a mutation fuzzer ran 5 minutes without a failure. ADR 0015 records four readings of the Kernel grammar.
- v1.8: step 1a.5 done: 923 spec snippets extracted from the PDFs by font (ADR 0016); the 191 valid KerML ones parse with zero diagnostics.
- v1.9: D3 adopted (ADR 0017), from the step 1a.6 element graph spike.
- v1.10: step 1a.6 done: the element graph and the lowering. All 36 KerML library files lower to the abstract syntax the pilot implementation built, element by element (ADR 0018).
- v1.11: D2 adopted (ADR 0019), from the step 1a.7 incremental query spike.
- v1.12: step 1a.7 done, with the owner-approved scope change: the implied specializations and redefinitions that name resolution needs are pulled forward from 1a.9. All 4003 names of the KerML libraries resolve to the element the pilot implementation resolved them to. ADR 0020 records the readings.
- v1.13: step 1a.8 done, in four PRs: one per layer, and model-level evaluation in a fourth, as the owner chose. All 90 derive rules and 84 operations are implemented and tested, except `memberElementId`, which no constraint needs. ADR 0021 records the readings.
- v1.14: D10 adopted (ADR 0022), from the step 1a.9 materialization spike, which also found and fixed a scaling bug in resolution (ADR 0020 decision 5a).
- v1.15: step 1a.9 in progress: the overlay and the view of ADR 0022, and 58 of the 59 specialization constraints, which the KerML libraries satisfy. ADRs 0020 and 0022 record the readings and the implied elements.
- v1.16: step 1a.9: all 59 specialization and 8 redefinition constraints done, and the KerML libraries satisfy them.
- v1.17: step 1a.9: the 6 type-featuring constraints done, with the constructed snapshots and Cartesian-product features. The KerML libraries satisfy all 73 implemented constraints.
- v1.18: step 1a.9 done: all 78 `check…` constraints that have OCL are implemented and tested, and the KerML libraries satisfy them, implied elements included. The 2 without OCL are deferred.
- v1.19: step 1a.10 in progress: the 39 Root and Core validation constraints, with zero diagnostics on the KerML libraries. The negative tests of 14 constraints that no text can violate move to step 1b.6, where models can be read from KerML JSON, as the owner chose. ADR 0023 records the readings.
- v1.20: step 1a.10 done: all 88 validation constraints, and the KerML libraries produce zero diagnostics. The negative tests of 38 constraints that no text can violate move to step 1b.6.
- v1.21: step 1a.11 in progress, in three PRs as the owner chose: the library UUIDs, then the D5 and D6 evaluation, then the snapshot, the benchmark and the gate. The UUIDs of all 24,859 written KerML library elements match the oracle.

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
- **Decisions on evidence.** Nine design and release decisions have recommended defaults (§2.4). Each default is evaluated in the step that first depends on it, against stated criteria and at least one alternative, and recorded in an ADR (§2.5).
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

All entry criteria are met. The open questions of §2.3 and §2.4 (the UUID reference and the design decisions) are not entry criteria. Each is settled in the step that first depends on it (§2.5).

### 2.2 Tooling

Already in place: Rust 1.100 stable and the 1.89 minimum toolchain, the wasm32 target, Python 3.11+, `pdftotext`, `cargo-deny`, `cargo-cyclonedx`, `gh`, Node 26, Docker, and (since step 1a.0) `wasm-bindgen-cli` 0.2.130 for running WASM tests under Node.

To add:
- `wasm-pack` 0.15.1 (installed in step 1a.1), for `wasm-pack build`.
- `poppler-utils` `pdftohtml` (font information), if step 1a.5 uses font-aware snippet extraction.
- The VS Code extension packaging tool (`@vscode/vsce`), in step 1b.8.

### 2.3 Reference data (test oracles)

Phase 1 is test-first, so each spec rule needs an oracle that is not our own code.

| Rule | Oracle | Status |
|---|---|---|
| Lexical structure and grammar (KerML 8.2, SysML 8.2.2) | The 94 library files and SimpleVehicleModel must parse with zero errors and round-trip byte for byte. Plus the spec snippets (step 1a.5) and the EBNF in the clause text. | Available |
| Name resolution (KerML 8.2.3.5) | The libraries must resolve with zero unresolved names. Plus hand-written cases from the clause's examples. | Available |
| Implied relationships (KerML 8.4.3.1, 8.4.4.1 tables; SysML 8.4) | One golden test per table row, written from the tables. The OMG pilot implementation is allowed as a behavioral cross-check only (CLAUDE.md: no reuse of its code). | Available; the pilot cross-check is optional |
| Library element UUIDs (KerML 9.1, SysML 9.1) | The XMI library serializations of SysML-v2-Release **2025-02**, whose library text is identical to the normative `.kpar` files. All 94 top-level packages and all 9,662 named elements match KerML 9.1 when recomputed independently (`scripts/check_uuid_oracle.py`, run in CI). Pinned in `specs.lock.json` under `docs/specs/SysML-v2-Release/2025-02/`. Root namespaces are outside 9.1 and not compared. | **Available** (ADR 0009) |
| JSON serialization (KerML 10.4) | `SysML.json` and `KerML.json`, through the property descriptors `sysml-model` generates. Plus the element examples in the API `OpenAPI.json`. | Available |
| Project interchange (KerML 10.3) | The 10 library archives, with the errata of ADR 0006 | Available |
| Element-count rules of `sysml-synth` | The parser's element counts (step 1b.4) | Arrives in Phase 1b |

### 2.4 Decisions and their recommended defaults

Each recommended default is a starting hypothesis, not a decision. It is evaluated in the step listed, using the methodology in §2.5.

| # | Decision | Evaluated in | Recommended default |
|---|---|---|---|
| D1 | Parser architecture: hand-written recursive descent producing a `rowan` lossless tree, or a parser generator | 1a.2, **adopted** (ADR 0011) | Hand-written recursive descent with `rowan`, the rust-analyzer approach PROJECT_PLAN §4.2 names. It gives the best error recovery and full control over trivia. *1a.0 spike: `rowan` 0.17.0, the newest stable release, meets the constraints; 0.18 is still alpha.* *Adopted, with a hand-written lexer: `logos` was faster but silent on an unterminated comment; tree-sitter does not build for wasm32 without clang and needs Rust 1.90; `pest` reports only the first error and gives no tree.* |
| D2 | `salsa` version and the boundaries of the incremental query graph (file → tree → elements → resolution → semantics) | 1a.7, **adopted** (ADR 0019) | The current `salsa` release, provided the 1a.0 spike shows it builds for wasm32 and on Rust 1.89. *1a.0 spike: `salsa` 0.28.5 builds and works on wasm32 and on Rust 1.89, with the minimal features `macros` and `inventory`. It needs `Zlib` in `deny.toml` (for `foldhash`, which cannot be turned off). The owner approved adding `Zlib` on 2026-11-20, so the license policy no longer blocks D2.* *Adopted: recomputing everything takes 0.66–1.03 s at 1.1M elements with a minimal resolver alone, while `salsa` takes about 3 ms per edit. Its event log shows exactly what reran, and circular imports converge by fixpoint iteration. Elements are (file, local index) inside the query graph.* |
| D3 | Arena and ID design for the element graph: element ids as API `DataIdentity` ids (PROJECT_PLAN §4.3), and storage of implied versus owned relationships | 1a.6, **adopted** (ADR 0017) | Interned dense indices in an arena, mapped to UUIDs. Implied relationships are kept apart from owned ones, so serialization can include or omit them (KerML 10.3 `includesImplied`). *Adopted: the graph stores the canonical core of the abstract syntax, since all 30 relationship-end attributes redefine `source` or `target`; the typed properties are views generated from `KerML.xmi`. Against a property bag keyed by UUID it is 3 times smaller and 35–65 times faster to walk.* |
| D4 | Diagnostics model: codes, severity, spans, the constraint name and the spec clause | 1a.2, **adopted** (ADR 0012) | One diagnostic type across the parser, resolver and validator. Every semantic diagnostic names its constraint, for example `validateFeatureMultiplicityDomain`. *Adopted: works as a `salsa` query result; ranges are bytes, converted to UTF-16 columns at the Language Server Protocol boundary.* |
| D5 | Snapshot format for the pre-resolved libraries | 1a.11 | A versioned binary format through a serde-compatible crate such as `postcard`, keyed by metamodel IRI, library version and crate version |
| D6 | How CI gates timings (ADR 0007): a dedicated runner or instruction counts | 1a.11 | Instruction counts on GitHub-hosted runners (for example a Cachegrind-based harness), with wall-clock budgets measured but not gated until a dedicated runner exists |
| D7 | Library UUID oracle (§2.3) | 1a.0, **adopted** (ADR 0009) | The library serializations in the SysML-v2-Release repository, if they carry element ids. *Adopted at release 2025-02.* |
| D8 | VS Code extension tooling. The extension is TypeScript, but PROJECT_PLAN v0.7 moved the pnpm/TypeScript workspace to Phase 3. | 1b.8 | A self-contained `editors/vscode` npm package: a thin `vscode-languageclient` wrapper, with its own lockfile and `npm audit` and an npm SBOM in its CI job. The pnpm workspace still waits for Phase 3. |
| D9 | Release 0.1: which crates go to crates.io, their license (Apache-2.0, or MIT OR Apache-2.0 per PROJECT_PLAN §13.1), and whether to sign the 0.1 artifacts | 1b.10 (owner decision) | Publish nothing to crates.io at 0.1; ship binaries through GitHub Releases. Attach the SBOMs (§4.8 requires an SBOM for every release) and sign with Sigstore now, since that is cheap to do from the first release. |
| D10 | How the implied relationships and elements of KerML 8.4.2 are materialized, given that each file's fragment is lowered before resolution (ADR 0019) and that ADR 0017 keeps implied relationships in the graph | 1a.9, **adopted** (ADR 0022) | An elaborated graph per file: the lowered fragment plus its implied relationships. *Adopted instead: an overlay per file, the implied part only, read with the lowered structure as one view. At 1M elements it adds 156 MiB, against 544 MiB for an elaborated graph and 21 MiB for records, which need a second read path in every reader. Edits cost the same with each, 9 ms per file.* |

### 2.5 Decision methodology

1. **When.** A decision is evaluated during its execution step (§2.4), in the step's first PR, before any code that depends on it is written. Choosing early would mean choosing without evidence; choosing late would mean reworking code built on an unexamined default.
2. **How.** The evaluation compares the recommended default with at least one real alternative, against stated criteria:
   - **correctness** against the spec clauses the step implements
   - **the constraints that every crate must meet:** it builds for wasm32, on the minimum Rust version (1.89), and passes `cargo deny`
   - **performance** against the §7 budgets the decision affects
   - **cost:** effort in this step and the later steps, and the maintenance and ecosystem risk of any new dependency
   - **fit** with the existing ADRs and PROJECT_PLAN §4
3. **Evidence.** The comparison rests on evidence, not opinion: a spike, a prototype, a benchmark in `sysml-bench`, or tests run against the corpus. The evidence is kept in the PR or the ADR, so the evaluation can be checked and repeated.
4. **Record.** Each decision gets an ADR in `docs/adr/` with its context, criteria, options, evidence, decision and consequences. The ADR is reviewed like code: CI and `/code-review` on its PR.
5. **Outcome.**
   - If the default meets the criteria, it is adopted.
   - If an alternative is better, the alternative is adopted, and this plan is updated where the change affects steps, estimates or exit criteria.
   - If neither is acceptable, the step stops, and the problem is raised with the owner before work continues.
6. **Owner decisions.** A decision that changes scope, budgets, licensing, release obligations or the schedule needs the owner's approval. That includes D9, and any evaluation whose outcome changes PROJECT_PLAN. Its ADR starts as "Proposed" and becomes "Accepted" when the owner approves the PR.
7. **Revisiting.** Later evidence can reopen an adopted decision, for example a benchmark that misses its budget. The reopened decision is evaluated the same way, and the new ADR supersedes the old one.

## 3. Detailed steps, in recommended sequence

### 3.1 Working rules for every step

- **Test first.** Write the failing tests from the spec rule and the oracle in §2.3, then implement (CLAUDE.md).
- **Decide with evidence.** A step that evaluates a decision (§2.4) starts with that evaluation, following §2.5.
- **Cite.** Every spec behavior cites its clause in a comment, for example `// KerML 8.4.4.6.2`. Every interpretation of an ambiguous clause gets an ADR. Every defect in an OMG file goes into an errata table with a test that fails when OMG fixes it (ADRs 0005, 0006).
- **Small PRs.** One branch and PR per item, signed-off commits, CI green, `/code-review` findings fixed in a further commit, then a rebase merge.
- **WASM parity.** From step 1a.1 on, every test also runs on the WASM build.
- **Performance stays visible.** New benchmarks go into `sysml-bench`, and a budget's `benchmark` field is set once it is measured.
- **Keep records current.** CLAUDE.md, the crate table in PROJECT_PLAN §4.2 and the ADR index are updated as crates arrive.

Each step below gives its goal, the spec clauses, what it delivers, the tests to write first, when it is done, and an indicative effort in weeks of full-time work.

### 3.2 Phase 1a: KerML core

#### 1a.0 Start-up: dependency spikes and the UUID oracle
- **Status:** done 2026-11-20. D7 adopted (ADR 0009); spike results in `docs/proj-mgmt/evidence/1a.0-dependency-spike.md`.
- **Goal:** remove the two unknowns that could change the design, and gather the evidence later evaluations need.
- **Evaluates:** D7.
- **Work:**
  - Spike the candidate dependencies (`rowan`, `salsa`, `uuid` with the `v5` feature, `wasm-bindgen`, `wasm-bindgen-test`). Each must build for wasm32, pass `cargo deny` and work on Rust 1.89. The results are evidence for D1 (1a.2) and D2 (1a.7); they do not decide them.
  - Evaluate D7: find and pin the library UUID oracle.
- **Done when:** the spike results are recorded, and D7's ADR is merged with the oracle in `specs.lock.json`, or with the fallback adopted. The fallback is to test the top-level package UUIDs and the `path()` rules from the clause text alone, and treat full UUID conformance as unverified.
- **Effort:** 0.5

#### 1a.1 WASM test harness (moved from Phase 0)
- **Status:** done 2026-11-20 (ADR 0010). All 77 tests in 13 test binaries pass as WebAssembly under Node, with the same counts as natively. The tests run through `wasm-bindgen-test-runner` rather than `wasm-pack test` (ADR 0010 explains why), and `wasm-pack build` packages `sysml-wasm`, which a Node script calls.
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
- **Evaluates:** D1 (parser architecture) and D4 (diagnostics model), in the step's first PR.
- **Status:** done 2026-11-20. D1 and D4 adopted (ADRs 0011, 0012; evidence in `docs/proj-mgmt/evidence/1a.2-parser-spike.md`). `sysml-syntax` has the hand-written lexer (with the 101 KerML and 128 SysML reserved words, 172 keyword kinds), the event-based framework with recovery and `precede`, and the `Diagnostic` type. The 95 corpus sources lex with no error tokens or lexical diagnostics and losslessly, natively and on WASM. The lexer handles the corpus in about 5 ms and the 1M-element model in about 53 ms (`sysml-bench`, `syntax` benchmark).
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
- **Status:** done 2026-11-20. Every Root and Core production parses into the tree, with a node per element and owned relationship. 65 grammar tests, with inline tree snapshots (`expect-test`), cover each production and the recovery cases, along with 20 clause 7 examples and a token-soup robustness test. ADR 0014 records two readings where the grammar and its examples disagree: `standard` is optional in a library package, and an import needs a visibility. The parser handles the 36 KerML library files in about 2.5 ms (`parse_kerml_library` benchmark).
- **Scope change (owner-approved, 2026-11-20):**
  - Every KerML library file is a `standard library package` and uses Kernel multiplicities, so almost no part of the library uses only Root and Core. The original done criterion could not be met as worded.
  - Pulled forward from 1a.4: packages and library packages (8.2.5.13), multiplicities with literal and feature-reference bounds (8.2.5.11, 8.2.5.8.3–8.2.5.8.4) and prefix metadata (8.2.5.12).
  - The other Kernel constructs are reported as `syntax::not-yet-supported` and skipped as a whole: classes, functions, value parts and filters, for example. The rest of the file still parses.
  - The library criterion is restated: every KerML library file parses losslessly, and every diagnostic is `not-yet-supported`. The count is 525 at the end of 1a.3, and 1a.4 takes it to zero.
- **Goal:** parse the Root and Core concrete syntax into the tree.
- **Clauses:**
  - KerML 8.2.3 (8.2.3.1 elements and relationships, 8.2.3.2 dependencies, 8.2.3.3 annotations, comments, documentation and textual representation, 8.2.3.4 namespaces, imports and namespace elements)
  - KerML 8.2.4 (8.2.4.1 types: specialization, conjugation, disjoining, unioning/intersecting/differencing, feature membership; 8.2.4.2 classifiers; 8.2.4.3 features: typing, subsetting, redefinition, chaining, inverting, type featuring)
- **Tests first:**
  - One parse test per production, from the EBNF and the clause examples, checked against stored tree snapshots.
  - Error recovery: a missing `;`, `}` or name still yields a tree with one diagnostic and the rest of the file parsed.
- **Done when:** every Root and Core production has a test, and the parts of the library files that use only these layers parse with zero errors. *Restated in the scope change above: every KerML library diagnostic is `syntax::not-yet-supported`.*
- **Effort:** 1–1.5

#### 1a.4 KerML grammar: Kernel layer and expressions
- **Status:** done 2026-11-20.
  - **The grammar:** `sysml-syntax` parses the whole KerML grammar. Expressions are a Pratt parser over Table 6. `syntax::not-yet-supported` is gone.
  - **The library:** all 36 KerML library files parse with **zero diagnostics** and round-trip byte for byte, in about 3.6 ms (`parse_kerml_library`).
  - **Tests:** 33 Kernel production and recovery tests with tree snapshots, and a Table 6 precedence and associativity table of 39 cases. Also 16 quoted KerML 7.4 examples, and the 1a.3 tests, updated.
  - **ADR 0015:** four readings of the grammar. The declaration of `inv`, `step`, `expr` and `bool` is optional, as the library's 42 `inv { … }` show. `new` is a contextual keyword. `@@` is two `@` tokens. `InvocationTypeMember` is `InstantiatedTypeMember`.
- **Fuzzing (agreed run time: 5 minutes):**
  - `tests/fuzz.rs` mutates the library: it deletes, duplicates and replaces token spans, and checks that the parser never panics, never stops early and stays lossless.
  - 200 mutants run with every test run, natively and on WASM. The long run is `cargo test -p sysml-syntax --test fuzz -- --ignored`, with `FUZZ_SECONDS` and `FUZZ_SEED`.
  - It found two ways for the parser to stop early. A lookahead scan spent the parser's fuel, and a stray `.` in a function body was taken for an expression that consumed nothing. Both are fixed and kept as regression tests.
  - The 5-minute run then passed 1.78 million mutants.
- **Goal:** complete the KerML grammar.
- **Clauses:**
  - KerML 8.2.5.1–8.2.5.7: data types, classes, structures, associations, connectors (including binding connectors and successions), behaviors and steps, functions, expressions, predicates, Boolean expressions and invariants
  - KerML 8.2.5.8, expressions: operator expressions with their precedence and associativity, primary, base and literal expressions
  - KerML 8.2.5.9–8.2.5.13: interactions and flows, feature values, multiplicities, metadata, packages. Packages, multiplicities with literal and feature-reference bounds, and prefix metadata were pulled forward into 1a.3. This step adds metadata features, element filters, general bound expressions and feature values, and removes `syntax::not-yet-supported`.
- **Tests first:**
  - One test per production.
  - An operator-precedence and associativity test table built from KerML Table 6 (8.2.5.8.1).
  - **All 36 KerML library files parse with zero errors and round-trip byte for byte.**
- **Done when:** the KerML library files pass, and fuzzing (`cargo fuzz` or property tests) finds no panics in an agreed run time.
- **Effort:** 1–1.5

#### 1a.5 Spec snippets in the corpus (moved from Phase 0)
- **Status:** done 2026-11-20 (ADR 0016).
  - **Extraction:** `scripts/extract_spec_snippets.py` takes the code out of the PDFs by font (`pdftohtml -xml`; code is in Courier). It finds 221 KerML snippets (clauses 7 and 8.4) and 702 SysML ones (Part 1, clause 7), in about 1 s, and CI runs it. The rules have unit tests.
  - **Expectations:** `crates/sysml-corpus/snippets.json` marks 20 KerML snippets as fragments and 10 as invalid, each with a reason. The invalid ones are the spec's own mistakes and one extraction artifact.
  - **Results:** all 191 valid KerML snippets parse with zero diagnostics. `sysml_corpus::snippets()` serves the snippets natively and on WASM, and the corpus test counts them per clause.
- **Goal:** add the textual examples from the KerML and SysML specification text to the corpus.
- **Approach:** extract the code from the PDFs with font information (`pdftohtml -xml`; code is set in a monospaced font), or use the parser to accept only text that parses. Mark fragments that are not complete root namespaces, and the examples of deliberately invalid models, with their own `Expectation`.
- **Delivers:** snippet sources in `sysml-corpus`, with a stable id per clause and example, and an ADR on the extraction rules.
- **Tests first:** the corpus test counts the snippets per clause. Every KerML snippet marked valid parses with zero errors. The SysML snippets are only collected here, and must parse by step 1b.1.
- **Done when:** the snippets are in the corpus and the KerML ones pass.
- **Effort:** 0.25–0.5

#### 1a.6 Arena element graph and the mapping to the abstract syntax
- **Status:** done 2026-11-21.
  - **The graph:** `sysml-model` has the element graph of ADR 0017. Its build reads `KerML.xmi` for the 76 stored attributes and the core slot each is a view of. That follows the redefinitions, including the derived ones that hide an inherited end.
  - **The lowering:** the new `sysml-semantics` crate lowers syntax trees into per-file fragments, with a span on every element. ADR 0018 records the readings.
  - **The oracle:** all 36 KerML library files lower to the abstract syntax the OMG pilot built from them (its XMI, ADR 0009), element by element: 24,931 elements in all. Three documented differences remain. Name resolution (1a.7) will add the values of the ends to the comparison.
  - **Tests:** 20 production tests, the element counts by metaclass, walking by ownership and by relationship, spans, and the 191 valid KerML snippets.
  - **Speed:** lowering the library takes about 3.8 ms (`lower_kerml_library`).
- **Goal:** turn syntax trees into model elements and relationships.
- **Evaluates:** D3 (arena and ID design). *Adopted (ADR 0017; evidence in `docs/proj-mgmt/evidence/1a.6-element-graph-spike.md`).*
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
- **Status:** done 2026-11-21, with the scope change the owner approved on 2026-11-21: the implied relationships resolution needs (step 1a.9) are computed here.
  - **The queries:** `sysml-semantics` resolves on `salsa` (ADR 0019). Files are inputs; parsing, lowering, a span-free `structure`, the owned, imported, inherited and visible memberships, effective names, and the resolution of each name are queries. Imports, specializations and naming features may be circular, and converge by fixpoint iteration.
  - **Implied relationships:** `implied_generals` gives each type the specializations of the KerML specialization constraints, with the redundancy rules of 8.4.2, and the end, parameter, result, flow-end and payload redefinitions. Type featurings, binding connectors and the constraints on implied elements stay in 1a.9 (ADR 0020).
  - **The oracle:** every one of the 4003 names in the three KerML libraries resolves to the element the pilot resolved it to, in its XMI (`tests/resolve_oracle.rs`). ADR 0020 records the readings of the spec text.
  - **Tests:** the clause examples of KerML 7.2.5 and 7.3 (visibility, aliases, imports including recursive ones, inheritance, redefinition with effective names, implied relationships), and incrementality from the event log: a layout edit stops at `structure`, and an edit of names reruns only that file's resolution. All of them pass natively and on WASM.
  - **Speed:** the KerML libraries parse, lower and resolve in about 55 ms from nothing. They re-resolve in about 7 ms after a layout edit (`cargo bench -p sysml-bench --bench semantics`).
- **Goal:** resolve every qualified name, incrementally.
- **Evaluates:** D2 (`salsa` and the query graph). *Adopted (ADR 0019; evidence in `docs/proj-mgmt/evidence/1a.7-incremental-query-spike.md`).*
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
- **Status:** done 2026-11-21, in four PRs: one per layer, and, as the owner chose, model-level evaluation (8.4.4.9.8) in a fourth (ADR 0021).
  - **The inventory:** `crates/sysml-semantics/derived.json` lists all 90 derive rules and 84 operations of `KerML.xmi`, each `done`, `open` or `deferred`. A test keeps it equal to the XMI. It also follows the OCL of the 168 `check…` and `validate…` constraints, and of `path()`, to the rules and operations they need: 56 of the 82 derived properties and 48 of the 58 operation names. A needed entry may not be deferred, and a complete layer has no open entry.
  - **Root:** 49 functions in `sysml_semantics::derived`, each with a test from its OCL. `memberElementId` is deferred to the UUID mapping of 1a.11.
  - **Core:** 67 functions (types, classifiers, features), each with a test. They see the implied specializations and redefinitions of 1a.7, and the type featuring a feature membership implies. A recursive import of a type follows its written specializations only (`excludeImplied`), in resolution too.
  - **Kernel:** 34 rules and operations, by 30 functions (connectors, associations, flows, expressions, multiplicities, metadata, packages), each with a test.
  - **Evaluation:** `evaluate` and `modelLevelEvaluable` for every kind of expression, with the evaluable functions of Tables 5 and 7, reflective metadata, `checkCondition`, multiplicity bounds, semantic metadata and filters. Filter conditions now apply in resolution too.
  - **The criterion is a test:** the inventory test marks all three layers complete, so no entry is open, and no needed entry is deferred. Model-level evaluation (23 entries) was the fourth PR.
  - **The oracle:** for all 24,859 elements of the KerML libraries inside a library package, `path()` gives the pilot's `elementId` by KerML 9.1.
- **Goal:** compute the derived properties and operations the semantics and the UUIDs need.
- **Clauses:**
  - KerML 8.3.2–8.3.4: the `derive…` rules (90) and the operations (84) of the KerML metaclasses
  - in particular `path()` (8.3.2.1.2, 8.3.2.1.3, 8.3.2.4.8), qualified names, effective names, inherited memberships, featuring types and feature directions
- **Delivers:** the derived properties as `salsa` queries, computed on read. Derived values sent by clients are ignored (PROJECT_PLAN §4.3, Derived Property Full Conformance).
- **Tests first:** one test per `derive…` rule and operation, from its OCL in the metamodel, prioritizing those used by the constraints in 1a.9 and 1a.10 and by `path()`.
- **Done when:** every rule and operation that the semantic and validation constraints use is covered, and the rest are listed with their status.
- **Effort:** 0.75–1

#### 1a.9 Implied relationships
- **Status:** done 2026-11-21, in five PRs: the overlay with the specialization constraints, then one per table.
  - **The overlay** (ADR 0022): each file's implied relationships, and the elements they own, are a query of their own, which the derived properties read with the lowered fragment as one view. Resolution's implied queries (ADR 0020) feed it, so the 1a.7 and 1a.8 oracles still pass.
  - **The constraints:** all 59 specialization, 8 redefinition, 6 type-featuring and 5 binding-connector constraints that have OCL, each with a check function in `checks.rs` and a golden test. The implied elements are crossing and result chains, snapshots and Cartesian-product features, and binding connectors. Two binding-connector constraints read "TBD" in KerML 1.0 and are deferred. The inventory is `crates/sysml-semantics/checks.json`.
  - **The criterion is a test:** the KerML libraries satisfy all 78 checks, their 53,090 implied elements included (`tests/checks.rs`). ADR 0020 records the readings, among them the places where the OCL and the libraries disagree.
  - **Speed:** on the KerML libraries from nothing, resolution takes 73 ms, the overlays 173 ms and every check 1.5 s, in release.
- **Goal:** add the relationships the semantic constraints require (KerML 8.4.2).
- **Clauses:**
  - KerML 8.4.2 (the four kinds and the two redundancy rules)
  - the tables in 8.4.3.1 (Core) and 8.4.4.1 (Kernel)
  - the per-construct semantics in 8.4.3.2–8.4.3.4 and 8.4.4.2–8.4.4.14
- **Evaluates:** D10 (how implied relationships are materialized). *Adopted (ADR 0022; evidence in `docs/proj-mgmt/evidence/1a.9-implied-materialization-spike.md`).*
- **Work:** implement table by table: specialization constraints (59), then redefinition (8), type-featuring (6), and binding-connector (7) constraints. Implied relationships are kept apart from owned ones (D3). Step 1a.7 already computes the specializations and redefinitions that resolution needs, as queries (ADR 0020). This step materializes them, adds the rest, and keeps the 1a.7 pilot oracle passing.
- **Tests first:**
  - One golden test per table row: a minimal model, and the implied relationships expected.
  - The redundancy rules: the clause's Structure/Class/Classifier example, and same-general duplicates.
  - **The KerML libraries satisfy every `check…` constraint after implication.**
  - Optionally, a behavioral comparison with the pilot on the libraries.
- **Done when:** all 80 KerML `check…` constraints are implemented and tested, and the libraries satisfy them. *All 78 with OCL are; the 2 without are deferred.*
- **Effort:** 1.5–2.5

#### 1a.10 Validation constraints
- **Status:** done 2026-11-23, in two PRs: Root and Core, then Kernel.
  - **The constraints:** all 88, each with a function in `validate.rs` and a test. The KerML libraries produce zero diagnostics, implied elements included. Every violation is an error, with the constraint's name as its code and its metaclass's clause (ADRs 0012, 0023).
  - **Negative tests:** 38 of the constraints cannot be violated by any text; the grammar, the lowering or name resolution ensures them. Their negative tests come with interchange input in step 1b.6, as the owner chose on 2026-11-23 (`validations.json`). The Phase 1a exit therefore has them as a known exception.
  - **Readings** (ADR 0023): inherited members that redefine a common feature are one member; a general that is a feature chain is its feature target, and a feature typed by a behavior is that behavior for its parameters; a binary crossing chains through the other end; an invocation's inputs are its arguments, named ones included; and two validations do not apply to the implied relationships that the semantic constraints require.
  - **Fixes found on the way:** `membership` now has unnamed memberships, and an invoked feature chain is the instantiated type.
- **Goal:** report every KerML well-formedness violation as a diagnostic.
- **Clauses:** the 88 `validate…` constraints of KerML 8.3, from their OCL.
- **Tests first:** one positive and one negative test per constraint. **The KerML libraries produce zero diagnostics.**
- **Done when:** all 88 constraints are implemented, with the constraint name and clause in each diagnostic (D4). *They are, with the 38 negative tests of step 1b.6 as the exception.*
- **Effort:** 0.75–1

#### 1a.11 Library UUIDs, the snapshot and the first budget
- **Status:** in progress, in three PRs as the owner chose on 2026-11-23: the UUIDs, then the D5 and D6 evaluation, then the snapshot, the benchmark and the gate.
  - **UUIDs:** `identity::element_id` gives the KerML 9.1 id of every element of a top-level standard library package, with the KerML or SysML URL prefix by the file's language. All 24,859 written elements of the KerML libraries have the pilot's ids (`tests/derived_oracle.rs`). The 53,090 implied elements get theirs from `path()` too (ADR 0022), and every id in the libraries is distinct. The top-level package ids are also tested against values computed apart from the engine.
- **Goal:** normative library identity, and fast library loading.
- **Evaluates:** D5 (snapshot format) and D6 (CI timing gates).
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
  - The negative tests of the validation constraints that no text can violate (step 1a.10, ADR 0023): a model read from JSON violates each one (`crates/sysml-semantics/validations.json`).
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
- **Evaluates:** D8 (VS Code extension tooling).
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
- **Evaluates:** D9, an owner decision (§2.5): proposed at the start of the step, so the release workflow is built for the approved choice.
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
| A5 | A UUID oracle can be found (§2.3, D7). **Holds** (ADR 0009). | UUID conformance rests on the clause text and is reported as unverified in the 0.1 release notes. |
| A6 | `rowan`, `salsa`, `uuid` (v5) and the LSP and WASM crates build for wasm32, pass `cargo deny`, and work on Rust 1.89. **Spiked in 1a.0:** holds for `rowan`, `uuid` and `wasm-bindgen`. `salsa` also needs `Zlib`, now allowed in `deny.toml` (owner approval, 2026-11-20). The LSP crates are checked in 1b.8. | The affected decision (D1 or D2) adopts an alternative (§2.5), or raising the minimum Rust version goes to the owner. |
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
| X1.10 | Decisions D1–D7 are evaluated per §2.5, each with an accepted ADR | `docs/adr/` |

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
| X2.12 | Decisions D8 and D9 are evaluated per §2.5, each with an accepted ADR, D9 with the owner's approval | `docs/adr/` |
