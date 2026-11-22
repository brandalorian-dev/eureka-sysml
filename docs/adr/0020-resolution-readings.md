# ADR 0020: Readings of name resolution, and the implied relationships it needs

- Status: Accepted
- Date: 2026-11-21
- Step: 1a.7 of the Phase 1 execution plan (name resolution), with the owner-approved scope change of 2026-11-21

## Context

Step 1a.7 resolves every qualified name by KerML 8.2.3.5 on the queries of ADR 0019. Its criterion is that the three KerML libraries resolve with zero unresolved names. We test more than that. Every name must resolve to the element the OMG pilot implementation resolved it to, in its XMI serialization of the libraries (ADR 0009; `crates/sysml-semantics/tests/resolve_oracle.rs`).

KerML 8.2.3.5.1 assumes that a namespace's memberships are "fully computed, including memberships resulting from implied Relationships (see 8.4.2)". Implied relationships were planned for step 1a.9. Without them, 66 of the 4003 library names did not match the pilot:
- 54 were unresolved, among them `that`, `self`, `startShot`, `endShot` and `localClock`, and an association's `source` and `target`;
- 12 resolved to a different element. For example, `startIndex = index` in `OccurrenceFunctions` found the invoked behavior's `index`, which the parameter at its position implicitly redefines and so hides.

The owner chose to pull the implied relationships that resolution needs forward into 1a.7. The spec text also leaves some cases open, which this ADR records.

## Decisions

### 1. Implied relationships are computed for resolution

`implied_generals` (`crates/sysml-semantics/src/implied.rs`) gives each type the specializations and redefinitions that KerML 8.4.2 implies. A type's general types are then its owned ones followed by these. Inheritance (8.3.3.1.10), the Redefinition-generals context (8.2.3.5.1), effective names and the hiding of redefined features all see them. They are computed, not added to the fragments.

- **Specializations:** all the specialization constraints of 8.3.3 and 8.3.4 whose general is a library element or the `instantiatedType` of an InvocationExpression. Among them are the Classifier, DataType, Class, Structure, Association, Behavior, Function, Predicate and Metaclass bases, `Base::things`, the feature constraints on data values, occurrences, objects, suboccurrences, subobjects, portions and participants, the Step, Expression, literal, Connector, Flow, MetadataFeature and Multiplicity constraints.
- **The redundancy rules of 8.4.2:** an implied specialization is dropped when its general is the type itself, a general of an owned specialization, or a strict supertype of an owned general, of another implied general or of the target of an implied redefinition. Duplicates are dropped too. This needs `supertypes`, the transitive closure, as a query.
- **Redefinitions** (`implied_redefinitions`, a query of its own so that following redefinitions does not compute the implied specializations): ends (`checkFeatureEndRedefinition`), parameters by position (`checkFeatureParameterRedefinition`), results (`checkFeatureResultRedefinition`), flow end features (`checkFeatureFlowFeatureRedefinition`) and payloads (`checkPayloadFeatureRedefinition`). Each applies unless the feature already redefines that target. The redundancy rules do not apply to them, as 8.4.2 says.
- **Effective names:** a feature without a declared name and without an owned redefinition is named by its first implied redefinition (8.3.3.3.4 `namingFeature`, reading `ownedRedefinition` as including the implied ones, as 8.4.2 intends).

Readings of the constraint text:
- **Names of library features.** The OCL names `Performance::enclosedPerformance`, `subperformance` and `Object::ownedPerformance`, but the library declares them in the plural: `enclosedPerformances`, `subperformances` and `ownedPerformances`. The library names are used.
- **`checkFeatureSubobjectSpecialization`.** Its OCL names `Occurrence::Occurrence::suboccurrences`; Table 9 names `Objects::Object::subobjects`. Table 9 is used, since a subobject constraint that only repeats the suboccurrence one would be pointless.
- **`checkFeatureFlowFeatureRedefinition`.** Its second target is written `Transfers::Transfer::source::targetInput`. It is read as `Transfers::Transfer::target::targetInput`, where the library declares `targetInput`. Its OCL takes the FlowEnd's position among all the owned features of the flow, but the grammar puts a payload feature (`flow of P …`) first. The position is therefore counted among the flow's FlowEnds.
- **The owning type's kind.** The subobject, suboccurrence, portion and owned-performance constraints ask whether the owning type is a Structure (or a Class), or a Feature whose `type` is one. That is the owner's own metaclass or, for a feature, the types of its typings and of the features it subsets, redefines or chains. A classifier that only specializes a structure is not a Structure.
- **`endFeature`.** For the binary-association and binary-connector constraints (`connectorEnd` is the connector's `endFeature`) and the positions of end redefinitions, a type's end features are its owned ends or, with none, those of its first owned general that has ends. Implied generals are not consulted, which keeps this query out of a cycle with them. Every library type these constraints name owns its ends.
- **Library lookup.** A library element is found by its qualified name (`specializesFromLibrary` resolves it globally): the first segment in the global namespace and each later one among the owned memberships of the one before. Every element the constraints name is declared there. Every root namespace with the first segment's name is tried, in project order, until the whole name resolves, so a user package named `Links` does not hide the library's `Links`. Operators are found in the same way (8.3.4.8.17).

Added in step 1a.9 (ADR 0022), and computed here because most of them change what a type inherits:
- the result specializations of expressions, whose results the grammar writes (8.2.5.8, EmptyResultMember): a FeatureReferenceExpression's result subsets its referent, the result of an invocation of a type that is not a Function specializes it, and the results of Select and Index expressions subset the first argument's result (an Index's unless that result specializes `Collections::Array`);
- `checkFeatureValuationSpecialization`: a feature without a direction or a written specialization subsets the result of each of its values;
- the cross-feature constraints: an owned cross feature specializes the types of its end and subsets the cross features of the features its end redefines, and an end with an owned cross feature crosses it (`checkFeatureCrossingSpecialization`), through a feature chain that the overlay builds.

Also added in step 1a.9, in its second PR:
- the redefinitions of a FeatureChainExpression's `sourceTargetFeature`, which the grammar writes (ADR 0018): it redefines `ControlFunctions::'.'::source::target` and the target feature (`checkFeatureChainExpressionTargetRedefinition`, `…SourceTargetRedefinition`). The result subsets the chain of the first input parameter and the `sourceTargetFeature` (`checkFeatureChainExpressionResultSpecialization`), which the overlay builds. The rules of 8.4.2 do not filter that subsetting, since its target is a new element;
- `checkConstructorExpressionResultFeatureRedefinition`: a feature of the result of `new T(…)` without a written redefinition, a positional argument, redefines the public feature of T at its position (8.4.4.1 Table 11 Note 6). T's features are read as `feature` does: owned, then inherited, without an inherited feature that an owned one redefines.

Added in step 1a.9, in its third PR: the six type-featuring constraints. They do not change what a name resolves to, but the overlay needs them computed from resolution, so `implied_featurings` and `featuring_types` are queries here too (ADR 0022). Their readings:
- **`checkMultiplicityTypeFeaturing`.** Its OCL asks whether the multiplicity's `owningType` is a Feature. A multiplicity is owned through an OwningMembership, so it never has an owning type, and the constraint would always require no featuring types. The text of 8.4.4.12.1 says owning namespace, and its example gives a multiplicity the featuring type of its feature. The owning namespace is used.
- **`ownedCrossFeature()`.** Its OCL rejects members that are `oclIsKindOf(FeatureValue)`, which no Feature is. It is read as rejecting the value of a FeatureValue, which is an owned member too. Otherwise the library's `end feature redefines target = that` has its value expression as a cross feature. This applies to the derived operation (ADR 0021) as well.
- **An only end.** `checkFeatureOwnedCrossFeatureTypeFeaturing` distinguishes one other end from many. An end that is its owner's only end, such as `Occurrences::Occurrence::surroundedByOccurrences::surroundingSpace`, has none. Its owned cross feature is then read as having no featuring types.
- **The other end's types.** "featuringType = otherEnds->first().type" uses `type`, which leaves out the types that another of them specializes. The implied featurings use the same.
- **The other ends** are the owning type's `endFeature`: its owned ends, then the inherited ones that an owned end does not redefine. That is wider than the owned ends that the binary-association constraints count (decision 1), so a specializing association that redefines one end still has the inherited other end.
- **The factors of a Cartesian product** are every type of each other end, in order, as the OCL's `asCartesianProduct() = otherEnds.type` compares them. A product is implied only when there are two or more other ends and each has a type.
- **Equal featuring types.** The equality constraints (multiplicities, bounds, values) compare featuring types without order, since the implied ones follow the written ones.

Still to come in step 1a.9: the binding-connector constraints. `checkMetadataFeatureSemanticSpecialization` needs model-level evaluation, which reads the view, so it is computed in the overlay, not here (ADR 0022).

More readings, from step 1a.9:
- **`checkFeatureParameterRedefinition`.** Its OCL quantifies over every general of the owning type's owned specializations, but Table 9 names the "parameters of supertypes of the owning Behavior or Step". A step can also subset a plain feature whose directed features are no parameters. The Table is followed: the implied redefinitions and the check both take the generals that are Behaviors or Steps.
- **Owned, not only implied.** `checkFeatureCrossingSpecialization` reads `crossFeature`, and `checkFeatureOwnedCrossFeatureRedefinitionSpecialization` reads `ownedSubsetting`: both need the relationship to be owned, not implied by a supertype. The redundancy rules of 8.4.2 would drop one whose target a supertype already has, so they do not apply to these two, as they do not to redefinitions.
- **`ownedTyping`, with the implied typings.** The data-value, occurrence, object, suboccurrence, subobject and portion constraints read `ownedTyping`, which includes the implied FeatureTypings (8.3.3.3.4). An owned cross feature typed by its end's class, or a result typed by the invoked behavior, is thus a feature typed by a class.
- **`checkFlowWithEndsSpecialization`.** Its OCL asks for `ownedEndFeatures`. KerML 8.4.4.10.2 says "if the Flow has ownedEndFeatures (see below)" and goes on to describe the FlowEnds of `flow … from … to …`. The library's own flows, `Transfers::transfers`, `messageTransfers` and `transfersBefore`, declare plain end features. Read literally, the constraint would make `messageTransfers` subset `flowTransfers`, and `transfers` subset `flowTransfers`, which subsets `transfers`; that cycle made the redundancy rules drop every implied specialization of a user flow. The end features are read as the flow's FlowEnds.
- **A body expression's referent.** The referent of a FeatureReferenceExpression is the member element of its first membership that is not a ParameterMembership (8.3.4.8.5). For a body expression, the grammar's BodyExpressionMember is a FeatureMembership, so the referent is the expression it owns.

- **Already redefined.** A redefinition constraint implies a Redefinition unless the feature already redefines that target with an owned Redefinition. Other relationships to the target, such as a subsetting, do not count, since the OCL asks for `redefines`.

### 1a. The generals a type declares

A type's owned generals, which resolution inherits from, are the generals of its owned specializations whose `special` is the type, and those of its conjugation. Following `Type::supertypes` (KerML 8.3.3.1.10), a conjugated type has its original type alone, even if it also has specializations, which a `type` declaration allows (8.2.4.1.1). A specialization written as a member in a type's body, such as `specialization X subtype B specializes A`, specializes B, not the type that owns it.

### 2. A Redefinition, in the generals of its owning type

KerML 8.2.3.5.1 repeats "the basic name resolution process" with each general of the owning type in turn as the local namespace. Read literally, the full resolution from the first general climbs to the namespaces that contain it before the next general is tried. In `classifier T specializes A, B { feature redefines x; }`, an `x` beside A in their package would then win over `B::x`, which T inherits.

**Reading:** the memberships of every general are looked up first, in order, and only then full resolution from each general in turn. A qualified name, such as `redefines A::b`, is resolved whole with each general in turn, so a general whose `A` has no `b` does not stop the next one. The owning type is never one of these generals, even when it specializes itself.

KerML 8.2.3.5.1 also resolves the `redefinedFeature` of a Redefinition owned by a feature with an owning type in the general types of that owning type, and says resolution fails if none of them has the name. The normative libraries need more than that. One example is `FeatureReferencingPerformances`, with `feature beforeTimeSlice : Occurrence [1] subsets timeSlices { feature redefines monitoredFeature; }`, where `monitoredFeature` belongs to the enclosing `monitoredOccurrence`. The pilot resolves it there.

**Reading:** when none of the generals resolves the name, resolution continues by full resolution from the namespace that owns the owning type. The owning type itself is skipped, because its member named by this very redefinition would resolve to itself.

### 3. Memberships hidden by redefinition are a last resort

`removeRedefinedFeatures` (8.3.3.1.10) removes the inherited memberships of redefined features. SpaceLink's ends redefine `BinaryLink::source` and `target`, so these two are not inherited by SpaceLink, nor by its subtypes. Yet the library writes `assoc all InsideOf specializes SpaceLink { end feature smallerSpace … redefines source …; }`, and the pilot resolves `source` to `BinaryLink::source`.

`removeRedefinedFeatures` removes a membership on two conditions: (1) its feature is among the features another inheritable membership's feature redefines, directly or indirectly; (2) it, or a feature it redefines, is directly redefined by an owned feature of the type. Both are applied, and a membership that arrives through several generals is removed only if it is removed on every path. For condition 1, each type keeps the set of features its non-private features redefine, together with its generals' sets (`redefined_in`), so each feature's redefinitions are followed once.

**Reading:** a removed membership stays in the inherited memberships, marked `hidden`. Resolution ignores hidden memberships, and only if the name then resolves nowhere is the whole resolution repeated with them. A hidden membership therefore never wins over a visible one, wherever that one is, and inheritance is as 8.3.3.1.10 defines it.

### 4. The scope of a chained name

KerML 8.2.3.5.2 resolves the target of a FeatureChainExpression `a.foo` among the features of the result of its argument `a`. `foo` is not resolved anywhere else: if the result of `a` has no `foo`, or `a` is a reference that does not resolve, `foo` is unresolved.

- **A cast's result.** `(that as StatePerformance).incomingTransitionTrigger` needs the features of the cast's result. A cast owns its result parameter, typed by the target type (8.2.5.8.1 `TypeResultMember`). An InvocationExpression's own result parameter is therefore used before that of the function it invokes.
- **A result not determined yet.** Some arguments have a result that is an implied element, such as the result of an operator expression without a cast. The fragments do not have those elements until step 1a.9. For these arguments only, the name is resolved from the namespace of the expression instead. No name of the KerML libraries needs this; step 1a.9 removes the fallback.

### 5. Imported memberships that collide

The description of `Namespace::importedMemberships` (8.3.2.4.5) excludes "Memberships that have distinguishability collisions with each other or with any ownedMembership"; its OCL body does not. The description is followed. Two memberships collide when they are not distinguishable by `isDistinguishableFrom` (8.3.2.4.3): a name or short name of one is a name or short name of the other, and one member element's metaclass conforms to the other's. Both of two colliding imported memberships are excluded, so `import A::*; import B::*;` with a class `X` in each imports no `X`. An imported membership that collides with an owned one is excluded, too. Step 1a.10 reports the collision as a validation error (`validateNamespaceDistinguishability`).

### 5a. What a namespace shows

`visibleMemberships` (8.3.2.4.5) is `membershipsOfVisibility(public)`: the public owned memberships, and what the public imports bring (`ownedImport->select(visibility = public).importedMemberships`). For a type, the public inherited memberships are added (8.3.3.1.10). `nonPrivateMemberships`, which inheritance uses, likewise takes the public and protected ones. Only the imports of those visibilities are evaluated: what a private import brings is no part of what a namespace shows.

This is followed exactly (`outside_memberships`). Taking all of a namespace's memberships and then keeping the public ones gives the same set, but it makes what a namespace shows depend on its private imports. Packages that each privately import the next then form one cycle around the whole ring, which every edit reopens. On a generated model of 1,300 such packages (1M elements), resolution took 281 s and an edit 276 s (the D10 spike, step 1a.9); following the OCL, they take 5.9 s and 9 ms.

The OCL of `membershipsOfVisibility` excludes no collisions, so neither do these views. A namespace's own `importedMembership`, in which its own names resolve, still excludes them (decision 5).

### 6. Cycles that do not converge

The queries that iterate to a fixpoint are not all monotone: a lookup takes the first match, and colliding imports are excluded. A cycle in a user model might therefore oscillate, and `salsa` panics after 200 iterations. Every fixpoint query has a `cycle_fn`. After 100 iterations it keeps the value of the last iteration, or for the resolution of a name, none, which ends the iteration. Such a name is unresolved, not a crash. Aliases that name each other have no member element.

### 7. `all` on an import with filter conditions

In `import all A::*[@Approved];` the grammar (8.2.3.4.2) puts `isImportAll` on the outer import, whose imported namespace is the FilterPackage. The FilterPackage's own import is an `ImportDeclaration`, which cannot be `all`.

**Reading:** `all` governs the FilterPackage, whose memberships are those its own import brings, once filtered. That import brings the visible memberships of `A`, so `all` does not reach `A`'s private members. The FilterPackage's own import has the default visibility of an Import, which is private. Its memberships are still what the outer import imports: otherwise an import with a filter and without `all` would import nothing, which 7.2.5.4 contradicts. Since step 1a.8, the filter conditions are evaluated (`Package::importedMemberships`, ADR 0021): of those memberships, only the ones whose element the conditions include are imported. A package's own `filter` conditions apply to all its imports in the same way.

Recursive imports follow `visibleMemberships(excluded, isRecursive, includeAll)` (8.3.2.4.5). They descend only into the namespaces a namespace owns, through a public owning membership unless `all`, and pass `all` to every level. For a type, they bring what it inherits through its written specializations only, because `Type::visibleMemberships` passes `isRecursive` as `excludeImplied` (8.3.3.1.10; ADR 0021).

### 8. The proper type of a resolution

KerML 8.2.3.5.1 says that "if the resulting Element does not have the proper type for its context, then the qualified name has no resolution". The proper type is read as the metaclass of the stored attribute that the end is a view of. That is Type for `FeatureTyping::type`, Classifier for `Subclassification::superclassifier`, Feature for `Subsetting::subsettedFeature`, Namespace for an import's namespace, and Element for `Membership::memberElement`. For a MembershipImport the resolution is the membership itself (8.2.3.4.2 Note 1), so its metaclass is checked against Membership. The search does not go on to another element of the same name: the name has no resolution. An alias whose target is not resolved yet is not rejected here; its own name is reported.

### 9. Resolution reads a span-free structure

As ADR 0019 foresaw, a lowered fragment has spans and no equality, so every query that read it reran after any edit. Re-resolving the KerML libraries after a layout edit to `Occurrences.kerml` took 54 ms, about half of resolving them from nothing. The `structure` query wraps the fragment with an equality that ignores spans (`Graph::same_structure`), and every resolution query reads that. It gives access only to the graph and the root: a fragment's diagnostics, whose spans it does not compare, are read through `lowered`. An edit of layout now stops at `structure`, and the same re-resolution takes about 7 ms. The test `an_edit_reruns_only_the_queries_that_depend_on_it` asserts this from the event log.

## Evidence

- **The pilot oracle** (`tests/resolve_oracle.rs`) checks that all 4003 names are compared, and identifies each element by its position in the ownership walk of its file, on both sides; the lowering equals the XMI element by element (ADR 0018). Its results:
  - before this ADR: 3932 of 4003 names the same, 12 different, 59 unresolved;
  - with decision 2: 3937 the same;
  - with decision 1: 4001;
  - with decision 3: all 4003.
- **Clause examples** (`tests/resolve.rs`), from the snippets of KerML 7.2.5.2, 7.2.5.4, 7.3.2.3 and 7.3.4.5: visibility, aliases, imports (including the recursive ones, and `all` with and without filters), inheritance, redefinition with effective names, and implied specializations and redefinitions (positions of parameters and flow ends, binary connectors, the redundancy of `Base::things` beside an implied redefinition). Also: a redefinition found in a later general before the namespace of an earlier one, a user package named like a library one, chained names, colliding imports, circular aliases and imports, `removeRedefinedFeatures` through several generals, a qualified redefined feature tried with each general, names of elements of the wrong metaclass, a type that specializes itself, and subobjects by the owner's kind.
- **Speed** (`cargo bench -p sysml-bench --bench semantics`, native):
  - parsing, lowering and resolving the 36 library files from nothing: about 55 ms (loading the corpus and creating the inputs are not counted);
  - re-resolving after a layout edit to `Occurrences.kerml`: about 7 ms.

## Consequences

- Step 1a.9 starts from these queries. It materializes the implied relationships, adds the deferred ones, and tests every table row. It must keep the pilot oracle passing.
- `supertypes` memoizes the transitive closure of every type, which costs memory in proportion to the depth of the library's hierarchies. It is measured at scale with the library snapshot (D5, step 1a.11).
- Step 1a.8 evaluates filter conditions, on the FilterPackage of decision 7 and on a package's own (ADR 0021).
- A name that resolves only through decision 2, 3 or 4 (its fallback) is a candidate for a validation warning in step 1a.10: the spec text alone would leave it unresolved.
