// SPDX-License-Identifier: Apache-2.0
//! The KerML Kernel grammar (KerML 8.2.5): one test per production, from its EBNF, against a tree
//! snapshot (see `grammar.rs` for the format; `UPDATE_EXPECT=1` rewrites them for review).

mod common;

use common::{check, codes};
use expect_test::expect;
use sysml_syntax::{parse, Language};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

// KerML 8.2.5.1–8.2.5.4, 8.2.5.6.1, 8.2.5.9.1, 8.2.5.12: the classifier-like definitions.

#[test]
fn definitions_share_the_classifier_declaration() {
    check(
        "abstract datatype D :> E; class C [1] specializes D { } struct S ~ T; assoc A; \
         assoc struct AS :> A; behavior B; interaction I; #M metaclass MC;",
        expect![[r##"
            DataType
              "abstract"
              "datatype"
              Identification "D"
              ":>"
              OwnedSubclassification
                QualifiedName "E"
              Body ";"
            Class
              "class"
              Identification "C"
              MultiplicityBounds
                "["
                LiteralInteger "1"
                "]"
              "specializes"
              OwnedSubclassification
                QualifiedName "D"
              Body "{" "}"
            Structure
              "struct"
              Identification "S"
              "~"
              OwnedConjugation
                QualifiedName "T"
              Body ";"
            Association
              "assoc"
              Identification "A"
              Body ";"
            AssociationStructure
              "assoc"
              "struct"
              Identification "AS"
              ":>"
              OwnedSubclassification
                QualifiedName "A"
              Body ";"
            Behavior
              "behavior"
              Identification "B"
              Body ";"
            Interaction
              "interaction"
              Identification "I"
              Body ";"
            Metaclass
              PrefixMetadata
                "#"
                QualifiedName "M"
              "metaclass"
              Identification "MC"
              Body ";"
        "##]],
    );
}

// KerML 8.2.5.5: connectors, binding connectors, successions.

#[test]
fn connector_with_a_declaration_only() {
    check(
        "connector c : C; connector d = x;",
        expect![[r#"
        Connector
          "connector"
          Identification "c"
          ":"
          OwnedFeatureTyping
            QualifiedName "C"
          Body ";"
        Connector
          "connector"
          Identification "d"
          FeatureValue
            "="
            FeatureReferenceExpression
              QualifiedName "x"
          Body ";"
    "#]],
    );
}

#[test]
fn binary_connector_forms() {
    check(
        "connector c : C from [1] a references x to [0..*] y; connector a to b; \
         connector all from a.b to c; connector : HappensDuring from occ.startShot to self;",
        expect![[r#"
            Connector
              "connector"
              Identification "c"
              ":"
              OwnedFeatureTyping
                QualifiedName "C"
              "from"
              ConnectorEnd
                MultiplicityBounds
                  "["
                  LiteralInteger "1"
                  "]"
                "a"
                "references"
                OwnedReferenceSubsetting
                  QualifiedName "x"
              "to"
              ConnectorEnd
                MultiplicityBounds
                  "["
                  LiteralInteger "0"
                  ".."
                  LiteralInfinity "*"
                  "]"
                OwnedReferenceSubsetting
                  QualifiedName "y"
              Body ";"
            Connector
              "connector"
              ConnectorEnd
                OwnedReferenceSubsetting
                  QualifiedName "a"
              "to"
              ConnectorEnd
                OwnedReferenceSubsetting
                  QualifiedName "b"
              Body ";"
            Connector
              "connector"
              "all"
              "from"
              ConnectorEnd
                OwnedReferenceSubsetting
                  FeatureChain
                    QualifiedName "a"
                    "."
                    QualifiedName "b"
              "to"
              ConnectorEnd
                OwnedReferenceSubsetting
                  QualifiedName "c"
              Body ";"
            Connector
              "connector"
              ":"
              OwnedFeatureTyping
                QualifiedName "HappensDuring"
              "from"
              ConnectorEnd
                OwnedReferenceSubsetting
                  FeatureChain
                    QualifiedName "occ"
                    "."
                    QualifiedName "startShot"
              "to"
              ConnectorEnd
                OwnedReferenceSubsetting
                  QualifiedName "self"
              Body ";"
        "#]],
    );
}

#[test]
fn nary_connector() {
    check(
        "connector c : C (a, b ::> x, [1] c);",
        expect![[r#"
        Connector
          "connector"
          Identification "c"
          ":"
          OwnedFeatureTyping
            QualifiedName "C"
          "("
          ConnectorEnd
            OwnedReferenceSubsetting
              QualifiedName "a"
          ","
          ConnectorEnd
            "b"
            "::>"
            OwnedReferenceSubsetting
              QualifiedName "x"
          ","
          ConnectorEnd
            MultiplicityBounds
              "["
              LiteralInteger "1"
              "]"
            OwnedReferenceSubsetting
              QualifiedName "c"
          ")"
          Body ";"
    "#]],
    );
}

#[test]
fn binding_connector_forms() {
    check(
        "binding b : B of a = c; binding a.x = c; binding [0..1] u.v = [1] self; binding b;",
        expect![[r#"
            BindingConnector
              "binding"
              Identification "b"
              ":"
              OwnedFeatureTyping
                QualifiedName "B"
              "of"
              ConnectorEnd
                OwnedReferenceSubsetting
                  QualifiedName "a"
              "="
              ConnectorEnd
                OwnedReferenceSubsetting
                  QualifiedName "c"
              Body ";"
            BindingConnector
              "binding"
              ConnectorEnd
                OwnedReferenceSubsetting
                  FeatureChain
                    QualifiedName "a"
                    "."
                    QualifiedName "x"
              "="
              ConnectorEnd
                OwnedReferenceSubsetting
                  QualifiedName "c"
              Body ";"
            BindingConnector
              "binding"
              ConnectorEnd
                MultiplicityBounds
                  "["
                  LiteralInteger "0"
                  ".."
                  LiteralInteger "1"
                  "]"
                OwnedReferenceSubsetting
                  FeatureChain
                    QualifiedName "u"
                    "."
                    QualifiedName "v"
              "="
              ConnectorEnd
                MultiplicityBounds
                  "["
                  LiteralInteger "1"
                  "]"
                OwnedReferenceSubsetting
                  QualifiedName "self"
              Body ";"
            BindingConnector
              "binding"
              Identification "b"
              Body ";"
        "#]],
    );
}

#[test]
fn succession_forms() {
    check(
        "succession s first a then b; succession [1] a then [*] b.c; succession all first a then b;",
        expect![[r#"
            Succession
              "succession"
              Identification "s"
              "first"
              ConnectorEnd
                OwnedReferenceSubsetting
                  QualifiedName "a"
              "then"
              ConnectorEnd
                OwnedReferenceSubsetting
                  QualifiedName "b"
              Body ";"
            Succession
              "succession"
              ConnectorEnd
                MultiplicityBounds
                  "["
                  LiteralInteger "1"
                  "]"
                OwnedReferenceSubsetting
                  QualifiedName "a"
              "then"
              ConnectorEnd
                MultiplicityBounds
                  "["
                  LiteralInfinity "*"
                  "]"
                OwnedReferenceSubsetting
                  FeatureChain
                    QualifiedName "b"
                    "."
                    QualifiedName "c"
              Body ";"
            Succession
              "succession"
              "all"
              "first"
              ConnectorEnd
                OwnedReferenceSubsetting
                  QualifiedName "a"
              "then"
              ConnectorEnd
                OwnedReferenceSubsetting
                  QualifiedName "b"
              Body ";"
        "#]],
    );
}

// KerML 8.2.5.6.2: steps.

#[test]
fn step_forms() {
    check(
        "step s : T = x; step : add { in x = y; } step :> monitor.startObservation;",
        expect![[r#"
            Step
              "step"
              Identification "s"
              ":"
              OwnedFeatureTyping
                QualifiedName "T"
              FeatureValue
                "="
                FeatureReferenceExpression
                  QualifiedName "x"
              Body ";"
            Step
              "step"
              ":"
              OwnedFeatureTyping
                QualifiedName "add"
              Body
                "{"
                Feature
                  "in"
                  Identification "x"
                  FeatureValue
                    "="
                    FeatureReferenceExpression
                      QualifiedName "y"
                  Body ";"
                "}"
            Step
              "step"
              ":>"
              OwnedSubsetting
                FeatureChain
                  QualifiedName "monitor"
                  "."
                  QualifiedName "startObservation"
              Body ";"
        "#]],
    );
}

// KerML 8.2.5.7: functions, expressions, predicates, Boolean expressions, invariants.

#[test]
fn function_with_parameters_return_and_result() {
    check(
        "function f { in x : Real; return : Real; x * 2 }",
        expect![[r#"
            Function
              "function"
              Identification "f"
              Body
                "{"
                Feature
                  "in"
                  Identification "x"
                  ":"
                  OwnedFeatureTyping
                    QualifiedName "Real"
                  Body ";"
                Feature
                  "return"
                  ":"
                  OwnedFeatureTyping
                    QualifiedName "Real"
                  Body ";"
                ResultExpressionMember
                  BinaryExpression
                    FeatureReferenceExpression
                      QualifiedName "x"
                    "*"
                    LiteralInteger "2"
                "}"
        "#]],
    );
}

#[test]
fn function_body_members_and_result() {
    // A name followed by what continues a feature is a feature, otherwise the result expression.
    check(
        "predicate p { in x; private feature y : T; x > y }",
        expect![[r#"
            Predicate
              "predicate"
              Identification "p"
              Body
                "{"
                Feature
                  "in"
                  Identification "x"
                  Body ";"
                Feature
                  "private"
                  "feature"
                  Identification "y"
                  ":"
                  OwnedFeatureTyping
                    QualifiedName "T"
                  Body ";"
                ResultExpressionMember
                  BinaryExpression
                    FeatureReferenceExpression
                      QualifiedName "x"
                    ">"
                    FeatureReferenceExpression
                      QualifiedName "y"
                "}"
        "#]],
    );
}

#[test]
fn expression_and_boolean_expression() {
    check(
        "expr e : E { 1 } bool b :>> signalCondition { a == b } expr f = g;",
        expect![[r#"
            Expression
              "expr"
              Identification "e"
              ":"
              OwnedFeatureTyping
                QualifiedName "E"
              Body
                "{"
                ResultExpressionMember
                  LiteralInteger "1"
                "}"
            BooleanExpression
              "bool"
              Identification "b"
              ":>>"
              OwnedRedefinition
                QualifiedName "signalCondition"
              Body
                "{"
                ResultExpressionMember
                  BinaryExpression
                    FeatureReferenceExpression
                      QualifiedName "a"
                    "=="
                    FeatureReferenceExpression
                      QualifiedName "b"
                "}"
            Expression
              "expr"
              Identification "f"
              FeatureValue
                "="
                FeatureReferenceExpression
                  QualifiedName "g"
              Body ";"
        "#]],
    );
}

#[test]
fn invariant_forms() {
    check(
        "inv { flattenedSize == size(elements) } inv true i { x } inv false { y }",
        expect![[r#"
            Invariant
              "inv"
              Body
                "{"
                ResultExpressionMember
                  BinaryExpression
                    FeatureReferenceExpression
                      QualifiedName "flattenedSize"
                    "=="
                    InvocationExpression
                      QualifiedName "size"
                      ArgumentList
                        "("
                        FeatureReferenceExpression
                          QualifiedName "elements"
                        ")"
                "}"
            Invariant
              "inv"
              "true"
              Identification "i"
              Body
                "{"
                ResultExpressionMember
                  FeatureReferenceExpression
                    QualifiedName "x"
                "}"
            Invariant
              "inv"
              "false"
              Body
                "{"
                ResultExpressionMember
                  FeatureReferenceExpression
                    QualifiedName "y"
                "}"
        "#]],
    );
}

#[test]
fn return_is_only_allowed_in_a_function_body() {
    assert_eq!(
        codes("classifier C { return x; }"),
        ["syntax::misplaced-prefix"]
    );
}

#[test]
fn the_result_expression_comes_last() {
    assert_eq!(
        codes("function f { x + 1 in y; }"),
        ["syntax::misplaced-member"]
    );
}

// KerML 8.2.5.8.2–8.2.5.8.4: primary, base and literal expressions.

#[test]
fn base_expressions() {
    check(
        r#"feature a = null; feature b = (); feature c = (1, 2.5, "s",); feature d = true;
           feature e = f(x, y); feature g = h(p = 1, q = 2); feature i = a.b(x);
           feature j = new T(1); feature k = .5e3;"#,
        expect![[r#"
            Feature
              "feature"
              Identification "a"
              FeatureValue
                "="
                NullExpression "null"
              Body ";"
            Feature
              "feature"
              Identification "b"
              FeatureValue
                "="
                NullExpression "(" ")"
              Body ";"
            Feature
              "feature"
              Identification "c"
              FeatureValue
                "="
                SequenceExpression
                  "("
                  LiteralInteger "1"
                  ","
                  LiteralReal "2" "." "5"
                  ","
                  LiteralString "\"s\""
                  ","
                  ")"
              Body ";"
            Feature
              "feature"
              Identification "d"
              FeatureValue
                "="
                LiteralBoolean "true"
              Body ";"
            Feature
              "feature"
              Identification "e"
              FeatureValue
                "="
                InvocationExpression
                  QualifiedName "f"
                  ArgumentList
                    "("
                    FeatureReferenceExpression
                      QualifiedName "x"
                    ","
                    FeatureReferenceExpression
                      QualifiedName "y"
                    ")"
              Body ";"
            Feature
              "feature"
              Identification "g"
              FeatureValue
                "="
                InvocationExpression
                  QualifiedName "h"
                  ArgumentList
                    "("
                    NamedArgument
                      QualifiedName "p"
                      "="
                      LiteralInteger "1"
                    ","
                    NamedArgument
                      QualifiedName "q"
                      "="
                      LiteralInteger "2"
                    ")"
              Body ";"
            Feature
              "feature"
              Identification "i"
              FeatureValue
                "="
                InvocationExpression
                  FeatureChain
                    QualifiedName "a"
                    "."
                    QualifiedName "b"
                  ArgumentList
                    "("
                    FeatureReferenceExpression
                      QualifiedName "x"
                    ")"
              Body ";"
            Feature
              "feature"
              Identification "j"
              FeatureValue
                "="
                ConstructorExpression
                  "new"
                  QualifiedName "T"
                  ArgumentList
                    "("
                    LiteralInteger "1"
                    ")"
              Body ";"
            Feature
              "feature"
              Identification "k"
              FeatureValue
                "="
                LiteralReal "." "5e3"
              Body ";"
        "#]],
    );
}

#[test]
fn primary_expressions() {
    check(
        "feature a = x.y; feature b = xs.{in v; v + 1}; feature c = xs.?{in v; v > 0}; \
         feature d = xs->collect {in v; v}; feature e = xs->reduce RealFunctions::max; \
         feature f = xs->size(); feature g = v#(1, 2); feature h = v[1]; feature i = e.metadata;",
        expect![[r##"
            Feature
              "feature"
              Identification "a"
              FeatureValue
                "="
                FeatureChainExpression
                  FeatureReferenceExpression
                    QualifiedName "x"
                  "."
                  QualifiedName "y"
              Body ";"
            Feature
              "feature"
              Identification "b"
              FeatureValue
                "="
                CollectExpression
                  FeatureReferenceExpression
                    QualifiedName "xs"
                  "."
                  BodyExpression
                    Body
                      "{"
                      Feature
                        "in"
                        Identification "v"
                        Body ";"
                      ResultExpressionMember
                        BinaryExpression
                          FeatureReferenceExpression
                            QualifiedName "v"
                          "+"
                          LiteralInteger "1"
                      "}"
              Body ";"
            Feature
              "feature"
              Identification "c"
              FeatureValue
                "="
                SelectExpression
                  FeatureReferenceExpression
                    QualifiedName "xs"
                  ".?"
                  BodyExpression
                    Body
                      "{"
                      Feature
                        "in"
                        Identification "v"
                        Body ";"
                      ResultExpressionMember
                        BinaryExpression
                          FeatureReferenceExpression
                            QualifiedName "v"
                          ">"
                          LiteralInteger "0"
                      "}"
              Body ";"
            Feature
              "feature"
              Identification "d"
              FeatureValue
                "="
                FunctionOperationExpression
                  FeatureReferenceExpression
                    QualifiedName "xs"
                  "->"
                  QualifiedName "collect"
                  BodyExpression
                    Body
                      "{"
                      Feature
                        "in"
                        Identification "v"
                        Body ";"
                      ResultExpressionMember
                        FeatureReferenceExpression
                          QualifiedName "v"
                      "}"
              Body ";"
            Feature
              "feature"
              Identification "e"
              FeatureValue
                "="
                FunctionOperationExpression
                  FeatureReferenceExpression
                    QualifiedName "xs"
                  "->"
                  QualifiedName "reduce"
                  QualifiedName "RealFunctions" "::" "max"
              Body ";"
            Feature
              "feature"
              Identification "f"
              FeatureValue
                "="
                FunctionOperationExpression
                  FeatureReferenceExpression
                    QualifiedName "xs"
                  "->"
                  QualifiedName "size"
                  ArgumentList "(" ")"
              Body ";"
            Feature
              "feature"
              Identification "g"
              FeatureValue
                "="
                IndexExpression
                  FeatureReferenceExpression
                    QualifiedName "v"
                  "#"
                  "("
                  LiteralInteger "1"
                  ","
                  LiteralInteger "2"
                  ")"
              Body ";"
            Feature
              "feature"
              Identification "h"
              FeatureValue
                "="
                BracketExpression
                  FeatureReferenceExpression
                    QualifiedName "v"
                  "["
                  LiteralInteger "1"
                  "]"
              Body ";"
            Feature
              "feature"
              Identification "i"
              FeatureValue
                "="
                MetadataAccessExpression
                  FeatureReferenceExpression
                    QualifiedName "e"
                  "."
                  "metadata"
              Body ";"
        "##]],
    );
}

#[test]
fn classification_expressions() {
    check(
        "feature a = x istype T; feature b = hastype T; feature c = @T; feature d = x as T; \
         feature e = x @@ M; feature f = x meta M; feature g = all T;",
        expect![[r#"
            Feature
              "feature"
              Identification "a"
              FeatureValue
                "="
                ClassificationExpression
                  FeatureReferenceExpression
                    QualifiedName "x"
                  "istype"
                  QualifiedName "T"
              Body ";"
            Feature
              "feature"
              Identification "b"
              FeatureValue
                "="
                ClassificationExpression
                  "hastype"
                  QualifiedName "T"
              Body ";"
            Feature
              "feature"
              Identification "c"
              FeatureValue
                "="
                ClassificationExpression
                  "@"
                  QualifiedName "T"
              Body ";"
            Feature
              "feature"
              Identification "d"
              FeatureValue
                "="
                ClassificationExpression
                  FeatureReferenceExpression
                    QualifiedName "x"
                  "as"
                  QualifiedName "T"
              Body ";"
            Feature
              "feature"
              Identification "e"
              FeatureValue
                "="
                ClassificationExpression
                  FeatureReferenceExpression
                    QualifiedName "x"
                  "@"
                  "@"
                  QualifiedName "M"
              Body ";"
            Feature
              "feature"
              Identification "f"
              FeatureValue
                "="
                ClassificationExpression
                  FeatureReferenceExpression
                    QualifiedName "x"
                  "meta"
                  QualifiedName "M"
              Body ";"
            Feature
              "feature"
              Identification "g"
              FeatureValue
                "="
                ExtentExpression
                  "all"
                  QualifiedName "T"
              Body ";"
        "#]],
    );
}

#[test]
fn a_body_expression() {
    check(
        "feature f = { in x : Real; x ^ 2 };",
        expect![[r#"
        Feature
          "feature"
          Identification "f"
          FeatureValue
            "="
            BodyExpression
              Body
                "{"
                Feature
                  "in"
                  Identification "x"
                  ":"
                  OwnedFeatureTyping
                    QualifiedName "Real"
                  Body ";"
                ResultExpressionMember
                  BinaryExpression
                    FeatureReferenceExpression
                      QualifiedName "x"
                    "^"
                    LiteralInteger "2"
                "}"
          Body ";"
    "#]],
    );
}

#[test]
fn new_is_a_name_unless_a_type_follows() {
    // `new` is not reserved (KerML 8.2.2.6, ADR 0015): it starts a constructor expression only
    // when a qualified name follows it.
    check(
        "feature new; feature x = new;",
        expect![[r#"
        Feature
          "feature"
          Identification "new"
          Body ";"
        Feature
          "feature"
          Identification "x"
          FeatureValue
            "="
            FeatureReferenceExpression
              QualifiedName "new"
          Body ";"
    "#]],
    );
}

#[test]
fn an_expression_is_expected() {
    check(
        "feature f = ; feature g = 1 +;",
        expect![[r#"
        Feature
          "feature"
          Identification "f"
          FeatureValue "="
          Body ";"
        Feature
          "feature"
          Identification "g"
          FeatureValue
            "="
            BinaryExpression
              LiteralInteger "1"
              "+"
          Body ";"
        syntax::expected-expression at 12..13 ";": expected an expression
        syntax::expected-expression at 29..30 ";": expected an expression
    "#]],
    );
}

// KerML 8.2.5.9.2: flows.

#[test]
fn flow_forms() {
    check(
        "flow f of p : P from a.x to b.y; flow a.x to b.y; flow of Integer from a to b; \
         succession flow s from a.x to b.y; flow all c.d to e.f;",
        expect![[r#"
            Flow
              "flow"
              Identification "f"
              "of"
              PayloadFeature
                Identification "p"
                ":"
                OwnedFeatureTyping
                  QualifiedName "P"
              "from"
              FlowEnd
                FeatureChain
                  QualifiedName "a"
                  "."
                  QualifiedName "x"
              "to"
              FlowEnd
                FeatureChain
                  QualifiedName "b"
                  "."
                  QualifiedName "y"
              Body ";"
            Flow
              "flow"
              FlowEnd
                FeatureChain
                  QualifiedName "a"
                  "."
                  QualifiedName "x"
              "to"
              FlowEnd
                FeatureChain
                  QualifiedName "b"
                  "."
                  QualifiedName "y"
              Body ";"
            Flow
              "flow"
              "of"
              PayloadFeature
                OwnedFeatureTyping
                  QualifiedName "Integer"
              "from"
              FlowEnd
                QualifiedName "a"
              "to"
              FlowEnd
                QualifiedName "b"
              Body ";"
            SuccessionFlow
              "succession"
              "flow"
              Identification "s"
              "from"
              FlowEnd
                FeatureChain
                  QualifiedName "a"
                  "."
                  QualifiedName "x"
              "to"
              FlowEnd
                FeatureChain
                  QualifiedName "b"
                  "."
                  QualifiedName "y"
              Body ";"
            Flow
              "flow"
              "all"
              FlowEnd
                FeatureChain
                  QualifiedName "c"
                  "."
                  QualifiedName "d"
              "to"
              FlowEnd
                FeatureChain
                  QualifiedName "e"
                  "."
                  QualifiedName "f"
              Body ";"
        "#]],
    );
}

// KerML 8.2.5.10: feature values.

#[test]
fn feature_value_forms() {
    check(
        "feature a = 1; feature b := 2; feature c default 3; feature d default = 4; \
         feature e default := 5;",
        expect![[r#"
            Feature
              "feature"
              Identification "a"
              FeatureValue
                "="
                LiteralInteger "1"
              Body ";"
            Feature
              "feature"
              Identification "b"
              FeatureValue
                ":="
                LiteralInteger "2"
              Body ";"
            Feature
              "feature"
              Identification "c"
              FeatureValue
                "default"
                LiteralInteger "3"
              Body ";"
            Feature
              "feature"
              Identification "d"
              FeatureValue
                "default"
                "="
                LiteralInteger "4"
              Body ";"
            Feature
              "feature"
              Identification "e"
              FeatureValue
                "default"
                ":="
                LiteralInteger "5"
              Body ";"
        "#]],
    );
}

// KerML 8.2.5.11: a bound is a literal or a feature reference.

#[test]
fn multiplicity_bounds_are_not_expressions() {
    assert_eq!(codes("feature a [n + 1];"), ["syntax::expected-bound"]);
}

// KerML 8.2.5.12: metadata.

#[test]
fn metadata_feature_forms() {
    check(
        "@M; metadata m : M about a, b::c { x = 1; feature y :>> z = 2; redefines w; } \
         #P @Q typed by R;",
        expect![[r##"
            MetadataFeature
              "@"
              OwnedFeatureTyping
                QualifiedName "M"
              Body ";"
            MetadataFeature
              "metadata"
              Identification "m"
              ":"
              OwnedFeatureTyping
                QualifiedName "M"
              "about"
              Annotation
                QualifiedName "a"
              ","
              Annotation
                QualifiedName "b" "::" "c"
              Body
                "{"
                Feature
                  OwnedRedefinition
                    QualifiedName "x"
                  FeatureValue
                    "="
                    LiteralInteger "1"
                  Body ";"
                Feature
                  "feature"
                  OwnedRedefinition
                    QualifiedName "y"
                  ":>>"
                  OwnedRedefinition
                    QualifiedName "z"
                  FeatureValue
                    "="
                    LiteralInteger "2"
                  Body ";"
                Feature
                  "redefines"
                  OwnedRedefinition
                    QualifiedName "w"
                  Body ";"
                "}"
            MetadataFeature
              PrefixMetadata
                "#"
                QualifiedName "P"
              "@"
              Identification "Q"
              "typed"
              "by"
              OwnedFeatureTyping
                QualifiedName "R"
              Body ";"
        "##]],
    );
}

// KerML 8.2.5.13: element filters, and the filter conditions of imports (8.2.3.4.2).

#[test]
fn element_filter_member() {
    check(
        "package P { private filter @Safety; filter not isAbstract; }",
        expect![[r#"
            Package
              "package"
              Identification "P"
              Body
                "{"
                ElementFilterMember
                  "private"
                  "filter"
                  ClassificationExpression
                    "@"
                    QualifiedName "Safety"
                  ";"
                ElementFilterMember
                  "filter"
                  UnaryExpression
                    "not"
                    FeatureReferenceExpression
                      QualifiedName "isAbstract"
                  ";"
                "}"
        "#]],
    );
}

#[test]
fn an_element_filter_is_only_allowed_in_a_package() {
    assert_eq!(
        codes("namespace N { filter x; }"),
        ["syntax::misplaced-member"]
    );
}

#[test]
fn import_with_filter_conditions() {
    check(
        "private import NA::*[@Approved][x and y];",
        expect![[r#"
        Import
          "private"
          "import"
          QualifiedName "NA"
          "::"
          "*"
          FilterPackageMember
            "["
            ClassificationExpression
              "@"
              QualifiedName "Approved"
            "]"
          FilterPackageMember
            "["
            BinaryExpression
              FeatureReferenceExpression
                QualifiedName "x"
              "and"
              FeatureReferenceExpression
                QualifiedName "y"
            "]"
          Body ";"
    "#]],
    );
}

// The examples of KerML 7.4 (the Kernel layer), quoted.

#[test]
fn clause_7_4_examples_parse_without_diagnostics() {
    let examples = [
        // 7.4.10
        "interaction Authorization {\n       end feature client[*] : Computer;\n       end feature server[*] : Computer;\n       composite step login;\n       composite step authorize;\n       composite succession login then authorize;\n\n}",
        // 7.4.10
        "flow of flowingFuel : Fuel from fuelTank.fuelOut to engine.fuelIn;",
        // 7.4.10
        "behavior TakePicture {\n       composite step focus : Focus { out image[1] : Image; }\n       composite step shoot : Shoot { in image[1] : Image; }\n       // The use of a succession flow means that focus must complete before\n       // the image is transferred, after which shoot can begin.\n       succession flow focus.image to shoot.image;\n\n}",
        // 7.4.11
        "var feature count[1] : Natural := 0;",
        // 7.4.12
        "struct Automobile {\n       feature n : Positive[1];\n       composite feature wheels : Wheel[n]; // Equivalent to [n..n] for n < *\n       feature driveWheels[2..n] subsets wheels;\n\n\n}\nfeature autoCollection : Automobile[*]; // Equivalent to [0..*]",
        // 7.4.13
        "metadata ApprovalAnnotation about Design {\n       feature redefines approved = true;\n       feature redefines approver = \"John Smith\";\n\n}",
        // 7.4.13
        "#SecurityRelated #command behavior Save;",
        // 7.4.14
        "package PackageApprovals {\n       private import Annotations::*;\n       private import KerML::*;\n\n       // This imports all structures from the DesignModel that have\n       // at least one owned feature and have been marked as approved.\n       public import DesignModel::**[@Structure and\n\n                                                 Structure::ownedFeature != null and\n                                                 @ApprovalAnnotation and\n                                                 ApprovalAnnotation::approved];\n}",
        // 7.4.5
        "assoc all AssetOwnership {\n       end feature owner : LegalEntity crosses ownedAsset.owningEntities;\n       end feature ownedAsset : Asset => owner.assetsOwned;\n       feature valuationOnPurchase : MonetaryValue;\n\n}",
        // 7.4.5
        "assoc SoleAssetOwnership specializes LegalAssetOwnership {\n       end [1] feature owner; // Redefines LegalAssetOwnership::owner.\n       // ownedAsset is inherited as an association end.\n       // valuationOnPurchase is inherited as a non-end feature.\n\n}",
        // 7.4.6
        "struct Axle {\n       composite feature halfAxles[2] : HalfAxle;\n\n}\nstruct Wheel {\n\n       composite feature hub : Hub[1];\n       composite feature tire : Tire[1];\n}\nstruct WheelAssembly {\n       composite feature axle[1] : Axle;\n       composite feature wheels[2] : Wheel;\n\n       connector mount : Mounting from axle.halfAxles to wheels.hub;\n}",
        // 7.4.6
        "behavior TakePicture {\n       composite step focus : Focus;\n       composite step shoot : Shoot;\n       succession controlFlow first focus then shoot;\n\n}",
        // 7.4.7
        "behavior TakePicture {\n       in scene : Scene;\n       out picture : Picture;\n\n       binding focus.scene = scene;\n       composite step focus : Focus;\n       succession focus then shoot;\n       composite flow focus.image to shoot.image;\n       composite step shoot : Shoot;\n       binding picture = shoot.picture;\n}",
        // 7.4.8
        "function Average {\n       in scores[1..*] : Rational;\n       return : Rational;\n\n       sum(scores) / size(scores)\n}",
        // 7.4.8
        "expr computation : ComputeDynamics {\n       // Parameters redefined parameters of ComputeDynamics.\n       in state;\n       in dt;\n       return result;\n\n}\nexpr vehicleComputation subsets computation {\n\n       // Input parameters are inherited, result is redefined.\n       return : VehicleState;\n}",
        // 7.4.8
        "predicate isFull {\n       in tank : FuelTank;\n       tank.fuelLevel == tank.maxFuelLevel\n\n}",
    ];
    for text in examples {
        let p = parse(Language::KerML, text);
        assert!(p.diagnostics().is_empty(), "{text}\n{:?}", p.diagnostics());
        assert_eq!(p.syntax().text().to_string(), text);
    }
}
