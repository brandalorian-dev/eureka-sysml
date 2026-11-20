# ADR 0012: Diagnostics model (decision D4)

- Status: Accepted
- Date: 2026-11-20
- Decision: D4 of the Phase 1 execution plan, evaluated in step 1a.2 under its §2.5 methodology
- Evidence: `docs/proj-mgmt/evidence/1a.2-parser-spike.md`

## Context

Every phase of the engine reports problems: the lexer and parser (step 1a.2), name resolution (1a.7), the implied-relationship constraints (1a.9) and the validation constraints (1a.10, 1b.3). The reports reach users through the CLI (1b.7), the language server (1b.8) and the browser editor (Phase 3). The validation constraints have names in the metamodel, for example `validateFeatureMultiplicityDomain`, and CLAUDE.md requires spec behavior to cite its clause.

## Criteria

- **One shape for all phases**, so the CLI, the language server and the web UI each handle diagnostics once.
- **`salsa` compatibility.** Diagnostics are query results (D2), so the type must be `Clone + Eq + Hash`.
- **Positions that work for every consumer.** The Language Server Protocol counts columns in UTF-16 code units by default, while the parser works in bytes.
- **Traceability:** a stable code, the metamodel constraint name for constraint violations, and the spec clause.
- **The workspace constraints** (wasm32, Rust 1.89, `cargo deny`), and no presentation concerns in the engine.

## Options

1. **One diagnostic type for all phases (the recommended default):** a code, a severity, a byte range, a message, and optionally the spec clause and related locations.
2. **One error type per phase,** converted to a common form only at each consumer.
3. **A diagnostic-reporting crate's type as the model,** such as `miette` or `codespan-reporting`.

## Evidence

- A `Vec<Diagnostic>` deriving `Clone, PartialEq, Eq, Hash, Debug` works as a `salsa` 0.28 tracked query result: two diagnostics for an input with two errors, and none after the input is fixed.
- After `package 'Café'`, byte offset 26 is UTF-16 column 25. Storing byte ranges and converting at the protocol boundary gives the right column; storing any one consumer's form would be wrong for the others.
- Option 2 means each of the three consumers converts from each phase's type: one conversion per phase per consumer, and codes and severities that drift apart.
- The crates of option 3 are built for presenting diagnostics in a terminal. Using one as the engine's model would tie the core library, which also runs in the browser, to one presentation. The CLI can still use such a crate to render the common type (step 1b.7).

## Decision

Adopt option 1. One `Diagnostic` type, in `sysml-syntax`, used by every phase:

- **`code`:** a stable identifier.
  - For syntax errors, `syntax::<kebab-name>`, for example `syntax::expected-semicolon` or `syntax::unterminated-comment`.
  - For a constraint violation, the metamodel constraint name, for example `validateFeatureMultiplicityDomain` or `checkDataTypeSpecialization`.
  - For name resolution, `resolve::<kebab-name>`.
- **`severity`:** error, warning, information or hint, the four severities of the Language Server Protocol.
- **`range`:** a byte range in the source text (a `rowan` `TextRange`), together with the file it belongs to.
- **`message`:** a human-readable sentence.
- **`clause`:** optionally, the spec clause, for example `KerML 8.2.2.2`.
- **`related`:** optionally, further locations with a message each.

The type derives `Clone, PartialEq, Eq, Hash, Debug`. Line and column numbers are computed only at a boundary: the language server converts to UTF-16 columns, and the CLI to whatever its renderer needs. The model depends on no presentation crate.

## Consequences

- Step 1a.2 introduces the type with the lexer's and parser's first diagnostics. Later phases add codes, not new types.
- The language server (1b.8) and the CLI (1b.7) each implement one conversion. The CLI chooses its rendering crate then.
- Every constraint violation names its constraint, so a user, or a test, can find the rule in the metamodel and the spec.
