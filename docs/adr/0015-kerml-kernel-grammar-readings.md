# ADR 0015: Readings of the KerML Kernel grammar

- Status: Accepted
- Date: 2026-11-20
- Context: step 1a.4, the Kernel grammar and expressions

## Context

Step 1a.4 completes the KerML grammar (KerML 8.2.5). In four places, the grammar text cannot be implemented as written: either it disagrees with the normative library, or it uses something that the lexical structure or the grammar itself does not define. As in ADR 0014, CLAUDE.md asks for each reading to be recorded. The normative standard libraries (KerML clause 9) are the strongest evidence, since a conforming tool must parse them.

## Decisions

### 1. The FeatureDeclaration of a step, expression, Boolean expression or invariant is optional

KerML 8.2.5.6.2 and 8.2.5.7 read, for example:

```text
Invariant =
       FeaturePrefix
       'inv' ( 'true' | isNegated ?= 'false' )?
       FeatureDeclaration ValuePart?
       FunctionBody
```

`Step`, `Expression` and `BooleanExpression` are written the same way. Taken literally, an invariant needs a name or a specialization. But the Kernel libraries write `inv { … }` 42 times with neither, for example `inv { flattenedSize == size(elements) }` in `Collections.kerml`. The examples in KerML 7.4.8 do the same (`inv { fuelLevel >= 0 & … }`).

**Reading:** `FeatureDeclaration?` in all four productions. They have the same shape, and the `feature` keyword form of Feature (8.2.4.3.1) already makes its declaration optional. The connector, binding and succession productions already have `FeatureDeclaration?`.

### 2. `new` is a contextual keyword

ConstructorExpression (8.2.5.8.3) is `'new' InstantiatedTypeMember ConstructorResultMember`, and the libraries use it, for example `new ChangeSignal(condition, monitor)` in `Triggers.kerml`. But `new` is not among the reserved words of KerML 8.2.2.6, nor of SysML 8.2.2.1.2.

**Reading:** `new` stays a name; the lexer is unchanged. In an expression, the name `new` followed by a qualified name starts a ConstructorExpression. Anywhere else, `new` is an ordinary name: `feature new;` declares a feature called `new`.

### 3. `@@` is two `@` tokens

MetaclassificationExpression (8.2.5.8.1) uses the terminal `'@@'`, but the symbols of KerML 8.2.2.7 have no `@@`. By the longest-match rule, `@@` is lexed as two `@` tokens.

**Reading:** the lexer keeps to 8.2.2.7. The expression parser takes two *adjacent* `@` tokens as the metaclassification operator, at the precedence of the classification operators (Table 6). The tree keeps the two tokens. Adjacent means with nothing between them, which is what longest-match lexing makes of `@@`. With white space or a note between them (`a @ @M`), they are two classification operators, and the second is an error.

### 4. InvocationTypeMember is InstantiatedTypeMember

FunctionOperationExpression (8.2.5.8.2) uses `InvocationTypeMember`, which no production defines.

**Reading:** it is InstantiatedTypeMember (8.2.5.8.3), `[QualifiedName]` or a feature chain, the same reference to the function as in an InvocationExpression.

## Also recorded

- **Mistakes in the clause 7 examples.** Several informative examples are not valid KerML, and the parser reports them:
  - `classifier Vehicle : RegisteredAsset` and `featuring … featured by` (7.3.4)
  - `readonly` (7.4.8)
  - `redefine step` (7.4.13)
  - `behavior setX(c : C, …)` (7.2.4)

  The grammar is not changed for any of them.
- **A multiplicity bound** is only a literal or a feature reference (MultiplicityExpressionMember, 8.2.5.11), as written. A longer expression in a bound is a syntax error, `syntax::expected-bound`.

## Consequences

- The four readings are implemented in `crates/sysml-syntax`, citing this ADR. They are pinned by `invariant_forms`, `new_is_a_name_unless_a_type_follows`, `classification_expressions`, `the_metaclassification_operator_is_two_adjacent_at_signs` and `primary_expressions`, and by the KerML library test.
- If the OMG corrects a production, its test changes and this ADR is superseded in part.
