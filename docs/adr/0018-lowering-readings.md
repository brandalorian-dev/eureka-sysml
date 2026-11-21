# ADR 0018: Readings of the lowering from syntax to abstract syntax

- Status: Accepted
- Date: 2026-11-21
- Context: step 1a.6, the lowering from syntax trees to the element graph (ADR 0017)

## Context

Each production of the KerML grammar states what it synthesizes in the abstract syntax (KerML 8.2.1, Table 3). Step 1a.6 implements these mappings in `sysml-semantics` (`lower`). Its main test is the abstract syntax the OMG pilot implementation built from the 36 KerML library files: their XMI serialization (the oracle of ADR 0009), compared element by element. CLAUDE.md allows the pilot as a behavioral reference, not as code.

In some places a production cannot be applied as written, or the pilot does something the production does not say. This ADR records how each case is read, and why. The oracle test lists the three differences from the pilot that remain (`ACCEPTED`), each with its reason. Any other difference fails the test.

## Decisions

### Values the constraints fix

The lowering sets the stored values that a validation constraint requires, since a model without them is invalid:
- **Parameter directions.** The feature of a ParameterMembership has direction `in`, and that of a ReturnParameterMembership has `out` (KerML 8.3.4.6.4, `validateParameterMembershipParameterDirection`, and 8.3.4.7.4). This applies to arguments, empty results and `return` features without a written direction.
- **Ends.** The feature of an EndFeatureMembership (a connector or flow end) has `isEnd` set (KerML 8.3.3.3.3, `validateEndFeatureMembershipIsEnd`).
- **The source of an owned relationship** is its owner when the metaclass stores its source end: `specific`, `typedFeature`, `subsettingFeature`, `redefiningFeature`, `conjugatedType`, `typeDisjoined`, `featureInverted` and `featureOfType`. When the source end is derived, it is not set. That covers `ReferenceSubsetting::referencingFeature`, `Unioning::typeUnioned` and `FeatureChaining::featureChained`, whose derived attributes redefine the stored end (ADR 0017's generator follows these redefinitions).

### Expressions (KerML 8.2.5.8)

1. **Every operand is an argument.** The grammar owns some operands through a FeatureMembership:
   - the second and third operands of `if`, `??`, `or`, `and` and `implies` (ArgumentExpressionMember);
   - the items of a sequence (OwnedExpressionMember, SequenceExpressionListMember);
   - the index list of `#( … )` and `[ … ]`.

   But KerML 8.4.4.9.5 parses an invocation with "input parameters that have FeatureValue relationships to the arguments". 8.4.4.9.6 parses the conditional operands "as if they were body Expressions" passed as arguments: `if expr_1 ? expr_2 else expr_3` as `ControlFunctions::'if' (expr_1, { expr_2 }, { expr_3 })`. A feature with no direction is not an input parameter. These operands are therefore ParameterMemberships of features with direction `in`, each with its value, as the pilot builds them.
2. **Every expression has its result.** An EmptyResultMember is added where the production omits one, as the pilot does:
   - the forms of FeatureReferenceExpression (BodyExpression, FunctionReferenceExpression, and the OwnedExpressionReference of an ArgumentExpressionMember), which are FeatureReferenceExpressions, whose own production has one;
   - FeatureChainExpression, CollectExpression and SelectExpression;
   - IndexExpression, the bracket expression, and the SequenceOperatorExpression.

   ExtentExpression (`all T`) keeps none, as its production says.
3. **A cast has one result.** For `as` and `meta`, the TypeResultMember is the result parameter, so no EmptyResultMember follows it: an expression has one result. The grammar names its metaclass `ResultParameterMembership`, which does not exist; it is a ReturnParameterMembership.
4. **The source target of a feature chain.** KerML 8.3.4.8.4 defines `sourceTargetFeature()` as the first owned feature of the first input parameter of a FeatureChainExpression. Its `check` constraints require that feature to redefine the target feature. The lowering creates it, through a FeatureMembership of the argument. Its redefinitions are implied relationships (step 1a.9).
5. **`LiteralReal`** is written as a LiteralRational, the metaclass of KerML 8.3.4.8.13 (there is no `LiteralReal` metaclass).
6. **Literal values have fixed sizes.** A LiteralInteger holds an `i64`, and a LiteralRational a finite `f64`. A literal outside them is reported (`lower::integer-too-large`, `lower::real-out-of-range`), and its element has no value. KerML's Integer is unbounded, so this is a limit of the implementation. A minus sign is an operator, applied to the literal after it (KerML 8.2.5.8.1), so `-9223372036854775808` is out of range too, although `i64::MIN` is that value.

### Differences from the pilot that remain

| The pilot | The lowering | Why |
|---|---|---|
| The membership of an argument is `private` | No visibility, so `public` | ArgumentMember (8.2.5.8.1) sets no visibility, in the grammar text and in the PDF. Arguments have no names, so the visibility has no effect on resolution. |
| The source-target feature of a chain is named `""` | No name | KerML 8.3.4.8.4 gives it none. |
| A comment body with CR LF line ends is stored with LF | CR LF is kept | KerML 8.2.3.3.2, Note 2: the body keeps "all line terminators and white space included as entered". |

## Consequences

- All 36 KerML library files lower to the pilot's abstract syntax, element by element, apart from the three differences above. That covers every metaclass, its position in the ownership tree, every attribute that differs from its default, and which relationship ends are set: 24,931 elements in all. The values of the ends, which are names until step 1a.7, are compared once they are resolved.
- The readings are tested by `sysml-semantics`' production tests (`tests/lower.rs`), and by the oracle test (`tests/oracle.rs`).
- A production that the KerML library does not use has no oracle; the lowering follows its synthesis notation, and its test cites the clause. Examples are dependencies, type featurings, filter packages and flows with payloads.
