# ADR 0024: The pre-resolved library snapshot (decision D5)

- Status: Accepted
- Date: 2026-11-24
- Decision: D5 of the Phase 1 execution plan, evaluated in step 1a.11 under its §2.5 methodology
- Evidence: `docs/proj-mgmt/evidence/1a.11-library-snapshot-and-timing-gates.md`

## Context

PROJECT_PLAN §4 gives the `sysml-libraries` crate the job of loading and resolving the 10 standard libraries once, computing their UUIDs (KerML 9.1, step 1a.11), and writing a pre-resolved binary snapshot that the server and the browser load. Its §7 budget is to load the libraries from that snapshot in < 300 ms on the server and < 1 s in the browser. The execution plan's default for D5 is a versioned binary format through a serde-compatible crate such as `postcard`, keyed by the metamodel IRI, the library version and the crate version.

The engine computes everything as `salsa` queries over the source text (ADR 0019): parsing, lowering, resolution, and the overlay of implied relationships (ADR 0022). So the decision is not only a byte format. It is what the snapshot holds, and how the queries use it.

## Criteria

From execution plan §2.5:
- **Correctness:** a library loaded from the snapshot answers every query as one loaded from its text does.
- **Workspace constraints:** builds for wasm32, works on Rust 1.89, passes `cargo deny`.
- **Performance:** the `library-load` budget, for the 10 libraries.
- **Cost and fit:** the work in this step and later ones, new dependencies, and the fit with ADRs 0017, 0019 and 0022.

## Options

1. **No snapshot:** load the libraries from their text.
2. **salsa's own persistence** (salsa 0.28, feature `persistence`): serialize a database's inputs and memos with serde, and load them into a new database.
3. **A hand-built library image:** per file, the lowered graph, every name end already resolved, and the overlay. Three queries, `lowered`, `resolve_end` and `overlay`, answer from it when a file has one. Every other query is computed when asked, as from text.

For options 2 and 3, the formats compared were postcard (the default) and bincode, with CBOR and JSON where those two could not be used.

## Evidence

All figures are from the evidence record, on the 36 files of the KerML libraries. The SysML libraries cannot be loaded before Phase 1b, so the 10 libraries are scaled by their text, which is 7.6 times the KerML libraries'.

| KerML libraries, until resolved with their overlays | Size | Native | WASM (Node) | 10 libraries, scaled |
|---|---|---|---|---|
| From text | 186 KB of text | 224–248 ms | 229–329 ms | about 1.8 s and 2.5 s |
| salsa persistence, JSON | 135 MB | 1.2 s to deserialize | | about 0.7 GB |
| Image, postcard | 4.8 MB | 25–36 ms | 24–32 ms | about 190 ms, 250 ms and 37 MB |
| Image, bincode | 11.7 MB | 23–24 ms | | |

- **From text** the libraries are six times over the server budget, and parsing and lowering are only 4% of the time. A snapshot must carry resolution and the overlays.
- **salsa's persistence** stores every memo with its dependencies. It is 700 times the text, and slower to load than the text itself. Its serializer also writes sequences without a length, which postcard and bincode cannot encode.
- **The image** loads ten times faster than text, natively and as WASM, within both budgets at the scale of 10 libraries. postcard loads as fast as bincode, at less than half the size.
- **The image leaves the rest lazy.** Queries it does not carry, such as a library type's inherited memberships, are computed when a user model asks for them, once per project. Taking all of them took 0.75 s in the spike. They belong to the budgets of language features and re-resolution, not to loading.
- **The image isolates the libraries.** Its answers do not depend on the project that asks, so a user's packages cannot change how library names resolve. That is what KerML 10.3's project usages mean for a library.

## Decision

Adopt option 3, the image, in postcard.

- **Content.** For each library file:
  - its lowered fragment: graph, root and spans, as `lowered` gives it;
  - the resolution of each name end, as `(relationship, slot, position)` with its membership entry;
  - its overlay.
  Each reference to an element of another file is a `(file position, index)` pair, mapped to the loading database's files.
- **Use.** A `SourceFile` can carry a loaded image. `lowered`, `resolve_end` and `overlay` answer from it. Everything else is the same query code as for text, so the two cannot disagree on anything the image does not hold.
- **Format and key.** postcard, behind a header with a format version, the metamodel IRI, the library release (the pinned archives of `specs.lock.json`) and the crate version. A header that does not match is not loaded; the libraries are then loaded from text, which is slower but correct.
- **Where it is made.** The snapshot is produced from the pinned archives in `docs/specs/` at build time, never committed (CLAUDE.md), by the `sysml-libraries` crate. That crate also gives the element UUIDs (KerML 9.1).

## Consequences

- Step 1a.11's third PR adds `sysml-libraries`: writing and loading the image, the optional image on `SourceFile`, and the three short-circuits. Its round-trip test checks that a library loaded from the image resolves every name to what text gives, and has the same overlay.
- serde becomes a dependency of `sysml-model` and `sysml-semantics`, for the image's types only. `postcard` is new; its license and its dependencies must pass `cargo deny`.
- The budget is met at the scale of 10 libraries only by scaling. Phase 1b loads the SysML libraries and measures it; if it misses, this decision is reopened (§2.5).
- A user model's first queries about library elements are not free. Their cost is measured with the language-feature and re-resolution budgets of Phase 1b.
