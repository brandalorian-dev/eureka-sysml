# ADR 0011: Parser architecture (decision D1)

- Status: Accepted
- Date: 2026-11-20
- Decision: D1 of the Phase 1 execution plan, evaluated in step 1a.2 under its §2.5 methodology
- Evidence: `docs/proj-mgmt/evidence/1a.2-parser-spike.md`, and the step 1a.0 dependency spike

## Context

`sysml-syntax` turns KerML and SysML text into a syntax tree (PROJECT_PLAN §4.2). The parser serves several users:
- the CLI and the server, over whole models of up to 1M elements
- the language server and the browser editor, keystroke by keystroke, where the text is usually *invalid* while the user types
- the formatter, which must keep comments and layout (the round trip in Phase 1b)

The lexical structure is KerML 8.2.2, identical in SysML except for its reserved words (SysML 8.2.2.1.2). The grammars are KerML 8.2.3–8.2.5 and SysML 8.2.2. They share most productions, including the whole expression language.

## Criteria

From execution plan §2.5:
- **Correctness:** the KerML 8.2.2 lexical rules. Notes are trivia, but regular comments are tokens (they become `Comment` elements), symbols are matched longest first, and the reserved words differ per language. The tree must be lossless (byte for byte), so the formatter can keep everything.
- **Error tolerance:** report every error, at the right place, and still produce a tree for invalid text. Completion and hover depend on it.
- **Workspace constraints:** builds for wasm32-unknown-unknown, works on Rust 1.89, passes `cargo deny`.
- **Performance:** the §7 budgets, in particular keystroke → diagnostics < 100 ms on a 50k-element working set.
- **Cost and fit:** one parser for both languages, no generated code committed (CLAUDE.md), and a tree that `salsa` (D2) and the rest of the engine can use.

## Options

1. **A, the recommended default:** a hand-written lexer, and a hand-written, event-based recursive-descent parser with error recovery, producing a `rowan` lossless tree. This is the rust-analyzer approach PROJECT_PLAN §4.2 names.
2. **B: tree-sitter**, a widely used incremental parser generator with error recovery.
3. **C: a parser generator producing a tree, such as `pest`** (PEG).
4. **Lexer variant of A:** generate the lexer with `logos` instead of writing it by hand.

`chumsky`, a parser-combinator library advertising error recovery, was considered. Its 1.0 is still an alpha (`1.0.0-alpha.8`), so it was not evaluated in depth.

## Evidence

All results come from the evidence record, which has the full sources.

- **The lexer, by hand or with `logos`.** Both lexed all 95 corpus sources (174,122 tokens) with no error tokens, losslessly, and produced identical tokens, on the corpus and on the 1M-element synthetic model. `logos` was about 1.6 times faster: 3 ms against 5 ms for the corpus, and 30 ms against 50 ms for the 1M model. But it lexed an unterminated `/* …` silently as `/`, `*` and names. It also needed `allow_greedy` for the single-line note rule, and cannot switch keyword tables per language. Both speeds are far inside the budgets: a 50k-element working set (about 0.4 MB) lexes in under 3 ms.
- **tree-sitter** fails two constraints. Its C runtime does not build for wasm32-unknown-unknown without a `clang` wasm32 toolchain, and the crate needs Rust 1.90. Its grammars are JavaScript compiled into a generated C parser, and its tree is its own type, not one our engine and `salsa` would share.
- **`pest`**, on an input with two independent errors, reported only the first, at the wrong place: "expected feature" at the start of the next feature, not the missing `;`. It produced no tree, and its implicit white space is silent, so its trees are not lossless.
- **Option A**, on the same input, reported **both** errors at the right places (the missing `;` right after `x`, and the missing name). It recovered a **lossless** tree with both packages and all three features. A `rowan` tree over every corpus token builds in 8 ms, and over the 1M model (1.9M tokens) in 142 ms.
- **`rowan` 0.17** meets the workspace constraints (step 1a.0 spike).

## Decision

Adopt option A, with a hand-written lexer.

- **Lexer.** One hand-written lexer implements KerML 8.2.2. The reserved-word table is chosen by language (KerML 8.2.2.6 or SysML 8.2.2.1.2). Symbols are matched longest first (8.2.2.7). Every byte belongs to exactly one token, and malformed input becomes error tokens with a diagnostic, never a failure. White space and notes (`//`, `//* … */`) are trivia; regular comments (`/* … */`) are tokens.
- **Parser.** A hand-written, event-based recursive-descent parser. Productions emit start, finish and token events; a separate builder turns them into a `rowan` green tree. Trivia is attached when the events are built. On an error, the parser records a diagnostic (ADR 0012), wraps the unexpected tokens in an `ERROR` node, and resumes at a synchronizing token of the enclosing production, so one error never hides the next.
- **One parser for both languages.** KerML and SysML share the lexer, the framework and the shared productions (the expression language, names, multiplicities). SysML adds its own productions in Phase 1b.
- **No generator, so no generated parser code.** The token and node kinds are a hand-written enum.

The lexer is the one prototyped in the evidence spike, which matches `logos` token for token on the whole corpus. It is chosen over `logos` for its error reporting, its keyword table per language and the absence of a code generator. Its speed is well within the budgets. If profiling later shows lexing matters, the hand-written lexer can be optimized; the spike's symbol matching is a linear scan.

## Consequences

- Step 1a.2 implements the lexer and the event framework in `sysml-syntax`, and steps 1a.3–1a.4 add the KerML productions. Every production has tests from the clause EBNF, including recovery tests.
- Error recovery is hand-written per production, which is more code than a generator would need. It is also where the quality of the editor experience comes from.
- `rowan` becomes a dependency of `sysml-syntax` (MIT OR Apache-2.0).
- The lexer and parser get benchmarks in `sysml-bench` on the corpus and the synthetic models, so performance stays visible.
