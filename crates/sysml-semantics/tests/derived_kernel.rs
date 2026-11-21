// SPDX-License-Identifier: Apache-2.0
//! Derived properties and operations of the Kernel layer (KerML 8.3.4; step 1a.8, ADR 0021): one
//! test per `derive…` rule and operation, from its OCL, named after it
//! (`tests/derived_inventory.rs` checks). Model-level evaluation follows in its own change.

mod common;

use common::{model, model_with_library, Model};
use sysml_model::Metaclass;
use sysml_semantics::derived::{self as d, Direction};
use sysml_semantics::Global;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

fn names<T: std::borrow::Borrow<Global>>(
    m: &Model,
    xs: impl IntoIterator<Item = T>,
) -> Vec<String> {
    m.labels(xs)
}

// ---- Behaviors (KerML 8.3.4.6) ----

#[test]
fn derive_behavior_step() {
    // step = feature->selectByKind(Step)
    let m = model("behavior B { step s; feature f; }");
    assert_eq!(names(&m, d::step(&m.db, m.project, m.get("B"))), ["B::s"]);
}

#[test]
fn derive_step_behavior() {
    // behavior = type->selectByKind(Behavior)
    let m = model("behavior B; classifier C; step s : B; step t : C;");
    let (db, p) = (&m.db, m.project);
    assert_eq!(names(&m, d::behavior(db, p, m.get("s"))), ["B"]);
    assert!(d::behavior(db, p, m.get("t")).is_empty());
}

const INVOKE: &str = "function f { in x; in y; return r; }
    feature v = f(1, y = 2); feature u = f(y = 5, x = 6);";

#[test]
fn parameter_membership_parameter_direction() {
    // ParameterMembership::parameterDirection() = in
    let m = model(INVOKE);
    let pm = m.relationship(
        "v::InvocationExpression#1",
        Metaclass::ParameterMembership,
        1,
    );
    assert_eq!(d::parameter_direction(&m.db, m.project, pm), Direction::In);
}

#[test]
fn return_parameter_membership_parameter_direction() {
    // ReturnParameterMembership::parameterDirection() = out
    let m = model(INVOKE);
    let rpm = m.relationship("f", Metaclass::ReturnParameterMembership, 1);
    assert_eq!(
        d::parameter_direction(&m.db, m.project, rpm),
        Direction::Out
    );
}

// ---- Functions and expressions (KerML 8.3.4.7, 8.3.4.8) ----

#[test]
fn derive_function_result() {
    // result: the parameter of the first ReturnParameterMembership, owned or inherited
    let m = model("function f { in x; return r; } function g specializes f; function h;");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.label_of(d::result(db, p, m.get("f"))),
        Some("f::r".into())
    );
    assert_eq!(
        m.label_of(d::result(db, p, m.get("g"))),
        Some("f::r".into())
    );
    assert_eq!(d::result(db, p, m.get("h")), None);
}

#[test]
fn derive_expression_result() {
    // Expression::result, likewise
    let m = model("function f { return r; } feature v = f(); expr e { return q; }");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.label_of(d::result(db, p, m.get("e"))),
        Some("e::q".into())
    );
    let invocation = m.get("v::InvocationExpression#1");
    let own = m.get("v::InvocationExpression#1::Feature#1");
    assert_eq!(d::result(db, p, invocation), Some(own));
}

#[test]
fn derive_instantiation_expression_instantiated_type() {
    // instantiatedType = instantiatedType()
    let m = model("classifier T { feature a; } function f { return r; } feature v = f(); feature w = new T();");
    let (db, p) = (&m.db, m.project);
    let i = m.get("v::InvocationExpression#1");
    assert_eq!(m.label_of(d::instantiated_type(db, p, i)), Some("f".into()));
    let c = m.get("w::ConstructorExpression#1");
    assert_eq!(m.label_of(d::instantiated_type(db, p, c)), Some("T".into()));
}

#[test]
fn instantiation_expression_instantiated_type() {
    // instantiatedType(): the member element of the first owned membership that is not a
    // FeatureMembership, if it is a Type
    let m = model("package P; function f { return r; } feature v = f(); feature w = P();");
    let (db, p) = (&m.db, m.project);
    let i = m.get("v::InvocationExpression#1");
    assert_eq!(m.label_of(d::instantiated_type(db, p, i)), Some("f".into()));
    let not_a_type = m.get("w::InvocationExpression#1");
    assert_eq!(d::instantiated_type(db, p, not_a_type), None);
}

#[test]
fn operator_expression_instantiated_type() {
    // OperatorExpression::instantiatedType(): the operator in BaseFunctions, DataFunctions or
    // ControlFunctions
    let m = model_with_library("feature s = 1 + 2; feature t = true and false;");
    let (db, p) = (&m.db, m.project);
    let plus = m.get("s::OperatorExpression#1");
    assert_eq!(
        m.label_of(d::instantiated_type(db, p, plus)),
        Some("DataFunctions::+".into())
    );
    let and = m.get("t::OperatorExpression#1");
    assert_eq!(
        m.label_of(d::instantiated_type(db, p, and)),
        Some("ControlFunctions::and".into())
    );
}

#[test]
fn derive_invocation_expression_argument() {
    // argument: for each input of the instantiated type, the value of the owned feature that
    // redefines it, by position or by name
    let m = model(INVOKE);
    let (db, p) = (&m.db, m.project);
    let v = "v::InvocationExpression#1";
    assert_eq!(
        d::argument(db, p, m.get(v)),
        [
            m.get(&format!("{v}::Feature#1::LiteralInteger#1")),
            m.get(&format!("{v}::Feature#2::LiteralInteger#1"))
        ]
    );
    let u = "u::InvocationExpression#1";
    assert_eq!(
        d::argument(db, p, m.get(u)),
        [
            m.get(&format!("{u}::Feature#2::LiteralInteger#1")),
            m.get(&format!("{u}::Feature#1::LiteralInteger#1"))
        ],
        "in the order of the inputs, x then y"
    );
}

#[test]
fn derive_constructor_expression_argument() {
    // argument: for each feature of the instantiated type, the value of the result's owned
    // feature that redefines it
    let m = model("classifier T { feature a; feature b; } feature w = new T(b = 4, a = 3);");
    let (db, p) = (&m.db, m.project);
    let c = "w::ConstructorExpression#1";
    assert_eq!(
        d::constructor_argument(db, p, m.get(c)),
        [
            m.get(&format!("{c}::Feature#1::Feature#2::LiteralInteger#1")),
            m.get(&format!("{c}::Feature#1::Feature#1::LiteralInteger#1"))
        ]
    );
}

#[test]
fn derive_feature_reference_expression_referent() {
    // referent: the member element of the first owned membership that is not a
    // ParameterMembership, if it is a Feature
    let m = model("feature f; feature g = f;");
    let r = d::referent(&m.db, m.project, m.get("g::FeatureReferenceExpression#1"));
    assert_eq!(m.label_of(r), Some("f".into()));
}

#[test]
fn derive_feature_chain_expression_target_feature() {
    // targetFeature, likewise
    let m = model("feature a { feature b; } feature c = a.b;");
    let t = d::chain_target_feature(&m.db, m.project, m.get("c::FeatureChainExpression#1"));
    assert_eq!(m.label_of(t), Some("a::b".into()));
}

#[test]
fn feature_chain_expression_source_target_feature() {
    // sourceTargetFeature(): the first owned feature of the first input parameter
    let m = model("feature a { feature b; } feature c = a.b;");
    let chain = m.get("c::FeatureChainExpression#1");
    let expected = m.get("c::FeatureChainExpression#1::Feature#1::Feature#1");
    assert_eq!(
        d::source_target_feature(&m.db, m.project, chain),
        Some(expected)
    );
}

#[test]
fn derive_metadata_access_expression_referencd_element() {
    // referencedElement: the member element of the first owned membership that is not a
    // FeatureMembership
    let m = model("classifier T; feature m = T.metadata;");
    let x = m.get("m::MetadataAccessExpression#1");
    assert_eq!(
        m.label_of(d::referenced_element(&m.db, m.project, x)),
        Some("T".into())
    );
}

// ---- Connectors and associations (KerML 8.3.4.4, 8.3.4.5) ----

const CONNECT: &str =
    "feature v; feature w; feature u; connector c from v to w; connector n (v, w, u);";

#[test]
fn derive_connector_related_feature() {
    // relatedFeature: the features the ends reference-subset
    let m = model(CONNECT);
    let (db, p) = (&m.db, m.project);
    assert_eq!(names(&m, d::related_feature(db, p, m.get("c"))), ["v", "w"]);
    assert_eq!(
        names(&m, d::related_feature(db, p, m.get("n"))),
        ["v", "w", "u"]
    );
}

#[test]
fn derive_connector_source_feature() {
    // sourceFeature: the first related feature
    let m = model(CONNECT);
    assert_eq!(
        m.label_of(d::source_feature(&m.db, m.project, m.get("c"))),
        Some("v".into())
    );
}

#[test]
fn derive_connector_target_feature() {
    // targetFeature: the related features after the first
    let m = model(CONNECT);
    let (db, p) = (&m.db, m.project);
    assert_eq!(names(&m, d::target_feature(db, p, m.get("c"))), ["w"]);
    assert_eq!(names(&m, d::target_feature(db, p, m.get("n"))), ["w", "u"]);
}

#[test]
fn derive_connector_default_featuring_type() {
    // defaultFeaturingType: the nearest type within which every related feature is featured
    let m = model(
        "classifier A { feature v; feature w; connector c from v to w; }
        classifier B { feature x { feature y; feature y2; connector c2 from y to y2; } feature z; }
        classifier C { feature p { feature q; } feature r; connector c3 from p.q to r; }
        feature v; feature w; connector c from v to w;",
    );
    let (db, p) = (&m.db, m.project);
    let at = |path: &str| m.label_of(d::default_featuring_type(db, p, m.get(path)));
    assert_eq!(at("A::c"), Some("A".into()));
    // The nearest: x features y and y2, and B features x.
    assert_eq!(at("B::x::c2"), Some("B::x".into()));
    // A chain is featured by the featuring types of its first chaining feature.
    assert_eq!(at("C::c3"), Some("C".into()));
    // Top-level features have no featuring type, so no common one.
    assert_eq!(at("c"), None);
}

const ASSOC: &str = "classifier T; classifier U; assoc A { end feature s : T; end feature t : U; }";

#[test]
fn derive_association_related_type() {
    // relatedType = associationEnd.type
    let m = model(ASSOC);
    assert_eq!(
        names(&m, d::related_type(&m.db, m.project, m.get("A"))),
        ["T", "U"]
    );
}

#[test]
fn derive_association_source_type() {
    // sourceType: the first related type
    let m = model(ASSOC);
    assert_eq!(
        m.label_of(d::source_type(&m.db, m.project, m.get("A"))),
        Some("T".into())
    );
}

#[test]
fn derive_association_target_type() {
    // targetType: the related types after the first
    let m = model(ASSOC);
    assert_eq!(
        names(&m, d::target_type(&m.db, m.project, m.get("A"))),
        ["U"]
    );
}

// ---- Flows (KerML 8.3.4.9) ----

const FLOW: &str = "classifier T; feature v { feature o; } feature w { feature i; }
    flow fl of T from v.o to w.i;";

#[test]
fn derive_flow_flow_end() {
    // flowEnd = connectorEnd->selectByKind(FlowEnd)
    let m = model(FLOW);
    let ends = d::flow_end(&m.db, m.project, m.get("fl"));
    assert_eq!(ends, [m.get("fl::FlowEnd#1"), m.get("fl::FlowEnd#2")]);
}

#[test]
fn derive_flow_payload_feature() {
    // payloadFeature: the first owned PayloadFeature
    let m = model(FLOW);
    let x = d::payload_feature(&m.db, m.project, m.get("fl"));
    assert_eq!(x, Some(m.get("fl::PayloadFeature#1")));
}

#[test]
fn derive_flow_payload_type() {
    // payloadType = payloadFeature.type
    let m = model(FLOW);
    assert_eq!(
        names(&m, d::payload_type(&m.db, m.project, m.get("fl"))),
        ["T"]
    );
}

#[test]
fn derive_flow_source_output_feature() {
    // sourceOutputFeature: the first owned feature of the first end
    let m = model(FLOW);
    let x = d::source_output_feature(&m.db, m.project, m.get("fl"));
    assert_eq!(x, Some(m.get("fl::FlowEnd#1::Feature#1")));
}

#[test]
fn derive_flow_target_input_feature() {
    // targetInputFeature: the first owned feature of the second end
    let m = model(FLOW);
    let x = d::target_input_feature(&m.db, m.project, m.get("fl"));
    assert_eq!(x, Some(m.get("fl::FlowEnd#2::Feature#1")));
}

// ---- Multiplicities (KerML 8.3.4.11) ----

const RANGES: &str = "feature k[1..3]; feature j[2];";

#[test]
fn derive_multiplicity_range_lower_bound() {
    // lowerBound: the first of two owned expressions
    let m = model(RANGES);
    let (db, p) = (&m.db, m.project);
    let k = m.get("k::MultiplicityRange#1");
    assert_eq!(
        d::lower_bound(db, p, k),
        Some(m.get("k::MultiplicityRange#1::LiteralInteger#1"))
    );
    assert_eq!(d::lower_bound(db, p, m.get("j::MultiplicityRange#1")), None);
}

#[test]
fn derive_multiplicity_range_upper_bound() {
    // upperBound: the only owned expression, or the second
    let m = model(RANGES);
    let (db, p) = (&m.db, m.project);
    let k = m.get("k::MultiplicityRange#1");
    assert_eq!(
        d::upper_bound(db, p, k),
        Some(m.get("k::MultiplicityRange#1::LiteralInteger#2"))
    );
    let j = m.get("j::MultiplicityRange#1");
    assert_eq!(
        d::upper_bound(db, p, j),
        Some(m.get("j::MultiplicityRange#1::LiteralInteger#1"))
    );
}

#[test]
fn derive_multiplicity_range_bound() {
    // bound: the lower bound, if any, then the upper bound
    let m = model(RANGES);
    let (db, p) = (&m.db, m.project);
    let k = "k::MultiplicityRange#1";
    assert_eq!(
        d::bound(db, p, m.get(k)),
        [
            m.get(&format!("{k}::LiteralInteger#1")),
            m.get(&format!("{k}::LiteralInteger#2"))
        ]
    );
    let j = "j::MultiplicityRange#1";
    assert_eq!(
        d::bound(db, p, m.get(j)),
        [m.get(&format!("{j}::LiteralInteger#1"))]
    );
}

// ---- Metadata (KerML 8.3.4.12) ----

#[test]
fn derive_metadata_feature_metaclass() {
    // metaclass: the first of the metadata feature's types that is a Metaclass
    let m = model("metaclass M; classifier X { @M; }");
    let mf = m.get("X::MetadataFeature#1");
    assert_eq!(
        m.label_of(d::metaclass_of(&m.db, m.project, mf)),
        Some("M".into())
    );
}

const METADATA: &str = "metaclass S specializes Metaobjects::SemanticMetadata; metaclass M;
    classifier X { @S; } classifier Y { @M; } classifier Z { @KerML::Classifier; }";

#[test]
fn metadata_feature_is_semantic() {
    // isSemantic() = specializesFromLibrary('Metaobjects::SemanticMetadata')
    let m = model_with_library(METADATA);
    let (db, p) = (&m.db, m.project);
    assert!(d::is_semantic(db, p, m.get("X::MetadataFeature#1")));
    assert!(!d::is_semantic(db, p, m.get("Y::MetadataFeature#1")));
}

#[test]
fn metadata_feature_is_syntactic() {
    // isSyntactic() = specializesFromLibrary('KerML::Element')
    let m = model_with_library(METADATA);
    let (db, p) = (&m.db, m.project);
    assert!(d::is_syntactic(db, p, m.get("Z::MetadataFeature#1")));
    assert!(!d::is_syntactic(db, p, m.get("Y::MetadataFeature#1")));
}

// ---- Packages (KerML 8.3.4.13) ----

#[test]
fn derive_package_filter_condition() {
    // filterCondition = ownedMembership->selectByKind(ElementFilterMembership).condition
    let m = model("package Q; package P { import Q::*[true]; }");
    let filter = m.get("P::Package#1");
    let conditions = d::filter_condition(&m.db, m.project, filter);
    assert_eq!(conditions, [m.get("P::Package#1::LiteralBoolean#1")]);
}
