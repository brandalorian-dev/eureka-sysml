# ADR 0017: Element graph: arena, identity and implied relationships (decision D3)

- Status: Accepted
- Date: 2026-11-20
- Decision: D3 of the Phase 1 execution plan, evaluated in step 1a.6 under its §2.5 methodology
- Evidence: `docs/proj-mgmt/evidence/1a.6-element-graph-spike.md`

## Context

Step 1a.6 turns syntax trees into model elements: the instances of the KerML abstract syntax (KerML 8.3.2–8.3.4). Everything after it builds on that graph:
- name resolution and incremental queries (1a.7, D2)
- the derived properties (1a.8)
- the implied relationships (1a.9) and validation (1a.10)
- the library snapshot (1a.11, D5)
- in later phases, the API, the store and the browser editor

PROJECT_PLAN §4.2 asks for "an arena-based element graph keyed by ID". §4.3 makes an element's id its API `DataIdentity` id: a v4 UUID that the client assigns, or the normative v5 UUID of a library element. KerML 10.3 requires interchange files to include the implied relationships, or to leave them all out (`includesImplied`).

## Criteria

From execution plan §2.5:
- **Correctness:** the abstract syntax of KerML 8.3. Every property must be readable, including through its redefined names. Implied relationships must follow KerML 8.3.2 and 10.3.
- **Workspace constraints:** builds for wasm32, works on Rust 1.89, passes `cargo deny`.
- **Performance:** the §7 budgets: re-resolve < 1 s, find references over 1M elements < 2 s, cold load of 1M elements < 30 s, and browser memory < 1 GB.
- **Cost and fit:** generated from the metamodel rather than written by hand (CLAUDE.md), and usable by `salsa` (D2, 1a.7) and the API types (ADR 0005).

## Options

1. **A, the recommended default:** an arena of elements addressed by dense `u32` indices, with a side table between indices and UUIDs. It stores the canonical core of the abstract syntax, and presents the typed properties as views.
2. **B: a property bag keyed by UUID,** close to the KerML 10.4 JSON. Each element is a map from property name to value, and references are UUIDs.
3. **C: one generated struct per metaclass,** with its properties flattened. Option A's analysis showed that the 76 stored attributes reduce to one shape, so C would generate 175 types that hold the same core and differ only in their accessors. It was therefore compared on its cost and not prototyped.

## Evidence

All results come from the evidence record, which has the full sources.

- **Only 76 of the 210 KerML attributes are stored; 134 are derived.** Of the stored ones, every relationship end (30 attributes of 20 metaclasses) redefines `Relationship::source` or `target`, directly or through another end. `FeatureTyping::type`, for example, redefines `Specialization::general`, which redefines `target`. A model therefore states only:
  - ownership links;
  - `source` and `target`;
  - names, about 30 flags and enumeration values, and literal values and texts.
- **At 1,002,000 elements,** A uses 186 MiB against 558 MiB for B. It walks the whole ownership tree in 5.7 ms against 376 ms, and reads every typing's type in 3.4 ms against 126 ms: 35–65 times faster for what resolution and queries do most. Building takes 209 ms against 414 ms. A million lookups by UUID take under 0.1 s in both.
- **Assembling** 1M elements from 100 per-file fragments takes 143 ms.
- **Implied relationships** are, by KerML 8.3.2, `ownedRelationship`s with `isImplied = true`, all or none per element (`isImpliedIncluded`).
- **The default** builds for wasm32 and on Rust 1.89. Its only dependency is `uuid`, already in the workspace.

## Decision

Adopt option A, as follows.

- **Storage.** `sysml-model` stores, for each element:
  - its metaclass;
  - its owning relationship and its ordered owned relationships;
  - for a relationship: its `source`, `target`, owned related elements and owning related element, in order;
  - its names, its flags and enumeration values, and its literal value or text.

  That is the canonical core, and every other stored KerML attribute is one of these under a redefined name.
- **Typed properties.** They are generated from the metamodel (`KerML.xmi`): for each attribute, the slot of the core it is a view of, and its position (for example `FeatureTyping::type` is `target[0]`). They are reached through the existing property descriptors. Derived properties are computed (step 1a.8), not stored.
- **Identity.**
  - Inside the graph, elements are addressed by a dense index (`ElementIx`, a `u32`).
  - Each element has its `DataIdentity` UUID, and one map goes from UUID to index. It uses the standard library's keyed SipHash, because the server receives UUIDs from clients.
  - A UUID is assigned when an element is created, and is kept from then on (PROJECT_PLAN §4.3). The stable matching of re-parsed text to existing UUIDs comes with the editor and the store.
- **Implied relationships.** They are elements like any other, with `isImplied` set, but they are kept apart from the owned ones: each element has a list of implied relationships beside its owned ones. `ownedRelationship` is read as the owned ones followed by the implied ones, when `isImpliedIncluded` is true. The semantics layer (1a.9) can then replace them all without touching what a modeler wrote, and interchange (`includesImplied`) can include or leave them out as a whole.
- **Lowering.** Each file lowers into its own fragment with file-local indices, and a graph is assembled from fragments. How the fragments become `salsa` queries is decided with D2 in step 1a.7; addressing elements by (file, local index) would make re-assembly unnecessary.

## Consequences

- Step 1a.6 adds the element graph to `sysml-model`, and the views of the KerML attributes, generated from `KerML.xmi` in `build.rs` (no generated code committed). It also adds the lowering from syntax trees.
- The spec's redefinitions become generated data. A future metamodel that stores a relationship end on its own (not as a view of `source` or `target`) makes the generator fail rather than silently store it twice.
- UUID lookups are needed only at the API boundary, and their hashing resists flooding; inside the graph, references are indices.
- `KerML.xmi` becomes an input of the build, besides `SysML.json`. It is already pinned in `specs.lock.json`.
- The SysML metaclasses (`SysML.xmi`) get the same treatment when the SysML productions are lowered, in Phase 1b. The analysis there must confirm that their stored attributes reduce to the same core.
