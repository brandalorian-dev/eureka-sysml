// SPDX-License-Identifier: Apache-2.0
//! The validation constraints (KerML 8.3; step 1a.10): one test per `validate…` constraint, named
//! after it (`tests/validations_inventory.rs` checks), with a model that satisfies it and, where
//! the textual notation can express one, a model that violates it. A constraint that no text can
//! violate, because the grammar, the lowering or name resolution already ensures it, has its
//! negative test in step 1b.6, with interchange input (`validations.json`).

mod common;

use common::{model_with_library, Model};
use sysml_model::Metaclass;
use sysml_semantics::validate as v;
use sysml_semantics::Global;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// The constraints of `VALIDATIONS` that an element violates.
fn violated(m: &Model, x: Global) -> Vec<&'static str> {
    v::violations(&m.db, m.project, m.file)
        .iter()
        .filter(|v| v.element == x)
        .map(|v| v.constraint)
        .collect()
}

/// The element at `path` in `text`, with the libraries, satisfies `constraint`.
fn satisfies(text: &str, path: &str, constraint: &str) {
    let m = model_with_library(text);
    assert!(
        !violated(&m, m.get(path)).contains(&constraint),
        "{path} in {text}"
    );
}

/// The element at `path` in `text`, with the libraries, violates `constraint`.
fn violates(text: &str, path: &str, constraint: &str) {
    let m = model_with_library(text);
    let found = violated(&m, m.get(path));
    assert!(found.contains(&constraint), "{path} in {text}: {found:?}");
}

fn relationship_violated(text: &str, path: &str, kind: Metaclass, n: usize) -> Vec<&'static str> {
    let m = model_with_library(text);
    let r = m.relationship(path, kind, n);
    violated(&m, r)
}

/// The n-th relationship of a metaclass at `path` satisfies `constraint`.
fn relationship_satisfies(text: &str, path: &str, kind: Metaclass, n: usize, constraint: &str) {
    let found = relationship_violated(text, path, kind, n);
    assert!(
        !found.contains(&constraint),
        "{kind:?} {n} of {path}: {found:?}"
    );
}

/// The n-th relationship of a metaclass at `path` violates `constraint`.
fn relationship_violates(text: &str, path: &str, kind: Metaclass, n: usize, constraint: &str) {
    let found = relationship_violated(text, path, kind, n);
    assert!(
        found.contains(&constraint),
        "{kind:?} {n} of {path}: {found:?}"
    );
}

// ---- Root (KerML 8.3.2) ----

#[test]
fn validate_element_is_implied_included() {
    // ownedRelationship->exists(isImplied) implies isImpliedIncluded: the view includes every
    // implied relationship (ADR 0022)
    satisfies("classifier C;", "C", "validateElementIsImpliedIncluded");
}

#[test]
fn validate_annotation_annotating_element() {
    // an annotation owns its annotating element, or is owned by it
    let text = "classifier C; comment about C /* x */ classifier D { doc /* d */ }";
    relationship_satisfies(
        text,
        "Comment#1",
        Metaclass::Annotation,
        1,
        "validateAnnotationAnnotatingElement",
    );
    let m = model_with_library(text);
    for x in v::violations(&m.db, m.project, m.file) {
        assert_ne!(x.constraint, "validateAnnotationAnnotatingElement");
    }
}

#[test]
fn validate_annotation_annotated_element_ownership() {
    // owned by the annotated element exactly when it owns the annotating element
    let m = model_with_library(
        "classifier C; comment about C /* x */ classifier D { doc /* d */ } metaclass M; #M classifier E;",
    );
    for x in v::violations(&m.db, m.project, m.file) {
        assert_ne!(x.constraint, "validateAnnotationAnnotatedElementOwnership");
    }
}

#[test]
fn validate_import_top_level_visibility() {
    // an import of a root namespace is private
    relationship_satisfies(
        "package P; import P::*; package Q { public import P::*; }",
        "",
        Metaclass::NamespaceImport,
        1,
        "validateImportTopLevelVisibility",
    );
    relationship_satisfies(
        "package P; package Q { public import P::*; }",
        "Q",
        Metaclass::NamespaceImport,
        1,
        "validateImportTopLevelVisibility",
    );
    relationship_violates(
        "package P; public import P::*;",
        "",
        Metaclass::NamespaceImport,
        1,
        "validateImportTopLevelVisibility",
    );
}

#[test]
fn validate_namespace_distinguishibility() {
    // every two memberships are distinguishable
    let m = model_with_library("classifier A; classifier B;");
    assert!(!violated(&m, m.root()).contains(&"validateNamespaceDistinguishibility"));
    let m = model_with_library("classifier A; classifier A;");
    assert!(violated(&m, m.root()).contains(&"validateNamespaceDistinguishibility"));
    // two inherited features that redefine a common one are one member (ADR 0023); two that do
    // not are not
    satisfies(
        "classifier V { feature d; } classifier A specializes V { feature redefines d; }
        classifier B specializes V { feature redefines d; } classifier C specializes A, B;",
        "C",
        "validateNamespaceDistinguishibility",
    );
    violates(
        "classifier A { feature d; } classifier B { feature d; } classifier C specializes A, B;",
        "C",
        "validateNamespaceDistinguishibility",
    );
}

// ---- Core: types (KerML 8.3.3.1) ----

#[test]
fn validate_type_at_most_one_conjugator() {
    // at most one owned Conjugation; the grammar allows one
    satisfies(
        "type A; type C ~ A;",
        "C",
        "validateTypeAtMostOneConjugator",
    );
}

#[test]
fn validate_type_owned_multiplicity() {
    // at most one owned Multiplicity
    satisfies("type T [1];", "T", "validateTypeOwnedMultiplicity");
    violates(
        "type T { multiplicity m [1]; multiplicity n [2]; }",
        "T",
        "validateTypeOwnedMultiplicity",
    );
}

#[test]
fn validate_type_owned_unioning_not_one() {
    satisfies(
        "type A; type B; type U unions A, B;",
        "U",
        "validateTypeOwnedUnioningNotOne",
    );
    violates(
        "type A; type U unions A;",
        "U",
        "validateTypeOwnedUnioningNotOne",
    );
}

#[test]
fn validate_type_owned_intersecting_not_one() {
    satisfies(
        "type A; type B; type I intersects A, B;",
        "I",
        "validateTypeOwnedIntersectingNotOne",
    );
    violates(
        "type A; type I intersects A;",
        "I",
        "validateTypeOwnedIntersectingNotOne",
    );
}

#[test]
fn validate_type_owned_differencing_not_one() {
    satisfies(
        "type A; type B; type D differences A, B;",
        "D",
        "validateTypeOwnedDifferencingNotOne",
    );
    violates(
        "type A; type D differences A;",
        "D",
        "validateTypeOwnedDifferencingNotOne",
    );
}

#[test]
fn validate_type_unioning_types_not_self() {
    satisfies(
        "type A; type B; type U unions A, B;",
        "U",
        "validateTypeUnioningTypesNotSelf",
    );
    violates(
        "type A; type U unions U, A;",
        "U",
        "validateTypeUnioningTypesNotSelf",
    );
}

#[test]
fn validate_type_intersecting_types_not_self() {
    satisfies(
        "type A; type B; type I intersects A, B;",
        "I",
        "validateTypeIntersectingTypesNotSelf",
    );
    violates(
        "type A; type I intersects I, A;",
        "I",
        "validateTypeIntersectingTypesNotSelf",
    );
}

#[test]
fn validate_type_differencing_types_not_self() {
    satisfies(
        "type A; type B; type D differences A, B;",
        "D",
        "validateTypeDifferencingTypesNotSelf",
    );
    violates(
        "type A; type D differences D, A;",
        "D",
        "validateTypeDifferencingTypesNotSelf",
    );
}

#[test]
fn validate_specification_specific_not_conjugated() {
    // the specific type is not conjugated; the grammar writes no specialization of a conjugated
    // type
    relationship_satisfies(
        "type A; type B specializes A;",
        "B",
        Metaclass::Specialization,
        1,
        "validateSpecificationSpecificNotConjugated",
    );
}

// ---- Core: classifiers (KerML 8.3.3.2) ----

#[test]
fn validate_classifier_multiplicity_domain() {
    // a classifier's multiplicity has no featuring types
    satisfies(
        "classifier C [1];",
        "C",
        "validateClassifierMultiplicityDomain",
    );
}

// ---- Core: features (KerML 8.3.3.3) ----

#[test]
fn validate_feature_multiplicity_domain() {
    // a feature's multiplicity has its featuring types, which the type featurings copy
    satisfies(
        "classifier C { feature f[1]; }",
        "C::f",
        "validateFeatureMultiplicityDomain",
    );
}

#[test]
fn validate_feature_is_variable() {
    // a variable feature is owned by an Occurrence
    satisfies(
        "class K { var feature v; }",
        "K::v",
        "validateFeatureIsVariable",
    );
    violates("var feature v;", "v", "validateFeatureIsVariable");
    violates(
        "classifier C { var feature v; }",
        "C::v",
        "validateFeatureIsVariable",
    );
}

#[test]
fn validate_feature_constant_is_variable() {
    // `const` is variable too in the grammar
    satisfies(
        "class K { const feature c; }",
        "K::c",
        "validateFeatureConstantIsVariable",
    );
}

#[test]
fn validate_feature_portion_not_variable() {
    satisfies(
        "class C { portion feature p : C; }",
        "C::p",
        "validateFeaturePortionNotVariable",
    );
    violates(
        "class C { portion var feature p : C; }",
        "C::p",
        "validateFeaturePortionNotVariable",
    );
}

#[test]
fn validate_feature_end_no_direction() {
    // the grammar gives an end no direction
    satisfies(
        "assoc A { end feature a; end feature b; }",
        "A::a",
        "validateFeatureEndNoDirection",
    );
}

#[test]
fn validate_feature_end_is_constant() {
    // the grammar makes a variable end constant
    satisfies(
        "assoc A { end feature a; end feature b; }",
        "A::a",
        "validateFeatureEndIsConstant",
    );
}

#[test]
fn validate_feature_end_not_derived_abstract_composite_or_portion() {
    // the grammar gives an end none of these
    satisfies(
        "assoc A { end feature a; end feature b; }",
        "A::a",
        "validateFeatureEndNotDerivedAbstractCompositeOrPortion",
    );
}

#[test]
fn validate_feature_end_multiplicity() {
    // an end has multiplicity 1..1: a binary association's from Links::BinaryLink, an N-ary one's
    // as written (KerML 8.4.4.5.1)
    satisfies(
        "assoc A { end feature a; end feature b; }",
        "A::a",
        "validateFeatureEndMultiplicity",
    );
    satisfies(
        "assoc A { end feature a[1]; end feature b[1]; end feature c[1]; }",
        "A::a",
        "validateFeatureEndMultiplicity",
    );
    violates(
        "assoc A { end feature a; end feature b; end feature c; }",
        "A::a",
        "validateFeatureEndMultiplicity",
    );
}

#[test]
fn validate_feature_chaining_feature_not_one() {
    // the grammar writes two chaining features or more
    satisfies(
        "classifier C { feature a; } feature c : C; feature x chains c.a;",
        "x",
        "validateFeatureChainingFeatureNotOne",
    );
}

#[test]
fn validate_feature_chaining_features_not_self() {
    satisfies(
        "classifier C { feature a; } feature c : C; feature x chains c.a;",
        "x",
        "validateFeatureChainingFeaturesNotSelf",
    );
    violates(
        "classifier C { feature a; } feature x : C chains x.a;",
        "x",
        "validateFeatureChainingFeaturesNotSelf",
    );
}

#[test]
fn validate_feature_chaining_feature_conformance() {
    // each chaining feature is featured within the one before; name resolution finds each in
    // the one before
    satisfies(
        "classifier C { feature a; } feature c : C; feature x chains c.a;",
        "x",
        "validateFeatureChainingFeatureConformance",
    );
}

#[test]
fn validate_feature_owned_reference_subsetting() {
    satisfies(
        "feature a; feature f ::> a;",
        "f",
        "validateFeatureOwnedReferenceSubsetting",
    );
    violates(
        "feature a; feature b; feature f ::> a ::> b;",
        "f",
        "validateFeatureOwnedReferenceSubsetting",
    );
}

const CROSSES: &str = "classifier T; classifier U; classifier W;
    feature c : U { feature x : W; feature y : W; }";

#[test]
fn validate_feature_owned_cross_subsetting() {
    let one = format!("{CROSSES} assoc A {{ end feature e : T crosses c.x; end feature f : U; }}");
    satisfies(&one, "A::e", "validateFeatureOwnedCrossSubsetting");
    let two = format!(
        "{CROSSES} assoc A {{ end feature e : T crosses c.x crosses c.y; end feature f : U; }}"
    );
    violates(&two, "A::e", "validateFeatureOwnedCrossSubsetting");
}

#[test]
fn validate_feature_cross_feature_type() {
    // a cross feature has the feature's types: an owned one by implication
    satisfies(
        "classifier T; classifier U; assoc A { end [1] feature a : T; end feature b : U; }",
        "A::a",
        "validateFeatureCrossFeatureType",
    );
    let text = format!("{CROSSES} assoc A {{ end feature e : T crosses c.x; end feature f : U; }}");
    violates(&text, "A::e", "validateFeatureCrossFeatureType");
}

#[test]
fn validate_feature_cross_feature_specialization() {
    // a cross feature specializes those of the features its feature redefines
    let text = "classifier T; classifier U;
        assoc A { end [1] feature a : T; end feature b : U; }
        assoc B specializes A { end [0..1] feature a2 : T; end feature b2 : U; }";
    satisfies(text, "B::a2", "validateFeatureCrossFeatureSpecialization");
    let text = format!(
        "{CROSSES} assoc A {{ end [1] feature a : T; end feature b : U; }}
        assoc B specializes A {{ end feature a2 : T crosses c.x; end feature b2 : U; }}"
    );
    violates(&text, "B::a2", "validateFeatureCrossFeatureSpecialization");
}

#[test]
fn validate_end_feature_membership_is_end() {
    // the lowering makes the feature of an EndFeatureMembership an end
    relationship_satisfies(
        "classifier C { feature x; feature y; connector k from x to y; }",
        "C::k",
        Metaclass::EndFeatureMembership,
        1,
        "validateEndFeatureMembershipIsEnd",
    );
}

#[test]
fn validate_subsetting_constant_conformance() {
    let kind = Metaclass::Subsetting;
    let c = "validateSubsettingConstantConformance";
    relationship_satisfies(
        "class C { const feature a; const feature b subsets a; }",
        "C::b",
        kind,
        1,
        c,
    );
    relationship_violates(
        "class C { const feature a; var feature b subsets a; }",
        "C::b",
        kind,
        1,
        c,
    );
}

#[test]
fn validate_subsetting_uniqueness_conformance() {
    let kind = Metaclass::Subsetting;
    let c = "validateSubsettingUniquenessConformance";
    relationship_satisfies("feature a[*]; feature b[*] subsets a;", "b", kind, 1, c);
    relationship_violates(
        "feature a[*]; feature b[*] nonunique subsets a;",
        "b",
        kind,
        1,
        c,
    );
}

#[test]
fn validate_subsetting_featuring_types() {
    // the subsetting feature can access the subsetted one
    let kind = Metaclass::Subsetting;
    let c = "validateSubsettingFeaturingTypes";
    relationship_satisfies(
        "classifier C { feature a; } classifier D specializes C { feature b subsets a; }",
        "D::b",
        kind,
        1,
        c,
    );
    relationship_satisfies("feature a; feature b subsets a;", "b", kind, 1, c);
    relationship_violates(
        "classifier C { feature a; } classifier D { feature b subsets C::a; }",
        "D::b",
        kind,
        1,
        c,
    );
}

#[test]
fn validate_redefinition_end_conformance() {
    let kind = Metaclass::Redefinition;
    let c = "validateRedefinitionEndConformance";
    let base = "assoc A { end feature a; end feature b; }";
    relationship_satisfies(
        &format!("{base} assoc B specializes A {{ end feature c redefines a; end feature d; }}"),
        "B::c",
        kind,
        1,
        c,
    );
    relationship_violates(
        &format!(
            "{base} assoc B specializes A {{ feature c redefines a; end feature d; end feature e; }}"
        ),
        "B::c",
        kind,
        1,
        c);
}

#[test]
fn validate_redefinition_featuring_types() {
    // a redefining feature has other featuring types than the redefined one
    let kind = Metaclass::Redefinition;
    let c = "validateRedefinitionFeaturingTypes";
    relationship_satisfies(
        "classifier C { feature a; } classifier D specializes C { feature b redefines a; }",
        "D::b",
        kind,
        1,
        c,
    );
    relationship_violates(
        // the name is resolved in C's generals first (KerML 8.2.3.5.1), so it is qualified
        "classifier C { feature a; feature b redefines C::a; }",
        "C::b",
        kind,
        1,
        c,
    );
}

#[test]
fn validate_redefinition_direction_conformance() {
    let kind = Metaclass::Redefinition;
    let c = "validateRedefinitionDirectionConformance";
    let base = "behavior B { in x; }";
    relationship_satisfies(
        &format!("{base} behavior B2 specializes B {{ in y redefines x; }}"),
        "B2::y",
        kind,
        1,
        c,
    );
    relationship_violates(
        &format!("{base} behavior B2 specializes B {{ out y redefines x; }}"),
        "B2::y",
        kind,
        1,
        c,
    );
}

#[test]
fn validate_cross_subsetting_crossing_feature() {
    // the crossing feature is an end of a type with more than one end
    let kind = Metaclass::CrossSubsetting;
    let c = "validateCrossSubsettingCrossingFeature";
    relationship_satisfies(
        &format!("{CROSSES} assoc A {{ end feature e : T crosses c.x; end feature f : U; }}"),
        "A::e",
        kind,
        1,
        c,
    );
    relationship_violates(
        &format!("{CROSSES} feature g crosses c.x;"),
        "g",
        kind,
        1,
        c,
    );
}

#[test]
fn validate_cross_subsetting_crossed_feature() {
    // for a binary type, the crossed feature chains through the other end: the implied one does
    // (ADR 0023); one through another feature does not
    let m = model_with_library(
        "classifier T; classifier U; assoc A { end [1] feature a : T; end feature b : U; }",
    );
    for x in v::violations(&m.db, m.project, m.file) {
        assert_ne!(x.constraint, "validateCrossSubsettingCrossedFeature");
    }
    relationship_violates(
        &format!("{CROSSES} assoc A {{ end feature e : T crosses c.x; end feature f : U; }}"),
        "A::e",
        Metaclass::CrossSubsetting,
        1,
        "validateCrossSubsettingCrossedFeature",
    );
    // the crossed feature is a chain of two features (the text of KerML 8.3.3.3.2; ADR 0023)
    relationship_violates(
        &format!("{CROSSES} assoc A {{ end feature e : T crosses c; end feature f : U; }}"),
        "A::e",
        Metaclass::CrossSubsetting,
        1,
        "validateCrossSubsettingCrossedFeature",
    );
}

// ---- diagnostics ----

#[test]
fn a_violation_is_an_error_with_its_constraint_clause_and_range() {
    // ADR 0012: the constraint's name is the code, the clause is its metaclass's, and the range
    // is the element's
    let text = "package P; public import P::*;";
    let m = model_with_library(text);
    let diagnostics = v::diagnostics(&m.db, m.project, m.file);
    let d = diagnostics
        .iter()
        .find(|d| d.code == "validateImportTopLevelVisibility")
        .unwrap();
    assert_eq!(d.severity, sysml_syntax::Severity::Error);
    assert_eq!(d.clause, Some("KerML 8.3.2.4.2"));
    let range = usize::from(d.range.start())..usize::from(d.range.end());
    assert_eq!(&text[range], "public import P::*;");
}

#[test]
fn indistinguishable_members_are_reported_at_the_later_one() {
    // not at the namespace, which for a root namespace is the whole file
    let text = "classifier A; classifier A;";
    let m = model_with_library(text);
    let diagnostics = v::diagnostics(&m.db, m.project, m.file);
    let d = diagnostics
        .iter()
        .find(|d| d.code == "validateNamespaceDistinguishibility")
        .unwrap();
    let range = usize::from(d.range.start())..usize::from(d.range.end());
    assert_eq!(range.start, "classifier A; ".len());
    assert_eq!(&text[range], "classifier A;");
}

#[test]
fn the_kerml_libraries_produce_zero_diagnostics() {
    // the criterion of step 1a.10, implied elements included
    let m = model_with_library("");
    let mut found = Vec::new();
    for f in m.project.files(&m.db) {
        for d in v::diagnostics(&m.db, m.project, *f) {
            found.push(format!("{} {}: {}", f.path(&m.db), d.code, d.message));
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}
