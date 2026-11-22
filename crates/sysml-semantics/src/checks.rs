// SPDX-License-Identifier: Apache-2.0
//! The semantic constraints of KerML (8.4.2; step 1a.9): one function per `check…` constraint,
//! from its OCL, read on the view of a file (ADR 0022), the implied relationships included.
//!
//! A check holds for an element when the constraint does not apply to it (its metaclass is not
//! the constraint's, or the OCL's condition is false) or when the element satisfies it. The
//! inventory of every constraint, with its status, is `crates/sysml-semantics/checks.json`, kept
//! equal to `KerML.xmi` by `tests/checks_inventory.rs`. The readings of the OCL are those of the
//! implied relationships that satisfy it (ADR 0020).

use sysml_model::{Attr, Metaclass as M, Slot};

use crate::derived as d;
use crate::derived::{end, metaclass};
use crate::resolve::{Global, Project, SourceFile};

/// A check: whether an element satisfies a constraint, or the constraint does not apply.
pub type Check = fn(&dyn salsa::Database, Project, Global) -> bool;

/// The checks implemented, by metaclass and constraint name, in the order of `KerML.xmi`'s
/// metaclasses.
pub const CHECKS: &[(M, &str, Check)] = &[
    // ---- Core (KerML 8.3.3) ----
    (
        M::Type,
        "checkTypeSpecialization",
        check_type_specialization,
    ),
    (
        M::Multiplicity,
        "checkMultiplicitySpecialization",
        check_multiplicity_specialization,
    ),
    (
        M::Feature,
        "checkFeatureSpecialization",
        check_feature_specialization,
    ),
    (
        M::Feature,
        "checkFeatureDataValueSpecialization",
        check_feature_data_value_specialization,
    ),
    (
        M::Feature,
        "checkFeatureOccurrenceSpecialization",
        check_feature_occurrence_specialization,
    ),
    (
        M::Feature,
        "checkFeatureObjectSpecialization",
        check_feature_object_specialization,
    ),
    (
        M::Feature,
        "checkFeatureSuboccurrenceSpecialization",
        check_feature_suboccurrence_specialization,
    ),
    (
        M::Feature,
        "checkFeatureSubobjectSpecialization",
        check_feature_subobject_specialization,
    ),
    (
        M::Feature,
        "checkFeaturePortionSpecialization",
        check_feature_portion_specialization,
    ),
    (
        M::Feature,
        "checkFeatureEndSpecialization",
        check_feature_end_specialization,
    ),
    (
        M::Feature,
        "checkFeatureCrossingSpecialization",
        check_feature_crossing_specialization,
    ),
    (
        M::Feature,
        "checkFeatureOwnedCrossFeatureSpecialization",
        check_feature_owned_cross_feature_specialization,
    ),
    (
        M::Feature,
        "checkFeatureOwnedCrossFeatureRedefinitionSpecialization",
        check_feature_owned_cross_feature_redefinition_specialization,
    ),
    (
        M::Feature,
        "checkFeatureValuationSpecialization",
        check_feature_valuation_specialization,
    ),
    (
        M::Feature,
        "checkFeatureEndRedefinition",
        check_feature_end_redefinition,
    ),
    (
        M::Feature,
        "checkFeatureParameterRedefinition",
        check_feature_parameter_redefinition,
    ),
    (
        M::Feature,
        "checkFeatureResultRedefinition",
        check_feature_result_redefinition,
    ),
    (
        M::Feature,
        "checkFeatureFlowFeatureRedefinition",
        check_feature_flow_feature_redefinition,
    ),
    // ---- Kernel (KerML 8.3.4) ----
    (
        M::DataType,
        "checkDataTypeSpecialization",
        check_data_type_specialization,
    ),
    (
        M::Class,
        "checkClassSpecialization",
        check_class_specialization,
    ),
    (
        M::Structure,
        "checkStructureSpecialization",
        check_structure_specialization,
    ),
    (
        M::Association,
        "checkAssociationSpecialization",
        check_association_specialization,
    ),
    (
        M::Association,
        "checkAssociationBinarySpecialization",
        check_association_binary_specialization,
    ),
    (
        M::AssociationStructure,
        "checkAssociationStructureSpecialization",
        check_association_structure_specialization,
    ),
    (
        M::AssociationStructure,
        "checkAssociationStructureBinarySpecialization",
        check_association_structure_binary_specialization,
    ),
    (
        M::Connector,
        "checkConnectorSpecialization",
        check_connector_specialization,
    ),
    (
        M::Connector,
        "checkConnectorBinarySpecialization",
        check_connector_binary_specialization,
    ),
    (
        M::Connector,
        "checkConnectorObjectSpecialization",
        check_connector_object_specialization,
    ),
    (
        M::Connector,
        "checkConnectorBinaryObjectSpecialization",
        check_connector_binary_object_specialization,
    ),
    (
        M::BindingConnector,
        "checkBindingConnectorSpecialization",
        check_binding_connector_specialization,
    ),
    (
        M::Succession,
        "checkSuccessionSpecialization",
        check_succession_specialization,
    ),
    (
        M::Behavior,
        "checkBehaviorSpecialization",
        check_behavior_specialization,
    ),
    (
        M::Step,
        "checkStepSpecialization",
        check_step_specialization,
    ),
    (
        M::Step,
        "checkStepEnclosedPerformanceSpecialization",
        check_step_enclosed_performance_specialization,
    ),
    (
        M::Step,
        "checkStepSubperformanceSpecialization",
        check_step_subperformance_specialization,
    ),
    (
        M::Step,
        "checkStepOwnedPerformanceSpecialization",
        check_step_owned_performance_specialization,
    ),
    (
        M::Function,
        "checkFunctionSpecialization",
        check_function_specialization,
    ),
    (
        M::Predicate,
        "checkPredicateSpecialization",
        check_predicate_specialization,
    ),
    (
        M::Expression,
        "checkExpressionSpecialization",
        check_expression_specialization,
    ),
    (
        M::BooleanExpression,
        "checkBooleanExpressionSpecialization",
        check_boolean_expression_specialization,
    ),
    (
        M::Invariant,
        "checkInvariantSpecialization",
        check_invariant_specialization,
    ),
    (
        M::ConstructorExpression,
        "checkConstructorExpressionSpecialization",
        check_constructor_expression_specialization,
    ),
    (
        M::ConstructorExpression,
        "checkConstructorExpressionResultSpecialization",
        check_constructor_expression_result_specialization,
    ),
    (
        M::ConstructorExpression,
        "checkConstructorExpressionResultFeatureRedefinition",
        check_constructor_expression_result_feature_redefinition,
    ),
    (
        M::FeatureChainExpression,
        "checkFeatureChainExpressionResultSpecialization",
        check_feature_chain_expression_result_specialization,
    ),
    (
        M::FeatureChainExpression,
        "checkFeatureChainExpressionSourceTargetRedefinition",
        check_feature_chain_expression_source_target_redefinition,
    ),
    (
        M::FeatureChainExpression,
        "checkFeatureChainExpressionTargetRedefinition",
        check_feature_chain_expression_target_redefinition,
    ),
    (
        M::FeatureReferenceExpression,
        "checkFeatureReferenceExpressionResultSpecialization",
        check_feature_reference_expression_result_specialization,
    ),
    (
        M::IndexExpression,
        "checkIndexExpressionResultSpecialization",
        check_index_expression_result_specialization,
    ),
    (
        M::InvocationExpression,
        "checkInvocationExpressionSpecialization",
        check_invocation_expression_specialization,
    ),
    (
        M::InvocationExpression,
        "checkInvocationExpressionBehaviorResultSpecialization",
        check_invocation_expression_behavior_result_specialization,
    ),
    (
        M::LiteralBoolean,
        "checkLiteralBooleanSpecialization",
        check_literal_boolean_specialization,
    ),
    (
        M::LiteralExpression,
        "checkLiteralExpressionSpecialization",
        check_literal_expression_specialization,
    ),
    (
        M::LiteralInfinity,
        "checkLiteralInfinitySpecialization",
        check_literal_infinity_specialization,
    ),
    (
        M::LiteralInteger,
        "checkLiteralIntegerSpecialization",
        check_literal_integer_specialization,
    ),
    (
        M::LiteralRational,
        "checkLiteralRationalSpecialization",
        check_literal_rational_specialization,
    ),
    (
        M::LiteralString,
        "checkLiteralStringSpecialization",
        check_literal_string_specialization,
    ),
    (
        M::MetadataAccessExpression,
        "checkMetadataAccessExpressionSpecialization",
        check_metadata_access_expression_specialization,
    ),
    (
        M::NullExpression,
        "checkNullExpressionSpecialization",
        check_null_expression_specialization,
    ),
    (
        M::SelectExpression,
        "checkSelectExpressionResultSpecialization",
        check_select_expression_result_specialization,
    ),
    (
        M::Flow,
        "checkFlowSpecialization",
        check_flow_specialization,
    ),
    (
        M::Flow,
        "checkFlowWithEndsSpecialization",
        check_flow_with_ends_specialization,
    ),
    (
        M::PayloadFeature,
        "checkPayloadFeatureRedefinition",
        check_payload_feature_redefinition,
    ),
    (
        M::SuccessionFlow,
        "checkSuccessionFlowSpecialization",
        check_succession_flow_specialization,
    ),
    (
        M::Metaclass,
        "checkMetaclassSpecialization",
        check_metaclass_specialization,
    ),
    (
        M::MetadataFeature,
        "checkMetadataFeatureSpecialization",
        check_metadata_feature_specialization,
    ),
    (
        M::MetadataFeature,
        "checkMetadataFeatureSemanticSpecialization",
        check_metadata_feature_semantic_specialization,
    ),
];

/// A constraint an element violates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Violation {
    /// The element.
    pub element: Global,
    /// The constraint, as `KerML.xmi` names it.
    pub constraint: &'static str,
}

/// The constraints of [`CHECKS`] that the elements of a file violate, implied elements included,
/// in element order.
pub fn violations(db: &dyn salsa::Database, project: Project, file: SourceFile) -> Vec<Violation> {
    let n = crate::view(db, project, file).len();
    let mut out = Vec::new();
    for index in 0..n as u32 {
        let x = Global { file, index };
        let m = metaclass(db, project, x);
        for (kind, name, check) in CHECKS {
            if m.specializes(*kind) && !check(db, project, x) {
                out.push(Violation {
                    element: x,
                    constraint: name,
                });
            }
        }
    }
    out
}

// ---- helpers ----

fn is(db: &dyn salsa::Database, project: Project, x: Global, m: M) -> bool {
    metaclass(db, project, x).specializes(m)
}

fn flag(db: &dyn salsa::Database, project: Project, x: Global, a: Attr) -> bool {
    crate::view(db, project, x.file).flag(x.ix(), a)
}

/// `specializesFromLibrary(name)` for an element of a metaclass; true for any other element.
fn from_library(db: &dyn salsa::Database, project: Project, x: Global, m: M, name: &str) -> bool {
    !is(db, project, x, m) || d::specializes_from_library(db, project, x, name)
}

/// `ownedTyping.type`.
fn owned_types(db: &dyn salsa::Database, project: Project, f: Global) -> Vec<Global> {
    d::owned_typing(db, project, f)
        .into_iter()
        .filter_map(|r| end(db, project, r, Slot::Target, 0))
        .collect()
}

fn owned_typed(db: &dyn salsa::Database, project: Project, f: Global, m: M) -> bool {
    owned_types(db, project, f)
        .into_iter()
        .any(|t| is(db, project, t, m))
}

/// `owningType.oclIsKindOf(m) or owningType.oclAsType(Feature).type->exists(oclIsKindOf(m))`
/// (KerML 8.3.3.3.4), false without an owning type.
fn owner_is(db: &dyn salsa::Database, project: Project, f: Global, m: M) -> bool {
    d::owning_type(db, project, f).is_some_and(|t| {
        is(db, project, t, m)
            || (is(db, project, t, M::Feature)
                && d::r#type(db, project, t)
                    .into_iter()
                    .any(|x| is(db, project, x, m)))
    })
}

/// `specializes(general)`, true when there is no general to specialize: the name of a general
/// that does not resolve is reported by resolution.
fn specializes(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
    general: Option<Global>,
) -> bool {
    general.is_none_or(|t| d::specializes(db, project, x, t))
}

/// `result.specializes(general)`, for an expression with a result.
fn result_specializes(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
    general: Option<Global>,
) -> bool {
    d::result(db, project, x).is_none_or(|r| specializes(db, project, r, general))
}

// ---- Core: Type (KerML 8.3.3.1.10), Multiplicity (8.3.3.1.9), Feature (8.3.3.3.4) ----

/// KerML 8.3.3.1.10 checkTypeSpecialization: `specializesFromLibrary('Base::Anything')`.
pub fn check_type_specialization(db: &dyn salsa::Database, project: Project, x: Global) -> bool {
    from_library(db, project, x, M::Type, "Base::Anything")
}

/// KerML 8.3.3.1.9 checkMultiplicitySpecialization: `specializesFromLibrary('Base::naturals')`.
pub fn check_multiplicity_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(db, project, x, M::Multiplicity, "Base::naturals")
}

/// KerML 8.3.3.3.4 checkFeatureSpecialization: `specializesFromLibrary('Base::things')`.
pub fn check_feature_specialization(db: &dyn salsa::Database, project: Project, x: Global) -> bool {
    from_library(db, project, x, M::Feature, "Base::things")
}

/// KerML 8.3.3.3.4 checkFeatureDataValueSpecialization: a feature with an owned typing by a
/// DataType specializes `Base::dataValues`.
pub fn check_feature_data_value_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Feature)
        || !owned_typed(db, project, x, M::DataType)
        || d::specializes_from_library(db, project, x, "Base::dataValues")
}

/// KerML 8.3.3.3.4 checkFeatureOccurrenceSpecialization: a feature with an owned typing by a
/// Class specializes `Occurrences::occurrences`.
pub fn check_feature_occurrence_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Feature)
        || !owned_typed(db, project, x, M::Class)
        || d::specializes_from_library(db, project, x, "Occurrences::occurrences")
}

/// KerML 8.3.3.3.4 checkFeatureObjectSpecialization: a feature with an owned typing by a
/// Structure specializes `Objects::objects` (the OCL's `specializesFromLibary`).
pub fn check_feature_object_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Feature)
        || !owned_typed(db, project, x, M::Structure)
        || d::specializes_from_library(db, project, x, "Objects::objects")
}

/// KerML 8.3.3.3.4 checkFeatureSuboccurrenceSpecialization: a composite feature typed by a Class,
/// owned by a Class or a Feature typed by one, specializes `Occurrences::Occurrence::
/// suboccurrences` (the OCL's `Occurrence::Occurrence`, read as the library's package).
pub fn check_feature_suboccurrence_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Feature)
        || !(flag(db, project, x, Attr::IsComposite)
            && owned_typed(db, project, x, M::Class)
            && owner_is(db, project, x, M::Class))
        || d::specializes_from_library(db, project, x, "Occurrences::Occurrence::suboccurrences")
}

/// KerML 8.3.3.3.4 checkFeatureSubobjectSpecialization: a composite feature typed by a
/// Structure, owned by a Structure or a Feature typed by one, specializes `Objects::Object::
/// subobjects`, the target of Table 9 (the OCL names `suboccurrences`; ADR 0020).
pub fn check_feature_subobject_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Feature)
        || !(flag(db, project, x, Attr::IsComposite)
            && owned_typed(db, project, x, M::Structure)
            && owner_is(db, project, x, M::Structure))
        || d::specializes_from_library(db, project, x, "Objects::Object::subobjects")
}

/// KerML 8.3.3.3.4 checkFeaturePortionSpecialization: a portion typed by a Class, owned by a
/// Class or a Feature typed by one, specializes `Occurrences::Occurrence::portions` (the OCL's
/// `Occurrence::Occurrence`).
pub fn check_feature_portion_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Feature)
        || !(flag(db, project, x, Attr::IsPortion)
            && owned_typed(db, project, x, M::Class)
            && owner_is(db, project, x, M::Class))
        || d::specializes_from_library(db, project, x, "Occurrences::Occurrence::portions")
}

/// KerML 8.3.3.3.4 checkFeatureEndSpecialization: an end owned by an Association or a Connector
/// specializes `Links::Link::participant`.
pub fn check_feature_end_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Feature)
        || !(flag(db, project, x, Attr::IsEnd)
            && d::owning_type(db, project, x).is_some_and(|t| {
                is(db, project, t, M::Association) || is(db, project, t, M::Connector)
            }))
        || d::specializes_from_library(db, project, x, "Links::Link::participant")
}

/// KerML 8.3.3.3.4 checkFeatureCrossingSpecialization: `ownedCrossFeature() <> null implies
/// crossFeature = ownedCrossFeature()`.
pub fn check_feature_crossing_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Feature)
        || d::owned_cross_feature(db, project, x)
            .is_none_or(|c| d::cross_feature(db, project, x) == Some(c))
}

/// KerML 8.3.3.3.4 checkFeatureOwnedCrossFeatureSpecialization: an owned cross feature
/// specializes every type of its end (`owner.oclAsType(Feature).type`).
pub fn check_feature_owned_cross_feature_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::Feature) || !d::is_owned_cross_feature(db, project, x) {
        return true;
    }
    d::owner(db, project, x).is_none_or(|e| {
        d::r#type(db, project, e)
            .into_iter()
            .all(|t| d::specializes(db, project, x, t))
    })
}

/// KerML 8.3.3.3.4 checkFeatureOwnedCrossFeatureRedefinitionSpecialization: an owned cross
/// feature's owned subsettings subset the cross features of the features its end redefines.
pub fn check_feature_owned_cross_feature_redefinition_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::Feature) || !d::is_owned_cross_feature(db, project, x) {
        return true;
    }
    let subsetted: Vec<Global> = d::owned_subsetting(db, project, x)
        .into_iter()
        .filter_map(|r| end(db, project, r, Slot::Target, 0))
        .collect();
    let Some(e) = d::owner(db, project, x) else {
        return true;
    };
    d::owned_redefinition(db, project, e)
        .into_iter()
        .filter_map(|r| end(db, project, r, Slot::Target, 0))
        .filter_map(|r| d::cross_feature(db, project, r))
        .all(|c| subsetted.contains(&c))
}

/// KerML 8.3.3.3.4 checkFeatureValuationSpecialization: a feature without a direction, whose owned
/// specializations are all implied, specializes the result of the value of each of its
/// FeatureValues.
pub fn check_feature_valuation_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::Feature) {
        return true;
    }
    let v = crate::view(db, project, x.file);
    if v.values(x.ix(), Attr::Direction).next().is_some()
        || d::owned_specialization(db, project, x)
            .into_iter()
            .any(|r| !flag(db, project, r, Attr::IsImplied))
    {
        return true;
    }
    d::owned_membership(db, project, x)
        .into_iter()
        .filter(|m| is(db, project, *m, M::FeatureValue))
        .filter_map(|m| target_element(db, project, m))
        .all(|value| specializes(db, project, x, d::result(db, project, value)))
}

/// The owned related element of a relationship.
fn target_element(db: &dyn salsa::Database, project: Project, r: Global) -> Option<Global> {
    let v = crate::view(db, project, r.file);
    v.owned_related_elements(r.ix()).first().map(|e| Global {
        file: r.file,
        index: e.index() as u32,
    })
}

/// The generals of a type's owned specializations, written and implied, without the type itself
/// (`ownedSpecialization.general`).
fn owned_generals(db: &dyn salsa::Database, project: Project, t: Global) -> Vec<Global> {
    d::owned_specialization(db, project, t)
        .into_iter()
        .filter_map(|r| end(db, project, r, Slot::Target, 0))
        .filter(|g| *g != t)
        .collect()
}

fn is_result_parameter(db: &dyn salsa::Database, project: Project, f: Global) -> bool {
    d::owning_membership(db, project, f)
        .is_some_and(|m| is(db, project, m, M::ReturnParameterMembership))
}

/// The owned features of a type with a direction that are not its result (the OCL's
/// `ownedFeature->select(direction <> null)->reject(…ReturnParameterMembership)`).
fn owned_parameters(db: &dyn salsa::Database, project: Project, t: Global) -> Vec<Global> {
    d::owned_feature(db, project, t)
        .into_iter()
        .filter(|f| {
            crate::view(db, project, f.file)
                .values(f.ix(), Attr::Direction)
                .next()
                .is_some()
                && !is_result_parameter(db, project, *f)
        })
        .collect()
}

/// KerML 8.3.3.3.4 checkFeatureEndRedefinition: the i-th owned end of a type redefines the i-th
/// end feature of each general of the type's owned specializations that has one.
pub fn check_feature_end_redefinition(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::Feature) || !flag(db, project, x, Attr::IsEnd) {
        return true;
    }
    let Some(t) = d::owning_type(db, project, x) else {
        return true;
    };
    let Some(i) = d::owned_end_feature(db, project, t)
        .iter()
        .position(|e| *e == x)
    else {
        return true;
    };
    owned_generals(db, project, t).into_iter().all(|s| {
        d::end_feature(db, project, s)
            .get(i)
            .is_none_or(|e| d::redefines(db, project, x, *e))
    })
}

/// KerML 8.3.3.3.4 checkFeatureParameterRedefinition: the i-th parameter of a Behavior or Step,
/// other than its result, redefines the i-th parameter of each general of its owning type's owned
/// specializations that is a Behavior or a Step (KerML 8.4.3.1 Table 9; ADR 0020); for an
/// InvocationExpression, unless it has a written redefinition.
pub fn check_feature_parameter_redefinition(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::Feature) || is_result_parameter(db, project, x) {
        return true;
    }
    let Some(t) = d::owning_type(db, project, x) else {
        return true;
    };
    let written = d::owned_redefinition(db, project, x)
        .into_iter()
        .any(|r| !flag(db, project, r, Attr::IsImplied));
    let applies = is(db, project, t, M::Behavior)
        || (is(db, project, t, M::Step)
            && !(is(db, project, t, M::InvocationExpression) && written));
    if !applies {
        return true;
    }
    let Some(i) = owned_parameters(db, project, t)
        .iter()
        .position(|p| *p == x)
    else {
        return true;
    };
    // The generals that are Behaviors or Steps, whose parameters Table 9 names (ADR 0020).
    let behavior = |s: &Global| is(db, project, *s, M::Behavior) || is(db, project, *s, M::Step);
    owned_generals(db, project, t)
        .into_iter()
        .filter(behavior)
        .all(|s| {
            owned_parameters(db, project, s)
                .get(i)
                .is_none_or(|p| d::redefines(db, project, x, *p))
        })
}

/// KerML 8.3.3.3.4 checkFeatureResultRedefinition: the result of a Function or Expression
/// redefines the result of each Function or Expression its owning type specializes directly.
pub fn check_feature_result_redefinition(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::Feature) {
        return true;
    }
    let Some(t) = d::owning_type(db, project, x) else {
        return true;
    };
    let function = |y: Global| is(db, project, y, M::Function) || is(db, project, y, M::Expression);
    if !function(t) || d::result(db, project, t) != Some(x) {
        return true;
    }
    owned_generals(db, project, t)
        .into_iter()
        .filter(|s| function(*s))
        .all(|s| d::result(db, project, s).is_none_or(|r| d::redefines(db, project, x, r)))
}

/// KerML 8.3.3.3.4 checkFeatureFlowFeatureRedefinition: the feature of the first FlowEnd of a
/// flow redefines `Transfers::Transfer::source::sourceOutput`, and that of the second
/// `Transfers::Transfer::target::targetInput` (the OCL's `source::targetInput`; the position is
/// counted among the FlowEnds; ADR 0020).
pub fn check_feature_flow_feature_redefinition(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::Feature) {
        return true;
    }
    let Some(end) = d::owning_type(db, project, x).filter(|e| is(db, project, *e, M::FlowEnd))
    else {
        return true;
    };
    if d::owned_feature(db, project, end).first() != Some(&x) {
        return true;
    }
    let Some(flow) = d::owning_type(db, project, end) else {
        return true;
    };
    let ends: Vec<Global> = d::owned_feature(db, project, flow)
        .into_iter()
        .filter(|f| is(db, project, *f, M::FlowEnd))
        .collect();
    match ends.iter().position(|e| *e == end) {
        Some(0) => {
            d::redefines_from_library(db, project, x, "Transfers::Transfer::source::sourceOutput")
        }
        Some(1) => {
            d::redefines_from_library(db, project, x, "Transfers::Transfer::target::targetInput")
        }
        _ => true,
    }
}

// ---- Kernel: classifiers (KerML 8.3.4.1–8.3.4.4) ----

/// KerML 8.3.4.1.2 checkDataTypeSpecialization: `specializesFromLibrary('Base::DataValue')`.
pub fn check_data_type_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(db, project, x, M::DataType, "Base::DataValue")
}

/// KerML 8.3.4.2.2 checkClassSpecialization: `specializesFromLibrary('Occurrences::Occurrence')`.
pub fn check_class_specialization(db: &dyn salsa::Database, project: Project, x: Global) -> bool {
    from_library(db, project, x, M::Class, "Occurrences::Occurrence")
}

/// KerML 8.3.4.3.2 checkStructureSpecialization: `specializesFromLibrary('Objects::Object')`.
pub fn check_structure_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(db, project, x, M::Structure, "Objects::Object")
}

/// KerML 8.3.4.4.2 checkAssociationSpecialization: `specializesFromLibrary('Links::Link')`.
pub fn check_association_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(db, project, x, M::Association, "Links::Link")
}

/// KerML 8.3.4.4.2 checkAssociationBinarySpecialization: an association with two
/// `associationEnd`s (its `endFeature`s) specializes `Links::BinaryLink`.
pub fn check_association_binary_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Association)
        || d::end_feature(db, project, x).len() != 2
        || d::specializes_from_library(db, project, x, "Links::BinaryLink")
}

/// KerML 8.3.4.4.3 checkAssociationStructureSpecialization:
/// `specializesFromLibrary('Objects::LinkObject')`.
pub fn check_association_structure_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(
        db,
        project,
        x,
        M::AssociationStructure,
        "Objects::LinkObject",
    )
}

/// KerML 8.3.4.4.3 checkAssociationStructureBinarySpecialization: one with two end features
/// specializes `Objects::BinaryLinkObject`.
pub fn check_association_structure_binary_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::AssociationStructure)
        || d::end_feature(db, project, x).len() != 2
        || d::specializes_from_library(db, project, x, "Objects::BinaryLinkObject")
}

// ---- Kernel: connectors (KerML 8.3.4.5) ----

/// A connector's `association`s: its types that are Associations (KerML 8.3.4.5.3).
fn object_association(db: &dyn salsa::Database, project: Project, c: Global) -> bool {
    d::r#type(db, project, c)
        .into_iter()
        .any(|t| is(db, project, t, M::AssociationStructure))
}

/// KerML 8.3.4.5.3 checkConnectorSpecialization: `specializesFromLibrary('Links::links')`.
pub fn check_connector_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(db, project, x, M::Connector, "Links::links")
}

/// KerML 8.3.4.5.3 checkConnectorBinarySpecialization: a connector with two `connectorEnd`s (its
/// `endFeature`s) specializes `Links::binaryLinks`.
pub fn check_connector_binary_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Connector)
        || d::end_feature(db, project, x).len() != 2
        || d::specializes_from_library(db, project, x, "Links::binaryLinks")
}

/// KerML 8.3.4.5.3 checkConnectorObjectSpecialization: a connector typed by an
/// AssociationStructure specializes `Objects::linkObjects`.
pub fn check_connector_object_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Connector)
        || !object_association(db, project, x)
        || d::specializes_from_library(db, project, x, "Objects::linkObjects")
}

/// KerML 8.3.4.5.3 checkConnectorBinaryObjectSpecialization: a binary connector typed by an
/// AssociationStructure specializes `Objects::binaryLinkObjects`.
pub fn check_connector_binary_object_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Connector)
        || d::end_feature(db, project, x).len() != 2
        || !object_association(db, project, x)
        || d::specializes_from_library(db, project, x, "Objects::binaryLinkObjects")
}

/// KerML 8.3.4.5.2 checkBindingConnectorSpecialization:
/// `specializesFromLibrary('Links::selfLinks')`.
pub fn check_binding_connector_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(db, project, x, M::BindingConnector, "Links::selfLinks")
}

/// KerML 8.3.4.5.4 checkSuccessionSpecialization:
/// `specializesFromLibrary('Occurrences::happensBeforeLinks')`.
pub fn check_succession_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(
        db,
        project,
        x,
        M::Succession,
        "Occurrences::happensBeforeLinks",
    )
}

// ---- Kernel: behaviors (KerML 8.3.4.6) ----

/// KerML 8.3.4.6.2 checkBehaviorSpecialization:
/// `specializesFromLibrary('Performances::Performance')`.
pub fn check_behavior_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(db, project, x, M::Behavior, "Performances::Performance")
}

/// KerML 8.3.4.6.3 checkStepSpecialization: `specializesFromLibrary('Performances::performances')`.
pub fn check_step_specialization(db: &dyn salsa::Database, project: Project, x: Global) -> bool {
    from_library(db, project, x, M::Step, "Performances::performances")
}

fn owned_by_behavior_or_step(db: &dyn salsa::Database, project: Project, x: Global) -> bool {
    d::owning_type(db, project, x)
        .is_some_and(|t| is(db, project, t, M::Behavior) || is(db, project, t, M::Step))
}

/// KerML 8.3.4.6.3 checkStepEnclosedPerformanceSpecialization: a step owned by a Behavior or a
/// Step specializes `Performances::Performance::enclosedPerformances` (the OCL's
/// `enclosedPerformance`, read as the library's feature; ADR 0020).
pub fn check_step_enclosed_performance_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Step)
        || !owned_by_behavior_or_step(db, project, x)
        || d::specializes_from_library(
            db,
            project,
            x,
            "Performances::Performance::enclosedPerformances",
        )
}

/// KerML 8.3.4.6.3 checkStepSubperformanceSpecialization: a composite step owned by a Behavior
/// or a Step specializes `Performances::Performance::subperformances` (the OCL's
/// `subperformance`).
pub fn check_step_subperformance_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Step)
        || !(owned_by_behavior_or_step(db, project, x) && flag(db, project, x, Attr::IsComposite))
        || d::specializes_from_library(db, project, x, "Performances::Performance::subperformances")
}

/// KerML 8.3.4.6.3 checkStepOwnedPerformanceSpecialization: a composite step owned by a
/// Structure, or a Feature typed by one, specializes `Objects::Object::ownedPerformances` (the
/// OCL's `ownedPerformance`).
pub fn check_step_owned_performance_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Step)
        || !(flag(db, project, x, Attr::IsComposite) && owner_is(db, project, x, M::Structure))
        || d::specializes_from_library(db, project, x, "Objects::Object::ownedPerformances")
}

// ---- Kernel: functions (KerML 8.3.4.7) ----

/// KerML 8.3.4.7.4 checkFunctionSpecialization:
/// `specializesFromLibrary('Performances::Evaluation')`.
pub fn check_function_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(db, project, x, M::Function, "Performances::Evaluation")
}

/// KerML 8.3.4.7.6 checkPredicateSpecialization:
/// `specializesFromLibrary('Performances::BooleanEvaluation')`.
pub fn check_predicate_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(
        db,
        project,
        x,
        M::Predicate,
        "Performances::BooleanEvaluation",
    )
}

/// KerML 8.3.4.7.3 checkExpressionSpecialization:
/// `specializesFromLibrary('Performances::evaluations')`.
pub fn check_expression_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(db, project, x, M::Expression, "Performances::evaluations")
}

/// KerML 8.3.4.7.2 checkBooleanExpressionSpecialization:
/// `specializesFromLibrary('Performances::booleanEvaluations')`.
pub fn check_boolean_expression_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(
        db,
        project,
        x,
        M::BooleanExpression,
        "Performances::booleanEvaluations",
    )
}

/// KerML 8.3.4.7.5 checkInvariantSpecialization: `Performances::falseEvaluations` if negated,
/// else `Performances::trueEvaluations`.
pub fn check_invariant_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    let name = if flag(db, project, x, Attr::IsNegated) {
        "Performances::falseEvaluations"
    } else {
        "Performances::trueEvaluations"
    };
    from_library(db, project, x, M::Invariant, name)
}

// ---- Kernel: expressions (KerML 8.3.4.8) ----

/// KerML 8.3.4.8.3 checkConstructorExpressionSpecialization: the OCL's
/// `specializes('Performances::constructorEvaluations')`, read as `specializesFromLibrary`.
pub fn check_constructor_expression_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(
        db,
        project,
        x,
        M::ConstructorExpression,
        "Performances::constructorEvaluations",
    )
}

/// KerML 8.3.4.8.3 checkConstructorExpressionResultSpecialization:
/// `result.specializes(instantiatedType)`.
pub fn check_constructor_expression_result_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::ConstructorExpression)
        || result_specializes(db, project, x, d::instantiated_type(db, project, x))
}

/// KerML 8.3.4.8.3 checkConstructorExpressionResultFeatureRedefinition: each owned feature of
/// the result redefines exactly one public feature of the instantiated type.
pub fn check_constructor_expression_result_feature_redefinition(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::ConstructorExpression) {
        return true;
    }
    let (Some(t), Some(r)) = (
        d::instantiated_type(db, project, x),
        d::result(db, project, x),
    ) else {
        return true;
    };
    let features: Vec<Global> = d::feature(db, project, t)
        .into_iter()
        .filter(|f| {
            d::owning_membership(db, project, *f).is_some_and(|m| {
                crate::view(db, project, m.file).visibility(m.ix()) == crate::Visibility::Public
            })
        })
        .collect();
    d::owned_feature(db, project, r).into_iter().all(|f| {
        let redefined: Vec<Global> = d::owned_redefinition(db, project, f)
            .into_iter()
            .filter_map(|r| end(db, project, r, Slot::Target, 0))
            .collect();
        {
            let mut seen = std::collections::HashSet::new();
            redefined
                .into_iter()
                .filter(|g| seen.insert(*g))
                .collect::<Vec<_>>()
        }
        .into_iter()
        .filter(|g| features.contains(g))
        .count()
            == 1
    })
}

/// KerML 8.3.4.8.4 checkFeatureChainExpressionResultSpecialization: there is a
/// sourceTargetFeature, and the result, owned by the expression, subsets the chain of the first
/// input parameter and it (`subsetsChain`).
pub fn check_feature_chain_expression_result_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::FeatureChainExpression) {
        return true;
    }
    let input = d::owned_feature(db, project, x).into_iter().find(|f| {
        matches!(
            crate::view(db, project, f.file)
                .values(f.ix(), Attr::Direction)
                .next(),
            Some(sysml_model::Value::Enumeration("in"))
        )
    });
    let (Some(input), Some(stf), Some(r)) = (
        input,
        d::source_target_feature(db, project, x),
        d::result(db, project, x),
    ) else {
        return false;
    };
    d::subsets_chain(db, project, r, input, stf) && d::owning_type(db, project, r) == Some(x)
}

/// KerML 8.3.4.8.4 checkFeatureChainExpressionSourceTargetRedefinition: the sourceTargetFeature
/// redefines the target feature.
pub fn check_feature_chain_expression_source_target_redefinition(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::FeatureChainExpression) {
        return true;
    }
    let Some(stf) = d::source_target_feature(db, project, x) else {
        return false;
    };
    // Without a target feature there is nothing to redefine, and the OCL's `redefines` is false.
    d::chain_target_feature(db, project, x).is_some_and(|t| d::redefines(db, project, stf, t))
}

/// KerML 8.3.4.8.4 checkFeatureChainExpressionTargetRedefinition: the sourceTargetFeature
/// redefines `ControlFunctions::'.'::source::target`.
pub fn check_feature_chain_expression_target_redefinition(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::FeatureChainExpression) {
        return true;
    }
    d::source_target_feature(db, project, x).is_some_and(|stf| {
        d::redefines_from_library(db, project, stf, "ControlFunctions::'.'::source::target")
    })
}

/// KerML 8.3.4.8.5 checkFeatureReferenceExpressionResultSpecialization: `result.owningType() =
/// self and result.specializes(referent)`.
pub fn check_feature_reference_expression_result_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::FeatureReferenceExpression) {
        return true;
    }
    d::result(db, project, x).is_some_and(|r| {
        d::owning_type(db, project, r) == Some(x)
            && specializes(db, project, r, d::referent(db, project, x))
    })
}

/// The result of an expression's first argument (KerML 8.3.4.8.8 `argument`).
fn first_argument_result(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> Option<Option<Global>> {
    let a = *d::argument(db, project, x).first()?;
    Some(d::result(db, project, a))
}

/// KerML 8.3.4.8.6 checkIndexExpressionResultSpecialization: unless the first argument's result
/// specializes `Collections::Array`, the result specializes it.
pub fn check_index_expression_result_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::IndexExpression) {
        return true;
    }
    match first_argument_result(db, project, x) {
        None => true,
        Some(r) => {
            r.is_some_and(|r| d::specializes_from_library(db, project, r, "Collections::Array"))
                || result_specializes(db, project, x, r)
        }
    }
}

/// KerML 8.3.4.8.18 checkSelectExpressionResultSpecialization: `arguments->notEmpty() implies
/// result.specializes(arguments->first().result)`.
pub fn check_select_expression_result_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::SelectExpression)
        || first_argument_result(db, project, x)
            .is_none_or(|r| result_specializes(db, project, x, r))
}

/// KerML 8.3.4.8.8 checkInvocationExpressionSpecialization: `specializes(instantiatedType)`.
pub fn check_invocation_expression_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::InvocationExpression)
        || specializes(db, project, x, d::instantiated_type(db, project, x))
}

/// KerML 8.3.4.8.8 checkInvocationExpressionBehaviorResultSpecialization: the result of an
/// invocation of a type that is neither a Function nor a Feature typed by one specializes it.
pub fn check_invocation_expression_behavior_result_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::InvocationExpression) {
        return true;
    }
    let Some(t) = d::instantiated_type(db, project, x) else {
        return true;
    };
    let function = |y: Global| is(db, project, y, M::Function);
    let invokes_function = function(t)
        || (is(db, project, t, M::Feature) && d::r#type(db, project, t).into_iter().any(function));
    invokes_function || result_specializes(db, project, x, Some(t))
}

/// KerML 8.3.4.8.9 checkLiteralBooleanSpecialization.
pub fn check_literal_boolean_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(
        db,
        project,
        x,
        M::LiteralBoolean,
        "Performances::literalBooleanEvaluations",
    )
}

/// KerML 8.3.4.8.10 checkLiteralExpressionSpecialization.
pub fn check_literal_expression_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(
        db,
        project,
        x,
        M::LiteralExpression,
        "Performances::literalEvaluations",
    )
}

/// KerML 8.3.4.8.11 checkLiteralInfinitySpecialization: `Performances::literalIntegerEvaluations`.
pub fn check_literal_infinity_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(
        db,
        project,
        x,
        M::LiteralInfinity,
        "Performances::literalIntegerEvaluations",
    )
}

/// KerML 8.3.4.8.12 checkLiteralIntegerSpecialization.
pub fn check_literal_integer_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(
        db,
        project,
        x,
        M::LiteralInteger,
        "Performances::literalIntegerEvaluations",
    )
}

/// KerML 8.3.4.8.13 checkLiteralRationalSpecialization.
pub fn check_literal_rational_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(
        db,
        project,
        x,
        M::LiteralRational,
        "Performances::literalRationalEvaluations",
    )
}

/// KerML 8.3.4.8.14 checkLiteralStringSpecialization.
pub fn check_literal_string_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(
        db,
        project,
        x,
        M::LiteralString,
        "Performances::literalStringEvaluations",
    )
}

/// KerML 8.3.4.8.15 checkMetadataAccessExpressionSpecialization.
pub fn check_metadata_access_expression_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(
        db,
        project,
        x,
        M::MetadataAccessExpression,
        "Performances::metadataAccessEvaluations",
    )
}

/// KerML 8.3.4.8.16 checkNullExpressionSpecialization.
pub fn check_null_expression_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(
        db,
        project,
        x,
        M::NullExpression,
        "Performances::nullEvaluations",
    )
}

// ---- Kernel: interactions (KerML 8.3.4.9) ----

/// KerML 8.3.4.9.2 checkFlowSpecialization: `specializesFromLibrary('Transfers::transfers')`.
pub fn check_flow_specialization(db: &dyn salsa::Database, project: Project, x: Global) -> bool {
    from_library(db, project, x, M::Flow, "Transfers::transfers")
}

/// KerML 8.3.4.9.2 checkFlowWithEndsSpecialization: a flow with owned end features specializes
/// `Transfers::flowTransfers`. The end features are read as FlowEnds (KerML 8.4.4.10.2): the
/// library's own flows, such as `Transfers::messageTransfers`, have plain ends (ADR 0020).
pub fn check_flow_with_ends_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Flow)
        || !d::owned_end_feature(db, project, x)
            .into_iter()
            .any(|f| metaclass(db, project, f) == M::FlowEnd)
        || d::specializes_from_library(db, project, x, "Transfers::flowTransfers")
}

/// KerML 8.3.4.9.6 checkSuccessionFlowSpecialization:
/// `specializesFromLibrary('Transfers::flowTransfersBefore')`.
pub fn check_succession_flow_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(
        db,
        project,
        x,
        M::SuccessionFlow,
        "Transfers::flowTransfersBefore",
    )
}

/// KerML 8.3.4.9.5 checkPayloadFeatureRedefinition:
/// `redefinesFromLibrary('Transfers::Transfer::payload')`.
pub fn check_payload_feature_redefinition(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::PayloadFeature)
        || d::redefines_from_library(db, project, x, "Transfers::Transfer::payload")
}

// ---- Kernel: metadata (KerML 8.3.4.12) ----

/// KerML 8.3.4.12.2 checkMetaclassSpecialization:
/// `specializesFromLibrary('Metaobjects::Metaobject')`.
pub fn check_metaclass_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(db, project, x, M::Metaclass, "Metaobjects::Metaobject")
}

/// KerML 8.3.4.12.3 checkMetadataFeatureSpecialization:
/// `specializesFromLibrary('Metaobjects::metaobjects')`.
pub fn check_metadata_feature_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    from_library(
        db,
        project,
        x,
        M::MetadataFeature,
        "Metaobjects::metaobjects",
    )
}

/// KerML 8.3.4.12.3 checkMetadataFeatureSemanticSpecialization: the first Type that semantic
/// metadata annotates specializes its base type, the syntax element of the first value of
/// `Metaobjects::SemanticMetadata::baseType`; a Classifier with a Feature as its base type
/// specializes the feature's types instead.
pub fn check_metadata_feature_semantic_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::MetadataFeature) || !d::is_semantic(db, project, x) {
        return true;
    }
    let Some(annotated) = d::annotated_element(db, project, x)
        .into_iter()
        .find(|t| is(db, project, *t, M::Type))
    else {
        return true;
    };
    let Some(base_type) = d::library(db, project, "Metaobjects::SemanticMetadata::baseType") else {
        return true;
    };
    // `baseTypes()->first().isSyntactic()`: reflective metadata always is; a written metadata
    // feature has no syntax element, so it gives no base type (ADR 0021).
    let Some(base) = d::evaluate_feature(db, project, x, base_type)
        .first()
        .and_then(d::syntax_element)
    else {
        return true;
    };
    if is(db, project, annotated, M::Classifier) && is(db, project, base, M::Feature) {
        d::r#type(db, project, base)
            .into_iter()
            .all(|t| d::specializes(db, project, annotated, t))
    } else if is(db, project, base, M::Type) {
        d::specializes(db, project, annotated, base)
    } else {
        true
    }
}
