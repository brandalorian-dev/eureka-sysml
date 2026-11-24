// SPDX-License-Identifier: Apache-2.0
//! The semantic constraints (KerML 8.4.2; step 1a.9): one golden test per `check…` constraint
//! of Tables 8–11 (KerML 8.4.3.1, 8.4.4.1), named after it (`tests/checks_inventory.rs` checks).
//!
//! Each test takes a model to which the constraint applies and shows that the implied
//! relationship of its table row is in the overlay (ADR 0022) and that the check then holds.
//! Where the constraint names a library type, the check fails on the same model without the
//! libraries, so it is not satisfied by accident.

mod common;

use common::{model, model_with_library, Model};
use sysml_model::{Attr, Metaclass};
use sysml_semantics::checks::{self as c, Check};
use sysml_semantics::derived as d;
use sysml_semantics::Global;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// An element as its qualified name, or its `path()`.
fn name(m: &Model, x: Global) -> String {
    d::qualified_name(&m.db, m.project, x)
        .as_deref()
        .map(str::to_owned)
        .unwrap_or_else(|| d::path(&m.db, m.project, x).to_string())
}

/// The implied relationships an element owns, as `Kind target`.
fn implied(m: &Model, x: Global) -> Vec<String> {
    let v = sysml_semantics::view(&m.db, m.project, x.file);
    v.all_owned_relationships(x.ix())
        .filter(|r| v.flag(*r, Attr::IsImplied))
        .map(|r| {
            let rel = Global {
                file: x.file,
                index: r.index() as u32,
            };
            let target = d::target(&m.db, m.project, rel);
            let t = target.first().map(|t| name(m, *t)).unwrap_or_default();
            format!("{} {t}", v.metaclass(r).name())
        })
        .collect()
}

fn holds(m: &Model, check: Check, x: Global) -> bool {
    check(&m.db, m.project, x)
}

/// The golden test of a constraint whose target is a library type: in `text`, the element at
/// `path` has the implied relationship `expected` and satisfies the check, which it violates
/// without the libraries.
fn golden(text: &str, path: &str, check: Check, expected: &str) {
    let m = model_with_library(text);
    let x = m.get(path);
    let i = implied(&m, x);
    assert!(i.iter().any(|s| s == expected), "{path}: {i:?}");
    assert!(holds(&m, check, x), "{path} violates the check");
    let bare = model(text);
    assert!(
        !holds(&bare, check, bare.get(path)),
        "{path} satisfies the check without the libraries"
    );
}

/// A check that does not apply holds, with or without the libraries.
fn not_applicable(text: &str, path: &str, check: Check) {
    let m = model(text);
    assert!(holds(&m, check, m.get(path)), "{path}");
    let m = model_with_library(text);
    assert!(holds(&m, check, m.get(path)), "{path}");
}

// ---- Table 8: Core (KerML 8.4.3.1) ----

#[test]
fn check_type_specialization() {
    // specializesFromLibrary('Base::Anything'): implied for a Classifier
    golden(
        "classifier C;",
        "C",
        c::check_type_specialization,
        "Subclassification Base::Anything",
    );
    // and satisfied by a Feature through Base::things (Table 8 Note 2)
    let m = model_with_library("feature f;");
    assert!(holds(&m, c::check_type_specialization, m.get("f")));
    // Table 8 Note 1: the constraint applies to all Types, but nothing is implied for a Type that
    // is neither a Classifier nor a Feature
    let m = model_with_library("type T;");
    assert!(implied(&m, m.get("T")).is_empty());
    assert!(!holds(&m, c::check_type_specialization, m.get("T")));
}

#[test]
fn check_feature_specialization() {
    // specializesFromLibrary('Base::things')
    golden(
        "feature f;",
        "f",
        c::check_feature_specialization,
        "Subsetting Base::things",
    );
}

// ---- Table 9: Core supporting Kernel (KerML 8.4.3.1) ----

#[test]
fn check_feature_data_value_specialization() {
    // ownedTyping.type->exists(DataType) implies specializesFromLibrary('Base::dataValues')
    golden(
        "datatype D; feature f : D;",
        "f",
        c::check_feature_data_value_specialization,
        "Subsetting Base::dataValues",
    );
    not_applicable(
        "feature g;",
        "g",
        c::check_feature_data_value_specialization,
    );
}

#[test]
fn check_feature_occurrence_specialization() {
    // ownedTyping.type->exists(Class) implies specializesFromLibrary('Occurrences::occurrences')
    golden(
        "class C; feature f : C;",
        "f",
        c::check_feature_occurrence_specialization,
        "Subsetting Occurrences::occurrences",
    );
    not_applicable(
        "feature g;",
        "g",
        c::check_feature_occurrence_specialization,
    );
}

#[test]
fn check_feature_object_specialization() {
    // ownedTyping.type->exists(Structure) implies specializesFromLibrary('Objects::objects')
    golden(
        "struct S; feature f : S;",
        "f",
        c::check_feature_object_specialization,
        "Subsetting Objects::objects",
    );
    // KerML 8.4.2: objects subsets occurrences and things, which are left out
    let m = model_with_library("struct S; feature f : S;");
    assert_eq!(implied(&m, m.get("f")), ["Subsetting Objects::objects"]);
}

#[test]
fn check_feature_suboccurrence_specialization() {
    // isComposite, typed by a Class, owned by a Class implies
    // specializesFromLibrary('Occurrences::Occurrence::suboccurrences')
    golden(
        "class C { composite feature f : C; }",
        "C::f",
        c::check_feature_suboccurrence_specialization,
        "Subsetting Occurrences::Occurrence::suboccurrences",
    );
    not_applicable(
        "class C { feature f : C; }",
        "C::f",
        c::check_feature_suboccurrence_specialization,
    );
}

#[test]
fn check_feature_subobject_specialization() {
    // isComposite, typed by a Structure, owned by a Structure implies
    // specializesFromLibrary('Objects::Object::subobjects') (Table 9; ADR 0020)
    golden(
        "struct S { composite feature f : S; }",
        "S::f",
        c::check_feature_subobject_specialization,
        "Subsetting Objects::Object::subobjects",
    );
    // owned by a Feature typed by a Structure
    golden(
        "struct S; feature s : S { composite feature f : S; }",
        "s::f",
        c::check_feature_subobject_specialization,
        "Subsetting Objects::Object::subobjects",
    );
}

#[test]
fn check_feature_portion_specialization() {
    // isPortion, typed by a Class, owned by a Class implies
    // specializesFromLibrary('Occurrences::Occurrence::portions')
    golden(
        "class C { portion feature p : C; }",
        "C::p",
        c::check_feature_portion_specialization,
        "Subsetting Occurrences::Occurrence::portions",
    );
}

#[test]
fn check_feature_end_specialization() {
    // isEnd, owned by an Association or a Connector implies
    // specializesFromLibrary('Links::Link::participant')
    golden(
        "assoc A { end feature a; end feature b; end feature c; }",
        "A::a",
        c::check_feature_end_specialization,
        "Subsetting Links::Link::participant",
    );
    not_applicable(
        "classifier C { end feature a; }",
        "C::a",
        c::check_feature_end_specialization,
    );
}

const CROSSING: &str = "classifier T; classifier U;
    assoc A { end [1] feature a : T; end feature b : U; }";

#[test]
fn check_feature_crossing_specialization() {
    // ownedCrossFeature() <> null implies crossFeature = ownedCrossFeature(): an implied
    // CrossSubsetting of a chain whose first feature is typed by the other end's types and
    // featured by the association, and whose second is the owned cross feature (Table 9 Note 2).
    // For a binary association that first feature is the other end itself, as
    // validateCrossSubsettingCrossedFeature requires (ADR 0023).
    let m = model_with_library(CROSSING);
    let (db, p) = (&m.db, m.project);
    let a = m.get("A::a");
    let cross = d::owned_cross_feature(db, p, a).unwrap();
    assert!(holds(&m, c::check_feature_crossing_specialization, a));
    assert_eq!(d::cross_feature(db, p, a), Some(cross));
    let subsetting = d::owned_cross_subsetting(db, p, a).unwrap();
    assert!(sysml_semantics::view(db, p, a.file).flag(subsetting.ix(), Attr::IsImplied));
    let chain = d::target(db, p, subsetting)[0];
    let chaining = d::chaining_feature(db, p, chain);
    assert_eq!(chaining.len(), 2);
    assert_eq!(chaining[0], m.get("A::b"));
    assert_eq!(chaining[1], cross);
    assert_eq!(m.labels(d::r#type(db, p, chaining[0])), ["U"]);
    assert_eq!(m.labels(d::featuring_type(db, p, chaining[0])), ["A"]);
    // the implied first feature satisfies the feature constraints itself
    for check in [
        c::check_feature_specialization as Check,
        c::check_feature_occurrence_specialization,
    ] {
        assert!(holds(&m, check, chaining[0]));
    }
    // an end without an owned cross feature
    not_applicable(CROSSING, "A::b", c::check_feature_crossing_specialization);
}

#[test]
fn check_feature_owned_cross_feature_specialization() {
    // isOwnedCrossFeature() implies owner.type->forAll(t | specializes(t))
    let m = model_with_library(CROSSING);
    let (db, p) = (&m.db, m.project);
    let cross = d::owned_cross_feature(db, p, m.get("A::a")).unwrap();
    assert!(implied(&m, cross).contains(&"FeatureTyping T".to_owned()));
    assert!(holds(
        &m,
        c::check_feature_owned_cross_feature_specialization,
        cross
    ));
    not_applicable(
        CROSSING,
        "A::b",
        c::check_feature_owned_cross_feature_specialization,
    );
}

#[test]
fn check_feature_owned_cross_feature_redefinition_specialization() {
    // isOwnedCrossFeature() implies ownedSubsetting.subsettedFeature includes the crossFeatures
    // of the features the end redefines: here A::a, which B::a2 redefines by position
    let text = "classifier T; classifier U;
        assoc A { end [1] feature a : T; end feature b : U; }
        assoc B specializes A { end [0..1] feature a2 : T; end feature b2 : U; }";
    let m = model_with_library(text);
    let (db, p) = (&m.db, m.project);
    let cross_a = d::owned_cross_feature(db, p, m.get("A::a")).unwrap();
    let cross_a2 = d::owned_cross_feature(db, p, m.get("B::a2")).unwrap();
    let subsetted: Vec<Global> = d::owned_subsetting(db, p, cross_a2)
        .into_iter()
        .flat_map(|r| d::target(db, p, r))
        .collect();
    assert!(subsetted.contains(&cross_a), "{:?}", implied(&m, cross_a2));
    assert!(holds(
        &m,
        c::check_feature_owned_cross_feature_redefinition_specialization,
        cross_a2
    ));
}

#[test]
fn check_feature_valuation_specialization() {
    // direction = null and no written specialization implies specializes(fv.value.result)
    let text = "feature a; feature b = a; feature c : Base::Anything = a; in feature d = a;";
    let m = model_with_library(text);
    let (db, p) = (&m.db, m.project);
    let b = m.get("b");
    let value = m.get("b::FeatureReferenceExpression#1");
    let result = d::result(db, p, value).unwrap();
    assert!(implied(&m, b).contains(&format!("Subsetting {}", name(&m, result))));
    assert!(holds(&m, c::check_feature_valuation_specialization, b));
    // with a written specialization or a direction, the constraint does not apply
    not_applicable(text, "c", c::check_feature_valuation_specialization);
    not_applicable(text, "d", c::check_feature_valuation_specialization);
}

#[test]
fn check_multiplicity_specialization() {
    // specializesFromLibrary('Base::naturals')
    golden(
        "feature f[1];",
        "f::MultiplicityRange#1",
        c::check_multiplicity_specialization,
        "Subsetting Base::naturals",
    );
}

// ---- Table 10: Kernel classifiers and features (KerML 8.4.4.1) ----

#[test]
fn check_data_type_specialization() {
    golden(
        "datatype D;",
        "D",
        c::check_data_type_specialization,
        "Subclassification Base::DataValue",
    );
}

#[test]
fn check_class_specialization() {
    golden(
        "class C;",
        "C",
        c::check_class_specialization,
        "Subclassification Occurrences::Occurrence",
    );
}

#[test]
fn check_structure_specialization() {
    golden(
        "struct S;",
        "S",
        c::check_structure_specialization,
        "Subclassification Objects::Object",
    );
    // KerML 8.4.2: Object specializes Occurrence and Anything, which are left out
    let m = model_with_library("struct S;");
    assert_eq!(
        implied(&m, m.get("S")),
        ["Subclassification Objects::Object"]
    );
}

const NARY: &str = "assoc A { end feature a; end feature b; end feature c; }";
const BINARY: &str = "assoc A { end feature a; end feature b; }";

#[test]
fn check_association_specialization() {
    golden(
        NARY,
        "A",
        c::check_association_specialization,
        "Subclassification Links::Link",
    );
}

#[test]
fn check_association_binary_specialization() {
    // associationEnd->size() = 2 implies specializesFromLibrary('Links::BinaryLink')
    golden(
        BINARY,
        "A",
        c::check_association_binary_specialization,
        "Subclassification Links::BinaryLink",
    );
    not_applicable(NARY, "A", c::check_association_binary_specialization);
}

#[test]
fn check_association_structure_specialization() {
    golden(
        "assoc struct A { end feature a; end feature b; end feature c; }",
        "A",
        c::check_association_structure_specialization,
        "Subclassification Objects::LinkObject",
    );
}

#[test]
fn check_association_structure_binary_specialization() {
    // endFeature->size() = 2 implies specializesFromLibrary('Objects::BinaryLinkObject')
    golden(
        "assoc struct A { end feature a; end feature b; }",
        "A",
        c::check_association_structure_binary_specialization,
        "Subclassification Objects::BinaryLinkObject",
    );
}

const CONNECTED: &str = "classifier C { feature x; feature y; feature z;
    connector n (x, y, z); connector k from x to y; binding g of x = y; }";

#[test]
fn check_connector_specialization() {
    golden(
        CONNECTED,
        "C::n",
        c::check_connector_specialization,
        "Subsetting Links::links",
    );
}

#[test]
fn check_connector_binary_specialization() {
    // connectorEnd->size() = 2 implies specializesFromLibrary('Links::binaryLinks')
    golden(
        CONNECTED,
        "C::k",
        c::check_connector_binary_specialization,
        "Subsetting Links::binaryLinks",
    );
    not_applicable(CONNECTED, "C::n", c::check_connector_binary_specialization);
}

const OBJECTS: &str = "assoc struct AS { end feature a; end feature b; end feature c; }
    assoc struct BS { end feature a; end feature b; }
    classifier C { feature x; feature y; feature z;
        connector n : AS (x, y, z); connector k : BS from x to y; }";

#[test]
fn check_connector_object_specialization() {
    // association->exists(AssociationStructure) implies
    // specializesFromLibrary('Objects::linkObjects')
    golden(
        OBJECTS,
        "C::n",
        c::check_connector_object_specialization,
        "Subsetting Objects::linkObjects",
    );
    not_applicable(CONNECTED, "C::n", c::check_connector_object_specialization);
}

#[test]
fn check_connector_binary_object_specialization() {
    // two ends and an AssociationStructure imply
    // specializesFromLibrary('Objects::binaryLinkObjects')
    golden(
        OBJECTS,
        "C::k",
        c::check_connector_binary_object_specialization,
        "Subsetting Objects::binaryLinkObjects",
    );
    not_applicable(
        OBJECTS,
        "C::n",
        c::check_connector_binary_object_specialization,
    );
}

#[test]
fn check_binding_connector_specialization() {
    golden(
        CONNECTED,
        "C::g",
        c::check_binding_connector_specialization,
        "Subsetting Links::selfLinks",
    );
}

#[test]
fn check_succession_specialization() {
    golden(
        "behavior B { step s1; step s2; succession t first s1 then s2; }",
        "B::t",
        c::check_succession_specialization,
        "Subsetting Occurrences::happensBeforeLinks",
    );
}

#[test]
fn check_behavior_specialization() {
    golden(
        "behavior B;",
        "B",
        c::check_behavior_specialization,
        "Subclassification Performances::Performance",
    );
}

#[test]
fn check_step_specialization() {
    golden(
        "step s;",
        "s",
        c::check_step_specialization,
        "Subsetting Performances::performances",
    );
}

#[test]
fn check_step_enclosed_performance_specialization() {
    // owned by a Behavior or a Step implies
    // specializesFromLibrary('Performances::Performance::enclosedPerformances')
    golden(
        "behavior B { step s; }",
        "B::s",
        c::check_step_enclosed_performance_specialization,
        "Subsetting Performances::Performance::enclosedPerformances",
    );
    not_applicable(
        "step s;",
        "s",
        c::check_step_enclosed_performance_specialization,
    );
}

#[test]
fn check_step_subperformance_specialization() {
    // composite, owned by a Behavior or a Step, implies
    // specializesFromLibrary('Performances::Performance::subperformances')
    golden(
        "behavior B { step s { composite step t; } }",
        "B::s::t",
        c::check_step_subperformance_specialization,
        "Subsetting Performances::Performance::subperformances",
    );
}

#[test]
fn check_step_owned_performance_specialization() {
    // composite, owned by a Structure, implies
    // specializesFromLibrary('Objects::Object::ownedPerformances')
    golden(
        "struct S { composite step s; }",
        "S::s",
        c::check_step_owned_performance_specialization,
        "Subsetting Objects::Object::ownedPerformances",
    );
    not_applicable(
        "struct S { step s; }",
        "S::s",
        c::check_step_owned_performance_specialization,
    );
}

#[test]
fn check_function_specialization() {
    golden(
        "function F;",
        "F",
        c::check_function_specialization,
        "Subclassification Performances::Evaluation",
    );
}

#[test]
fn check_predicate_specialization() {
    golden(
        "predicate P;",
        "P",
        c::check_predicate_specialization,
        "Subclassification Performances::BooleanEvaluation",
    );
}

#[test]
fn check_expression_specialization() {
    golden(
        "expr e;",
        "e",
        c::check_expression_specialization,
        "Subsetting Performances::evaluations",
    );
}

#[test]
fn check_boolean_expression_specialization() {
    golden(
        "bool b;",
        "b",
        c::check_boolean_expression_specialization,
        "Subsetting Performances::booleanEvaluations",
    );
}

#[test]
fn check_invariant_specialization() {
    // trueEvaluations, or falseEvaluations if negated
    golden(
        "inv i { true }",
        "i",
        c::check_invariant_specialization,
        "Subsetting Performances::trueEvaluations",
    );
    golden(
        "inv false j { false }",
        "j",
        c::check_invariant_specialization,
        "Subsetting Performances::falseEvaluations",
    );
}

// ---- Table 11: Kernel expressions and others (KerML 8.4.4.1) ----

#[test]
fn check_constructor_expression_specialization() {
    golden(
        "class C; feature f = new C();",
        "f::ConstructorExpression#1",
        c::check_constructor_expression_specialization,
        "Subsetting Performances::constructorEvaluations",
    );
}

/// The implied relationships of an expression's result.
fn result_implied(m: &Model, x: Global) -> Vec<String> {
    implied(m, d::result(&m.db, m.project, x).unwrap())
}

#[test]
fn check_constructor_expression_result_specialization() {
    // result.specializes(instantiatedType)
    let m = model_with_library("class C; feature f = new C();");
    let x = m.get("f::ConstructorExpression#1");
    assert!(result_implied(&m, x).contains(&"FeatureTyping C".to_owned()));
    assert!(holds(
        &m,
        c::check_constructor_expression_result_specialization,
        x
    ));
}

#[test]
fn check_feature_reference_expression_result_specialization() {
    // result.owningType() = self and result.specializes(referent)
    let m = model_with_library("feature a; feature b = a;");
    let x = m.get("b::FeatureReferenceExpression#1");
    assert!(result_implied(&m, x).contains(&"Subsetting a".to_owned()));
    assert!(holds(
        &m,
        c::check_feature_reference_expression_result_specialization,
        x
    ));
}

#[test]
fn check_index_expression_result_specialization() {
    // unless the first argument's result specializes Collections::Array, the result
    // specializes it
    let text = "feature s : ScalarValues::Integer[*]; feature e = s#(1);
        feature arr : Collections::Array; feature e2 = arr#(1);";
    let m = model_with_library(text);
    let (db, p) = (&m.db, m.project);
    let x = m.get("e::IndexExpression#1");
    let argument = d::argument(db, p, x)[0];
    let first = d::result(db, p, argument).unwrap();
    assert!(result_implied(&m, x).contains(&format!("Subsetting {}", name(&m, first))));
    assert!(holds(
        &m,
        c::check_index_expression_result_specialization,
        x
    ));
    // an index into an Array
    let y = m.get("e2::IndexExpression#1");
    let array = d::result(db, p, d::argument(db, p, y)[0]).unwrap();
    assert!(!result_implied(&m, y).contains(&format!("Subsetting {}", name(&m, array))));
    assert!(holds(
        &m,
        c::check_index_expression_result_specialization,
        y
    ));
}

#[test]
fn check_select_expression_result_specialization() {
    // arguments->notEmpty() implies result.specializes(arguments->first().result)
    let m =
        model_with_library("feature s : ScalarValues::Integer[*]; feature e = s.?{in x; true};");
    let (db, p) = (&m.db, m.project);
    let x = m.get("e::SelectExpression#1");
    let first = d::result(db, p, d::argument(db, p, x)[0]).unwrap();
    assert!(result_implied(&m, x).contains(&format!("Subsetting {}", name(&m, first))));
    assert!(holds(
        &m,
        c::check_select_expression_result_specialization,
        x
    ));
}

#[test]
fn check_invocation_expression_specialization() {
    // specializes(instantiatedType)
    let m = model_with_library("function F { in x; return r; } feature v = F(1);");
    let x = m.get("v::InvocationExpression#1");
    assert!(implied(&m, x).contains(&"FeatureTyping F".to_owned()));
    assert!(holds(&m, c::check_invocation_expression_specialization, x));
}

#[test]
fn check_invocation_expression_behavior_result_specialization() {
    // the result of an invocation of a type that is not a Function specializes it
    let m = model_with_library(
        "behavior B { in x; } feature v = B(1); function F { in x; return r; } feature w = F(1);",
    );
    let x = m.get("v::InvocationExpression#1");
    assert!(result_implied(&m, x).contains(&"FeatureTyping B".to_owned()));
    assert!(holds(
        &m,
        c::check_invocation_expression_behavior_result_specialization,
        x
    ));
    // of a Function, the result redefines the function's result instead
    let y = m.get("w::InvocationExpression#1");
    assert!(!result_implied(&m, y).contains(&"FeatureTyping F".to_owned()));
    assert!(holds(
        &m,
        c::check_invocation_expression_behavior_result_specialization,
        y
    ));
}

#[test]
fn check_literal_expression_specialization() {
    golden(
        "feature f = true;",
        "f::LiteralBoolean#1",
        c::check_literal_expression_specialization,
        "Subsetting Performances::literalBooleanEvaluations",
    );
    // KerML 8.4.2: literalBooleanEvaluations subsets literalEvaluations, which is left out
    let m = model_with_library("feature f = true;");
    assert_eq!(
        implied(&m, m.get("f::LiteralBoolean#1")),
        ["Subsetting Performances::literalBooleanEvaluations"]
    );
}

#[test]
fn check_literal_boolean_specialization() {
    golden(
        "feature f = true;",
        "f::LiteralBoolean#1",
        c::check_literal_boolean_specialization,
        "Subsetting Performances::literalBooleanEvaluations",
    );
}

#[test]
fn check_literal_infinity_specialization() {
    golden(
        "feature f = *;",
        "f::LiteralInfinity#1",
        c::check_literal_infinity_specialization,
        "Subsetting Performances::literalIntegerEvaluations",
    );
}

#[test]
fn check_literal_integer_specialization() {
    golden(
        "feature f = 1;",
        "f::LiteralInteger#1",
        c::check_literal_integer_specialization,
        "Subsetting Performances::literalIntegerEvaluations",
    );
}

#[test]
fn check_literal_rational_specialization() {
    golden(
        "feature f = 1.5;",
        "f::LiteralRational#1",
        c::check_literal_rational_specialization,
        "Subsetting Performances::literalRationalEvaluations",
    );
}

#[test]
fn check_literal_string_specialization() {
    golden(
        "feature f = \"s\";",
        "f::LiteralString#1",
        c::check_literal_string_specialization,
        "Subsetting Performances::literalStringEvaluations",
    );
}

#[test]
fn check_metadata_access_expression_specialization() {
    golden(
        "classifier C; feature f = C.metadata;",
        "f::MetadataAccessExpression#1",
        c::check_metadata_access_expression_specialization,
        "Subsetting Performances::metadataAccessEvaluations",
    );
}

#[test]
fn check_null_expression_specialization() {
    golden(
        "feature f = null;",
        "f::NullExpression#1",
        c::check_null_expression_specialization,
        "Subsetting Performances::nullEvaluations",
    );
}

const FLOWS: &str = "behavior B { step s { out o; } step t { in i; }
    flow s.o to t.i; succession flow s.o to t.i; }";

#[test]
fn check_flow_specialization() {
    golden(
        "flow f;",
        "f",
        c::check_flow_specialization,
        "Subsetting Transfers::transfers",
    );
}

#[test]
fn check_flow_with_ends_specialization() {
    // ownedEndFeatures->notEmpty() implies specializesFromLibrary('Transfers::flowTransfers')
    golden(
        FLOWS,
        "B::Flow#1",
        c::check_flow_with_ends_specialization,
        "Subsetting Transfers::flowTransfers",
    );
    not_applicable("flow f;", "f", c::check_flow_with_ends_specialization);
}

#[test]
fn check_succession_flow_specialization() {
    golden(
        FLOWS,
        "B::SuccessionFlow#1",
        c::check_succession_flow_specialization,
        "Subsetting Transfers::flowTransfersBefore",
    );
}

#[test]
fn check_metaclass_specialization() {
    golden(
        "metaclass M;",
        "M",
        c::check_metaclass_specialization,
        "Subclassification Metaobjects::Metaobject",
    );
}

#[test]
fn check_metadata_feature_specialization() {
    golden(
        "metaclass M; classifier C { @M; }",
        "C::MetadataFeature#1",
        c::check_metadata_feature_specialization,
        "Subsetting Metaobjects::metaobjects",
    );
}

const SEMANTIC: &str = "classifier Kind; feature kinds : Kind[*];
    metaclass K specializes Metaobjects::SemanticMetadata {
        :>> baseType = kinds meta KerML::Feature;
    }
    #K classifier C; #K feature g;";

#[test]
fn check_metadata_feature_semantic_specialization() {
    // isSemantic() implies the annotated type specializes the base type: a Classifier the
    // types of a Feature base type, a Feature the base type itself
    let m = model_with_library(SEMANTIC);
    assert!(implied(&m, m.get("C")).contains(&"Subclassification Kind".to_owned()));
    assert!(implied(&m, m.get("g")).contains(&"Subsetting kinds".to_owned()));
    for path in ["C::MetadataFeature#1", "g::MetadataFeature#1"] {
        assert!(holds(
            &m,
            c::check_metadata_feature_semantic_specialization,
            m.get(path)
        ));
    }
    // metadata that is not semantic
    not_applicable(
        "metaclass M; classifier C { @M; }",
        "C::MetadataFeature#1",
        c::check_metadata_feature_semantic_specialization,
    );
}

// ---- Table 9 and 11: redefinitions (KerML 8.4.3.1, 8.4.4.1) ----

/// Whether the element has the implied relationship.
fn has(m: &Model, x: Global, expected: &str) -> bool {
    implied(m, x).iter().any(|s| s == expected)
}

#[test]
fn check_feature_end_redefinition() {
    // the i-th owned end redefines the i-th end of each general of the owning type
    let text = "assoc A { end feature a; end feature b; }
        assoc B specializes A { end feature c; end feature d; }";
    let m = model_with_library(text);
    assert!(has(&m, m.get("B::d"), "Redefinition A::b"));
    // and those of the library's BinaryLink, which a binary association specializes
    assert!(has(
        &m,
        m.get("A::a"),
        "Redefinition Links::BinaryLink::source"
    ));
    for path in ["A::a", "A::b", "B::c", "B::d"] {
        assert!(
            holds(&m, c::check_feature_end_redefinition, m.get(path)),
            "{path}"
        );
    }
    not_applicable(text, "A", c::check_feature_end_redefinition);
}

#[test]
fn check_feature_parameter_redefinition() {
    // the i-th parameter of a Behavior or Step redefines the i-th parameter of each general
    let text = "behavior P { in x; in y; } behavior Q specializes P { in a; in b; }
        function F { in x; in y; return r; } feature v = F(y = 2);";
    let m = model_with_library(text);
    assert!(has(&m, m.get("Q::b"), "Redefinition P::y"));
    assert!(holds(
        &m,
        c::check_feature_parameter_redefinition,
        m.get("Q::b")
    ));
    // the argument of an invocation with a written redefinition: none is implied
    let named = m.get("v::InvocationExpression#1::Feature#1");
    assert!(!implied(&m, named)
        .iter()
        .any(|s| s.starts_with("Redefinition")));
    assert!(holds(&m, c::check_feature_parameter_redefinition, named));
    // a general that is no Behavior or Step has no parameters to redefine (Table 9; ADR 0020)
    let m = model_with_library(
        "feature f { in x; } behavior B { in y; } step s : B subsets f { in z; }",
    );
    assert!(has(&m, m.get("s::z"), "Redefinition B::y"));
    assert!(!has(&m, m.get("s::z"), "Redefinition f::x"));
    assert!(holds(
        &m,
        c::check_feature_parameter_redefinition,
        m.get("s::z")
    ));
}

#[test]
fn check_feature_result_redefinition() {
    // the result of a Function redefines the result of each Function it specializes
    let m = model_with_library("function F { return r; } function G specializes F { return s; }");
    assert!(has(&m, m.get("G::s"), "Redefinition F::r"));
    assert!(holds(
        &m,
        c::check_feature_result_redefinition,
        m.get("G::s")
    ));
    // and that of Performances::Evaluation, which F specializes by implication
    assert!(has(
        &m,
        m.get("F::r"),
        "Redefinition Performances::Evaluation::result"
    ));
}

#[test]
fn check_feature_flow_feature_redefinition() {
    // the features of the first and second FlowEnds redefine sourceOutput and targetInput
    golden(
        FLOWS,
        "B::Flow#1::FlowEnd#1::Feature#1",
        c::check_feature_flow_feature_redefinition,
        "Redefinition Transfers::Transfer::source::sourceOutput",
    );
    golden(
        FLOWS,
        "B::Flow#1::FlowEnd#2::Feature#1",
        c::check_feature_flow_feature_redefinition,
        "Redefinition Transfers::Transfer::target::targetInput",
    );
}

#[test]
fn check_payload_feature_redefinition() {
    // redefinesFromLibrary('Transfers::Transfer::payload')
    golden(
        "behavior B { step s { out o; } step t { in i; }
            flow of ScalarValues::Integer from s.o to t.i; }",
        "B::Flow#1::PayloadFeature#1",
        c::check_payload_feature_redefinition,
        "Redefinition Transfers::Transfer::payload",
    );
}

#[test]
fn check_constructor_expression_result_feature_redefinition() {
    // each argument redefines one public feature of the instantiated type: a positional one
    // the feature at its position (8.4.4.1 Table 11 Note 6), a named one the feature it names
    let text = "class T { feature a; private feature p; feature b; }
        feature f = new T(1, 2); feature g = new T(b = 3);";
    let m = model_with_library(text);
    let (db, p) = (&m.db, m.project);
    let x = m.get("f::ConstructorExpression#1");
    let args = d::owned_feature(db, p, d::result(db, p, x).unwrap());
    assert!(has(&m, args[0], "Redefinition T::a"));
    assert!(has(&m, args[1], "Redefinition T::b"));
    assert!(holds(
        &m,
        c::check_constructor_expression_result_feature_redefinition,
        x
    ));
    let y = m.get("g::ConstructorExpression#1");
    let named = d::owned_feature(db, p, d::result(db, p, y).unwrap())[0];
    assert!(!implied(&m, named)
        .iter()
        .any(|s| s.starts_with("Redefinition")));
    assert!(holds(
        &m,
        c::check_constructor_expression_result_feature_redefinition,
        y
    ));
    // a named argument whose name does not resolve is not taken as positional: it violates
    let m = model_with_library("class T { feature a; } feature g = new T(c = 3);");
    let (db, p) = (&m.db, m.project);
    let y = m.get("g::ConstructorExpression#1");
    let named = d::owned_feature(db, p, d::result(db, p, y).unwrap())[0];
    assert!(!implied(&m, named)
        .iter()
        .any(|s| s.starts_with("Redefinition")));
    assert!(!holds(
        &m,
        c::check_constructor_expression_result_feature_redefinition,
        y
    ));
}

const CHAIN: &str = "classifier C { feature d; } feature c : C; feature e = c.d;";

#[test]
fn check_feature_chain_expression_source_target_redefinition() {
    // the sourceTargetFeature redefines the target feature
    let m = model_with_library(CHAIN);
    let (db, p) = (&m.db, m.project);
    let x = m.get("e::FeatureChainExpression#1");
    let stf = d::source_target_feature(db, p, x).unwrap();
    assert!(has(&m, stf, "Redefinition C::d"));
    assert!(holds(
        &m,
        c::check_feature_chain_expression_source_target_redefinition,
        x
    ));
    // without a target feature, nothing is redefined: it violates
    let m = model_with_library("classifier C; feature c : C; feature e = c.zzz;");
    assert!(!holds(
        &m,
        c::check_feature_chain_expression_source_target_redefinition,
        m.get("e::FeatureChainExpression#1")
    ));
}

#[test]
fn check_feature_chain_expression_target_redefinition() {
    // the sourceTargetFeature redefines ControlFunctions::'.'::source::target
    let m = model_with_library(CHAIN);
    let (db, p) = (&m.db, m.project);
    let x = m.get("e::FeatureChainExpression#1");
    let stf = d::source_target_feature(db, p, x).unwrap();
    let target = d::library(db, p, "ControlFunctions::'.'::source::target").unwrap();
    assert!(has(&m, stf, &format!("Redefinition {}", name(&m, target))));
    assert!(holds(
        &m,
        c::check_feature_chain_expression_target_redefinition,
        x
    ));
    let bare = model(CHAIN);
    assert!(!holds(
        &bare,
        c::check_feature_chain_expression_target_redefinition,
        bare.get("e::FeatureChainExpression#1")
    ));
}

#[test]
fn check_feature_chain_expression_result_specialization() {
    // the result subsets the chain of the first input parameter and the sourceTargetFeature,
    // which the overlay builds
    let m = model_with_library(CHAIN);
    let (db, p) = (&m.db, m.project);
    let x = m.get("e::FeatureChainExpression#1");
    let stf = d::source_target_feature(db, p, x).unwrap();
    let input = d::owner(db, p, stf).unwrap();
    let result = d::result(db, p, x).unwrap();
    let chains: Vec<Vec<Global>> = d::owned_subsetting(db, p, result)
        .into_iter()
        .flat_map(|r| d::target(db, p, r))
        .map(|t| d::chaining_feature(db, p, t))
        .collect();
    assert!(chains.contains(&vec![input, stf]), "{chains:?}");
    assert!(holds(
        &m,
        c::check_feature_chain_expression_result_specialization,
        x
    ));
    assert!(holds(&m, c::check_feature_specialization, result));
}

// ---- Tables 9 and 11: type featurings (KerML 8.4.3.1, 8.4.4.1) ----

#[test]
fn check_feature_feature_membership_type_featuring() {
    // a feature owned through a FeatureMembership is featured by its owning type
    let text = "classifier C { feature f; } class K { var feature v; }
        classifier D { feature g featured by D; }";
    let m = model_with_library(text);
    let (db, p) = (&m.db, m.project);
    assert!(has(&m, m.get("C::f"), "TypeFeaturing C"));
    // a written featuring that satisfies the constraint: none is implied
    assert!(!implied(&m, m.get("D::g"))
        .iter()
        .any(|s| s.starts_with("TypeFeaturing")));
    // a variable feature, by a constructed feature that redefines Occurrence::snapshots and is
    // featured by the owning type (KerML 8.4.4.3)
    let v = m.get("K::v");
    let snapshots = d::featuring_type(db, p, v).clone();
    assert_eq!(snapshots.len(), 1);
    assert!(sysml_semantics::view(db, p, m.file).is_implied(snapshots[0].ix()));
    assert!(d::redefines_from_library(
        db,
        p,
        snapshots[0],
        "Occurrences::Occurrence::snapshots"
    ));
    assert_eq!(m.labels(d::featuring_type(db, p, snapshots[0])), ["K"]);
    // a variable feature of Occurrence itself, by Occurrence::snapshots
    let incoming = d::library(db, p, "Occurrences::Occurrence::incomingTransfers").unwrap();
    let featuring: Vec<String> = d::featuring_type(db, p, incoming)
        .iter()
        .map(|t| name(&m, *t))
        .collect();
    assert_eq!(featuring, ["Occurrences::Occurrence::snapshots"]);
    for path in ["C::f", "K::v", "D::g"] {
        assert!(holds(
            &m,
            c::check_feature_feature_membership_type_featuring,
            m.get(path)
        ));
    }
    assert!(holds(&m, c::check_feature_specialization, snapshots[0]));
}

#[test]
fn check_feature_owned_cross_feature_type_featuring() {
    // an owned cross feature is featured by the type of the other end
    let m = model_with_library(CROSSING);
    let (db, p) = (&m.db, m.project);
    let cross = d::owned_cross_feature(db, p, m.get("A::a")).unwrap();
    assert!(has(&m, cross, "TypeFeaturing U"));
    assert!(holds(
        &m,
        c::check_feature_owned_cross_feature_type_featuring,
        cross
    ));
    // or, for more ends, by a Cartesian product of the other ends' types (KerML 8.4.4.5.1),
    // which also types the first feature of the end's crossing chain
    let m = model_with_library(
        "classifier T; classifier U; classifier V;
        assoc A { end [1] feature a : T; end feature b : U; end feature c : V; }",
    );
    let (db, p) = (&m.db, m.project);
    let a = m.get("A::a");
    let cross = d::owned_cross_feature(db, p, a).unwrap();
    let featuring = d::featuring_type(db, p, cross).clone();
    let [product] = featuring[..] else {
        panic!("{featuring:?}");
    };
    assert!(d::is_cartesian_product(db, p, product));
    assert_eq!(
        m.labels(d::as_cartesian_product(db, p, product)),
        ["U", "V"]
    );
    let chain = d::target(db, p, d::owned_cross_subsetting(db, p, a).unwrap())[0];
    let first = d::chaining_feature(db, p, chain)[0];
    assert_eq!(d::r#type(db, p, first), [product]);
    assert!(holds(
        &m,
        c::check_feature_owned_cross_feature_type_featuring,
        cross
    ));
    assert!(holds(&m, c::check_feature_specialization, product));
}

#[test]
fn check_multiplicity_type_featuring() {
    // a multiplicity has the featuring types of the feature that owns it, or none
    let text = "classifier C { feature f[1]; } feature g[1];";
    let m = model_with_library(text);
    let (db, p) = (&m.db, m.project);
    let mult = m.get("C::f::MultiplicityRange#1");
    assert!(has(&m, mult, "TypeFeaturing C"));
    assert!(holds(&m, c::check_multiplicity_type_featuring, mult));
    let package = m.get("g::MultiplicityRange#1");
    assert!(d::featuring_type(db, p, package).is_empty());
    assert!(holds(&m, c::check_multiplicity_type_featuring, package));
}

#[test]
fn check_multiplicity_range_expression_type_featuring() {
    // the bounds of a MultiplicityRange have its featuring types
    let m = model_with_library("classifier C { feature n : ScalarValues::Natural; feature f[n]; }");
    let (db, p) = (&m.db, m.project);
    let range = m.get("C::f::MultiplicityRange#1");
    let bound = d::bound(db, p, range)[0];
    assert!(has(&m, bound, "TypeFeaturing C"));
    assert!(holds(
        &m,
        c::check_multiplicity_range_expression_type_featuring,
        range
    ));
}

#[test]
fn check_expression_type_featuring() {
    // the value of a FeatureValue has the featuring types of the feature with the value
    let m = model_with_library("classifier C { feature a; feature b = a; }");
    let x = m.get("C::b::FeatureReferenceExpression#1");
    assert!(has(&m, x, "TypeFeaturing C"));
    assert!(holds(&m, c::check_expression_type_featuring, x));
}

#[test]
fn check_connector_type_featuring() {
    // a connector without an owning type is featured by its defaultFeaturingType (8.4.4.1
    // Table 11 Note 2); one with an owning type is featured by it
    let m = model_with_library(
        "classifier C { feature x; feature y; connector j from x to y; }
        connector k from C::x to C::y;",
    );
    let k = m.get("k");
    assert!(has(&m, k, "TypeFeaturing C"));
    for path in ["k", "C::j"] {
        assert!(
            holds(&m, c::check_connector_type_featuring, m.get(path)),
            "{path}"
        );
    }
}

// ---- type featurings: the cases the review of PR #27 found ----

#[test]
fn a_copied_featuring_follows_a_connector_default() {
    // the multiplicity of a connector featured by its defaultFeaturingType has the same
    let m = model_with_library(
        "classifier C { feature x; feature y; } connector k[1] from C::x to C::y;",
    );
    let (db, p) = (&m.db, m.project);
    let mult = m.get("k::MultiplicityRange#1");
    assert_eq!(m.labels(d::featuring_type(db, p, m.get("k"))), ["C"]);
    assert_eq!(m.labels(d::featuring_type(db, p, mult)), ["C"]);
    assert!(holds(&m, c::check_multiplicity_type_featuring, mult));
}

#[test]
fn a_connector_featuring_another_feature_still_gets_its_default() {
    // a featuring of another feature in the connector's body is no featuring of the connector:
    // the textual notation owns it through a membership, and an owned TypeFeaturing counts only
    // when its featureOfType is the connector (written_featurings)
    let m = model_with_library(
        "classifier C { feature x; feature y; }
        connector k from C::x to C::y { featuring C::x by C; }",
    );
    assert_eq!(
        m.labels(d::featuring_type(&m.db, m.project, m.get("k"))),
        ["C"]
    );
    assert!(holds(&m, c::check_connector_type_featuring, m.get("k")));
}

#[test]
fn the_other_ends_include_the_inherited_ones() {
    // B's own end a2 redefines a, and B inherits b: b is the other end of a2
    let m = model_with_library(
        "classifier T; classifier U;
        assoc A { end feature a : T; end feature b : U; }
        assoc B specializes A { end [1] feature a2 redefines a; }",
    );
    let (db, p) = (&m.db, m.project);
    let cross = d::owned_cross_feature(db, p, m.get("B::a2")).unwrap();
    assert_eq!(m.labels(d::featuring_type(db, p, cross)), ["U"]);
    assert!(holds(
        &m,
        c::check_feature_owned_cross_feature_type_featuring,
        cross
    ));
}

#[test]
fn a_product_of_ends_with_several_types() {
    // the product has every type of each other end, in order
    let m = model_with_library(
        "classifier T; classifier U; classifier W; classifier V;
        assoc A { end [1] feature a : T; end feature b : U, W; end feature c : V; }",
    );
    let (db, p) = (&m.db, m.project);
    let cross = d::owned_cross_feature(db, p, m.get("A::a")).unwrap();
    let product = d::featuring_type(db, p, cross)[0];
    assert_eq!(
        m.labels(d::as_cartesian_product(db, p, product)),
        ["U", "W", "V"]
    );
    assert!(holds(
        &m,
        c::check_feature_owned_cross_feature_type_featuring,
        cross
    ));
}

#[test]
fn every_implied_relationship_has_its_ends() {
    // without the libraries an end of a ternary association is untyped, so no product can be
    // built, and no TypeFeaturing or FeatureTyping is left without a target
    for text in [
        IMPLIED,
        "assoc A { end [1] feature a; end feature b; end feature c; }",
    ] {
        for m in [model(text), model_with_library(text)] {
            let (db, p) = (&m.db, m.project);
            let v = sysml_semantics::view(db, p, m.file);
            for index in m.graph().graph.len() as u32..v.len() as u32 {
                let x = Global {
                    file: m.file,
                    index,
                };
                let k = v.metaclass(x.ix());
                if k.specializes(Metaclass::Specialization)
                    || k == Metaclass::TypeFeaturing
                    || k == Metaclass::FeatureChaining
                {
                    assert!(!d::target(db, p, x).is_empty(), "{k:?} {}", name(&m, x));
                }
            }
        }
    }
}

#[test]
fn a_written_snapshots_featuring_satisfies_a_variable_feature() {
    // s redefines snapshots and is featured by K through its implied featuring: v needs no
    // constructed one
    let m = model_with_library(
        "class K { feature s redefines Occurrences::Occurrence::snapshots;
            var feature v featured by s; }",
    );
    let (db, p) = (&m.db, m.project);
    assert_eq!(m.labels(d::featuring_type(db, p, m.get("K::v"))), ["K::s"]);
    assert!(holds(
        &m,
        c::check_feature_feature_membership_type_featuring,
        m.get("K::v")
    ));
}

#[test]
fn a_chain_that_gives_the_owning_type_needs_no_featuring() {
    // x's chain starts with a, which C features, so x is featured by C already
    let m = model_with_library("classifier C { feature a { feature b; } feature x chains a.b; }");
    let x = m.get("C::x");
    assert!(!implied(&m, x)
        .iter()
        .any(|s| s.starts_with("TypeFeaturing")));
    assert_eq!(m.labels(d::featuring_type(&m.db, m.project, x)), ["C"]);
    assert!(holds(
        &m,
        c::check_feature_feature_membership_type_featuring,
        x
    ));
}

// ---- Table 11: binding connectors (KerML 8.4.4.1) ----

/// The related features of the implied binding connectors an element owns.
fn bindings(m: &Model, x: Global) -> Vec<Vec<Global>> {
    let v = sysml_semantics::view(&m.db, m.project, x.file);
    d::owned_member(&m.db, m.project, x)
        .into_iter()
        .filter(|b| v.is_implied(b.ix()) && v.metaclass(b.ix()) == Metaclass::BindingConnector)
        .map(|b| d::related_feature(&m.db, m.project, b))
        .collect()
}

#[test]
fn check_invocation_expression_behavior_binding_connector() {
    // an invocation of a Behavior binds itself to its result; of a Function, it does not
    let m = model_with_library(
        "behavior B { in x; } feature v = B(1); function F { in x; return r; } feature w = F(1);",
    );
    let (db, p) = (&m.db, m.project);
    let x = m.get("v::InvocationExpression#1");
    assert_eq!(bindings(&m, x), [vec![x, d::result(db, p, x).unwrap()]]);
    let y = m.get("w::InvocationExpression#1");
    assert!(bindings(&m, y).is_empty());
    for x in [x, y] {
        assert!(holds(
            &m,
            c::check_invocation_expression_behavior_binding_connector,
            x
        ));
    }
}

#[test]
fn check_feature_reference_expression_binding_connector() {
    // the referent is bound to the result
    let m = model_with_library("feature a; feature b = a;");
    let (db, p) = (&m.db, m.project);
    let x = m.get("b::FeatureReferenceExpression#1");
    assert_eq!(
        bindings(&m, x),
        [vec![m.get("a"), d::result(db, p, x).unwrap()]]
    );
    assert!(holds(
        &m,
        c::check_feature_reference_expression_binding_connector,
        x
    ));
}

#[test]
fn check_function_result_binding_connector() {
    // a function's result is bound to the result of its result expression
    let m = model_with_library("function F { in x; x }");
    let (db, p) = (&m.db, m.project);
    let f = m.get("F");
    let body = m.get("F::FeatureReferenceExpression#1");
    let expected = vec![
        d::result(db, p, f).unwrap(),
        d::result(db, p, body).unwrap(),
    ];
    assert_eq!(bindings(&m, f), [expected]);
    assert!(holds(&m, c::check_function_result_binding_connector, f));
}

#[test]
fn check_expression_result_binding_connector() {
    // an expression's result is bound to the result of its result expression
    let m = model_with_library("expr e { 1 }");
    let (db, p) = (&m.db, m.project);
    let e = m.get("e");
    let literal = m.get("e::LiteralInteger#1");
    let expected = vec![
        d::result(db, p, e).unwrap(),
        d::result(db, p, literal).unwrap(),
    ];
    assert_eq!(bindings(&m, e), [expected]);
    assert!(holds(&m, c::check_expression_result_binding_connector, e));
}

#[test]
fn check_feature_value_binding_connector() {
    // the feature is bound to the chain of the value and its result, featured as the feature is;
    // for an initial value, by that.startShot; for a default, there is none
    let text = "classifier C { feature a; feature b = a; }
        class K { feature a; var feature v := a; } classifier D { feature e default = 1; }";
    let m = model_with_library(text);
    let (db, p) = (&m.db, m.project);
    let b = m.get("C::b");
    let value = m.get("C::b::FeatureReferenceExpression#1");
    let related = bindings(&m, b);
    assert_eq!(related.len(), 1);
    assert_eq!(related[0][0], b);
    let chain = d::chaining_feature(db, p, related[0][1]);
    assert_eq!(chain, [value, d::result(db, p, value).unwrap()]);
    let fv = m.relationship("C::b", Metaclass::FeatureValue, 1);
    assert!(holds(&m, c::check_feature_value_binding_connector, fv));
    // initial
    let v = m.get("K::v");
    assert_eq!(bindings(&m, v).len(), 1);
    let fv = m.relationship("K::v", Metaclass::FeatureValue, 1);
    assert!(holds(&m, c::check_feature_value_binding_connector, fv));
    // without the libraries there is no that.startShot to feature the initial value's binding,
    // which the result of `a` still implies
    let bare = model(text);
    let fv = bare.relationship("K::v", Metaclass::FeatureValue, 1);
    assert!(!holds(&bare, c::check_feature_value_binding_connector, fv));
    // default
    assert!(bindings(&m, m.get("D::e")).is_empty());
    let fv = m.relationship("D::e", Metaclass::FeatureValue, 1);
    assert!(holds(&m, c::check_feature_value_binding_connector, fv));
}

#[test]
fn implied_binding_connectors_meet_the_other_constraints() {
    // an implied binding connector, its ends and their chains are checked like any element
    let m = model_with_library(
        "classifier C { feature a; feature b = a; } class K { var feature v := 1; }
        function F { in x; x } behavior B { in x; } feature w = B(1);",
    );
    assert!(c::violations(&m.db, m.project, m.file).is_empty());
}

// ---- KerML 8.4.2: the redundancy rules ----

#[test]
fn the_redundancy_rules() {
    // Rule 1, the clause's example: a Structure's implied Subclassification of Objects::Object
    // also satisfies checkClassSpecialization and checkTypeSpecialization, whose generals are
    // left out
    let m = model_with_library("struct S; class C specializes S;");
    assert_eq!(
        implied(&m, m.get("S")),
        ["Subclassification Objects::Object"]
    );
    // and a written specialization whose general specializes the implied one leaves it out
    assert!(implied(&m, m.get("C")).is_empty());
    for check in [
        c::check_type_specialization as Check,
        c::check_class_specialization,
    ] {
        assert!(holds(&m, check, m.get("S")));
        assert!(holds(&m, check, m.get("C")));
    }
    // Rule 2: an invocation of Performances::evaluations, which checkInvocationExpression and
    // checkExpressionSpecialization both imply, subsets it once
    let m = model_with_library("feature v = Performances::evaluations();");
    let i = implied(&m, m.get("v::InvocationExpression#1"));
    let n = i
        .iter()
        .filter(|s| *s == "Subsetting Performances::evaluations")
        .count();
    assert_eq!(n, 1, "{i:?}");
}

// ---- implied elements (ADR 0022) ----

/// A model with every kind of implied element and relationship: specializations, redefinitions,
/// the chain and features of an implied CrossSubsetting, and the chain a FeatureChainExpression's
/// result subsets.
const IMPLIED: &str = "classifier T; classifier U;
    assoc A { end [1] feature a : T; end feature b : U; }
    assoc B specializes A { end [0..1] feature a2 : T; end feature b2 : U; }
    behavior P { in x; } feature v = P(1); feature w = v;
    classifier C { feature d; } feature c : C; feature e = c.d;
    class K { feature k; var feature w; } feature n = new K(1);
    classifier V; assoc N { end [1] feature a : C; end feature b : K; end feature c : V; }
    connector j from C::d to K::k;";

#[test]
fn derived_properties_of_implied_elements() {
    // Every derived property and operation of one element, on every element of the view, the
    // implied ones included: none reads past the lowered fragment.
    let m = model_with_library(IMPLIED);
    let (db, p) = (&m.db, m.project);
    let v = sysml_semantics::view(db, p, m.file);
    let base = m.graph().graph.len();
    assert!(v.len() > base, "the model has implied elements");
    for index in (0..v.len() as u32).rev() {
        let x = Global {
            file: m.file,
            index,
        };
        let _ = d::owner(db, p, x);
        let _ = d::owned_element(db, p, x);
        let _ = d::owning_membership(db, p, x);
        let _ = d::owning_namespace(db, p, x);
        let _ = d::name(db, p, x);
        let _ = d::short_name(db, p, x);
        let _ = d::effective_name(db, p, x);
        let _ = d::effective_short_name(db, p, x);
        let _ = d::escaped_name(db, p, x);
        let _ = d::qualified_name(db, p, x);
        let _ = d::path(db, p, x);
        let _ = d::owned_annotation(db, p, x);
        let _ = d::documentation(db, p, x);
        let _ = d::textual_representation(db, p, x);
        let _ = d::library_namespace(db, p, x);
        let _ = d::is_library_element(db, p, x);
        let _ = d::source(db, p, x);
        let _ = d::target(db, p, x);
        let _ = d::related_element(db, p, x);
        let _ = d::annotation_annotated_element(db, p, x);
        let _ = d::owned_annotating_element(db, p, x);
        let _ = d::owning_annotating_element(db, p, x);
        let _ = d::annotating_element(db, p, x);
        let _ = d::owned_annotating_relationship(db, p, x);
        let _ = d::owning_annotating_relationship(db, p, x);
        let _ = d::annotation(db, p, x);
        let _ = d::annotated_element(db, p, x);
        let _ = d::owned_membership(db, p, x);
        let _ = d::owned_member(db, p, x);
        let _ = d::owned_import(db, p, x);
        let _ = d::imported_membership(db, p, x);
        let _ = d::membership(db, p, x);
        let _ = d::member(db, p, x);
        let _ = d::namespace_imported_memberships(db, p, x);
        let _ = d::import_imported_memberships(db, p, x);
        let _ = d::owned_member_name(db, p, x);
        let _ = d::owned_member_short_name(db, p, x);
        let _ = d::imported_element(db, p, x);
        let _ = d::owned_specialization(db, p, x);
        let _ = d::owned_conjugator(db, p, x);
        let _ = d::is_conjugated(db, p, x);
        let _ = d::owned_unioning(db, p, x);
        let _ = d::owned_intersecting(db, p, x);
        let _ = d::owned_differencing(db, p, x);
        let _ = d::owned_disjoining(db, p, x);
        let _ = d::unioning_type(db, p, x);
        let _ = d::intersecting_type(db, p, x);
        let _ = d::differencing_type(db, p, x);
        let _ = d::owned_feature_membership(db, p, x);
        let _ = d::owned_feature(db, p, x);
        let _ = d::inherited_membership(db, p, x);
        let _ = d::inherited_feature(db, p, x);
        let _ = d::feature_membership(db, p, x);
        let _ = d::feature(db, p, x);
        let _ = d::end_feature(db, p, x);
        let _ = d::owned_end_feature(db, p, x);
        let _ = d::multiplicity(db, p, x);
        let _ = d::owning_type(db, p, x);
        let _ = d::input(db, p, x);
        let _ = d::output(db, p, x);
        let _ = d::directed_feature(db, p, x);
        let _ = d::all_supertypes(db, p, x);
        let _ = d::multiplicities(db, p, x);
        let _ = d::all_redefined_features_of(db, p, x);
        let _ = d::owned_subclassification(db, p, x);
        let _ = d::owned_subsetting(db, p, x);
        let _ = d::owned_redefinition(db, p, x);
        let _ = d::owned_typing(db, p, x);
        let _ = d::owned_reference_subsetting(db, p, x);
        let _ = d::owned_cross_subsetting(db, p, x);
        let _ = d::owned_feature_chaining(db, p, x);
        let _ = d::chaining_feature(db, p, x);
        let _ = d::feature_target(db, p, x);
        let _ = d::cross_feature(db, p, x);
        let _ = d::owned_type_featuring(db, p, x);
        let _ = d::owned_feature_inverting(db, p, x);
        let _ = d::featuring_type(db, p, x);
        let _ = d::typing_features(db, p, x);
        let _ = d::r#type(db, p, x);
        let _ = d::naming_feature(db, p, x);
        let _ = d::all_redefined_features(db, p, x);
        let _ = d::owned_cross_feature(db, p, x);
        let _ = d::is_owned_cross_feature(db, p, x);
        let _ = d::is_cartesian_product(db, p, x);
        let _ = d::as_cartesian_product(db, p, x);
        let _ = d::step(db, p, x);
        let _ = d::behavior(db, p, x);
        let _ = d::parameter_direction(db, p, x);
        let _ = d::result(db, p, x);
        let _ = d::instantiated_type(db, p, x);
        let _ = d::argument(db, p, x);
        let _ = d::constructor_argument(db, p, x);
        let _ = d::referent(db, p, x);
        let _ = d::chain_target_feature(db, p, x);
        let _ = d::source_target_feature(db, p, x);
        let _ = d::referenced_element(db, p, x);
        let _ = d::related_feature(db, p, x);
        let _ = d::source_feature(db, p, x);
        let _ = d::target_feature(db, p, x);
        let _ = d::default_featuring_type(db, p, x);
        let _ = d::related_type(db, p, x);
        let _ = d::source_type(db, p, x);
        let _ = d::target_type(db, p, x);
        let _ = d::flow_end(db, p, x);
        let _ = d::payload_feature(db, p, x);
        let _ = d::payload_type(db, p, x);
        let _ = d::source_output_feature(db, p, x);
        let _ = d::target_input_feature(db, p, x);
        let _ = d::lower_bound(db, p, x);
        let _ = d::upper_bound(db, p, x);
        let _ = d::bound(db, p, x);
        let _ = d::metaclass_of(db, p, x);
        let _ = d::is_semantic(db, p, x);
        let _ = d::is_syntactic(db, p, x);
        let _ = d::filter_condition(db, p, x);
        let _ = d::function_is_model_level_evaluable(db, p, x);
        let _ = d::is_model_level_evaluable(db, p, x);
        let _ = d::metaclass_feature(db, p, x);
        let _ = d::package_imported_memberships(db, p, x);
    }
    let _ = c::violations(db, p, m.file);
    // The view's implied relationships, given to resolution, resolve nothing.
    for index in base as u32..v.len() as u32 {
        let x = Global {
            file: m.file,
            index,
        };
        for slot in [sysml_model::Slot::Source, sysml_model::Slot::Target] {
            assert!(sysml_semantics::resolve_end(db, p, x, slot, 0).is_none());
        }
    }
}

// ---- all checks ----

#[test]
fn violations_lists_what_an_element_violates() {
    // without the libraries, a class violates the constraints that name library types
    let m = model("class C;");
    let found: Vec<&str> = c::violations(&m.db, m.project, m.file)
        .into_iter()
        .filter(|v| v.element == m.get("C"))
        .map(|v| v.constraint)
        .collect();
    assert_eq!(
        found,
        ["checkTypeSpecialization", "checkClassSpecialization"]
    );
    let m = model_with_library("class C;");
    assert!(c::violations(&m.db, m.project, m.file).is_empty());
}

#[test]
fn the_kerml_libraries_satisfy_the_checks() {
    // KerML 8.4.2: the libraries, with their implied relationships, meet the semantic
    // constraints; implied elements are checked too
    let m = model_with_library("");
    let mut found = Vec::new();
    for f in m.project.files(&m.db) {
        for v in c::violations(&m.db, m.project, *f) {
            found.push(format!("{} {}", v.constraint, name(&m, v.element)));
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}

#[test]
fn the_checks_are_the_constraints_of_their_metaclasses() {
    // each entry of CHECKS names a check… constraint of its metaclass in KerML.xmi
    for (m, n, _) in c::CHECKS {
        assert!(
            sysml_model::CONSTRAINTS
                .iter()
                .any(|r| r.metaclass == m.name() && r.name == *n),
            "{}::{n}",
            m.name()
        );
    }
}
