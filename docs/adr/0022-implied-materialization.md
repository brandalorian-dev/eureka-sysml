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

## Amendment: step 1a.9, the specialization constraints (2026-11-21)

Implementing the overlay and the 59 specialization constraints showed where the decision needs more detail. Nothing here changes option C.

- **The query** is `overlay(project, file)`, read through `view(project, file)` (`crates/sysml-semantics/src/materialize.rs`). For each element of the fragment, in order, it adds the relationships of `implied_generals` and then those that the element's semantic metadata implies.
- **Implied elements.** An implied CrossSubsetting (`checkFeatureCrossingSpecialization`) targets a feature chain, which KerML 8.4.3.1 Table 9 Note 2 describes. The overlay builds that chain as the owned related element of the CrossSubsetting. The chain's first feature is owned by the chain through an OwningMembership. It is typed by the other end's types and featured by the end's owning type. It also gets the feature constraints its typing implies, such as `Base::things` and `Occurrences::occurrences`, filtered by KerML 8.4.2. The relationships inside an implied element are part of it: only the relationship that an element of the fragment owns is marked `isImplied`. An implied relationship has a source end only where its metaclass stores one, as in the fragment. For an association with more than two ends, the first feature's type is a Cartesian product, which the type-featuring constraints construct later in step 1a.9.
- **Chains for results.** A FeatureChainExpression's result subsets a chain of its first input parameter and its `sourceTargetFeature` (`checkFeatureChainExpressionResultSpecialization`). The overlay builds that chain in the same way: a Feature with two FeatureChainings, owned by the implied Subsetting. Resolution inherits from the chain's last feature, its `featureTarget`.
- **Type featurings** (KerML 8.4.2, the type-featuring constraints). Each is a TypeFeaturing owned by the feature, after its implied specializations and redefinitions. Resolution's `implied_featurings` and `featuring_types` compute them, as `implied_generals` computes the specializations, and the derived `featuringType` reads them through the view. Two kinds of featuring type are elements the overlay constructs, as ownedRelatedElements of the TypeFeaturing:
  - for a variable feature, a feature that redefines `Occurrences::Occurrence::snapshots` and is featured by the owning type (8.4.4.3);
  - for the owned cross feature of an end of more than two, a Cartesian-product feature of the other ends' types (8.4.4.5.1). It also types the first feature of the end's crossing chain.
  Other elements can have such a constructed type as a featuring type too: a multiplicity, bound or value copies its owner's. Those ends are filled in after the file's overlay is built, from this overlay or from another file's (`constructed`).
- **Connectors.** `checkConnectorTypeFeaturing` implies the `defaultFeaturingType` of a connector without an owning type or a written featuring (8.4.4.1 Table 11 Note 2). That needs the derived featuring types of the related features, so the overlay reads the view for these connectors. This is a third fixpoint cycle, entered only by such connectors.
- **Resolution and implied elements.** An implied element has an index after the fragment's. The resolution queries keyed by element give it no names, memberships, generals or redefinitions (`in_fragment`), so a derived property of an implied element never reads past the lowered fragment.
- **Cycles.** The decision said that resolution never reads the view. Two of its readings do, through the derived properties, and the connector featurings above are a third:
  - a filter condition, which resolution applies to a package's imports by model-level evaluation (ADR 0021);
  - the base type of semantic metadata, which the overlay evaluates (`checkMetadataFeatureSemanticSpecialization`, KerML 8.3.4.12.3).
  Both are fixpoints from an empty overlay (`cycle_initial`), which converge because implied relationships are only added. A file without filtered packages or metadata features does not enter either cycle. Resolution's own test of whether a package has filter conditions reads the lowered fragment, so the cycle starts only when there is a condition to evaluate.
- **What semantic metadata implies.** Resolution does not see these specializations, since computing them needs evaluation. A name inherited only through them does not resolve. The KerML libraries have none, and the readings for SysML are left to Phase 2. The overlay adds a specialization only to an annotated type of the same file. Metadata written `about` a type in another file implies nothing for it yet. Validation (step 1a.10) reports the check instead.
