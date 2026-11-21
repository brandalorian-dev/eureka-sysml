# ADR 0022: Implied relationships as an overlay on each fragment (decision D10)

- Status: Accepted
- Date: 2026-11-21
- Decision: D10 of the Phase 1 execution plan, evaluated in step 1a.9 under its §2.5 methodology; the owner chose the option on 2026-11-21
- Evidence: `docs/proj-mgmt/evidence/1a.9-implied-materialization-spike.md`

## Context

Step 1a.9 adds the relationships that KerML's semantic constraints require (8.4.2): 59 specialization, 8 redefinition, 6 type-featuring and 7 binding-connector constraints. ADR 0017 stores implied relationships in the graph, kept apart from the owned ones, and read through `isImpliedIncluded`. ADR 0019 lowers each file's fragment before resolution, and resolution computes the implied specializations and redefinitions it needs as queries (ADR 0020).

The two do not meet. Implied relationships depend on resolution across files, so they cannot go into the lowered fragment without a cycle, and most of their targets are in another file. 13 constraints also need implied *elements*: binding connectors, result parameters that are not written, and `snapshots` features. Those need an identity that the library UUIDs of step 1a.11 can use.

## Criteria

From execution plan §2.5:
- **Correctness:** the implied relationships of KerML 8.4.2 and its tables in 8.4.3.1 and 8.4.4.1, kept apart from what a modeler wrote, all or none per element (`isImpliedIncluded`, ADR 0017), and interchange with or without them (`includesImplied`, KerML 10.3).
- **Workspace constraints:** builds for wasm32, works on Rust 1.89, passes `cargo deny`. No new dependency is needed.
- **Performance:** re-resolve after a typical commit < 1 s on the server (1M elements); keystroke to diagnostics < 100 ms and browser memory < 1 GB for a 50k-element working set.
- **Cost and fit:** one way to read a model for the derived properties (ADR 0021), validation and interchange; a fit with ADRs 0017 and 0019.

## Options

1. **A, an elaborated graph:** a per-file query clones the lowered graph and adds the implied relationships and elements to it.
2. **B, records:** a per-file query returns the implied relationships and elements as records beside the graph.
3. **C, an overlay:** a per-file query returns a small graph of the implied part only. It shares the lowered structure, which it does not copy, and readers see the two as one view.

## Evidence

All results come from the evidence record. The figures are at 1M elements; at 50k elements every option is within the budgets.

- **Time does not separate the options.** An edit costs the same with each: 9 ms for the edited file's queries, and 265 ms for every file's.
- **Memory does.** Materializing adds 544 MiB with A (a copy of every graph), 156 MiB with C, and 21 MiB with B. The peaks are 1.95, 1.54 and 1.45 GB.
- **Reading.** A and C give every reader one graph or view, with `Graph`'s accessors. B needs a second path in every reader.
- **Cross-file targets** need an endpoint to another fragment in A and C. B holds `Global`s.
- **A scaling bug in resolution, found by the spike and fixed apart** (PR #23, ADR 0020 decision 5a). What a namespace shows evaluated its private imports. Around a ring of 1,300 packages, that made resolution take 281 s and an edit 276 s; they now take 5.9 s and 9 ms.

## Decision

Adopt C, the overlay.

- **The overlay.** A per-file query, `implied(file)`, gives the file's implied relationships and the implied elements they own. It is computed from resolution's queries: `implied_generals`, `implied_redefinitions`, and the type-featuring and binding-connector constraints that 1a.9 adds. It is a `Graph` of those elements alone, with each implied relationship's owner and its ends. It backdates on its own equality, so an edit that changes no implied relationship stops there.
- **Identity.** An implied element is a `Global` of its file, with an index after the lowered fragment's: `lowered.len() + k`, in a deterministic order. That order is the owners in fragment order, then each owner's constraints in the order of KerML's tables. The ids are stable while the lowered fragment is.
- **Endpoints.** An end of an implied relationship can be an element of the lowered fragment, of the overlay, or of another file. The overlay holds such ends as `Global`s. `Graph`'s own endpoints, which name elements of their fragment, are unchanged.
- **The view.** Readers go through a view of a file: the lowered structure plus its overlay, with `Graph`'s accessors and the same indices. An element's `ownedRelationship` is its owned relationships followed by its implied ones, since the implied are all included (`isImpliedIncluded`, KerML 8.3.2.1.2). The derived properties (ADR 0021), validation (1a.10) and interchange (1a.12) read the view. Interchange can leave the overlay out as a whole (`includesImplied`, KerML 10.3).
- **No cycle.** Resolution goes on reading the lowered structure and its own queries, and never the view. The overlay depends on resolution, never the reverse.
- **UUIDs.** An implied element's `path()` (ADR 0021) counts its position among its owner's relationships, the implied ones after the owned ones. Step 1a.11 gives it a UUID from that path, as KerML 9.1 gives every other element of a library.

## Consequences

- Step 1a.9 starts with the view and the overlay query, and moves the derived properties from the lowered structure to the view. It then adds the implied relationships table by table.
- The overlay costs about 156 MiB per 1M elements. Most of it is `Graph`'s element size, about 400 bytes with its vectors. A more compact element would shrink the overlay and the lowered graphs alike. That is a candidate for the library snapshot work (D5, step 1a.11), and it is measured there.
- Resolution's own implied queries and the overlay compute the same relationships. The overlay is built from those queries, so the two cannot disagree.
- `Graph` needs no cross-fragment endpoint: cross-file ends exist only in the overlay.
