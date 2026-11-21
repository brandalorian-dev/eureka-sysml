# ADR 0021: Derived properties and operations, and their readings

- Status: Accepted
- Date: 2026-11-21
- Step: 1a.8 of the Phase 1 execution plan (derived properties and operations)

## Context

KerML 8.3 defines 90 `derive…` rules and 84 operations on the KerML metaclasses, each with OCL in `KerML.xmi`. The semantic constraints (step 1a.9), the validation constraints (1a.10) and the library UUIDs (1a.11, through `path()`) use many of them. PROJECT_PLAN §4.3 asks for Derived Property Full Conformance: derived values are computed by us, and those a client sends are ignored.

Step 1a.8 must cover every rule and operation those constraints use, and list the rest with their status. This ADR records how the work is organized, and the readings it needs.

## Decisions

### 1. One function per rule or operation, on the queries of resolution

`sysml_semantics::derived` has a function for each derive rule and each operation, named after the property or operation in snake case (`owned_member`, `qualified_name`, `path`). Each cites its rule. Every function takes the database, the project and a `Global` (ADR 0019), because most properties depend on resolution. Elements come back as `Global`s, names as `Arc<str>`.

The values are computed on read from the queries of name resolution (ADRs 0019, 0020). Only the recursive ones are `salsa` queries of their own, `qualified_name` and `path`, so that a deep path is computed once. Like every fixpoint query, they settle (ADR 0020 decision 6). A membership that a redefinition removes (ADR 0020 decision 3) is not a `membership`, a `member` or a visible membership.

Where the 1a.7 queries already computed a property, the function uses them: `importedMemberships`, `visibleMemberships`, `effectiveName` and `isDistinguishableFrom`. In time, the queries and these functions converge, rather than two implementations being kept.

### 2. The inventory is data, checked against `KerML.xmi`

`crates/sysml-semantics/derived.json` lists every derive rule and operation, each `done`, `open`, or `deferred` with a reason. `sysml-model`'s build generates `DERIVE_RULES`, `OPERATIONS` and `CONSTRAINTS` (the `check…` and `validate…` rules) from `KerML.xmi`, with their OCL. `tests/derived_inventory.rs` then checks the following:
- **The same set.** The file lists exactly the XMI's rules and operations.
- **Done entries.** Each names its function, and has a test named after the rule (`derive_element_owner`) or after the metaclass and the operation (`element_path`).
- **Needed entries.** The constraints' OCL, followed transitively through the OCL of what it uses, gives the rules and operations that are *needed*. So does `path()`. A needed entry may not be deferred. The analysis matches identifiers, which over-approximates. It finds 56 of the 82 derived properties and 48 of the 58 operation names, and the test pins these numbers so that a new `KerML.xmi` is reviewed.
- **Complete layers.** In a layer marked complete (Root, Core, Kernel), no entry is open.

The step's "done when" is therefore a test: all three layers complete.

### 3. Readings

- **`source` and `target` where a metaclass derives them.** The graph stores the ends that are written (ADR 0017). Where a metaclass stores no source, its source is its owning related element: `membershipOwningNamespace`, `importOwningNamespace`, `featureChained`, a specialization's owning type, and so on. Where it stores no target, its target is its owned related elements, as for an OwningMembership's `ownedMemberElement`. An Annotation's source is its annotating element, owned or owning, and its target its annotated element. `relatedElement` is these, source first.
- **`ownedRelationship`** includes the implied relationships once `isImpliedIncluded` is true (KerML 8.3.2.1.2, `Graph::all_owned_relationships`). Every `owned…` property, and the positions in `path()`, use it. An implied relationship that is not included yet has no position among its owner's relationships, and its `path()` is that of an Element.
- **`Annotation::annotatedElement` of an owned annotation.** A PrefixMetadataAnnotation (8.2.3.2), and an OwnedAnnotation in a relationship body (8.2.3.1), write no annotated element. `owningAnnotatedElement` subsets `annotatedElement`, and `validateAnnotationAnnotatedElementOwnership` requires the owner to be the annotated element. So when no target is written, the annotated element is the Annotation's owning related element.
- **`namesOf(element)`.** The OCL selects the memberships of the element (`elementMemberships`) and then takes the names of all `memberships`. The description, "the names of the given element", and the unused `let` say the selected ones, which are used. Short names come first, then names, once each (`memberShortName->union(memberName)->asSet()`).
- **The `excluded` parameters** of `importedMemberships`, `visibleMemberships` and `membershipsOfVisibility` guard the OCL's recursion against circular imports. Resolution instead iterates to a fixpoint (ADR 0019), so the functions have no such parameter.
- **`escapedName()`.** A name is written as it is when it has the form of a basic name (8.2.2.3), even if it is a keyword. Otherwise it is an unrestricted name, in single quotes, with the escapes of Table 4 for `\`, `'`, backspace, form feed, tab and newline. The library oracle confirms this reading (Evidence).
- **`qualificationOf` and `unqualifiedNameOf`** have no OCL. A qualified name given as text is read by the grammar of 8.2.3.4.1: `::`-separated basic or unrestricted names, after an optional `$::`, with white space allowed between the tokens (8.2.2.1). The qualification is given back as text, its segments escaped again, and `$` for a global name of one segment. The unqualified name is given back unescaped.
- **Imported memberships, with and without collisions.** `importedMembership` and `visibilityOf` see the namespace's imported memberships, from which collisions are excluded (ADR 0020 decision 5); `visibilityOf` tests `importedMembership->includes(mem)` before it looks for the import. `membershipsOfVisibility` takes each import's own `importedMemberships`, as its OCL does, so colliding memberships are among them. A membership that several imports bring counts once in `importedMembership`, `membership` and `member`.
- **`resolveLocal` in a root namespace.** The OCL resolves in the global namespace when there is no owning namespace, without first looking at the root namespace's own memberships. It is followed as written: the global namespace has the visible memberships of the roots (8.2.3.5.2).
- **`memberElementId`** is the `elementId` of the member element. Inside the query graph, an element is (file, index), and `DataIdentity` ids are assigned outside it (ADR 0019). It is deferred to the UUID mapping of step 1a.11, and no constraint needs it.

## Evidence

- **The library oracle** (`tests/derived_oracle.rs`). KerML 9.1 makes the id of each element of a library package a v5 UUID of the package's id and the element's `path()`. For all 24,859 elements of the 36 KerML library files that are in a library package, our `path()` gives the pilot's `elementId`. This holds for every element, including those named only through an implied redefinition, which `scripts/check_uuid_oracle.py` cannot recompute. The test checks `path()`, `qualifiedName`, `escapedName`, `name`, `owningNamespace`, `ownedMember` and `libraryNamespace` together.
- **The Root layer** (KerML 8.3.2): 49 tests, one per rule and operation, from the OCL. That is 25 derive rules, and 24 operations including `LibraryPackage::libraryNamespace` from Kernel. `memberElementId` is deferred.

## Consequences

- Steps 1a.9 to 1a.11 call these functions rather than re-deriving properties. The library UUIDs of 1a.11 can use `path()` directly.
- The Core (8.3.3) and Kernel (8.3.4) layers follow in their own PRs. Each marks its layer complete in the inventory test.
- When `KerML.xmi` changes, the inventory test fails until `derived.json` and the needed counts are reviewed.
