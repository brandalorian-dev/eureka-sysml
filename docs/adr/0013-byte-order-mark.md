# ADR 0013: A leading byte-order mark

- Status: Accepted
- Date: 2026-11-20
- Context: step 1a.2, from the PR #11 code review

## Context

KerML 8.2.2.1 defines the input as a sequence of characters, and leaves its encoding to the implementation. Editors on Windows often save UTF-8 files with a leading byte-order mark (U+FEFF, bytes `EF BB BF`). Before this decision the lexer reported such a file as an unexpected character at offset 0, so a valid model failed to parse because of how it was saved.

The tree must stay lossless (ADR 0011): the formatter writes back exactly what it read.

## Decision

- **A byte-order mark at offset 0 is trivia.** The lexer emits it as a `ByteOrderMark` token, with no diagnostic. It is kept in the tree like white space, so the file round-trips byte for byte, mark included.
- **Anywhere else, U+FEFF is an unexpected character** (`syntax::unexpected-character`), as KerML 8.2.2 allows it in no token outside names and strings. Inside a quoted name or a string it is an ordinary character.
- Only UTF-8 is accepted: the input is a Rust `&str`. Decoding other encodings, if ever needed, belongs to whoever reads the file.

## Consequences

- Files saved with a byte-order mark parse cleanly, and the formatter keeps the mark.
- `SyntaxKind::ByteOrderMark` is trivia (`is_trivia`), so productions never see it.
