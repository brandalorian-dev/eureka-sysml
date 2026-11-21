# ADR 0019: Incremental queries with salsa, and their boundaries (decision D2)

- Status: Accepted
- Date: 2026-11-21
- Decision: D2 of the Phase 1 execution plan, evaluated in step 1a.7 under its §2.5 methodology
- Evidence: `docs/proj-mgmt/evidence/1a.7-incremental-query-spike.md`, and the step 1a.0 dependency spike

## Context

From step 1a.7, `sysml-semantics` resolves names, and later steps add derived properties (1a.8), implied relationships (1a.9) and validation (1a.10). Each must stay correct as text changes, keystroke by keystroke in the editor, and commit by commit on the server. PROJECT_PLAN §4.2 names `salsa` for this. The execution plan's recommended default is the current `salsa` release, if it meets the workspace constraints; the 1a.0 spike showed that 0.28.5 does.

D2 is also where the query graph's boundaries are decided: which results are queries, which of them can cut a recomputation short, and how elements are identified across them. ADR 0017 left this open: per-file fragments, and possibly addressing elements by (file, local index).

## Criteria

From execution plan §2.5:
- **Correctness:** the resolution rules of KerML 8.2.3.5, including circular imports and specializations. A test must be able to observe incrementality, from `salsa`'s event log (the plan's step 1a.7).
- **Workspace constraints:** builds for wasm32, works on Rust 1.89, passes `cargo deny`.
- **Performance:** re-resolve after a typical commit < 1 s on the server (1M elements), and keystroke to diagnostics < 100 ms in the browser (50k elements), each with all the semantics still to come.
- **Cost and fit:** less code than doing the tracking by hand, no unsafe code, and a fit with ADRs 0011 and 0017.

## Options

1. **A, the recommended default:** `salsa` 0.28.5, minimal features (`macros`, `inventory`).
2. **B: no framework.** Recompute parsing, lowering and resolution after each change.
3. **C: hand-written incrementality,** caching each file's results by its text, with our own dependency tracking. Option A's evidence makes it unnecessary: it would rebuild what `salsa` provides (dependency tracking, early cutoff, fixpoint cycles, and an event log), and its correctness would have to be proven separately. It was therefore compared on its cost and not prototyped.

## Evidence

All results come from the evidence record, which has the full sources.

- **Recomputing everything misses the budget.** On a synthetic KerML model of 1.1M elements, B takes 0.66–1.03 s with only a minimal resolver: at or over the re-resolve budget before any of the semantics still to come. With `salsa`, an edit to one file costs about 3 ms (and about 1 ms at 50k elements).
- **`salsa`'s event log shows exactly what reran.** Retyping a feature reruns the queries of the edited file only. Adding a classifier also reruns the resolution of the two files that import it.
- **Cycles converge.** Circular public imports resolve by `salsa`'s fixpoint iteration (`cycle_initial`), and resolve again after an edit.
- **Early cutoff needs projections.** A change of white space alone still reruns resolution, because a lowered fragment has spans and no equality. A span-free projection between lowering and resolution fixes that.
- **Memory:** about 0.65 KB per element when everything is memoized (719 MiB at 1.1M elements, 36 MiB at 55k).
- **The constraints are met.** It builds for wasm32 and on Rust 1.89. The only license outside the policy is `Zlib` (`foldhash`), which the owner approved on 2026-11-20.

## Decision

Adopt `salsa` 0.28.5, with default features off and `macros` and `inventory` on, as the query engine of `sysml-semantics`. The boundaries are these:

- **Inputs.** A source file (its text, language and path), and the set of files of a project, with its library units. The three KerML libraries are one unit, which may be circular.
- **Per-file queries:**
  - `parse`, the tree (ADR 0011);
  - `lower`, the fragment (ADR 0017, ADR 0018).
- **Projections that backdate**, so that an edit stops where nothing it changes is used:
  - a file's named memberships by namespace (its exports);
  - its references to resolve, without spans.
- **Scopes:** the local, visible and full resolution of KerML 8.2.3.5.2–8.2.3.5.4, per namespace. Queries that can meet a cycle, such as visibility through public imports and inherited memberships through specialization, use fixpoint iteration from the empty set. KerML 8.2.3.5.1 lets the visited namespaces be skipped; iteration to a fixpoint gives the same result.
- **Resolution** produces the resolved ends of each file.
- **Identity.** Inside the query graph, an element is (file, local index), with no global re-assembly; the indices are those of its fragment (ADR 0017). The `DataIdentity` UUIDs are mapped to these identities outside the query graph, because queries must be pure. The mapping is v5 ids for the libraries (step 1a.11), and the ids the editor and the store keep (PROJECT_PLAN §4.3).
- **Tests** observe incrementality through `salsa`'s event callback (`WillExecute`).

## Consequences

- `salsa` 0.28 becomes a dependency of `sysml-semantics` (Apache-2.0 OR MIT; `foldhash` is Zlib, which is allowed).
- The lowering runs inside a query, so its id source must be deterministic there. A query gives each element an id from its file and index, and the real UUIDs are assigned by the mapping layer.
- Memory grows with everything memoized: about 0.7 GB for a 1M-element branch on the server. If that becomes a problem, the library snapshot (D5, step 1a.11) and `salsa`'s LRU limits on large queries are the levers.
- Upgrading `salsa` is a reviewed change: its API moves between minor releases.
