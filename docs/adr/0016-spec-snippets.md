# ADR 0016: Spec snippets in the corpus

- Status: Accepted
- Date: 2026-11-20
- Context: step 1a.5, moved from Phase 0 (PROJECT_PLAN §6)

## Context

The KerML and SysML specifications illustrate the language with hundreds of textual examples. They are test cases written by the authors of the language: a conforming parser must accept the valid ones. PROJECT_PLAN §6 moved them out of Phase 0 because the per-clause text (`scripts/extract_spec_text.py`, `pdftotext -layout`) does not separate code from prose. In that text, code, keywords in sentences, bullet lists and table cells all look alike.

The specifications may not be committed (ADR 0004), so neither may the snippets. They must be derived from the pinned PDFs on every machine, in the same way every time.

## Decision

`scripts/extract_spec_snippets.py` extracts the snippets into `docs/specs/snippets/` (git-ignored): one file per snippet and an `index.json`. CI runs it after fetching the specs. It needs `pdftohtml` (poppler-utils, 24.02 on Ubuntu 24.04 both locally and on `ubuntu-latest`). The rules are these.

1. **Fonts.** `pdftohtml -xml` gives every run of text with its position and font. Both specifications set code in Courier New, prose in Times New Roman and headings in Arial.
2. **Code lines.** A line whose runs are all Courier is a code line. A sentence with a keyword in Courier also has Times runs, so it is prose.
3. **Blocks.** Consecutive code lines form a block. A prose line ends it, but the page header and footer (the top 7% and bottom 8% of a page) do not, so a block that continues on the next page stays whole.
4. **Clause.** A block belongs to the last numbered Arial heading of at least 14 points before it.
5. **Scope.** The scope is KerML clauses 7 (language description) and 8.4 (semantics, whose examples show the implied relationships), and SysML Part 1 clause 7. The other clauses hold EBNF (8.2), OCL (8.3), JSON (KerML 10) and library listings, which are not models. Within the scope, a block is kept only if it has `;`, `{`, `}` or the end of a comment, and does not start like an EBNF production. That removes keyword tables and productions quoted in the text.
6. **Wrapped words.** A line at the start or end of a block that is one bare identifier, in one run, is the last word of a wrapped sentence, set in Courier. It is dropped.
7. **Layout.** The text keeps its indentation (from the runs' positions, in characters of the median Courier width) and its blank lines (a gap of more than 1.6 times the smallest line spacing).

**Ids** are `<spec>/<clause>/<n>`, for example `kerml/7.2.5.4/1`, numbering the snippets of each clause from 1. They are stable while the PDFs and these rules are. `sysml-corpus` tests the count per clause, so any change shows up as a test failure.

**Expectations.** Every snippet is `Expectation::Valid` unless `crates/sysml-corpus/snippets.json` says otherwise. That table holds only ids and a reason, never spec text. There are two other expectations:
- `Fragment`: part of a model, or a schematic with placeholders such as `...`, `{ body }` or `expr`. A fragment is not a complete root namespace, so it is not required to parse.
- `Invalid`: not valid KerML. The parser must report an error. This covers the spec's own mistakes, such as `readonly`, `import C;` (ADR 0014), `redefine step` or a missing `{`, and one extraction artifact.

A KerML snippet marked valid must parse with zero diagnostics. A snippet marked invalid must have at least one, so a stale entry fails the test once the parser starts to accept it. The table is the place to record a judgment, and each entry is reviewed against the PDF.

## Alternatives

- **`pdftotext` and the parser as the filter**, keeping whatever parses. This was rejected: it cannot tell a parser bug from prose, and a snippet that should parse but does not would silently disappear.
- **Hand-copied examples**, as in the step 1a.3 tests. These were not extended: hand copies do not scale to 900 snippets, and they would put spec text in the repository.

## Consequences

- 221 KerML snippets: 191 valid, all parsing with zero diagnostics; 20 fragments; 10 invalid. There are also 702 SysML snippets, collected only; they must parse by step 1b.1, when their expectations are added.
- The `rust`, `wasm` and `msrv` CI jobs install poppler-utils and run the extraction, which takes about a second. On wasm32, `sysml-corpus/build.rs` embeds the snippets.
- On a new machine, `python scripts/extract_spec_snippets.py` must run once after `fetch_specs.py fetch`. Until it has, `sysml_corpus::snippets()` returns an error that names the command.
- Rule 5's scope can grow, for example to SysML clause 8.4, by adding the clause to `DOCS`. The per-clause counts then change in a reviewed commit.
