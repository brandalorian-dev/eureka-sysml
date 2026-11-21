// SPDX-License-Identifier: Apache-2.0
//! The Kernel layer (KerML 8.3.4): behaviors, functions and expressions, connectors and flows,
//! associations, multiplicities, metadata and packages. Model-level evaluation (8.4.4.9.8)
//! follows in its own change.

use sysml_model::{Metaclass as M, Slot};

use super::root::{membership_element, owned_member, owned_membership};
use super::types::{
    end_feature, feature, feature_membership, featuring_type, is_featured_within, owned_feature,
    owned_reference_subsetting, r#type, redefines, specializes_from_library, Direction,
};
use super::{at, end, metaclass, unique};
use crate::resolve::{structure, Global, Project};

fn kind_of(db: &dyn salsa::Database, x: Global, m: M) -> bool {
    metaclass(db, x).specializes(m)
}

fn owned_element_of(db: &dyn salsa::Database, rel: Global) -> Option<Global> {
    let s = structure(db, rel.file);
    s.graph()
        .owned_related_elements(rel.ix())
        .first()
        .map(|e| at(rel.file, *e))
}

/// The value Expression of a feature's FeatureValue (`valuation.value`, KerML 8.3.4.10), if any.
fn valuation_value(db: &dyn salsa::Database, f: Global) -> Option<Global> {
    let s = structure(db, f.file);
    let g = s.graph();
    g.all_owned_relationships(f.ix())
        .find(|r| g.metaclass(*r).specializes(M::FeatureValue))
        .and_then(|r| owned_element_of(db, at(f.file, r)))
}

/// The member element of the first owned membership that is not of a kind.
fn first_member_not(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
    not: M,
) -> Option<Global> {
    owned_membership(db, project, x)
        .into_iter()
        .find(|m| !kind_of(db, *m, not))
        .and_then(|m| membership_element(db, project, m))
}

// ---- Behaviors (KerML 8.3.4.6) ----

/// `step` (deriveBehaviorStep): `feature->selectByKind(Step)`.
pub fn step(db: &dyn salsa::Database, project: Project, b: Global) -> Vec<Global> {
    let fs = feature(db, project, b);
    fs.into_iter()
        .filter(|f| kind_of(db, *f, M::Step))
        .collect()
}

/// `behavior` (deriveStepBehavior): `type->selectByKind(Behavior)`.
pub fn behavior(db: &dyn salsa::Database, project: Project, s: Global) -> Vec<Global> {
    let ts = r#type(db, project, s);
    ts.into_iter()
        .filter(|t| kind_of(db, *t, M::Behavior))
        .collect()
}

/// `parameterDirection()`: `in` for a ParameterMembership (KerML 8.3.4.6.4), `out` for a
/// ReturnParameterMembership (8.3.4.7.8).
pub fn parameter_direction(db: &dyn salsa::Database, _project: Project, m: Global) -> Direction {
    if kind_of(db, m, M::ReturnParameterMembership) {
        Direction::Out
    } else {
        Direction::In
    }
}

// ---- Functions and expressions (KerML 8.3.4.7, 8.3.4.8) ----

/// `result` of a Function (deriveFunctionResult) or an Expression (deriveExpressionResult): the
/// parameter of its first ReturnParameterMembership, owned or inherited.
pub fn result(db: &dyn salsa::Database, project: Project, x: Global) -> Option<Global> {
    feature_membership(db, project, x)
        .into_iter()
        .find(|m| kind_of(db, *m, M::ReturnParameterMembership))
        .and_then(|m| owned_element_of(db, m))
}

/// `instantiatedType()`: for an InstantiationExpression (KerML 8.3.4.8.7), the member element of
/// its first owned membership that is not a FeatureMembership, if it is a Type; for an
/// OperatorExpression (8.3.4.8.17), its operator in BaseFunctions, DataFunctions or
/// ControlFunctions. The OCL's `typeMembers` is read as `members` (ADR 0021).
pub fn instantiated_type(db: &dyn salsa::Database, project: Project, x: Global) -> Option<Global> {
    crate::implied::instantiated_type(db, project, x)
}

/// `argument` of an InvocationExpression (deriveInvocationExpressionArgument): for each input of
/// the instantiated type, the value of the owned feature that redefines it.
pub fn argument(db: &dyn salsa::Database, project: Project, x: Global) -> Vec<Global> {
    let Some(t) = instantiated_type(db, project, x) else {
        return Vec::new();
    };
    let owned = owned_feature(db, project, x);
    let inputs = super::types::input(db, project, t);
    inputs
        .into_iter()
        .flat_map(|inp| {
            owned
                .iter()
                .filter(|f| redefines(db, project, **f, inp))
                .filter_map(|f| valuation_value(db, *f))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// `argument` of a ConstructorExpression (deriveConstructorExpressionArgument): for each feature
/// of the instantiated type, the value of the result's owned feature that redefines it.
pub fn constructor_argument(db: &dyn salsa::Database, project: Project, x: Global) -> Vec<Global> {
    let (Some(t), Some(r)) = (instantiated_type(db, project, x), result(db, project, x)) else {
        return Vec::new();
    };
    let owned = owned_feature(db, project, r);
    feature(db, project, t)
        .into_iter()
        .flat_map(|f| {
            owned
                .iter()
                .filter(|o| redefines(db, project, **o, f))
                .filter_map(|o| valuation_value(db, *o))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// `referent` (deriveFeatureReferenceExpressionReferent) and `targetFeature`
/// (deriveFeatureChainExpressionTargetFeature): the member element of the first owned membership
/// that is not a ParameterMembership, if it is a Feature.
pub fn referent(db: &dyn salsa::Database, project: Project, x: Global) -> Option<Global> {
    first_member_not(db, project, x, M::ParameterMembership).filter(|e| kind_of(db, *e, M::Feature))
}

/// `targetFeature` of a FeatureChainExpression (deriveFeatureChainExpressionTargetFeature): the
/// member element of the first owned membership that is not a ParameterMembership, if it is a
/// Feature, as for `referent`.
pub fn chain_target_feature(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> Option<Global> {
    referent(db, project, x)
}

/// `sourceTargetFeature()` (KerML 8.3.4.8.4): the first owned feature of the first input
/// parameter.
pub fn source_target_feature(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> Option<Global> {
    let s = structure(db, x.file);
    let g = s.graph();
    let input = owned_feature(db, project, x).into_iter().find(|f| {
        matches!(
            g.values(f.ix(), sysml_model::Attr::Direction).next(),
            Some(sysml_model::Value::Enumeration("in"))
        )
    })?;
    owned_feature(db, project, input).first().copied()
}

/// `referencedElement` (deriveMetadataAccessExpressionReferencdElement): the member element of
/// the first owned membership that is not a FeatureMembership.
pub fn referenced_element(db: &dyn salsa::Database, project: Project, x: Global) -> Option<Global> {
    first_member_not(db, project, x, M::FeatureMembership)
}

// ---- Connectors and associations (KerML 8.3.4.4, 8.3.4.5) ----

/// `relatedFeature` (deriveConnectorRelatedFeature): the features the connector's ends
/// reference-subset.
pub fn related_feature(db: &dyn salsa::Database, project: Project, c: Global) -> Vec<Global> {
    end_feature(db, project, c)
        .into_iter()
        .filter_map(|e| owned_reference_subsetting(db, project, e))
        .filter_map(|r| end(db, project, r, Slot::Target, 0))
        .collect()
}

/// `sourceFeature` (deriveConnectorSourceFeature): the first related feature.
pub fn source_feature(db: &dyn salsa::Database, project: Project, c: Global) -> Option<Global> {
    related_feature(db, project, c).first().copied()
}

/// `targetFeature` (deriveConnectorTargetFeature): the related features after the first.
pub fn target_feature(db: &dyn salsa::Database, project: Project, c: Global) -> Vec<Global> {
    unique(related_feature(db, project, c).into_iter().skip(1))
}

/// The featuring types of the features, and theirs, transitively: not the features themselves,
/// unless one is a featuring type of another.
fn featuring_closure(db: &dyn salsa::Database, project: Project, xs: &[Global]) -> Vec<Global> {
    let mut out: Vec<Global> = Vec::new();
    let mut frontier: Vec<Global> = xs.to_vec();
    while let Some(x) = frontier.pop() {
        if !kind_of(db, x, M::Feature) {
            continue;
        }
        for t in featuring_type(db, project, x) {
            if !out.contains(t) {
                out.push(*t);
                frontier.push(*t);
            }
        }
    }
    out
}

/// `defaultFeaturingType` (deriveConnectorDefaultFeaturingType): of the featuring types of the
/// related features, transitively, those within which every related feature is featured, and of
/// these the first that is not a featuring type, transitively, of another. A related feature is
/// a candidate only as the featuring type of another (ADR 0021).
pub fn default_featuring_type(
    db: &dyn salsa::Database,
    project: Project,
    c: Global,
) -> Option<Global> {
    let related = related_feature(db, project, c);
    let common: Vec<Global> = featuring_closure(db, project, &related)
        .into_iter()
        .filter(|t| {
            related
                .iter()
                .all(|f| is_featured_within(db, project, *f, Some(*t)))
        })
        .collect();
    common.iter().copied().find(|t1| {
        !common
            .iter()
            .any(|t2| t2 != t1 && featuring_closure(db, project, &[*t2]).contains(t1))
    })
}

/// `relatedType` (deriveAssociationRelatedType): `associationEnd.type`, the types of the end
/// features in order.
pub fn related_type(db: &dyn salsa::Database, project: Project, a: Global) -> Vec<Global> {
    end_feature(db, project, a)
        .into_iter()
        .flat_map(|e| r#type(db, project, e))
        .collect()
}

/// `sourceType` (deriveAssociationSourceType): the first related type.
pub fn source_type(db: &dyn salsa::Database, project: Project, a: Global) -> Option<Global> {
    related_type(db, project, a).first().copied()
}

/// `targetType` (deriveAssociationTargetType): the related types after the first.
pub fn target_type(db: &dyn salsa::Database, project: Project, a: Global) -> Vec<Global> {
    unique(related_type(db, project, a).into_iter().skip(1))
}

// ---- Flows (KerML 8.3.4.9) ----

/// `flowEnd` (deriveFlowFlowEnd): `connectorEnd->selectByKind(FlowEnd)`.
pub fn flow_end(db: &dyn salsa::Database, project: Project, f: Global) -> Vec<Global> {
    let ends = end_feature(db, project, f);
    ends.into_iter()
        .filter(|e| kind_of(db, *e, M::FlowEnd))
        .collect()
}

/// `payloadFeature` (deriveFlowPayloadFeature): the first owned PayloadFeature.
pub fn payload_feature(db: &dyn salsa::Database, project: Project, f: Global) -> Option<Global> {
    owned_feature(db, project, f)
        .into_iter()
        .find(|x| kind_of(db, *x, M::PayloadFeature))
}

/// `payloadType` (deriveFlowPayloadType): the types of the payload feature.
pub fn payload_type(db: &dyn salsa::Database, project: Project, f: Global) -> Vec<Global> {
    payload_feature(db, project, f).map_or_else(Vec::new, |p| r#type(db, project, p))
}

/// `sourceOutputFeature` (deriveFlowSourceOutputFeature): the first owned feature of the first
/// end. The OCL's `connectorEnd.ownedFeature->first()` is read as the first end's, as
/// `targetInputFeature` reads the second's (ADR 0021).
pub fn source_output_feature(
    db: &dyn salsa::Database,
    project: Project,
    f: Global,
) -> Option<Global> {
    let first = *end_feature(db, project, f).first()?;
    owned_feature(db, project, first).first().copied()
}

/// `targetInputFeature` (deriveFlowTargetInputFeature): the first owned feature of the second
/// end.
pub fn target_input_feature(
    db: &dyn salsa::Database,
    project: Project,
    f: Global,
) -> Option<Global> {
    let second = *end_feature(db, project, f).get(1)?;
    owned_feature(db, project, second).first().copied()
}

// ---- Multiplicities (KerML 8.3.4.11) ----

fn owned_expressions(db: &dyn salsa::Database, project: Project, m: Global) -> Vec<Global> {
    owned_member(db, project, m)
        .into_iter()
        .filter(|e| kind_of(db, *e, M::Expression))
        .collect()
}

/// `lowerBound` (deriveMultiplicityRangeLowerBound): the first of two owned expressions.
pub fn lower_bound(db: &dyn salsa::Database, project: Project, m: Global) -> Option<Global> {
    let es = owned_expressions(db, project, m);
    (es.len() >= 2).then(|| es[0])
}

/// `upperBound` (deriveMultiplicityRangeUpperBound): the only owned expression, or the second.
pub fn upper_bound(db: &dyn salsa::Database, project: Project, m: Global) -> Option<Global> {
    let es = owned_expressions(db, project, m);
    match es.len() {
        0 => None,
        1 => Some(es[0]),
        _ => Some(es[1]),
    }
}

/// `bound` (deriveMultiplicityRangeBound): the lower bound, if any, and the upper bound.
pub fn bound(db: &dyn salsa::Database, project: Project, m: Global) -> Vec<Global> {
    let Some(upper) = upper_bound(db, project, m) else {
        return Vec::new();
    };
    lower_bound(db, project, m)
        .into_iter()
        .chain([upper])
        .collect()
}

// ---- Metadata (KerML 8.3.4.12) ----

/// `metaclass` (deriveMetadataFeatureMetaclass): the first of its types that is a Metaclass.
/// The OCL's `metaClassTypes` is read as `metaclassTypes`.
pub fn metaclass_of(db: &dyn salsa::Database, project: Project, mf: Global) -> Option<Global> {
    r#type(db, project, mf)
        .into_iter()
        .find(|t| kind_of(db, *t, M::Metaclass))
}

/// `isSemantic()` (KerML 8.3.4.12.3): `specializesFromLibrary('Metaobjects::SemanticMetadata')`.
pub fn is_semantic(db: &dyn salsa::Database, project: Project, mf: Global) -> bool {
    specializes_from_library(db, project, mf, "Metaobjects::SemanticMetadata")
}

/// `isSyntactic()` (KerML 8.3.4.12.3): `specializesFromLibrary('KerML::Element')`.
pub fn is_syntactic(db: &dyn salsa::Database, project: Project, mf: Global) -> bool {
    specializes_from_library(db, project, mf, "KerML::Element")
}

// ---- Packages (KerML 8.3.4.13) ----

/// `filterCondition` (derivePackageFilterCondition): the conditions of the owned
/// ElementFilterMemberships.
pub fn filter_condition(db: &dyn salsa::Database, project: Project, p: Global) -> Vec<Global> {
    owned_membership(db, project, p)
        .into_iter()
        .filter(|m| kind_of(db, *m, M::ElementFilterMembership))
        .filter_map(|m| owned_element_of(db, m))
        .collect()
}
