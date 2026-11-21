// SPDX-License-Identifier: Apache-2.0
//! The KerML Root and Core grammar (KerML 8.2.3, 8.2.4) and the Kernel parts they need first
//! (8.2.5.11–8.2.5.13): one test per production, from its EBNF, against a tree snapshot.
//!
//! The snapshots are inline (`expect-test`); `UPDATE_EXPECT=1 cargo test -p sysml-syntax` rewrites
//! them, and every rewrite is reviewed against the clause. In a snapshot, a node is a line with
//! its kind, its tokens follow it (inline if it has no child nodes), and trivia is left out; the
//! diagnostics come last.

mod common;

use common::{check, codes};
use expect_test::expect;
use sysml_syntax::{parse, Language, SyntaxKind as K};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

// KerML 8.2.3.1: Identification and RelationshipBody.

#[test]
fn identification_with_a_short_name_and_a_name() {
    check(
        "namespace <'1.1'> N1;",
        expect![[r#"
            Namespace
              "namespace"
              Identification
                ShortName "<" "'1.1'" ">"
                "N1"
              Body ";"
        "#]],
    );
}

#[test]
fn identification_forms() {
    check(
        "namespace <s>; namespace n; namespace 'unrestricted name'; namespace;",
        expect![[r#"
            Namespace
              "namespace"
              Identification
                ShortName "<" "s" ">"
              Body ";"
            Namespace
              "namespace"
              Identification "n"
              Body ";"
            Namespace
              "namespace"
              Identification "'unrestricted name'"
              Body ";"
            Namespace
              "namespace"
              Body ";"
        "#]],
    );
}

#[test]
fn a_relationship_body_owns_elements_and_annotations() {
    // RelationshipOwnedElement: an OwnedRelatedElement or an OwnedAnnotation.
    check(
        "alias A for B { doc /* d */ classifier C; }",
        expect![[r#"
            AliasMember
              "alias"
              Identification "A"
              "for"
              QualifiedName "B"
              Body
                "{"
                Documentation "doc" "/* d */"
                Classifier
                  "classifier"
                  Identification "C"
                  Body ";"
                "}"
        "#]],
    );
}

// KerML 8.2.3.2: Dependency.

#[test]
fn dependency_without_identification() {
    check(
        "dependency A, B::C to D, E;",
        expect![[r#"
            Dependency
              "dependency"
              QualifiedName "A"
              ","
              QualifiedName "B" "::" "C"
              "to"
              QualifiedName "D"
              ","
              QualifiedName "E"
              Body ";"
        "#]],
    );
}

#[test]
fn dependency_with_identification_and_prefix_metadata() {
    check(
        "#Trace dependency <d1> Use from 'Application Layer' to 'Service Layer' { /* why */ }",
        expect![[r##"
            Dependency
              PrefixMetadata
                "#"
                QualifiedName "Trace"
              "dependency"
              Identification
                ShortName "<" "d1" ">"
                "Use"
              "from"
              QualifiedName "'Application Layer'"
              "to"
              QualifiedName "'Service Layer'"
              Body
                "{"
                Comment "/* why */"
                "}"
        "##]],
    );
}

#[test]
fn dependency_with_an_empty_identification() {
    check(
        "dependency from A to B;",
        expect![[r#"
        Dependency
          "dependency"
          "from"
          QualifiedName "A"
          "to"
          QualifiedName "B"
          Body ";"
    "#]],
    );
}

// KerML 8.2.3.3: Annotations, Comments, Documentation, TextualRepresentation.

#[test]
fn a_regular_comment_is_a_comment_element() {
    check(
        "/* This is the comment body text. */",
        expect![[r#"
        Comment "/* This is the comment body text. */"
    "#]],
    );
}

#[test]
fn comment_with_about_and_locale() {
    check(
        r#"comment <c1> C about A, B::C locale "en_US" /* text */"#,
        expect![[r#"
            Comment
              "comment"
              Identification
                ShortName "<" "c1" ">"
                "C"
              "about"
              Annotation
                QualifiedName "A"
              ","
              Annotation
                QualifiedName "B" "::" "C"
              "locale"
              "\"en_US\""
              "/* text */"
        "#]],
    );
}

#[test]
fn comment_with_locale_only() {
    check(
        r#"locale "en_US" /* text */"#,
        expect![[r#"
        Comment "locale" "\"en_US\"" "/* text */"
    "#]],
    );
}

#[test]
fn documentation_forms() {
    check(
        r#"doc X_Comment /* about X */ doc locale "fr" /* à propos */"#,
        expect![[r#"
            Documentation
              "doc"
              Identification "X_Comment"
              "/* about X */"
            Documentation "doc" "locale" "\"fr\"" "/* à propos */"
        "#]],
    );
}

#[test]
fn textual_representation_forms() {
    check(
        r#"rep inOCL language "ocl" /* self.x > 0.0 */ language "alf" /* x = 1; */"#,
        expect![[r#"
            TextualRepresentation
              "rep"
              Identification "inOCL"
              "language"
              "\"ocl\""
              "/* self.x > 0.0 */"
            TextualRepresentation "language" "\"alf\"" "/* x = 1; */"
        "#]],
    );
}

#[test]
fn a_comment_needs_its_body() {
    check(
        "comment C about A;",
        expect![[r#"
        Comment
          "comment"
          Identification "C"
          "about"
          Annotation
            QualifiedName "A"
        Error ";"
        syntax::expected-comment at 17..18 ";": expected a comment
    "#]],
    );
}

// KerML 8.2.3.4.1: Namespaces, members, aliases, qualified names.

#[test]
fn namespace_with_members_and_visibility() {
    check(
        "namespace N { public classifier A; private feature f : A; protected namespace M; }",
        expect![[r#"
            Namespace
              "namespace"
              Identification "N"
              Body
                "{"
                Classifier
                  "public"
                  "classifier"
                  Identification "A"
                  Body ";"
                Feature
                  "private"
                  "feature"
                  Identification "f"
                  ":"
                  OwnedFeatureTyping
                    QualifiedName "A"
                  Body ";"
                Namespace
                  "protected"
                  "namespace"
                  Identification "M"
                  Body ";"
                "}"
        "#]],
    );
}

#[test]
fn alias_member_forms() {
    check(
        "namespace N { alias <C> CCC for B { doc /* d */ } private alias D for $::B::C; alias for E; }",
        expect![[r#"
            Namespace
              "namespace"
              Identification "N"
              Body
                "{"
                AliasMember
                  "alias"
                  Identification
                    ShortName "<" "C" ">"
                    "CCC"
                  "for"
                  QualifiedName "B"
                  Body
                    "{"
                    Documentation "doc" "/* d */"
                    "}"
                AliasMember
                  "private"
                  "alias"
                  Identification "D"
                  "for"
                  QualifiedName "$" "::" "B" "::" "C"
                  Body ";"
                AliasMember
                  "alias"
                  "for"
                  QualifiedName "E"
                  Body ";"
                "}"
        "#]],
    );
}

#[test]
fn qualified_names_with_the_global_scope() {
    check(
        "alias a for $::A::'b c'::D;",
        expect![[r#"
        AliasMember
          "alias"
          Identification "a"
          "for"
          QualifiedName "$" "::" "A" "::" "'b c'" "::" "D"
          Body ";"
    "#]],
    );
}

// KerML 8.2.3.4.2: Imports.

#[test]
fn membership_imports() {
    check(
        "public import A::B; private import all A::B::**;",
        expect![[r#"
            Import
              "public"
              "import"
              QualifiedName "A" "::" "B"
              Body ";"
            Import
              "private"
              "import"
              "all"
              QualifiedName "A" "::" "B"
              "::"
              "**"
              Body ";"
        "#]],
    );
}

#[test]
fn namespace_imports() {
    check(
        "protected import A::*; private import A::*::** { doc /* d */ }",
        expect![[r#"
            Import
              "protected"
              "import"
              QualifiedName "A"
              "::"
              "*"
              Body ";"
            Import
              "private"
              "import"
              QualifiedName "A"
              "::"
              "*"
              "::"
              "**"
              Body
                "{"
                Documentation "doc" "/* d */"
                "}"
        "#]],
    );
}

#[test]
fn an_import_needs_a_visibility() {
    // The grammar requires it (KerML 8.2.3.4.2), although the KerML 7.2.5 example `import C;`
    // omits it (ADR 0014). The import is still parsed.
    check(
        "import C;",
        expect![[r#"
        Import
          "import"
          QualifiedName "C"
          Body ";"
        syntax::import-without-visibility at 0..6 "import": an import needs a visibility: `public`, `private` or `protected`
    "#]],
    );
}

// KerML 8.2.4.1: Types.

#[test]
fn type_with_every_declaration_part() {
    check(
        "abstract #M type all T[1] :> A, B.c ~ C disjoint from D, E unions F intersects G differences H { feature f; }",
        expect![[r##"
            Type
              "abstract"
              PrefixMetadata
                "#"
                QualifiedName "M"
              "type"
              "all"
              Identification "T"
              MultiplicityBounds
                "["
                LiteralInteger "1"
                "]"
              ":>"
              OwnedSpecialization
                QualifiedName "A"
              ","
              OwnedSpecialization
                FeatureChain
                  QualifiedName "B"
                  "."
                  QualifiedName "c"
              "~"
              OwnedConjugation
                QualifiedName "C"
              "disjoint"
              "from"
              OwnedDisjoining
                QualifiedName "D"
              ","
              OwnedDisjoining
                QualifiedName "E"
              "unions"
              Unioning
                QualifiedName "F"
              "intersects"
              Intersecting
                QualifiedName "G"
              "differences"
              Differencing
                QualifiedName "H"
              Body
                "{"
                Feature
                  "feature"
                  Identification "f"
                  Body ";"
                "}"
        "##]],
    );
}

#[test]
fn type_specialization_keywords() {
    check(
        "type A specializes B conjugates C;",
        expect![[r#"
            Type
              "type"
              Identification "A"
              "specializes"
              OwnedSpecialization
                QualifiedName "B"
              "conjugates"
              OwnedConjugation
                QualifiedName "C"
              Body ";"
        "#]],
    );
}

#[test]
fn a_type_needs_a_specialization_or_conjugation() {
    // TypeDeclaration has `( SpecializationPart | ConjugationPart )+` (KerML 8.2.4.1.1).
    check(
        "type T;",
        expect![[r#"
        Type
          "type"
          Identification "T"
          Body ";"
        syntax::expected-specialization at 6..7 ";": expected `specializes` or `conjugates`: a type declaration has at least one
    "#]],
    );
}

#[test]
fn specialization_forms() {
    check(
        "specialization Gen subtype A specializes B; subtype C :> A; subtype a.b :> c.d;",
        expect![[r#"
            Specialization
              "specialization"
              Identification "Gen"
              "subtype"
              QualifiedName "A"
              "specializes"
              QualifiedName "B"
              Body ";"
            Specialization
              "subtype"
              QualifiedName "C"
              ":>"
              QualifiedName "A"
              Body ";"
            Specialization
              "subtype"
              FeatureChain
                QualifiedName "a"
                "."
                QualifiedName "b"
              ":>"
              FeatureChain
                QualifiedName "c"
                "."
                QualifiedName "d"
              Body ";"
        "#]],
    );
}

#[test]
fn conjugation_forms() {
    check(
        "conjugation c1 conjugate Conjugate1 conjugates Original; conjugate a.b ~ c { doc /* d */ }",
        expect![[r#"
            Conjugation
              "conjugation"
              Identification "c1"
              "conjugate"
              QualifiedName "Conjugate1"
              "conjugates"
              QualifiedName "Original"
              Body ";"
            Conjugation
              "conjugate"
              FeatureChain
                QualifiedName "a"
                "."
                QualifiedName "b"
              "~"
              QualifiedName "c"
              Body
                "{"
                Documentation "doc" "/* d */"
                "}"
        "#]],
    );
}

#[test]
fn disjoining_forms() {
    check(
        "disjoining Disj disjoint A from B; disjoint a.b from c;",
        expect![[r#"
            Disjoining
              "disjoining"
              Identification "Disj"
              "disjoint"
              QualifiedName "A"
              "from"
              QualifiedName "B"
              Body ";"
            Disjoining
              "disjoint"
              FeatureChain
                QualifiedName "a"
                "."
                QualifiedName "b"
              "from"
              QualifiedName "c"
              Body ";"
        "#]],
    );
}

#[test]
fn type_feature_member() {
    // TypeFeatureMember: `MemberPrefix 'member' FeatureElement` (KerML 8.2.4.1.6).
    check(
        "classifier C { private member feature f; }",
        expect![[r#"
        Classifier
          "classifier"
          Identification "C"
          Body
            "{"
            Feature
              "private"
              "member"
              "feature"
              Identification "f"
              Body ";"
            "}"
    "#]],
    );
}

#[test]
fn member_is_only_allowed_in_a_type_body() {
    assert_eq!(
        codes("namespace N { member feature f; }"),
        ["syntax::misplaced-prefix"]
    );
}

// KerML 8.2.4.2: Classifiers.

#[test]
fn classifier_forms() {
    check(
        "classifier <c123> AClassifier; abstract classifier all C[0..1] :> A, B::C disjoint from D; classifier FuelOutPort conjugates FuelInPort;",
        expect![[r#"
            Classifier
              "classifier"
              Identification
                ShortName "<" "c123" ">"
                "AClassifier"
              Body ";"
            Classifier
              "abstract"
              "classifier"
              "all"
              Identification "C"
              MultiplicityBounds
                "["
                LiteralInteger "0"
                ".."
                LiteralInteger "1"
                "]"
              ":>"
              OwnedSubclassification
                QualifiedName "A"
              ","
              OwnedSubclassification
                QualifiedName "B" "::" "C"
              "disjoint"
              "from"
              OwnedDisjoining
                QualifiedName "D"
              Body ";"
            Classifier
              "classifier"
              Identification "FuelOutPort"
              "conjugates"
              OwnedConjugation
                QualifiedName "FuelInPort"
              Body ";"
        "#]],
    );
}

#[test]
fn subclassification_forms() {
    check(
        "specialization Super subclassifier A specializes B; subclassifier C :> A;",
        expect![[r#"
            Subclassification
              "specialization"
              Identification "Super"
              "subclassifier"
              QualifiedName "A"
              "specializes"
              QualifiedName "B"
              Body ";"
            Subclassification
              "subclassifier"
              QualifiedName "C"
              ":>"
              QualifiedName "A"
              Body ";"
        "#]],
    );
}

// KerML 8.2.4.3: Features.

#[test]
fn feature_with_the_keyword_and_no_declaration() {
    check(
        "feature; feature { }",
        expect![[r#"
        Feature
          "feature"
          Body ";"
        Feature
          "feature"
          Body "{" "}"
    "#]],
    );
}

#[test]
fn feature_without_the_keyword() {
    check(
        "x : T; all y :> z; [1] ordered;",
        expect![[r#"
        Feature
          Identification "x"
          ":"
          OwnedFeatureTyping
            QualifiedName "T"
          Body ";"
        Feature
          "all"
          Identification "y"
          ":>"
          OwnedSubsetting
            QualifiedName "z"
          Body ";"
        Feature
          MultiplicityBounds
            "["
            LiteralInteger "1"
            "]"
          "ordered"
          Body ";"
    "#]],
    );
}

#[test]
fn basic_feature_prefix_in_order() {
    check(
        "in derived abstract composite var feature x : T; out portion const y; inout const z;",
        expect![[r#"
            Feature
              "in"
              "derived"
              "abstract"
              "composite"
              "var"
              "feature"
              Identification "x"
              ":"
              OwnedFeatureTyping
                QualifiedName "T"
              Body ";"
            Feature
              "out"
              "portion"
              "const"
              Identification "y"
              Body ";"
            Feature
              "inout"
              "const"
              Identification "z"
              Body ";"
        "#]],
    );
}

#[test]
fn feature_prefix_out_of_order() {
    check(
        "abstract in feature x;",
        expect![[r#"
        Feature
          "abstract"
          Error "in"
          "feature"
          Identification "x"
          Body ";"
        syntax::misplaced-prefix at 9..11 "in": `in` is not allowed here
    "#]],
    );
}

#[test]
fn end_features() {
    check(
        "const end feature e : T; end f : T; end #M g;",
        expect![[r##"
            Feature
              "const"
              "end"
              "feature"
              Identification "e"
              ":"
              OwnedFeatureTyping
                QualifiedName "T"
              Body ";"
            Feature
              "end"
              Identification "f"
              ":"
              OwnedFeatureTyping
                QualifiedName "T"
              Body ";"
            Feature
              "end"
              PrefixMetadata
                "#"
                QualifiedName "M"
              Identification "g"
              Body ";"
        "##]],
    );
}

#[test]
fn end_feature_with_a_cross_feature() {
    // OwnedCrossFeature (KerML 8.2.4.3.1), as in the Kernel Semantic Library.
    check(
        "end within [1..*] feature largerOccurrence : Occurrence redefines longerOccurrence; end [n] feature x;",
        expect![[r#"
            Feature
              "end"
              CrossFeature
                Identification "within"
                MultiplicityBounds
                  "["
                  LiteralInteger "1"
                  ".."
                  LiteralInfinity "*"
                  "]"
              "feature"
              Identification "largerOccurrence"
              ":"
              OwnedFeatureTyping
                QualifiedName "Occurrence"
              "redefines"
              OwnedRedefinition
                QualifiedName "longerOccurrence"
              Body ";"
            Feature
              "end"
              CrossFeature
                MultiplicityBounds
                  "["
                  FeatureReferenceExpression
                    QualifiedName "n"
                  "]"
              "feature"
              Identification "x"
              Body ";"
        "#]],
    );
}

#[test]
fn a_feature_prefix_after_end_needs_a_cross_feature() {
    check(
        "end in x;",
        expect![[r#"
        Feature
          "end"
          "in"
          Identification "x"
          Body ";"
        syntax::expected-token at 8..9 ";": expected `feature`: after `end`, a feature prefix belongs to a cross feature
    "#]],
    );
}

#[test]
fn prefix_metadata_alone_makes_a_feature() {
    // `FeaturePrefix PrefixMetadataMember FeatureDeclaration?` (KerML 8.2.4.3.1).
    check(
        "#M; #M x : T; #A::B #c.d feature y;",
        expect![[r##"
        Feature
          PrefixMetadata
            "#"
            QualifiedName "M"
          Body ";"
        Feature
          PrefixMetadata
            "#"
            QualifiedName "M"
          Identification "x"
          ":"
          OwnedFeatureTyping
            QualifiedName "T"
          Body ";"
        Feature
          PrefixMetadata
            "#"
            QualifiedName "A" "::" "B"
          PrefixMetadata
            "#"
            FeatureChain
              QualifiedName "c"
              "."
              QualifiedName "d"
          "feature"
          Identification "y"
          Body ";"
    "##]],
    );
}

#[test]
fn feature_specializations() {
    check(
        "feature x typed by A, B subsets f, g redefines h references r crosses c; feature y : A :> f :>> h ::> r => c;",
        expect![[r#"
            Feature
              "feature"
              Identification "x"
              "typed"
              "by"
              OwnedFeatureTyping
                QualifiedName "A"
              ","
              OwnedFeatureTyping
                QualifiedName "B"
              "subsets"
              OwnedSubsetting
                QualifiedName "f"
              ","
              OwnedSubsetting
                QualifiedName "g"
              "redefines"
              OwnedRedefinition
                QualifiedName "h"
              "references"
              OwnedReferenceSubsetting
                QualifiedName "r"
              "crosses"
              OwnedCrossSubsetting
                QualifiedName "c"
              Body ";"
            Feature
              "feature"
              Identification "y"
              ":"
              OwnedFeatureTyping
                QualifiedName "A"
              ":>"
              OwnedSubsetting
                QualifiedName "f"
              ":>>"
              OwnedRedefinition
                QualifiedName "h"
              "::>"
              OwnedReferenceSubsetting
                QualifiedName "r"
              "=>"
              OwnedCrossSubsetting
                QualifiedName "c"
              Body ";"
        "#]],
    );
}

#[test]
fn feature_specializations_with_chains() {
    check(
        "feature x : a.b :> c.d.e;",
        expect![[r#"
        Feature
          "feature"
          Identification "x"
          ":"
          OwnedFeatureTyping
            FeatureChain
              QualifiedName "a"
              "."
              QualifiedName "b"
          ":>"
          OwnedSubsetting
            FeatureChain
              QualifiedName "c"
              "."
              QualifiedName "d"
              "."
              QualifiedName "e"
          Body ";"
    "#]],
    );
}

#[test]
fn multiplicity_parts() {
    check(
        "feature a[*] nonunique ordered : T; feature b : T[0..1] ordered nonunique; feature c ordered; feature d [2] : T :> e;",
        expect![[r#"
            Feature
              "feature"
              Identification "a"
              MultiplicityBounds
                "["
                LiteralInfinity "*"
                "]"
              "nonunique"
              "ordered"
              ":"
              OwnedFeatureTyping
                QualifiedName "T"
              Body ";"
            Feature
              "feature"
              Identification "b"
              ":"
              OwnedFeatureTyping
                QualifiedName "T"
              MultiplicityBounds
                "["
                LiteralInteger "0"
                ".."
                LiteralInteger "1"
                "]"
              "ordered"
              "nonunique"
              Body ";"
            Feature
              "feature"
              Identification "c"
              "ordered"
              Body ";"
            Feature
              "feature"
              Identification "d"
              MultiplicityBounds
                "["
                LiteralInteger "2"
                "]"
              ":"
              OwnedFeatureTyping
                QualifiedName "T"
              ":>"
              OwnedSubsetting
                QualifiedName "e"
              Body ";"
        "#]],
    );
}

#[test]
fn a_feature_has_one_multiplicity_part() {
    check(
        "feature x [1] : T [2];",
        expect![[r#"
        Feature
          "feature"
          Identification "x"
          MultiplicityBounds
            "["
            LiteralInteger "1"
            "]"
          ":"
          OwnedFeatureTyping
            QualifiedName "T"
          MultiplicityBounds
            "["
            LiteralInteger "2"
            "]"
          Body ";"
        syntax::duplicate-multiplicity at 18..19 "[": a feature has at most one multiplicity part
    "#]],
    );
}

#[test]
fn feature_conjugation() {
    check(
        "feature fuelOutPort ~ fuelInPort;",
        expect![[r#"
        Feature
          "feature"
          Identification "fuelOutPort"
          "~"
          OwnedConjugation
            QualifiedName "fuelInPort"
          Body ";"
    "#]],
    );
}

#[test]
fn feature_relationship_parts() {
    check(
        "feature cousins : Person[*] chains parents.siblings.children featured by Person, Family; feature children inverse of parents disjoint from x; feature p chains q;",
        expect![[r#"
            Feature
              "feature"
              Identification "cousins"
              ":"
              OwnedFeatureTyping
                QualifiedName "Person"
              MultiplicityBounds
                "["
                LiteralInfinity "*"
                "]"
              "chains"
              FeatureChain
                QualifiedName "parents"
                "."
                QualifiedName "siblings"
                "."
                QualifiedName "children"
              "featured"
              "by"
              OwnedTypeFeaturing
                QualifiedName "Person"
              ","
              OwnedTypeFeaturing
                QualifiedName "Family"
              Body ";"
            Feature
              "feature"
              Identification "children"
              "inverse"
              "of"
              OwnedFeatureInverting
                QualifiedName "parents"
              "disjoint"
              "from"
              OwnedDisjoining
                QualifiedName "x"
              Body ";"
            Feature
              "feature"
              Identification "p"
              "chains"
              QualifiedName "q"
              Body ";"
        "#]],
    );
}

#[test]
fn feature_typing_forms() {
    check(
        "specialization t1 typing customer typed by Person; typing c : P.q;",
        expect![[r#"
            FeatureTyping
              "specialization"
              Identification "t1"
              "typing"
              QualifiedName "customer"
              "typed"
              "by"
              QualifiedName "Person"
              Body ";"
            FeatureTyping
              "typing"
              QualifiedName "c"
              ":"
              FeatureChain
                QualifiedName "P"
                "."
                QualifiedName "q"
              Body ";"
        "#]],
    );
}

#[test]
fn subsetting_forms() {
    check(
        "specialization Sub subset parent subsets person; subset a.b :> c;",
        expect![[r#"
            Subsetting
              "specialization"
              Identification "Sub"
              "subset"
              QualifiedName "parent"
              "subsets"
              QualifiedName "person"
              Body ";"
            Subsetting
              "subset"
              FeatureChain
                QualifiedName "a"
                "."
                QualifiedName "b"
              ":>"
              QualifiedName "c"
              Body ";"
        "#]],
    );
}

#[test]
fn redefinition_forms() {
    check(
        "specialization Redef redefinition LegalRecord::guardian redefines parent; redefinition a :>> b.c;",
        expect![[r#"
            Redefinition
              "specialization"
              Identification "Redef"
              "redefinition"
              QualifiedName "LegalRecord" "::" "guardian"
              "redefines"
              QualifiedName "parent"
              Body ";"
            Redefinition
              "redefinition"
              QualifiedName "a"
              ":>>"
              FeatureChain
                QualifiedName "b"
                "."
                QualifiedName "c"
              Body ";"
        "#]],
    );
}

#[test]
fn feature_inverting_forms() {
    check(
        "inverting parent_child inverse Person::parent of Person::child { doc /* d */ } inverse a.b of c.d;",
        expect![[r#"
            FeatureInverting
              "inverting"
              Identification "parent_child"
              "inverse"
              QualifiedName "Person" "::" "parent"
              "of"
              QualifiedName "Person" "::" "child"
              Body
                "{"
                Documentation "doc" "/* d */"
                "}"
            FeatureInverting
              "inverse"
              FeatureChain
                QualifiedName "a"
                "."
                QualifiedName "b"
              "of"
              FeatureChain
                QualifiedName "c"
                "."
                QualifiedName "d"
              Body ";"
        "#]],
    );
}

#[test]
fn type_featuring_forms() {
    check(
        "featuring f by T; featuring <t1> tf of f by T;",
        expect![[r#"
            TypeFeaturing
              "featuring"
              QualifiedName "f"
              "by"
              QualifiedName "T"
              Body ";"
            TypeFeaturing
              "featuring"
              Identification
                ShortName "<" "t1" ">"
                "tf"
              "of"
              QualifiedName "f"
              "by"
              QualifiedName "T"
              Body ";"
        "#]],
    );
}

// KerML 8.2.5.11–8.2.5.13: the Kernel parts that step 1a.3 needs.

#[test]
fn package_forms() {
    check(
        "package P; #M package Q { private import P::*; }",
        expect![[r##"
            Package
              "package"
              Identification "P"
              Body ";"
            Package
              PrefixMetadata
                "#"
                QualifiedName "M"
              "package"
              Identification "Q"
              Body
                "{"
                Import
                  "private"
                  "import"
                  QualifiedName "P"
                  "::"
                  "*"
                  Body ";"
                "}"
        "##]],
    );
}

#[test]
fn library_package_forms() {
    // `standard` is optional (KerML 7.4.14, ADR 0014).
    check(
        "standard library package Base { } library #M package AddressBooks;",
        expect![[r##"
            LibraryPackage
              "standard"
              "library"
              "package"
              Identification "Base"
              Body "{" "}"
            LibraryPackage
              "library"
              PrefixMetadata
                "#"
                QualifiedName "M"
              "package"
              Identification "AddressBooks"
              Body ";"
        "##]],
    );
}

#[test]
fn library_package_metadata_comes_after_library() {
    assert_eq!(codes("#M library package P;"), ["syntax::misplaced-prefix"]);
}

#[test]
fn multiplicity_forms() {
    check(
        "multiplicity m :> n; multiplicity [0..*]; multiplicity <one> exactlyOne [1] { doc /* d */ }",
        expect![[r#"
            MultiplicitySubset
              "multiplicity"
              Identification "m"
              ":>"
              OwnedSubsetting
                QualifiedName "n"
              Body ";"
            MultiplicityRange
              "multiplicity"
              MultiplicityBounds
                "["
                LiteralInteger "0"
                ".."
                LiteralInfinity "*"
                "]"
              Body ";"
            MultiplicityRange
              "multiplicity"
              Identification
                ShortName "<" "one" ">"
                "exactlyOne"
              MultiplicityBounds
                "["
                LiteralInteger "1"
                "]"
              Body
                "{"
                Documentation "doc" "/* d */"
                "}"
        "#]],
    );
}

#[test]
fn multiplicity_bounds_are_literals_or_feature_references() {
    check(
        r#"feature a [true]; feature b [1.5]; feature c [.5..1e3]; feature d [n..m::k]; feature e ["s"];"#,
        expect![[r#"
            Feature
              "feature"
              Identification "a"
              MultiplicityBounds
                "["
                LiteralBoolean "true"
                "]"
              Body ";"
            Feature
              "feature"
              Identification "b"
              MultiplicityBounds
                "["
                LiteralReal "1" "." "5"
                "]"
              Body ";"
            Feature
              "feature"
              Identification "c"
              MultiplicityBounds
                "["
                LiteralReal "." "5"
                ".."
                LiteralReal "1e3"
                "]"
              Body ";"
            Feature
              "feature"
              Identification "d"
              MultiplicityBounds
                "["
                FeatureReferenceExpression
                  QualifiedName "n"
                ".."
                FeatureReferenceExpression
                  QualifiedName "m" "::" "k"
                "]"
              Body ";"
            Feature
              "feature"
              Identification "e"
              MultiplicityBounds
                "["
                LiteralString "\"s\""
                "]"
              Body ";"
        "#]],
    );
}

// Error recovery: one diagnostic each, and the rest of the file parsed (step 1a.3).

#[test]
fn a_missing_semicolon() {
    check(
        "classifier A feature b : B; classifier C;",
        expect![[r#"
            Classifier
              "classifier"
              Identification "A"
            Feature
              "feature"
              Identification "b"
              ":"
              OwnedFeatureTyping
                QualifiedName "B"
              Body ";"
            Classifier
              "classifier"
              Identification "C"
              Body ";"
            syntax::expected-body at 13..20 "feature": expected `;` or `{`
        "#]],
    );
}

#[test]
fn a_missing_closing_brace() {
    check(
        "package P { classifier A;",
        expect![[r#"
        Package
          "package"
          Identification "P"
          Body
            "{"
            Classifier
              "classifier"
              Identification "A"
              Body ";"
        syntax::expected-token at 25..25 "": expected `}`
    "#]],
    );
}

#[test]
fn a_missing_name() {
    check(
        "feature a : ; classifier B :> ; classifier C;",
        expect![[r#"
            Feature
              "feature"
              Identification "a"
              ":"
              Body ";"
            Classifier
              "classifier"
              Identification "B"
              ":>"
              Body ";"
            Classifier
              "classifier"
              Identification "C"
              Body ";"
            syntax::expected-qualified-name at 12..13 ";": expected a qualified name
            syntax::expected-qualified-name at 30..31 ";": expected a qualified name
        "#]],
    );
}

#[test]
fn unexpected_tokens_after_a_declaration() {
    check(
        "feature a : A ) ] ; classifier B;",
        expect![[r#"
        Feature
          "feature"
          Identification "a"
          ":"
          OwnedFeatureTyping
            QualifiedName "A"
          Error ")" "]" ";"
        Classifier
          "classifier"
          Identification "B"
          Body ";"
        syntax::expected-body at 14..15 ")": expected `;` or `{`
    "#]],
    );
}

#[test]
fn a_stray_closing_brace() {
    check(
        "classifier A; } classifier B;",
        expect![[r#"
        Classifier
          "classifier"
          Identification "A"
          Body ";"
        Error "}"
        Classifier
          "classifier"
          Identification "B"
          Body ";"
        syntax::unexpected-token at 14..15 "}": unexpected `}`
    "#]],
    );
}

#[test]
fn no_visibility_aliases_or_imports_in_a_relationship_body() {
    assert_eq!(
        codes("alias A for B { public doc /* d */ private import C::*; alias D for E; }"),
        [
            "syntax::misplaced-visibility", // `public`
            "syntax::misplaced-visibility", // `private`
            "syntax::misplaced-member",     // `import`
            "syntax::misplaced-member"      // `alias`
        ]
    );
}

#[test]
fn garbage_is_reported_and_kept() {
    check(
        "+ - 42 classifier A;",
        expect![[r#"
        Error "+" "-" "42"
        Classifier
          "classifier"
          Identification "A"
          Body ";"
        syntax::expected-member at 0..1 "+": expected a namespace member
    "#]],
    );
}

// The examples of KerML 7.2–7.3 that use only the Root and Core layers (quoted), and the corpus.

#[test]
fn clause_7_examples_parse_without_diagnostics() {
    let examples = [
        // 7.2.3
        "dependency Use\n       from 'Application Layer' to 'Service Layer';\n\
         // 'Service Layer' is the client of this dependency, not its name.\n\
         dependency 'Service Layer'\n       to 'Data Layer', 'External Interface Layer';",
        "dependency 'Service Layer'\n       to 'Data Layer', 'External Interface Layer' {\n\
         /* 'Service Layer' is the client of this dependency,\n  * not its name. */\n}",
        // 7.2.4
        "comment C /* This is a comment about N. */\n/* This is also a comment about N. */",
        "doc X_Comment\n  /* This is a documentation comment about X. */\n\
         doc /* This is more documentation about X. */",
        // 7.2.5
        "namespace N11 {\n  public import N4::A {\n    /* The imported membership is visible outside N11. */\n  }\n\
         private import N5::* {\n    doc /* None of the imported memberships are visible\n * outside of N11. */\n  }\n}",
        "private import N8::*::**;",
        // 7.3.2
        "// This Type has exactly one instance.\ntype Singleton[1] specializes Base::Anything;",
        "type all Car specializes MaterialThing {\n  feature wheels[4] : Wheel;\n}",
        "type Original specializes Base::Anything {\n  in feature Input;\n}\n\
         type Conjugate1 specializes Base::Anything;\ntype Conjugate2 specializes Base::Anything;\n\
         conjugation c1 conjugate Conjugate1 conjugates Original;\n\
         conjugation c2 conjugate Conjugate2 ~ Original {\n  doc /* This conjugation is equivalent to c1. */\n}",
        "type C specializes Anything disjoint from A, B;",
        // 7.3.3
        "classifier FuelInPort {\n  in feature fuelFlow : Fuel;\n}\nclassifier FuelOutPort conjugates FuelInPort;",
        "specialization Super subclassifier A specializes B;\nsubclassifier C specializes A;",
        // 7.3.4
        "classifier Tanks {\n  feature fuelInPort {\n    in feature fuelFlow : Fuel;\n  }\n\
         feature fuelOutPort ~ fuelInPort;\n}",
        "feature cousins : Person[*] chains parents.siblings.children featured by Person;\n\
         feature children : Person[*] featured by Person inverse of parents;",
        "classifier Automobile {\n  composite feature wheels[4] ordered subsets anyWheels;\n\
         // driveWheels must be ordered because wheels is ordered.\n\
         composite feature driveWheels[2] ordered : DriveWheel subsets wheels;\n}",
        "classifier MotorizedVehicle specializes WheeledVehicle {\n  composite feature redefines wheels[2..4];\n}",
        "feature x typed by A, B subsets f redefines g;",
        "specialization Redef redefinition LegalRecord::guardian redefines parent;",
        "inverting parent_child inverse Person::parent of Person::child {\n\
         doc /* A Person is the parent of their children. */\n}",
        "feature urls[*] nonunique : URL;",
    ];
    for text in examples {
        let p = parse(Language::KerML, text);
        assert!(p.diagnostics().is_empty(), "{text}\n{:?}", p.diagnostics());
        assert_eq!(p.syntax().text().to_string(), text);
    }
}

#[test]
fn the_kerml_library_parses_without_diagnostics() {
    // Step 1a.4: all 36 KerML library files parse with zero diagnostics and round-trip byte for
    // byte.
    let corpus = sysml_corpus::Corpus::load().unwrap();
    let mut files = 0;
    for s in corpus.sources() {
        if s.language != sysml_corpus::Language::KerML {
            continue;
        }
        files += 1;
        let p = parse(Language::KerML, &s.text);
        assert_eq!(p.syntax().text().to_string(), s.text, "{}", s.id);
        if let Some(d) = p.diagnostics().first() {
            let line = s.text[..usize::from(d.range.start())].matches('\n').count() + 1;
            panic!(
                "{}:{line}: {} {} ({} diagnostics)",
                s.id,
                d.code,
                d.message,
                p.diagnostics().len()
            );
        }
        // Every library file is one library package.
        let packages: Vec<_> = p
            .syntax()
            .children()
            .filter(|n| n.kind() == K::LibraryPackage)
            .collect();
        assert_eq!(packages.len(), 1, "{}", s.id);
    }
    assert_eq!(files, 36);
}

#[test]
fn token_soup_never_sticks_and_stays_lossless() {
    // Deterministic pseudo-random sequences of keywords, symbols, names, literals and comments:
    // whatever the input, the grammar makes progress on its own (no `syntax::parser-stuck`, which
    // only the framework's safety net reports) and the tree keeps every byte.
    let words: Vec<&str> = sysml_syntax::KERML_RESERVED
        .iter()
        .copied()
        .chain([
            "{", "}", "(", ")", "[", "]", ";", ",", "~", "@", "#", "*", "**", "->", "$", ".", "..",
            ":", "::", ":>", ":>>", "::>", "=>", "<", ">", "=", ":=", "a", "B", "'c d'", "1",
            "2.5", "\"s\"", "/* c */", "// n\n", "?", "??", ".?", "==", "!=", "===", "!==", "<=",
            ">=", "+", "-", "/", "%", "^", "|", "&", "new",
        ])
        .collect();
    let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..2000 {
        let len = (next() % 40) as usize;
        let text: Vec<&str> = (0..len)
            .map(|_| words[(next() % words.len() as u64) as usize])
            .collect();
        let text = text.join(" ");
        let p = parse(Language::KerML, &text);
        assert_eq!(p.syntax().text().to_string(), text);
        assert!(
            p.diagnostics()
                .iter()
                .all(|d| d.code != "syntax::parser-stuck"),
            "{text}"
        );
    }
}

// Recovery cases from the PR #12 review.

/// The kinds of the root's child nodes.
fn top_kinds(text: &str) -> Vec<K> {
    parse(Language::KerML, text)
        .syntax()
        .children()
        .map(|n| n.kind())
        .collect()
}

#[test]
fn a_body_with_only_a_visibility_keeps_its_closing_brace() {
    // The `}` closes P's body; Q is a member of the root, not of P.
    assert_eq!(
        top_kinds("package P { public } package Q { }"),
        [K::Package, K::Package]
    );
    assert_eq!(
        codes("package P { public } package Q { }"),
        ["syntax::expected-member"]
    );
    assert_eq!(
        top_kinds("classifier C { private } classifier D;"),
        [K::Classifier, K::Classifier]
    );
}

#[test]
fn a_missing_closing_bracket_stops_at_the_semicolon() {
    // While typing `feature a[1`: the next features still parse.
    let text = "feature a[1; feature b : T; feature c : U;";
    assert_eq!(top_kinds(text), [K::Feature, K::Feature, K::Feature]);
    assert_eq!(codes(text), ["syntax::expected-token"]);
}

#[test]
fn recovery_stops_at_every_member_keyword() {
    // A missing `;` before a member starting with any keyword that starts a member: the member
    // still parses, so the missing `;` is the only diagnostic.
    for (member, kind) in [
        ("subtype A :> B;", K::Specialization),
        ("subclassifier B specializes C;", K::Subclassification),
        ("typing x : T;", K::FeatureTyping),
        ("subset a :> b;", K::Subsetting),
        ("redefinition a :>> b;", K::Redefinition),
        ("conjugate a ~ b;", K::Conjugation),
        ("disjoint a from b;", K::Disjoining),
        ("inverse a of b;", K::FeatureInverting),
        ("featuring f by T;", K::TypeFeaturing),
        ("locale \"en\" /* c */", K::Comment),
        ("language \"ocl\" /* c */", K::TextualRepresentation),
        ("multiplicity m [1];", K::MultiplicityRange),
    ] {
        let text = format!("type T :> A ) {member}");
        assert_eq!(top_kinds(&text), [K::Type, kind], "{text}");
        assert_eq!(codes(&text), ["syntax::expected-body"], "{text}");
    }
}

#[test]
fn feature_values_with_body_expressions() {
    // A `{` that starts an operand is a body expression (KerML 8.2.5.8.3), part of the value; a
    // `{` after a complete value starts the feature's body.
    let text =
        "classifier C { feature x = if a ? {1} else {2}; feature y = 1 + {} { doc /* d */ } }";
    assert_eq!(codes(text), Vec::<&str>::new());
    let p = parse(Language::KerML, text);
    let body = p.syntax().first_child().unwrap().last_child().unwrap();
    let features: Vec<_> = body.children().filter(|n| n.kind() == K::Feature).collect();
    assert_eq!(features.len(), 2);
    assert!(features[1]
        .children()
        .any(|n| n.kind() == K::Body && n.text().to_string().contains("doc")));
}

#[test]
fn different_errors_at_the_same_token_are_both_reported() {
    // `type T` has neither a specialization nor a body: two problems at the same `}`.
    assert_eq!(
        codes("classifier C { type T }"),
        ["syntax::expected-body", "syntax::expected-specialization"]
    );
}
