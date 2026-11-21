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
- **`annotation`.** The OCL of deriveAnnotatingElementAnnotation is `owningAnnotatingRelationship->prepend(owningAnnotatingRelationship)`, which prepends an element to itself. The description says the owned annotating relationships, with the owning one, if any, first. That is what is used: `ownedAnnotatingRelationship->prepend(owningAnnotatingRelationship)`.
- **Ends typed Membership.** `MembershipImport::importedMembership` redefines `target` and is a Membership (8.3.2.4.4). The name resolves to the membership itself (8.2.3.4.2, Note 1), so `target` and `relatedElement` give the membership, not its member element. That holds for any end whose stored attribute is typed Membership.
- **`importedElement` with filter conditions** is the outer import's `importedNamespace`: the FilterPackage that the import owns (8.2.3.4.2), which is also its `target`. The namespace that the FilterPackage's own import names is that import's `importedElement`.
- **`namesOf(element)`.** The OCL selects the memberships of the element (`elementMemberships`) and then takes the names of all `memberships`. The description, "the names of the given element", and the unused `let` say the selected ones, which are used. Short names come first, then names, once each (`memberShortName->union(memberName)->asSet()`).
- **The `excluded` parameters** of `importedMemberships`, `visibleMemberships` and `membershipsOfVisibility` guard the OCL's recursion against circular imports. Resolution instead iterates to a fixpoint (ADR 0019), so the functions have no such parameter.
- **`escapedName()`.** A name is written as it is when it has the form of a basic name (8.2.2.3), even if it is a keyword. Otherwise it is an unrestricted name, in single quotes, with the escapes of Table 4 for `\`, `'`, backspace, form feed, tab and newline. The library oracle confirms this reading (Evidence).
- **`qualificationOf` and `unqualifiedNameOf`** have no OCL. A qualified name given as text is read by the grammar of 8.2.3.4.1: `::`-separated basic or unrestricted names, after an optional `$::`, with white space allowed between the tokens (8.2.2.1). The qualification is given back as text, its segments escaped again, and `$` for a global name of one segment. The unqualified name is given back unescaped, by the same rule the lowering applies to a declared name: an escape outside Table 4 keeps its backslash.
- **Imported memberships, with and without collisions.** `importedMembership` and `visibilityOf` see the namespace's imported memberships, from which collisions are excluded (ADR 0020 decision 5); `visibilityOf` tests `importedMembership->includes(mem)` before it looks for the import. `membershipsOfVisibility` takes each import's own `importedMemberships`, as its OCL does, so colliding memberships are among them. A membership that several imports bring counts once in `importedMembership`, `membership` and `member`.
- **`resolveLocal` in a root namespace.** The OCL resolves in the global namespace when there is no owning namespace, without first looking at the root namespace's own memberships. It is followed as written: the global namespace has the visible memberships of the roots (8.2.3.5.2).
- **Relationships and the types they relate (Core).** A relationship-valued property, such as `ownedSpecialization`, `ownedTyping` or `ownedRedefinition`, lists the relationships in the graph: those written and, once step 1a.9 includes them, the implied ones. A property or operation over types also sees the implied specializations and redefinitions that resolution computes (ADR 0020). That covers `supertypes`, `allSupertypes`, `type`, `typingFeatures`, `namingFeature`, `redefines` and `multiplicities`. `supertypes(true)` leaves the implied ones out.
- **Relationships written elsewhere.** `Feature::subsetting` (in `typingFeatures`) and `Feature::featuring` (in `featuringType`) include relationships that name the feature but are owned by another element, such as a `subset a :> b;` written at the package level. Only the feature's own relationships are followed. Finding the others needs an index from each element to the relationships that name it, which steps 1a.9 and 1a.10 add if their constraints need it.
- **`featuringType` of an owned feature.** In KerML 1.0, a FeatureMembership is not a Featuring: the featuring type comes from the TypeFeaturing that `checkFeatureFeatureMembershipTypeFeaturing` implies (8.4.3.1, Table 9). Its target is a type for which `isFeaturingType` is true. That is the owning type for a feature that is not variable, and `Occurrences::Occurrence::snapshots` for a variable feature of `Occurrence`. These are included. For a variable feature of another type, the target is an implied feature redefining `snapshots`, which does not exist before step 1a.9. Until then such a feature has no featuring type from its membership. The other implied type featurings (Table 11) also follow in 1a.9.
- **A recursive import of a type** (`Type::visibleMemberships`). The OCL passes `isRecursive` as the `excludeImplied` argument of `inheritedMemberships`. So a recursive import brings what a type inherits through its written specializations, but not what it inherits from the library types that are implied. This is followed, in the queries of resolution too. Without it, `::**` would import much of the libraries along with every type.
- **Circular specializations in `inheritedMemberships`.** The OCL passes `excludedTypes->including(self)` down, so the walk never comes back to the type. Here the memberships are memoized per type and converge by fixpoint iteration instead. Around a cycle, the only memberships that would come back are the type's own owned and imported ones, and `inheritableMemberships` leaves them out. Every other contribution reaches the type by a direct path anyway. With `A specializes B; B specializes A`, `A` inherits nothing of its own and `B` inherits `A`'s.
- **`supertypes(true)`** leaves out the owned specializations flagged `isImplied` (`reject(isImplied)`), once step 1a.9 adds implied relationships to the graph, as well as the implied generals that resolution computes.
- **The order of `inheritedMemberships`.** The OCL composes it as `removeRedefinedFeatures(supertypes.nonPrivateMemberships)`, where each supertype contributes its public memberships, then its protected ones, then those it inherits. `type_inherited_memberships` follows that order. Resolution's query (ADR 0020 decision 3) has the same memberships in declaration order, which matters only for the order of lookups.
- **`allSupertypes()`**: `OrderedSet{self}->closure(supertypes(false))` is taken depth first, in pre-order, each type once. Circular specializations converge.
- **`directionOfExcluding`** excludes the type and the excluded types from the supertypes it visits (`supertypes(false)->excluding(excludedSelf)` excludes a set).
- **Typos in the Core OCL**, read as follows:
  - **`Feature::isCompatibleWith`:** `supertype.oclIsKindOf(Feature)` is read as `otherType`, and `ownedRedefinitions.allRedefinedFeatures()` as `allRedefinedFeatures()`. Two features are compatible if they redefine a common feature, neither has owned features, and one can access the other.
  - **`canAccess`:** `featuringTypes` is read as `featuringType`, and `subsettingFeature.resolveGlobal` as `resolveGlobal`.
  - **`deriveFeatureType`:** `OrderedSet(Types)` and `types->exist` are read as `OrderedSet(Type)` and `exists`.
  - **`ownedCrossFeature`:** it rejects `oclIsKindOf(FeatureValue)` among features; a FeatureValue is a membership, so this has no effect.
- **Kernel readings.**
  - `instantiatedType()` (8.3.4.8.7): the OCL's `typeMembers->first()` is read as `members->first()`.
  - `deriveMetadataFeatureMetaclass`: `metaClassTypes` is read as `metaclassTypes`.
  - `deriveFlowSourceOutputFeature`: `connectorEnd.ownedFeature->first()` would be the first owned feature of any end. It is read as the first end's, as `targetInputFeature` reads the second's.
  - `deriveConnectorDefaultFeaturingType`: `relatedFeature->closure(featuringType)` includes the related features themselves, as OCL's `closure` includes its source.
  - Two properties have the same name in two metaclasses: `targetFeature` (Connector and FeatureChainExpression) and `argument` (InvocationExpression and ConstructorExpression). The second of each has its own function, `chain_target_feature` and `constructor_argument`.
- **The result of a ConstructorExpression** specializes its instantiated type (`checkConstructorExpressionResultSpecialization`, 8.4.4.1 Table 10). Unlike the results of other expressions, it is written in the grammar (8.2.5.8.4), so `implied_generals` gives it now. Without it, the named arguments of `new T(a = 1)`, which are the result's features redefining T's, would not resolve.
- **Model-level evaluation** (8.4.4.9.8) is the rest of the Kernel layer, in a fourth PR of this step. It covers `evaluate`, `modelLevelEvaluable`, `isModelLevelEvaluable`, `checkCondition`, `MultiplicityRange::valueOf` and `hasBounds`, `MetadataFeature::evaluateFeature` and `syntaxElement`, `MetadataAccessExpression::metaclassFeature`, `Package::importedMemberships` and `includeAsMember`. The owner chose this split on 2026-11-21. The constraints need all of these, so Kernel is marked complete only after that PR.
- **`memberElementId`** is the `elementId` of the member element. Inside the query graph, an element is (file, index), and `DataIdentity` ids are assigned outside it (ADR 0019). It is deferred to the UUID mapping of step 1a.11, and no constraint needs it.

## Evidence

- **The library oracle** (`tests/derived_oracle.rs`). KerML 9.1 makes the id of each element of a library package a v5 UUID of the package's id and the element's `path()`. For all 24,859 elements of the 36 KerML library files that are in a library package, our `path()` gives the pilot's `elementId`. This holds for every element, including those named only through an implied redefinition, which `scripts/check_uuid_oracle.py` cannot recompute. The test checks `path()`, `qualifiedName`, `escapedName`, `name`, `owningNamespace`, `ownedMember` and `libraryNamespace` together.
- **The Root layer** (KerML 8.3.2): 49 tests, one per rule and operation, from the OCL. That is 25 derive rules, and 24 operations including `LibraryPackage::libraryNamespace` from Kernel. `memberElementId` is deferred.
- **The Core layer** (KerML 8.3.3): 67 tests, one per rule and operation (35 derive rules, 32 operations). The library tests check implied supertypes (`Base::Anything`) and recursive imports.
- **The Kernel layer, without evaluation** (KerML 8.3.4): 34 tests (27 derive rules, 7 operations), for connectors, associations, flows, expressions, multiplicities, metadata and packages.

## Consequences

- Steps 1a.9 to 1a.11 call these functions rather than re-deriving properties. The library UUIDs of 1a.11 can use `path()` directly.
- Model-level evaluation follows in its own PR, which marks the Kernel layer complete in the inventory test and completes the step.
- When `KerML.xmi` changes, the inventory test fails until `derived.json` and the needed counts are reviewed.
