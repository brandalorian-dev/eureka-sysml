// SPDX-License-Identifier: Apache-2.0
//! Model-level evaluation (KerML 8.4.4.9.8, 8.3.4.8; step 1a.8, ADR 0021): one test per rule and
//! operation, from its OCL and the rules of 8.4.4.9.8, named after it
//! (`tests/derived_inventory.rs` checks).

mod common;

use common::{model, model_with_library, Model};
use sysml_semantics::derived::{self as d, Item, Natural};
use sysml_semantics::Global;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// The value expression of a top-level feature (its FeatureValue's).
fn value(m: &Model, feature: &str) -> Global {
    let l = sysml_semantics::lowered(&m.db, m.file).clone();
    let f = m.get(feature);
    let g = &l.graph;
    let r = g
        .owned_relationships(f.ix())
        .iter()
        .find(|r| g.metaclass(**r) == sysml_model::Metaclass::FeatureValue)
        .unwrap_or_else(|| panic!("{feature} has no value"));
    let e = g.owned_related_elements(*r)[0];
    Global {
        file: m.file,
        index: e.index() as u32,
    }
}

fn eval(m: &Model, feature: &str) -> Vec<Item> {
    d::evaluate(&m.db, m.project, value(m, feature), None)
}

fn s(v: &str) -> Item {
    Item::String(v.into())
}

// ---- null and literal expressions ----

#[test]
fn null_expression_evaluate() {
    // NullExpression::evaluate(target) = Sequence{}
    let m = model("feature n = null;");
    assert!(eval(&m, "n").is_empty());
}

#[test]
fn null_expression_model_level_evaluable() {
    // NullExpression::modelLevelEvaluable(visited) = true
    let m = model("feature n = null;");
    assert!(d::model_level_evaluable(
        &m.db,
        m.project,
        value(&m, "n"),
        &[]
    ));
}

#[test]
fn literal_expression_evaluate() {
    // LiteralExpression::evaluate(target) = Sequence{self}: a literal is its value
    let m = model(
        r#"feature a = 3; feature b = true; feature c = "s"; feature e = *; feature r = 1.5;"#,
    );
    assert_eq!(eval(&m, "a"), [Item::Integer(3)]);
    assert_eq!(eval(&m, "b"), [Item::Boolean(true)]);
    assert_eq!(eval(&m, "c"), [s("s")]);
    assert_eq!(eval(&m, "e"), [Item::Infinity]);
    assert_eq!(eval(&m, "r"), [Item::Rational(1.5)]);
}

#[test]
fn literal_expression_model_level_evaluable() {
    // LiteralExpression::modelLevelEvaluable(visited) = true
    let m = model("feature a = 3;");
    assert!(d::model_level_evaluable(
        &m.db,
        m.project,
        value(&m, "a"),
        &[]
    ));
}

#[test]
fn derive_literal_expression_is_model_level_evaluable() {
    // isModelLevelEvaluable = true
    let m = model("feature a = true;");
    assert!(d::is_model_level_evaluable(
        &m.db,
        m.project,
        value(&m, "a")
    ));
}

// ---- feature references ----

const REFERENCES: &str = "feature x = 5; feature y = x; feature z; feature w = z;
    classifier T { feature f = 1; } classifier U specializes T { feature redefines f = 2; }
    feature r = T::f; feature loop = loop;
    metaclass M { feature level; } feature q = M::level;";

#[test]
fn feature_reference_expression_evaluate() {
    // The value of the target's feature that is or redefines the referent; else, without
    // featuring types, the referent's own value; else the referent, if not an Expression
    let m = model(REFERENCES);
    let (db, p) = (&m.db, m.project);
    assert_eq!(eval(&m, "y"), [Item::Integer(5)]);
    assert_eq!(eval(&m, "w"), [Item::Element(m.get("z"))]);
    let on = |t: &str| d::evaluate(db, p, value(&m, "r"), Some(Item::Element(m.get(t))));
    assert_eq!(on("U"), [Item::Integer(2)]);
    assert_eq!(on("T"), [Item::Integer(1)]);
    assert!(
        eval(&m, "loop").is_empty(),
        "a value that refers to itself has none"
    );
}

#[test]
fn feature_reference_expression_model_level_evaluable() {
    // A reference to a feature without featuring types whose value is evaluable, or to a feature
    // of a metaclass; not to a feature of an ordinary classifier
    let m = model(REFERENCES);
    let (db, p) = (&m.db, m.project);
    assert!(d::model_level_evaluable(db, p, value(&m, "y"), &[]));
    assert!(d::model_level_evaluable(db, p, value(&m, "q"), &[]));
    assert!(!d::model_level_evaluable(db, p, value(&m, "r"), &[]));
}

// ---- invocations (Tables 5 and 7) ----

const ARITHMETIC: &str = r#"
    feature a = 1 + 2 * 3; feature b = 7 / 2; feature c = "a" + "b"; feature d = not true;
    feature e = 1..3; feature f = if 1 < 2 ? "y" else "n"; feature g = null ?? 4;
    feature h = true and false; feature i = 1 == 1.0; feature j = (1, 2, 3) # (2);
    feature k = 2 ^ 3; feature l = -4; feature n = false or true; feature o = 5 % 3;
    feature t = 3 istype ScalarValues::Natural; feature u = "x" @ ScalarValues::Integer;
    function g { return r; } feature v = g();"#;

#[test]
fn invocation_expression_evaluate() {
    // An application of the function to the evaluated arguments (8.4.4.9.8 rule 5)
    let m = model_with_library(ARITHMETIC);
    let cases: Vec<(&str, Vec<Item>)> = vec![
        ("a", vec![Item::Integer(7)]),
        ("b", vec![Item::Rational(3.5)]),
        ("c", vec![s("ab")]),
        ("d", vec![Item::Boolean(false)]),
        (
            "e",
            vec![Item::Integer(1), Item::Integer(2), Item::Integer(3)],
        ),
        ("f", vec![s("y")]),
        ("g", vec![Item::Integer(4)]),
        ("h", vec![Item::Boolean(false)]),
        ("i", vec![Item::Boolean(true)]),
        ("j", vec![Item::Integer(2)]),
        ("k", vec![Item::Integer(8)]),
        ("l", vec![Item::Integer(-4)]),
        ("n", vec![Item::Boolean(true)]),
        ("o", vec![Item::Integer(2)]),
        ("t", vec![Item::Boolean(true)]),
        ("u", vec![Item::Boolean(false)]),
    ];
    for (f, expected) in cases {
        assert_eq!(eval(&m, f), expected, "{f}");
    }
}

#[test]
fn invocation_expression_model_level_evaluable() {
    // Its arguments are evaluable and it invokes an evaluable library function
    let m = model_with_library(ARITHMETIC);
    let (db, p) = (&m.db, m.project);
    assert!(d::model_level_evaluable(db, p, value(&m, "a"), &[]));
    assert!(
        !d::model_level_evaluable(db, p, value(&m, "v"), &[]),
        "a user function"
    );
}

#[test]
fn constructor_expression_model_level_evaluable() {
    // ConstructorExpression::modelLevelEvaluable: its arguments are evaluable
    let m = model_with_library(
        "classifier T { feature a; } function g { return r; }
        feature w = new T(a = 1); feature x = new T(a = g());",
    );
    let (db, p) = (&m.db, m.project);
    assert!(d::model_level_evaluable(db, p, value(&m, "w"), &[]));
    assert!(!d::model_level_evaluable(db, p, value(&m, "x"), &[]));
}

// ---- expressions ----

const EXPRESSIONS: &str = "expr e { 1 + 1 } expr e2 { in x; x + 1 } function F; expr e3 : F { 1 }
    feature c = 1 < 2; feature c1 = 1; feature c2 = (true, true);";

#[test]
fn expression_evaluate() {
    // Expression::evaluate: the value of the first result expression
    let m = model_with_library(EXPRESSIONS);
    assert_eq!(
        d::evaluate(&m.db, m.project, m.get("e"), None),
        [Item::Integer(2)]
    );
}

#[test]
fn expression_model_level_evaluable() {
    // Only implied specializations, inputs without features or values, and evaluable result
    // expressions
    let m = model_with_library(EXPRESSIONS);
    let (db, p) = (&m.db, m.project);
    assert!(d::model_level_evaluable(db, p, m.get("e"), &[]));
    assert!(d::model_level_evaluable(db, p, m.get("e2"), &[]));
    assert!(
        !d::model_level_evaluable(db, p, m.get("e3"), &[]),
        "a written typing"
    );
}

#[test]
fn derive_expression_is_model_level_evaluable() {
    // isModelLevelEvaluable = modelLevelEvaluable(Set(Element){})
    let m = model_with_library(EXPRESSIONS);
    let (db, p) = (&m.db, m.project);
    assert!(d::is_model_level_evaluable(db, p, m.get("e")));
    assert!(!d::is_model_level_evaluable(db, p, m.get("e3")));
}

#[test]
fn expression_check_condition() {
    // checkCondition(target): it evaluates to the single value true
    let m = model_with_library(EXPRESSIONS);
    let (db, p) = (&m.db, m.project);
    assert!(d::check_condition(db, p, value(&m, "c"), None));
    assert!(
        !d::check_condition(db, p, value(&m, "c1"), None),
        "not a Boolean"
    );
    assert!(
        !d::check_condition(db, p, value(&m, "c2"), None),
        "two values"
    );
}

// ---- metadata ----

const METADATA: &str = "metaclass M { feature level : ScalarValues::Integer; }
    classifier X { @M { level = 3; } } classifier Y;
    feature m = X.metadata; feature k = X meta KerML::Classifier; feature j = X @@ M;";

#[test]
fn metadata_access_expression_evaluate() {
    // The element's metadata features that annotate it, then its reflective metadata
    let m = model_with_library(METADATA);
    assert_eq!(
        eval(&m, "m"),
        [
            Item::Element(m.get("X::MetadataFeature#1")),
            Item::Reflective(m.get("X"))
        ]
    );
    // `meta` and `@@` act on the metadata (7.4.9.2).
    assert_eq!(eval(&m, "k"), [Item::Reflective(m.get("X"))]);
    assert_eq!(eval(&m, "j"), [Item::Boolean(true)]);
}

#[test]
fn metadata_access_expression_metaclass_feature() {
    // metaclassFeature(): the reflective metadata of the referenced element
    let m = model_with_library(METADATA);
    let x = value(&m, "m");
    assert_eq!(
        d::metaclass_feature(&m.db, m.project, x),
        Some(Item::Reflective(m.get("X")))
    );
}

#[test]
fn metadata_access_expression_model_level_evaluable() {
    // MetadataAccessExpression::modelLevelEvaluable(visited) = true
    let m = model_with_library(METADATA);
    assert!(d::model_level_evaluable(
        &m.db,
        m.project,
        value(&m, "m"),
        &[]
    ));
}

#[test]
fn metadata_feature_syntax_element() {
    // syntaxElement(): the element reflective metadata represents
    let m = model_with_library(METADATA);
    assert_eq!(
        d::syntax_element(&Item::Reflective(m.get("X"))),
        Some(m.get("X"))
    );
    assert_eq!(
        d::syntax_element(&Item::Element(m.get("X::MetadataFeature#1"))),
        None
    );
}

#[test]
fn metadata_feature_evaluate_feature() {
    // evaluateFeature(baseFeature): the value of the feature that redefines it
    let m = model_with_library(METADATA);
    let (db, p) = (&m.db, m.project);
    let mf = m.get("X::MetadataFeature#1");
    assert_eq!(
        d::evaluate_feature(db, p, mf, m.get("M::level")),
        [Item::Integer(3)]
    );
    assert!(d::evaluate_feature(db, p, mf, m.get("X")).is_empty());
}

// ---- multiplicities ----

// A bound is a literal or a feature reference (KerML 8.2.5.11, MultiplicityExpressionMember).
const BOUNDS: &str = "feature n = 4; feature k[2..*]; feature j[n]; feature s[*]; feature o[3];
    feature minus = -1; feature neg[0..minus];";

fn range(m: &Model, f: &str) -> Global {
    m.get(&format!("{f}::MultiplicityRange#1"))
}

#[test]
fn multiplicity_range_value_of() {
    // valueOf(bound): an evaluable bound's single value, a natural number or *
    let m = model_with_library(BOUNDS);
    let (db, p) = (&m.db, m.project);
    let k = range(&m, "k");
    assert_eq!(
        d::value_of(db, p, d::lower_bound(db, p, k)),
        Some(Natural::Value(2))
    );
    assert_eq!(
        d::value_of(db, p, d::upper_bound(db, p, k)),
        Some(Natural::Infinite)
    );
    let j = d::upper_bound(db, p, range(&m, "j"));
    assert_eq!(d::value_of(db, p, j), Some(Natural::Value(4)));
    let neg = d::upper_bound(db, p, range(&m, "neg"));
    assert_eq!(d::value_of(db, p, neg), None, "negative");
    assert_eq!(d::value_of(db, p, None), None);
}

#[test]
fn multiplicity_range_has_bounds() {
    // hasBounds(lower, upper): `[n]` means n..n, and `[*]` 0..*
    let m = model_with_library(BOUNDS);
    let (db, p) = (&m.db, m.project);
    let (v, inf) = (Natural::Value, Natural::Infinite);
    assert!(d::has_bounds(db, p, range(&m, "k"), v(2), inf));
    assert!(d::has_bounds(db, p, range(&m, "o"), v(3), v(3)));
    assert!(d::has_bounds(db, p, range(&m, "s"), v(0), inf));
    assert!(!d::has_bounds(db, p, range(&m, "o"), v(0), v(3)));
}

// ---- packages and filters ----

const FILTERS: &str = "metaclass Approved { feature level : ScalarValues::Integer; }
    package D { class A { @Approved { level = 2; } } class B { @Approved { level = 0; } } class C; feature f; }
    package P { private import D::*; filter @Approved; feature x : A; feature y : C; }
    package Q { private import D::*[@Approved and Approved::level > 1]; feature x : A; feature y : B; }
    package R { private import D::*[@KerML::Class]; }";

#[test]
fn package_include_as_member() {
    // includeAsMember(element): every filter condition holds on some metadata of the element,
    // its reflective metadata included (7.4.14)
    let m = model_with_library(FILTERS);
    let (db, p) = (&m.db, m.project);
    let pkg = m.get("P");
    assert!(d::include_as_member(db, p, pkg, m.get("D::A")));
    assert!(!d::include_as_member(db, p, pkg, m.get("D::C")));
    let r = m.get("R::Package#1");
    assert!(
        d::include_as_member(db, p, r, m.get("D::C")),
        "every class is a KerML::Class"
    );
    assert!(!d::include_as_member(db, p, r, m.get("D::f")));
}

#[test]
fn package_imported_memberships() {
    // Package::importedMemberships: the imported memberships the filters include; resolution
    // sees them the same way
    let m = model_with_library(FILTERS);
    let (db, p) = (&m.db, m.project);
    let imported = m.labels(d::package_imported_memberships(db, p, m.get("P")));
    assert_eq!(
        imported,
        ["OwningMembership(D::A)", "OwningMembership(D::B)"]
    );
    let q = m.labels(d::package_imported_memberships(
        db,
        p,
        m.get("Q::Package#1"),
    ));
    assert_eq!(q, ["OwningMembership(D::A)"]);
    let typing = |f: &str| {
        let t = m.relationship(f, sysml_model::Metaclass::FeatureTyping, 1);
        m.labels(d::target(db, p, t))
    };
    assert_eq!(typing("P::x"), ["D::A"]);
    assert!(typing("P::y").is_empty(), "C has no Approved metadata");
    assert_eq!(typing("Q::x"), ["D::A"]);
    assert!(typing("Q::y").is_empty(), "B's level is 0");
}

// ---- regressions from review ----

#[test]
fn evaluation_stays_bounded_and_does_not_overflow() {
    // A range longer than MAX_RANGE, an index past usize, and integer overflow have no value;
    // an integer to a negative power is a Rational (RationalFunctions::'^').
    let m = model_with_library(
        "feature r = (1..9223372036854775807) # (1); feature x = (10, 20, 30) # (4294967297);
        feature y = (-9223372036854775807 - 1) % -1; feature z = -(-9223372036854775807 - 1);
        feature p = 2 ^ -1; feature q = (1..3) # (3);",
    );
    assert!(eval(&m, "r").is_empty());
    assert!(eval(&m, "x").is_empty());
    assert!(eval(&m, "y").is_empty());
    assert!(eval(&m, "z").is_empty());
    assert_eq!(eval(&m, "p"), [Item::Rational(0.5)]);
    assert_eq!(eval(&m, "q"), [Item::Integer(3)]);
}

#[test]
fn a_feature_of_metadata_has_no_value_on_other_metadata() {
    // `Approved::level` has no value on metadata that is not Approved (KerML 8.3.4.8.5
    // `else Sequence{}`), and `null != 0` holds (BaseFunctions '!=' is `not ==`), so on every
    // element's reflective metadata `Approved::level != 0` holds too: it includes every element.
    // Testing the metadata first, `@Approved and …`, includes only A.
    let m = model_with_library(
        "metaclass Approved { feature level : ScalarValues::Integer; }
        package D { class A { @Approved { level = 2; } } class B { @Approved { level = 0; } } class C; }
        package P { private import D::*; filter Approved::level != 0; }
        package Q { private import D::*; filter @Approved and Approved::level != 0; }
        feature v = Approved::level;",
    );
    let (db, p) = (&m.db, m.project);
    let reflective = Some(Item::Reflective(m.get("D::C")));
    assert!(d::evaluate(db, p, value(&m, "v"), reflective).is_empty());
    let imported = |pkg: &str| m.labels(d::package_imported_memberships(db, p, m.get(pkg)));
    assert_eq!(imported("P").len(), 3);
    assert_eq!(imported("Q"), ["OwningMembership(D::A)"]);
}

#[test]
fn metadata_and_feature_values_by_the_rules_of_8_4_4_9_8() {
    // Each metadata feature once; a target feature without a value gives the referent; a feature
    // that subsets self is the target; enumeration values are the library's literals.
    let m = model_with_library(
        "metaclass M1; metaclass M2; classifier A; classifier B; #M1 #M2 dependency X from A to B;
        feature mx = X.metadata;
        classifier Car { feature engine; } classifier Car2 specializes Car { feature redefines engine; }
        feature e = Car::engine;
        classifier K specializes Base::Anything { feature me subsets self; feature k = me; }
        classifier C { in feature i; }
        feature t = (C::i meta KerML::Feature).direction == KerML::FeatureDirectionKind::'in';",
    );
    let (db, p) = (&m.db, m.project);
    let metadata = eval(&m, "mx");
    assert_eq!(
        metadata.len(),
        3,
        "M1, M2 and the reflective metadata, once each: {metadata:?}"
    );
    let on = |f: &str, t: &str| d::evaluate(db, p, value(&m, f), Some(Item::Element(m.get(t))));
    assert_eq!(on("e", "Car2"), [Item::Element(m.get("Car::engine"))]);
    let k = m.get("K::k");
    let kv = value_in(&m, k);
    assert_eq!(
        d::evaluate(db, p, kv, Some(Item::Element(m.get("A")))),
        [Item::Element(m.get("A"))]
    );
    assert_eq!(eval(&m, "t"), [Item::Boolean(true)]);
}

/// The value expression of a feature anywhere.
fn value_in(m: &Model, f: Global) -> Global {
    let l = sysml_semantics::lowered(&m.db, f.file).clone();
    let g = &l.graph;
    let r = g
        .owned_relationships(f.ix())
        .iter()
        .find(|r| g.metaclass(**r) == sysml_model::Metaclass::FeatureValue)
        .unwrap();
    Global {
        file: f.file,
        index: g.owned_related_elements(*r)[0].index() as u32,
    }
}

#[test]
fn a_package_filter_filters_the_names_its_conditions_use() {
    // KerML 7.4.14: "a filter condition in a package will filter all imports of that Package.
    // That is why full qualification is used …, since imported elements … would be filtered out
    // by the very filter condition in which the elements are intended to be used". Unqualified,
    // `Approved` comes through P's own import, which its filter removes; qualified, it resolves.
    let m = model_with_library(
        "package Lib { metaclass Approved; } package D { class A { @Lib::Approved; } class B; }
        package P { private import Lib::*; private import D::*; filter @Approved; feature x : A; }
        package Q { private import D::*; filter @Lib::Approved; feature x : A; feature y : B; }",
    );
    let (db, p) = (&m.db, m.project);
    let typing = |f: &str| {
        let t = m.relationship(f, sysml_model::Metaclass::FeatureTyping, 1);
        m.labels(d::target(db, p, t))
    };
    assert!(typing("P::x").is_empty(), "as 7.4.14 warns");
    assert_eq!(typing("Q::x"), ["D::A"]);
    assert!(typing("Q::y").is_empty());
}

#[test]
fn a_package_filter_selects_after_the_exclusion_of_collisions() {
    // KerML 8.3.4.13.4: the filter selects from Namespace::importedMemberships, which has already
    // excluded the two colliding X, the one with the metadata too.
    let m = model_with_library(
        "package Lib { metaclass Approved; }
        package A1 { class X { @Lib::Approved; } } package B1 { class X; }
        package P { private import A1::*; private import B1::*; filter @Lib::Approved; feature x : X; }",
    );
    let (db, p) = (&m.db, m.project);
    let t = m.relationship("P::x", sysml_model::Metaclass::FeatureTyping, 1);
    assert!(d::target(db, p, t).is_empty());
}

#[test]
fn evaluation_has_a_step_budget() {
    // Nested ranges and doubling sequences exceed MAX_STEPS and have no value, quickly.
    let m = model_with_library(
        "feature a = 1..99999; feature b = a->collect{in i : ScalarValues::Integer; 1..99999};
        feature c1 = (a, a); feature c2 = (c1, c1); feature c3 = (c2, c2); feature c4 = (c3, c3);
        feature c5 = (c4, c4); feature c6 = (c5, c5); feature c7 = (c6, c6); feature c8 = (c7, c7);",
    );
    assert!(eval(&m, "b").is_empty());
    assert!(eval(&m, "c8").is_empty());
    assert_eq!(eval(&m, "a").len(), 99_999);
}

#[test]
fn integers_beyond_two_to_the_fifty_third_compare_as_integers() {
    let m = model_with_library(
        "feature e = 9007199254740993 == 9007199254740992; feature g = 9007199254740993 > 9007199254740992;
        feature r = 2 == 2.0;",
    );
    assert_eq!(eval(&m, "e"), [Item::Boolean(false)]);
    assert_eq!(eval(&m, "g"), [Item::Boolean(true)]);
    assert_eq!(eval(&m, "r"), [Item::Boolean(true)]);
}

#[test]
fn a_users_metadata_feature_is_not_a_mof_property() {
    // `Tag::name` is a feature of the user's metaclass Tag, not KerML's `name`: on the reflective
    // metadata of an element without Tag, it has no value.
    let m = model_with_library(
        "metaclass Tag { feature name : ScalarValues::String; }
        package D { class A; } package P { private import D::*; filter Tag::name == \"A\"; }
        feature probe = Tag::name;",
    );
    let (db, p) = (&m.db, m.project);
    let on_a = d::evaluate(
        db,
        p,
        value(&m, "probe"),
        Some(Item::Reflective(m.get("D::A"))),
    );
    assert!(on_a.is_empty());
    assert!(d::package_imported_memberships(db, p, m.get("P")).is_empty());
}

#[test]
fn operands_keep_their_positions_and_types_are_the_written_ones() {
    // A type operand is the type its TypeReference names; an unresolved one keeps its place and
    // gives no value; a reference without a referent is not evaluable; NaN is no value.
    let m = model_with_library(
        r#"feature s = "x" istype ScalarValues::String; feature n = 3 istype ScalarValues::String;
        feature u = 3 istype Undefined; feature w = undefinedName; feature v = (-8.0) ^ 0.5;"#,
    );
    let (db, p) = (&m.db, m.project);
    assert_eq!(eval(&m, "s"), [Item::Boolean(true)]);
    assert_eq!(eval(&m, "n"), [Item::Boolean(false)]);
    assert!(eval(&m, "u").is_empty());
    assert!(!d::is_model_level_evaluable(db, p, value(&m, "w")));
    assert!(eval(&m, "v").is_empty());
}
