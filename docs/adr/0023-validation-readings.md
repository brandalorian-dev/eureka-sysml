# ADR 0023: Readings of the validation constraints

- Status: Accepted
- Date: 2026-11-23
- Step: 1a.10 of the Phase 1 execution plan (validation constraints)

## Context

Step 1a.10 implements the 88 `validate…` constraints of KerML 8.3 from their OCL, one function each in `crates/sysml-semantics/src/validate.rs`. Each violation becomes a diagnostic (ADR 0012). The step's criterion is that the KerML libraries produce zero diagnostics, with the implied relationships and elements of step 1a.9 included (ADR 0022).

Run on the libraries, the constraints as written reported violations in five groups. Some were in what the libraries declare; most were in the relationships that the semantic constraints of KerML 8.4.2 require. This ADR records how each group is read, and the other decisions of the step. The Root and Core constraints come first; the Kernel constraints follow in the step's second PR.

## Decisions

### 1. Diagnostics

- Every violation is an **error**, confirming ADR 0012's open point 4: "the semantics of a model are only defined if it meets all semantic and validation constraints" (KerML 8.4.2).
- The code is the constraint's name, and the clause is that of the constraint's metaclass, for example `KerML 8.3.2.4.2` for `validateImportTopLevelVisibility`.
- The range is the element's span in the lowered fragment. A violation by an implied element is reported at the nearest written element that owns it. A namespace whose memberships are not distinguishable is reported at the later of the first two such memberships, if it is written in the file, rather than over the whole namespace.
- The violations of a file are a query (`validate::violations`), span-free like the rest of the semantics. The diagnostics add the spans.

### 2. Negative tests that no text can write

The plan asks for one positive and one negative test per constraint. For 14 of the 39 Root and Core constraints, no valid KerML text produces a violation: the grammar, the lowering or name resolution already ensure them. One example is `validateFeatureChainingFeatureNotOne`, since the grammar writes two chaining features or more. Their positive tests are written now. The owner chose on 2026-11-23 that their negative tests come when models can be read from KerML JSON, which can express any graph: step 1b.6 (JSON and `.kpar` interchange). They are thus an exception to Phase 1a's exit. `validations.json` records each one's reason, and its inventory test checks that every other constraint has a negative test.

### 3. Inherited members that redefine a common feature

KerML 7.3.2.1 requires the member names of all inherited memberships to be distinct, and `validateNamespaceDistinguishibility` checks every two memberships. In 21 library namespaces a type inherits two features of the same name from two generals, neither redefining the other, but both redefining a common feature. An example is `VectorFunctions::CartesianThreeVectorOf::result`, which gets `dimension` from `ThreeVectorValue` and from `CartesianVectorOf::result`. Two such inherited memberships are read as one member reached through two generals, and so as distinguishable. Two inherited features of the same name that do not redefine a common feature still violate the constraint, as does an owned member with the name of an inherited one.

### 4. A general that is a feature chain

`checkFeatureEndRedefinition`, `checkFeatureParameterRedefinition` and `checkFeatureResultRedefinition` take the ends, parameters or results of each general of the owning type. In `Triggers::TriggerWhen`, `step :> monitor.startObservation { in observer = receiver; }` subsets a feature chain, which owns no parameters. So `observer` redefined nothing, and its name collided with the inherited `startObservation::observer`. A general that is a feature chain is read as its `featureTarget`, as `supertypes()` reads it (KerML 8.3.3.3.4). Both the implied redefinitions and the checks of step 1a.9 follow this reading.

### 5. Cross subsettings

- **A binary crossing goes through the other end.** Table 9 Note 2 (KerML 8.4.3.1) has the crossing chain's first feature constructed, typed by the cross feature's featuring type and featured by the end's owning type. `validateCrossSubsettingCrossedFeature` requires, for a binary owning type, that it be the other end itself. The other end is typed and featured in exactly that way, so for a binary owning type the overlay chains through the other end (ADR 0022). The constructed feature stays for more ends, as in the ternary example of 8.4.4.5.1.
- **An only end crosses nothing.** `validateCrossSubsettingCrossingFeature` requires a crossing end's owning type to have more than one end. An end that is its owning type's only end, as `Occurrences::Occurrence::surroundedByOccurrences::surroundingSpace` is, gets no implied CrossSubsetting. `checkFeatureCrossingSpecialization` does not apply to it, which matches the reading of its type featuring in ADR 0020.

### 6. Validations of the implied relationships

Two validations would reject relationships that the semantic constraints themselves require.

- **`validateSubsettingFeaturingTypes`** (`canAccess`). An implied subsetting often reaches into a nested expression: a select's result subsets its argument's result, a feature with a value subsets the value's result, and an implied binding connector's end references a nested result. `canAccess` looks only outward through featuring types, so these always fail.
- **`validateRedefinitionEndConformance`.** A FeatureChainExpression's `sourceTargetFeature` is never an end, but it redefines the target feature, which can be one (`checkFeatureChainExpressionSourceTargetRedefinition`). An example is `transitionLink.laterOccurrence` in `StatePerformances`.

Both are read as applying to the relationships written, not to those the overlay adds to satisfy the semantic constraints. This is the same reading ADR 0020 gives `checkConnectorTypeFeaturing` and the implied binding connectors.

### 7. Readings of the OCL text

- `validateFeatureEndNoDirection` has `implied` where `implies` is meant.
- `validateRedefinitionFeaturingTypes` compares with `redefinedFeaturingType`, read as `redefinedFeaturingTypes`, and resolves `Base::Anything` with `modelElement`, read as `memberElement`. The constraint requires the two sets to differ: a feature cannot redefine a feature in the same context.
- `validateFeatureIsVariable` has `specializes('Occurrences::Occurrence')`, read as `specializesFromLibrary`.
- `validateFeatureMultiplicityDomain` compares featuring types without order, as the type-featuring constraints do (ADR 0020).
- `validateCrossSubsettingCrossedFeature` says in its text that the crossed feature "must have exactly two chainingFeatures". Its OCL asks for the other end only of a chain of two, and so lets a crossed feature that is no chain pass. The text is followed: the crossed feature of an end's CrossSubsetting is a chain of two features.
- An end of an N-ary association must have multiplicity 1..1 as written. `validateFeatureEndMultiplicity` finds none otherwise, and KerML 8.4.4.5.1 shows the ends written `[1..1]`. A binary association's ends get it from `Links::BinaryLink`.

## Consequences

- The KerML libraries produce zero diagnostics, of the checks and of the Root and Core validations, their 53,090 implied elements included.
- The implied relationships of step 1a.9 change in two places: a binary crossing chain, and an only end. The step 1a.9 checks still hold on the libraries.
- Step 1b.6 adds the 14 negative tests listed in `validations.json`.
