// SPDX-License-Identifier: Apache-2.0
//! The validation constraints of KerML (8.3; step 1a.10): one function per `validate…`
//! constraint, from its OCL, read on the view of a file (ADR 0022), and the diagnostics of their
//! violations (ADR 0012).
//!
//! A validation holds for an element when the constraint does not apply to it (its metaclass is
//! not the constraint's) or when the element satisfies it. Every violation is an error, since "the
//! semantics of a model are only defined if it meets all semantic and validation constraints"
//! (KerML 8.4.2; ADR 0012). Its code is the constraint's name and its clause that of the
//! constraint's metaclass. The inventory of every constraint, with its status, is
//! `crates/sysml-semantics/validations.json`, kept equal to `KerML.xmi` by
//! `tests/validations_inventory.rs`. ADR 0023 records the readings of the OCL.

use std::collections::{HashMap, HashSet};
use sysml_model::{Attr, Metaclass as M, Slot};
use sysml_syntax::Diagnostic;
use sysml_syntax::{TextRange, TextSize};

use crate::derived as d;
use crate::derived::{end, metaclass, Direction};
use crate::resolve::{Global, Project, SourceFile};

/// A validation: whether an element satisfies a constraint, or the constraint does not apply.
pub type Validation = fn(&dyn salsa::Database, Project, Global) -> bool;

/// The validations implemented: metaclass, constraint name, the clause of the metaclass, and the
/// function, in the order of the layers.
pub const VALIDATIONS: &[(M, &str, &str, Validation)] = &[
    // ---- Root (KerML 8.3.2) ----
    (
        M::Element,
        "validateElementIsImpliedIncluded",
        "KerML 8.3.2.1.2",
        validate_element_is_implied_included,
    ),
    (
        M::Annotation,
        "validateAnnotationAnnotatingElement",
        "KerML 8.3.2.3.3",
        validate_annotation_annotating_element,
    ),
    (
        M::Annotation,
        "validateAnnotationAnnotatedElementOwnership",
        "KerML 8.3.2.3.3",
        validate_annotation_annotated_element_ownership,
    ),
    (
        M::Import,
        "validateImportTopLevelVisibility",
        "KerML 8.3.2.4.2",
        validate_import_top_level_visibility,
    ),
    (
        M::Namespace,
        "validateNamespaceDistinguishibility",
        "KerML 8.3.2.4.5",
        validate_namespace_distinguishibility,
    ),
    // ---- Core: types (KerML 8.3.3.1) ----
    (
        M::Type,
        "validateTypeAtMostOneConjugator",
        "KerML 8.3.3.1.10",
        validate_type_at_most_one_conjugator,
    ),
    (
        M::Type,
        "validateTypeOwnedMultiplicity",
        "KerML 8.3.3.1.10",
        validate_type_owned_multiplicity,
    ),
    (
        M::Type,
        "validateTypeOwnedUnioningNotOne",
        "KerML 8.3.3.1.10",
        validate_type_owned_unioning_not_one,
    ),
    (
        M::Type,
        "validateTypeOwnedIntersectingNotOne",
        "KerML 8.3.3.1.10",
        validate_type_owned_intersecting_not_one,
    ),
    (
        M::Type,
        "validateTypeOwnedDifferencingNotOne",
        "KerML 8.3.3.1.10",
        validate_type_owned_differencing_not_one,
    ),
    (
        M::Type,
        "validateTypeUnioningTypesNotSelf",
        "KerML 8.3.3.1.10",
        validate_type_unioning_types_not_self,
    ),
    (
        M::Type,
        "validateTypeIntersectingTypesNotSelf",
        "KerML 8.3.3.1.10",
        validate_type_intersecting_types_not_self,
    ),
    (
        M::Type,
        "validateTypeDifferencingTypesNotSelf",
        "KerML 8.3.3.1.10",
        validate_type_differencing_types_not_self,
    ),
    (
        M::Specialization,
        "validateSpecificationSpecificNotConjugated",
        "KerML 8.3.3.1.8",
        validate_specification_specific_not_conjugated,
    ),
    // ---- Core: classifiers (KerML 8.3.3.2) ----
    (
        M::Classifier,
        "validateClassifierMultiplicityDomain",
        "KerML 8.3.3.2.2",
        validate_classifier_multiplicity_domain,
    ),
    // ---- Core: features (KerML 8.3.3.3) ----
    (
        M::Feature,
        "validateFeatureMultiplicityDomain",
        "KerML 8.3.3.3.4",
        validate_feature_multiplicity_domain,
    ),
    (
        M::Feature,
        "validateFeatureIsVariable",
        "KerML 8.3.3.3.4",
        validate_feature_is_variable,
    ),
    (
        M::Feature,
        "validateFeatureConstantIsVariable",
        "KerML 8.3.3.3.4",
        validate_feature_constant_is_variable,
    ),
    (
        M::Feature,
        "validateFeaturePortionNotVariable",
        "KerML 8.3.3.3.4",
        validate_feature_portion_not_variable,
    ),
    (
        M::Feature,
        "validateFeatureEndNoDirection",
        "KerML 8.3.3.3.4",
        validate_feature_end_no_direction,
    ),
    (
        M::Feature,
        "validateFeatureEndIsConstant",
        "KerML 8.3.3.3.4",
        validate_feature_end_is_constant,
    ),
    (
        M::Feature,
        "validateFeatureEndNotDerivedAbstractCompositeOrPortion",
        "KerML 8.3.3.3.4",
        validate_feature_end_not_derived_abstract_composite_or_portion,
    ),
    (
        M::Feature,
        "validateFeatureEndMultiplicity",
        "KerML 8.3.3.3.4",
        validate_feature_end_multiplicity,
    ),
    (
        M::Feature,
        "validateFeatureChainingFeatureNotOne",
        "KerML 8.3.3.3.4",
        validate_feature_chaining_feature_not_one,
    ),
    (
        M::Feature,
        "validateFeatureChainingFeaturesNotSelf",
        "KerML 8.3.3.3.4",
        validate_feature_chaining_features_not_self,
    ),
    (
        M::Feature,
        "validateFeatureChainingFeatureConformance",
        "KerML 8.3.3.3.4",
        validate_feature_chaining_feature_conformance,
    ),
    (
        M::Feature,
        "validateFeatureOwnedReferenceSubsetting",
        "KerML 8.3.3.3.4",
        validate_feature_owned_reference_subsetting,
    ),
    (
        M::Feature,
        "validateFeatureOwnedCrossSubsetting",
        "KerML 8.3.3.3.4",
        validate_feature_owned_cross_subsetting,
    ),
    (
        M::Feature,
        "validateFeatureCrossFeatureType",
        "KerML 8.3.3.3.4",
        validate_feature_cross_feature_type,
    ),
    (
        M::Feature,
        "validateFeatureCrossFeatureSpecialization",
        "KerML 8.3.3.3.4",
        validate_feature_cross_feature_specialization,
    ),
    (
        M::EndFeatureMembership,
        "validateEndFeatureMembershipIsEnd",
        "KerML 8.3.3.3.3",
        validate_end_feature_membership_is_end,
    ),
    (
        M::Subsetting,
        "validateSubsettingConstantConformance",
        "KerML 8.3.3.3.10",
        validate_subsetting_constant_conformance,
    ),
    (
        M::Subsetting,
        "validateSubsettingUniquenessConformance",
        "KerML 8.3.3.3.10",
        validate_subsetting_uniqueness_conformance,
    ),
    (
        M::Subsetting,
        "validateSubsettingFeaturingTypes",
        "KerML 8.3.3.3.10",
        validate_subsetting_featuring_types,
    ),
    (
        M::Redefinition,
        "validateRedefinitionEndConformance",
        "KerML 8.3.3.3.8",
        validate_redefinition_end_conformance,
    ),
    (
        M::Redefinition,
        "validateRedefinitionFeaturingTypes",
        "KerML 8.3.3.3.8",
        validate_redefinition_featuring_types,
    ),
    (
        M::Redefinition,
        "validateRedefinitionDirectionConformance",
        "KerML 8.3.3.3.8",
        validate_redefinition_direction_conformance,
    ),
    (
        M::CrossSubsetting,
        "validateCrossSubsettingCrossingFeature",
        "KerML 8.3.3.3.2",
        validate_cross_subsetting_crossing_feature,
    ),
    (
        M::CrossSubsetting,
        "validateCrossSubsettingCrossedFeature",
        "KerML 8.3.3.3.2",
        validate_cross_subsetting_crossed_feature,
    ),
    // ---- Kernel (KerML 8.3.4) ----
    (
        M::Behavior,
        "validateBehaviorSpecialization",
        "KerML 8.3.4.6.2",
        validate_behavior_specialization,
    ),
    (
        M::ParameterMembership,
        "validateParameterMembershipParameterDirection",
        "KerML 8.3.4.6.4",
        validate_parameter_membership_parameter_direction,
    ),
    (
        M::ParameterMembership,
        "validateParameterMembershipOwningType",
        "KerML 8.3.4.6.4",
        validate_parameter_membership_owning_type,
    ),
    (
        M::Function,
        "validateFunctionResultParameterMembership",
        "KerML 8.3.4.7.4",
        validate_function_result_parameter_membership,
    ),
    (
        M::Function,
        "validateFunctionResultExpressionMembership",
        "KerML 8.3.4.7.4",
        validate_function_result_expression_membership,
    ),
    (
        M::Expression,
        "validateExpressionResultParameterMembership",
        "KerML 8.3.4.7.3",
        validate_expression_result_parameter_membership,
    ),
    (
        M::Expression,
        "validateExpressionResultExpressionMembership",
        "KerML 8.3.4.7.3",
        validate_expression_result_expression_membership,
    ),
    (
        M::ResultExpressionMembership,
        "validateResultExpressionMembershipOwningType",
        "KerML 8.3.4.7.7",
        validate_result_expression_membership_owning_type,
    ),
    (
        M::ReturnParameterMembership,
        "validateReturnParameterMembershipOwningType",
        "KerML 8.3.4.7.8",
        validate_return_parameter_membership_owning_type,
    ),
    (
        M::InstantiationExpression,
        "validateInstantiationExpressionInstantiatedType",
        "KerML 8.3.4.8.7",
        validate_instantiation_expression_instantiated_type,
    ),
    (
        M::InstantiationExpression,
        "validateInstantiationExpressionResult",
        "KerML 8.3.4.8.7",
        validate_instantiation_expression_result,
    ),
    (
        M::InvocationExpression,
        "validateInvocationExpressionInstantiatedType",
        "KerML 8.3.4.8.8",
        validate_invocation_expression_instantiated_type,
    ),
    (
        M::InvocationExpression,
        "validateInvocationExpressionOwnedFeatures",
        "KerML 8.3.4.8.8",
        validate_invocation_expression_owned_features,
    ),
    (
        M::InvocationExpression,
        "validateInvocationExpressionParameterRedefinition",
        "KerML 8.3.4.8.8",
        validate_invocation_expression_parameter_redefinition,
    ),
    (
        M::InvocationExpression,
        "validateInvocationExpressionNoDuplicateParameterRedefinition",
        "KerML 8.3.4.8.8",
        validate_invocation_expression_no_duplicate_parameter_redefinition,
    ),
    (
        M::ConstructorExpression,
        "validateConstructorExpressionOwnedFeatures",
        "KerML 8.3.4.8.3",
        validate_constructor_expression_owned_features,
    ),
    (
        M::ConstructorExpression,
        "validateConstructorExpressionNoDuplicateFeatureRedefinition",
        "KerML 8.3.4.8.3",
        validate_constructor_expression_no_duplicate_feature_redefinition,
    ),
    (
        M::FeatureReferenceExpression,
        "validateFeatureReferenceExpressionReferentIsFeature",
        "KerML 8.3.4.8.5",
        validate_feature_reference_expression_referent_is_feature,
    ),
    (
        M::FeatureReferenceExpression,
        "validateFeatureReferenceExpressionResult",
        "KerML 8.3.4.8.5",
        validate_feature_reference_expression_result,
    ),
    (
        M::FeatureChainExpression,
        "validateFeatureChainExpressionOperator",
        "KerML 8.3.4.8.4",
        validate_feature_chain_expression_operator,
    ),
    (
        M::FeatureChainExpression,
        "validateFeatureChainExpressionConformance",
        "KerML 8.3.4.8.4",
        validate_feature_chain_expression_conformance,
    ),
    (
        M::SelectExpression,
        "validateSelectExpressionOperator",
        "KerML 8.3.4.8.18",
        validate_select_expression_operator,
    ),
    (
        M::CollectExpression,
        "validateCollectExpressionOperator",
        "KerML 8.3.4.8.2",
        validate_collect_expression_operator,
    ),
    (
        M::IndexExpression,
        "validateIndexExpressionOperator",
        "KerML 8.3.4.8.6",
        validate_index_expression_operator,
    ),
    (
        M::MetadataAccessExpression,
        "validateMetadataAccessExpressionReferencedElement",
        "KerML 8.3.4.8.15",
        validate_metadata_access_expression_referenced_element,
    ),
    (
        M::Flow,
        "validateFlowPayloadFeature",
        "KerML 8.3.4.9.2",
        validate_flow_payload_feature,
    ),
    (
        M::FlowEnd,
        "validateFlowEndIsEnd",
        "KerML 8.3.4.9.3",
        validate_flow_end_is_end,
    ),
    (
        M::FlowEnd,
        "validateFlowEndNestedFeature",
        "KerML 8.3.4.9.3",
        validate_flow_end_nested_feature,
    ),
    (
        M::FlowEnd,
        "validateFlowEndOwningType",
        "KerML 8.3.4.9.3",
        validate_flow_end_owning_type,
    ),
    (
        M::FeatureValue,
        "validateFeatureValueIsInitial",
        "KerML 8.3.4.10.2",
        validate_feature_value_is_initial,
    ),
    (
        M::FeatureValue,
        "validateFeatureValueOverriding",
        "KerML 8.3.4.10.2",
        validate_feature_value_overriding,
    ),
    (
        M::MultiplicityRange,
        "validateMultiplicityRangeBounds",
        "KerML 8.3.4.11.2",
        validate_multiplicity_range_bounds,
    ),
    (
        M::MultiplicityRange,
        "validateMultiplicityRangeBoundResultTypes",
        "KerML 8.3.4.11.2",
        validate_multiplicity_range_bound_result_types,
    ),
    (
        M::ElementFilterMembership,
        "validateElementFilterMembershipConditionIsBoolean",
        "KerML 8.3.4.13.2",
        validate_element_filter_membership_condition_is_boolean,
    ),
    (
        M::ElementFilterMembership,
        "validateElementFilterMembershipConditionIsModelLevelEvaluable",
        "KerML 8.3.4.13.2",
        validate_element_filter_membership_condition_is_model_level_evaluable,
    ),
    (
        M::DataType,
        "validateDataTypeSpecialization",
        "KerML 8.3.4.1.2",
        validate_data_type_specialization,
    ),
    (
        M::Class,
        "validateClassSpecialization",
        "KerML 8.3.4.2.2",
        validate_class_specialization,
    ),
    (
        M::Structure,
        "validateStructureSpecialization",
        "KerML 8.3.4.3.2",
        validate_structure_specialization,
    ),
    (
        M::Association,
        "validateAssociationStructureIntersection",
        "KerML 8.3.4.4.2",
        validate_association_structure_intersection,
    ),
    (
        M::Association,
        "validateAssociationBinarySpecialization",
        "KerML 8.3.4.4.2",
        validate_association_binary_specialization,
    ),
    (
        M::Association,
        "validateAssociationRelatedTypes",
        "KerML 8.3.4.4.2",
        validate_association_related_types,
    ),
    (
        M::Association,
        "validateAssociationEndTypes",
        "KerML 8.3.4.4.2",
        validate_association_end_types,
    ),
    (
        M::Connector,
        "validateConnectorBinarySpecialization",
        "KerML 8.3.4.5.3",
        validate_connector_binary_specialization,
    ),
    (
        M::Connector,
        "validateConnectorRelatedFeatures",
        "KerML 8.3.4.5.3",
        validate_connector_related_features,
    ),
    (
        M::BindingConnector,
        "validateBindingConnectorIsBinary",
        "KerML 8.3.4.5.2",
        validate_binding_connector_is_binary,
    ),
    (
        M::MetadataFeature,
        "validateMetadataFeatureMetaclass",
        "KerML 8.3.4.12.3",
        validate_metadata_feature_metaclass,
    ),
    (
        M::MetadataFeature,
        "validateMetadataFeatureMetaclassNotAbstract",
        "KerML 8.3.4.12.3",
        validate_metadata_feature_metaclass_not_abstract,
    ),
    (
        M::MetadataFeature,
        "validateMetadataFeatureBody",
        "KerML 8.3.4.12.3",
        validate_metadata_feature_body,
    ),
    (
        M::MetadataFeature,
        "validateMetadataFeatureAnnotatedElement",
        "KerML 8.3.4.12.3",
        validate_metadata_feature_annotated_element,
    ),
];

/// A constraint an element violates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Violation {
    /// The element.
    pub element: Global,
    /// The constraint, as `KerML.xmi` names it.
    pub constraint: &'static str,
    /// The clause of its metaclass.
    pub clause: &'static str,
}

/// The validation constraints that the elements of a file violate, implied elements included,
/// in element order.
#[salsa::tracked(returns(ref))]
pub fn violations(db: &dyn salsa::Database, project: Project, file: SourceFile) -> Vec<Violation> {
    let n = crate::view(db, project, file).len();
    let mut out = Vec::new();
    for index in 0..n as u32 {
        let x = Global { file, index };
        let m = metaclass(db, project, x);
        for (kind, constraint, clause, validation) in VALIDATIONS {
            if m.specializes(*kind) && !validation(db, project, x) {
                out.push(Violation {
                    element: x,
                    constraint,
                    clause,
                });
            }
        }
    }
    out
}

/// The diagnostics of a file's violations (ADR 0012): an error with the constraint's name as its
/// code, at the element or, for an implied one, at the nearest element written that owns it.
pub fn diagnostics(
    db: &dyn salsa::Database,
    project: Project,
    file: SourceFile,
) -> Vec<Diagnostic> {
    let lowered = crate::lowered(db, file);
    let g = &lowered.graph;
    let v = crate::view(db, project, file);
    let range = |x: Global| -> TextRange {
        let mut ix = Some(x.ix());
        while let Some(i) = ix {
            if !v.is_implied(i) {
                if let Some(s) = g.span(i) {
                    return TextRange::new(TextSize::from(s.start), TextSize::from(s.end));
                }
            }
            ix = v.owner(i).or_else(|| v.owning_related_element(i));
        }
        TextRange::default()
    };
    violations(db, project, file)
        .iter()
        .map(|x| {
            // A namespace whose members are not distinguishable is reported at the later of the
            // first two, when it is written in the file (ADR 0023).
            let at = if x.constraint == "validateNamespaceDistinguishibility" {
                indistinguishable(db, project, x.element)
                    .filter(|m| m.file == file)
                    .unwrap_or(x.element)
            } else {
                x.element
            };
            let message = format!(
                "{} {} violates {}",
                metaclass(db, project, x.element).name(),
                d::qualified_name(db, project, x.element)
                    .as_deref()
                    .map_or_else(
                        || d::path(db, project, x.element).to_string(),
                        str::to_owned
                    ),
                x.constraint
            );
            Diagnostic::error(x.constraint, range(at), message).with_clause(x.clause)
        })
        .collect()
}

// ---- helpers ----

fn is(db: &dyn salsa::Database, project: Project, x: Global, m: M) -> bool {
    metaclass(db, project, x).specializes(m)
}

fn flag(db: &dyn salsa::Database, project: Project, x: Global, a: Attr) -> bool {
    crate::view(db, project, x.file).flag(x.ix(), a)
}

fn direction(db: &dyn salsa::Database, project: Project, f: Global) -> Option<Direction> {
    let v = crate::view(db, project, f.file);
    v.values(f.ix(), Attr::Direction)
        .next()
        .map(|value| match value {
            sysml_model::Value::Enumeration("in") => Direction::In,
            sysml_model::Value::Enumeration("out") => Direction::Out,
            _ => Direction::Inout,
        })
}

/// The first source and target of a relationship.
fn ends(db: &dyn salsa::Database, project: Project, r: Global) -> (Option<Global>, Option<Global>) {
    (
        d::source(db, project, r).first().copied(),
        end(db, project, r, Slot::Target, 0),
    )
}

fn set(xs: impl IntoIterator<Item = Global>) -> HashSet<Global> {
    xs.into_iter().collect()
}

// ---- Root (KerML 8.3.2) ----

/// KerML 8.3.2.1.2 validateElementIsImpliedIncluded:
/// `ownedRelationship->exists(isImplied) implies isImpliedIncluded`.
pub fn validate_element_is_implied_included(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    let v = crate::view(db, project, x.file);
    !v.all_owned_relationships(x.ix())
        .any(|r| v.flag(r, Attr::IsImplied))
        || v.flag(x.ix(), Attr::IsImpliedIncluded)
}

/// KerML 8.3.2.3.3 validateAnnotationAnnotatingElement: an annotation owns its annotating element
/// or is owned by it, not both.
pub fn validate_annotation_annotating_element(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Annotation)
        || (d::owned_annotating_element(db, project, x).is_some()
            != d::owning_annotating_element(db, project, x).is_some())
}

/// KerML 8.3.2.3.3 validateAnnotationAnnotatedElementOwnership: an annotation is owned by its
/// annotated element (`owningAnnotatedElement`, the annotated element when it is the owning
/// related element) exactly when it owns its annotating element.
pub fn validate_annotation_annotated_element_ownership(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::Annotation) {
        return true;
    }
    let v = crate::view(db, project, x.file);
    let owning = v.owning_related_element(x.ix()).map(|e| Global {
        file: x.file,
        index: e.index() as u32,
    });
    let owning_annotated =
        owning.is_some() && owning == d::annotation_annotated_element(db, project, x);
    owning_annotated == d::owned_annotating_element(db, project, x).is_some()
}

/// KerML 8.3.2.4.2 validateImportTopLevelVisibility: an import of a root namespace is private.
pub fn validate_import_top_level_visibility(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::Import) {
        return true;
    }
    let v = crate::view(db, project, x.file);
    let top_level = v
        .owning_related_element(x.ix())
        .is_some_and(|ns| v.owner(ns).is_none());
    !top_level || v.visibility(x.ix()) == crate::Visibility::Private
}

/// KerML 8.3.2.4.5 validateNamespaceDistinguishibility: every two memberships of a namespace are
/// distinguishable. Only memberships that share a name or a short name can fail
/// `isDistinguishableFrom`, so only those are compared.
pub fn validate_namespace_distinguishibility(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Namespace) || indistinguishable(db, project, x).is_none()
}

/// The later membership, in membership order, of the first two memberships of a namespace that
/// are not distinguishable (validateNamespaceDistinguishibility), if any.
pub fn indistinguishable(db: &dyn salsa::Database, project: Project, x: Global) -> Option<Global> {
    let memberships = d::membership(db, project, x);
    let mut by_name: HashMap<String, Vec<Global>> = HashMap::new();
    for m in &memberships {
        let e = crate::derived::membership_entry(db, project, *m);
        let names: HashSet<String> = [e.name, e.short_name]
            .into_iter()
            .flatten()
            .map(|n| n.to_string())
            .collect();
        for n in names {
            by_name.entry(n).or_default().push(*m);
        }
    }
    // Two inherited memberships whose features redefine a common feature stand for one member,
    // reached through two generals, so they are distinguishable (ADR 0023).
    let owned: HashSet<Global> = set(d::owned_membership(db, project, x));
    let redefined = |m: Global| -> HashSet<Global> {
        crate::derived::membership_entry(db, project, m)
            .owned_element
            .filter(|e| is(db, project, *e, M::Feature))
            .map(|e| {
                let mut r = set(d::all_redefined_features(db, project, e));
                r.remove(&e);
                r
            })
            .unwrap_or_default()
    };
    let same_member = |a: Global, b: Global| {
        !owned.contains(&a) && !owned.contains(&b) && !redefined(a).is_disjoint(&redefined(b))
    };
    let order: HashMap<Global, usize> = memberships
        .iter()
        .enumerate()
        .map(|(i, m)| (*m, i))
        .collect();
    by_name
        .values()
        .filter_map(|ms| {
            ms.iter().enumerate().find_map(|(i, a)| {
                ms[i + 1..].iter().copied().find(|b| {
                    a != b
                        && !d::is_distinguishable_from(db, project, *a, *b)
                        && !same_member(*a, *b)
                })
            })
        })
        .min_by_key(|m| order.get(m).copied().unwrap_or(usize::MAX))
}

// ---- Core: types (KerML 8.3.3.1.10) ----

/// KerML 8.3.3.1.10 validateTypeAtMostOneConjugator:
/// `ownedRelationship->selectByKind(Conjugation)->size() <= 1`.
pub fn validate_type_at_most_one_conjugator(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::Type) {
        return true;
    }
    let v = crate::view(db, project, x.file);
    v.all_owned_relationships(x.ix())
        .filter(|r| v.metaclass(*r).specializes(M::Conjugation))
        .count()
        <= 1
}

/// KerML 8.3.3.1.10 validateTypeOwnedMultiplicity:
/// `ownedMember->selectByKind(Multiplicity)->size() <= 1`.
pub fn validate_type_owned_multiplicity(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Type)
        || d::owned_member(db, project, x)
            .into_iter()
            .filter(|m| is(db, project, *m, M::Multiplicity))
            .count()
            <= 1
}

/// KerML 8.3.3.1.10 validateTypeOwnedUnioningNotOne: `ownedUnioning->size() <> 1`.
pub fn validate_type_owned_unioning_not_one(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Type) || d::owned_unioning(db, project, x).len() != 1
}

/// KerML 8.3.3.1.10 validateTypeOwnedIntersectingNotOne: `ownedIntersecting->size() <> 1`.
pub fn validate_type_owned_intersecting_not_one(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Type) || d::owned_intersecting(db, project, x).len() != 1
}

/// KerML 8.3.3.1.10 validateTypeOwnedDifferencingNotOne: `ownedDifferencing->size() <> 1`.
pub fn validate_type_owned_differencing_not_one(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Type) || d::owned_differencing(db, project, x).len() != 1
}

/// KerML 8.3.3.1.10 validateTypeUnioningTypesNotSelf: `unioningType->excludes(self)`.
pub fn validate_type_unioning_types_not_self(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Type) || !d::unioning_type(db, project, x).contains(&x)
}

/// KerML 8.3.3.1.10 validateTypeIntersectingTypesNotSelf: `intersectingType->excludes(self)`.
pub fn validate_type_intersecting_types_not_self(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Type) || !d::intersecting_type(db, project, x).contains(&x)
}

/// KerML 8.3.3.1.10 validateTypeDifferencingTypesNotSelf: `differencingType->excludes(self)`.
pub fn validate_type_differencing_types_not_self(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Type) || !d::differencing_type(db, project, x).contains(&x)
}

/// KerML 8.3.3.1.8 validateSpecificationSpecificNotConjugated: `not specific.isConjugated`.
pub fn validate_specification_specific_not_conjugated(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Specialization)
        || ends(db, project, x)
            .0
            .is_none_or(|s| !d::is_conjugated(db, project, s))
}

// ---- Core: classifiers (KerML 8.3.3.2.2) ----

/// KerML 8.3.3.2.2 validateClassifierMultiplicityDomain: a classifier's multiplicity has no
/// featuring types.
pub fn validate_classifier_multiplicity_domain(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Classifier)
        || d::multiplicity(db, project, x)
            .is_none_or(|m| d::featuring_type(db, project, m).is_empty())
}

// ---- Core: features (KerML 8.3.3.3) ----

/// KerML 8.3.3.3.4 validateFeatureMultiplicityDomain: a feature's multiplicity has the feature's
/// featuring types (compared without order, as in the type-featuring constraints; ADR 0020).
pub fn validate_feature_multiplicity_domain(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Feature)
        || d::multiplicity(db, project, x).is_none_or(|m| {
            set(d::featuring_type(db, project, m).iter().copied())
                == set(d::featuring_type(db, project, x).iter().copied())
        })
}

/// KerML 8.3.3.3.4 validateFeatureIsVariable: a variable feature has an owning type that
/// specializes `Occurrences::Occurrence` (the OCL's `specializes('…')`, read as
/// `specializesFromLibrary`).
pub fn validate_feature_is_variable(db: &dyn salsa::Database, project: Project, x: Global) -> bool {
    !is(db, project, x, M::Feature)
        || !flag(db, project, x, Attr::IsVariable)
        || d::owning_type(db, project, x)
            .is_some_and(|t| d::specializes_from_library(db, project, t, "Occurrences::Occurrence"))
}

/// KerML 8.3.3.3.4 validateFeatureConstantIsVariable: `isConstant implies isVariable`.
pub fn validate_feature_constant_is_variable(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Feature)
        || !flag(db, project, x, Attr::IsConstant)
        || flag(db, project, x, Attr::IsVariable)
}

/// KerML 8.3.3.3.4 validateFeaturePortionNotVariable: `isPortion implies not isVariable`.
pub fn validate_feature_portion_not_variable(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Feature)
        || !flag(db, project, x, Attr::IsPortion)
        || !flag(db, project, x, Attr::IsVariable)
}

/// KerML 8.3.3.3.4 validateFeatureEndNoDirection: an end has no direction (the OCL's `implied`
/// read as `implies`).
pub fn validate_feature_end_no_direction(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Feature)
        || !flag(db, project, x, Attr::IsEnd)
        || direction(db, project, x).is_none()
}

/// KerML 8.3.3.3.4 validateFeatureEndIsConstant: `isEnd and isVariable implies isConstant`.
pub fn validate_feature_end_is_constant(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Feature)
        || !(flag(db, project, x, Attr::IsEnd) && flag(db, project, x, Attr::IsVariable))
        || flag(db, project, x, Attr::IsConstant)
}

/// KerML 8.3.3.3.4 validateFeatureEndNotDerivedAbstractCompositeOrPortion: an end is not
/// derived, abstract, composite or a portion.
pub fn validate_feature_end_not_derived_abstract_composite_or_portion(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Feature)
        || !flag(db, project, x, Attr::IsEnd)
        || ![
            Attr::IsDerived,
            Attr::IsAbstract,
            Attr::IsComposite,
            Attr::IsPortion,
        ]
        .into_iter()
        .any(|a| flag(db, project, x, a))
}

/// KerML 8.3.3.3.4 validateFeatureEndMultiplicity: among the supertypes of an end's
/// multiplicities is a MultiplicityRange with bounds 1..1.
pub fn validate_feature_end_multiplicity(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::Feature) || !flag(db, project, x, Attr::IsEnd) {
        return true;
    }
    d::multiplicities(db, project, x)
        .into_iter()
        .flat_map(|m| d::all_supertypes(db, project, m).clone())
        .filter(|s| is(db, project, *s, M::MultiplicityRange))
        .any(|r| d::has_bounds(db, project, r, d::Natural::Value(1), d::Natural::Value(1)))
}

/// KerML 8.3.3.3.4 validateFeatureChainingFeatureNotOne: `chainingFeature->size() <> 1`.
pub fn validate_feature_chaining_feature_not_one(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Feature) || d::chaining_feature(db, project, x).len() != 1
}

/// KerML 8.3.3.3.4 validateFeatureChainingFeaturesNotSelf: `chainingFeature->excludes(self)`.
pub fn validate_feature_chaining_features_not_self(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Feature) || !d::chaining_feature(db, project, x).contains(&x)
}

/// KerML 8.3.3.3.4 validateFeatureChainingFeatureConformance: each chaining feature after the
/// first is featured within the one before.
pub fn validate_feature_chaining_feature_conformance(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Feature)
        || d::chaining_feature(db, project, x)
            .windows(2)
            .all(|w| d::is_featured_within(db, project, w[1], Some(w[0])))
}

fn owned_subsettings_of(db: &dyn salsa::Database, project: Project, x: Global, m: M) -> usize {
    d::owned_subsetting(db, project, x)
        .into_iter()
        .filter(|r| is(db, project, *r, m))
        .count()
}

/// KerML 8.3.3.3.4 validateFeatureOwnedReferenceSubsetting: at most one owned ReferenceSubsetting.
pub fn validate_feature_owned_reference_subsetting(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Feature)
        || owned_subsettings_of(db, project, x, M::ReferenceSubsetting) <= 1
}

/// KerML 8.3.3.3.4 validateFeatureOwnedCrossSubsetting: at most one owned CrossSubsetting.
pub fn validate_feature_owned_cross_subsetting(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Feature) || owned_subsettings_of(db, project, x, M::CrossSubsetting) <= 1
}

/// KerML 8.3.3.3.4 validateFeatureCrossFeatureType: a cross feature has the feature's types.
pub fn validate_feature_cross_feature_type(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Feature)
        || d::cross_feature(db, project, x)
            .is_none_or(|c| set(d::r#type(db, project, c)) == set(d::r#type(db, project, x)))
}

/// KerML 8.3.3.3.4 validateFeatureCrossFeatureSpecialization: a cross feature specializes the
/// cross features of the features the feature redefines.
pub fn validate_feature_cross_feature_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::Feature) {
        return true;
    }
    let Some(c) = d::cross_feature(db, project, x) else {
        return true;
    };
    d::owned_redefinition(db, project, x)
        .into_iter()
        .filter_map(|r| end(db, project, r, Slot::Target, 0))
        .filter_map(|r| d::cross_feature(db, project, r))
        .all(|f| d::specializes(db, project, c, f))
}

/// KerML 8.3.3.3.3 validateEndFeatureMembershipIsEnd: `ownedMemberFeature.isEnd`.
pub fn validate_end_feature_membership_is_end(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::EndFeatureMembership) {
        return true;
    }
    let v = crate::view(db, project, x.file);
    v.owned_related_elements(x.ix())
        .first()
        .is_none_or(|f| v.flag(*f, Attr::IsEnd))
}

/// KerML 8.3.3.3.10 validateSubsettingConstantConformance: a variable feature that subsets a
/// constant one is constant.
pub fn validate_subsetting_constant_conformance(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::Subsetting) {
        return true;
    }
    let (Some(s), Some(t)) = ends(db, project, x) else {
        return true;
    };
    !(flag(db, project, t, Attr::IsConstant) && flag(db, project, s, Attr::IsVariable))
        || flag(db, project, s, Attr::IsConstant)
}

/// KerML 8.3.3.3.10 validateSubsettingUniquenessConformance: a feature that subsets a unique
/// feature is unique.
pub fn validate_subsetting_uniqueness_conformance(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::Subsetting) {
        return true;
    }
    let (Some(s), Some(t)) = ends(db, project, x) else {
        return true;
    };
    !flag(db, project, t, Attr::IsUnique) || flag(db, project, s, Attr::IsUnique)
}

/// KerML 8.3.3.3.10 validateSubsettingFeaturingTypes: `subsettingFeature.canAccess(
/// subsettedFeature)`, for the subsettings written. The implied ones that the semantic
/// constraints require reach into nested expressions, which `canAccess` does not see, so it does
/// not apply to the overlay's (ADR 0023).
pub fn validate_subsetting_featuring_types(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::Subsetting) || crate::view(db, project, x.file).is_implied(x.ix()) {
        return true;
    }
    let (Some(s), Some(t)) = ends(db, project, x) else {
        return true;
    };
    d::can_access(db, project, s, t)
}

/// KerML 8.3.3.3.8 validateRedefinitionEndConformance: a feature that redefines an end is an end,
/// for the redefinitions written. A FeatureChainExpression's sourceTargetFeature, which is no end,
/// redefines the target feature, an end or not
/// (checkFeatureChainExpressionSourceTargetRedefinition), so it does not apply to the overlay's
/// (ADR 0023).
pub fn validate_redefinition_end_conformance(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::Redefinition) || crate::view(db, project, x.file).is_implied(x.ix()) {
        return true;
    }
    let (Some(s), Some(t)) = ends(db, project, x) else {
        return true;
    };
    !flag(db, project, t, Attr::IsEnd) || flag(db, project, s, Attr::IsEnd)
}

/// The featuring types of a feature as validateRedefinitionFeaturingTypes reads them: the owning
/// type of a variable feature, else its featuring types with `Base::Anything`.
fn redefinition_featuring(
    db: &dyn salsa::Database,
    project: Project,
    f: Global,
) -> HashSet<Global> {
    if flag(db, project, f, Attr::IsVariable) {
        return d::owning_type(db, project, f).into_iter().collect();
    }
    let mut out = set(d::featuring_type(db, project, f).iter().copied());
    out.extend(d::library(db, project, "Base::Anything"));
    out
}

/// KerML 8.3.3.3.8 validateRedefinitionFeaturingTypes: a feature does not redefine one with the
/// same featuring types (the OCL's `redefinedFeaturingType` read as `redefinedFeaturingTypes`, and
/// `modelElement` as `memberElement`).
pub fn validate_redefinition_featuring_types(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::Redefinition) {
        return true;
    }
    let (Some(s), Some(t)) = ends(db, project, x) else {
        return true;
    };
    redefinition_featuring(db, project, s) != redefinition_featuring(db, project, t)
}

/// KerML 8.3.3.3.8 validateRedefinitionDirectionConformance: a feature redefining an `in` or `out`
/// feature of each of its featuring types has that direction, and one redefining an `inout`
/// feature has a direction.
pub fn validate_redefinition_direction_conformance(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::Redefinition) {
        return true;
    }
    let (Some(s), Some(t)) = ends(db, project, x) else {
        return true;
    };
    let featuring: Vec<Global> = if flag(db, project, s, Attr::IsVariable) {
        d::owning_type(db, project, s).into_iter().collect()
    } else {
        d::featuring_type(db, project, s).clone()
    };
    let own = direction(db, project, s);
    featuring
        .into_iter()
        .all(|ty| match d::direction_of(db, project, ty, t) {
            Some(Direction::In) => own == Some(Direction::In),
            Some(Direction::Out) => own == Some(Direction::Out),
            Some(Direction::Inout) => own.is_some(),
            None => true,
        })
}

/// KerML 8.3.3.3.2 validateCrossSubsettingCrossingFeature: the crossing feature is an end of an
/// owning type with more than one end.
pub fn validate_cross_subsetting_crossing_feature(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::CrossSubsetting) {
        return true;
    }
    let Some(s) = ends(db, project, x).0 else {
        return true;
    };
    flag(db, project, s, Attr::IsEnd)
        && d::owning_type(db, project, s).is_some_and(|t| d::end_feature(db, project, t).len() > 1)
}

/// KerML 8.3.3.3.2 validateCrossSubsettingCrossedFeature: the crossed feature of an end's
/// CrossSubsetting is a chain of two features, the first of them the other end for a binary owning
/// type. The OCL asks for the other end only of a chain of two; the text also requires the two
/// (ADR 0023).
pub fn validate_cross_subsetting_crossed_feature(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::CrossSubsetting) {
        return true;
    }
    let (Some(s), Some(crossed)) = ends(db, project, x) else {
        return true;
    };
    if !flag(db, project, s, Attr::IsEnd) {
        return true;
    }
    let Some(t) = d::owning_type(db, project, s) else {
        return true;
    };
    let ends = d::end_feature(db, project, t);
    let chaining = d::chaining_feature(db, project, crossed);
    chaining.len() == 2 && (ends.len() != 2 || ends.iter().find(|e| **e != s) == Some(&chaining[0]))
}

// ---- Kernel (KerML 8.3.4) ----

/// The generals of a type's owned specializations, written and implied.
fn generals(db: &dyn salsa::Database, project: Project, t: Global) -> Vec<Global> {
    d::owned_specialization(db, project, t)
        .into_iter()
        .filter_map(|r| end(db, project, r, Slot::Target, 0))
        .filter(|g| *g != t)
        .collect()
}

/// `owningType` of a membership: its owning namespace, if a Type (KerML 8.3.3.1.5).
fn membership_owning_type(db: &dyn salsa::Database, project: Project, m: Global) -> Option<Global> {
    let v = crate::view(db, project, m.file);
    v.owning_related_element(m.ix())
        .map(|e| Global {
            file: m.file,
            index: e.index() as u32,
        })
        .filter(|t| is(db, project, *t, M::Type))
}

fn owned_related(db: &dyn salsa::Database, project: Project, r: Global) -> Option<Global> {
    let v = crate::view(db, project, r.file);
    v.owned_related_elements(r.ix()).first().map(|e| Global {
        file: r.file,
        index: e.index() as u32,
    })
}

fn operator(db: &dyn salsa::Database, project: Project, x: Global) -> Option<&str> {
    crate::view(db, project, x.file).string(x.ix(), Attr::Operator)
}

/// The owned features of an expression, without the binding connectors that the
/// binding-connector constraints imply (ADR 0023).
fn written_features(db: &dyn salsa::Database, project: Project, x: Global) -> Vec<Global> {
    let v = crate::view(db, project, x.file);
    d::owned_feature(db, project, x)
        .into_iter()
        .filter(|f| !(v.is_implied(f.ix()) && v.metaclass(f.ix()).specializes(M::BindingConnector)))
        .collect()
}

/// The features a feature redefines with its owned redefinitions (the OCL's `redefinition`).
fn redefined(db: &dyn salsa::Database, project: Project, f: Global) -> Vec<Global> {
    d::owned_redefinition(db, project, f)
        .into_iter()
        .filter_map(|r| end(db, project, r, Slot::Target, 0))
        .collect()
}

/// KerML 8.3.4.6.2 validateBehaviorSpecialization: a behavior specializes no Structure.
pub fn validate_behavior_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Behavior)
        || generals(db, project, x)
            .into_iter()
            .all(|g| !is(db, project, g, M::Structure))
}

/// KerML 8.3.4.6.4 validateParameterMembershipParameterDirection: the parameter has the
/// membership's `parameterDirection()`.
pub fn validate_parameter_membership_parameter_direction(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::ParameterMembership) {
        return true;
    }
    owned_related(db, project, x)
        .is_none_or(|p| direction(db, project, p) == Some(d::parameter_direction(db, project, x)))
}

/// KerML 8.3.4.6.4 validateParameterMembershipOwningType: a parameter is owned by a Behavior or a
/// Step, or by the result of a ConstructorExpression.
pub fn validate_parameter_membership_owning_type(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::ParameterMembership) {
        return true;
    }
    let Some(t) = membership_owning_type(db, project, x) else {
        return false;
    };
    is(db, project, t, M::Behavior)
        || is(db, project, t, M::Step)
        || (d::owning_membership(db, project, t)
            .is_some_and(|m| is(db, project, m, M::ReturnParameterMembership))
            && d::owning_namespace(db, project, t)
                .is_some_and(|n| is(db, project, n, M::ConstructorExpression)))
}

fn result_parameter_memberships(db: &dyn salsa::Database, project: Project, x: Global) -> usize {
    d::feature_membership(db, project, x)
        .into_iter()
        .filter(|m| is(db, project, *m, M::ReturnParameterMembership))
        .count()
}

fn result_expression_memberships(db: &dyn salsa::Database, project: Project, x: Global) -> usize {
    d::membership(db, project, x)
        .into_iter()
        .filter(|m| is(db, project, *m, M::ResultExpressionMembership))
        .count()
}

/// KerML 8.3.4.7.4 validateFunctionResultParameterMembership: exactly one
/// ReturnParameterMembership, owned or inherited.
pub fn validate_function_result_parameter_membership(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Function) || result_parameter_memberships(db, project, x) == 1
}

/// KerML 8.3.4.7.4 validateFunctionResultExpressionMembership: at most one
/// ResultExpressionMembership.
pub fn validate_function_result_expression_membership(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Function) || result_expression_memberships(db, project, x) <= 1
}

/// KerML 8.3.4.7.3 validateExpressionResultParameterMembership: exactly one
/// ReturnParameterMembership, owned or inherited.
pub fn validate_expression_result_parameter_membership(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Expression) || result_parameter_memberships(db, project, x) == 1
}

/// KerML 8.3.4.7.3 validateExpressionResultExpressionMembership: at most one
/// ResultExpressionMembership.
pub fn validate_expression_result_expression_membership(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Expression) || result_expression_memberships(db, project, x) <= 1
}

fn owned_by_function_or_expression(db: &dyn salsa::Database, project: Project, m: Global) -> bool {
    membership_owning_type(db, project, m)
        .is_some_and(|t| is(db, project, t, M::Function) || is(db, project, t, M::Expression))
}

/// KerML 8.3.4.7.7 validateResultExpressionMembershipOwningType: owned by a Function or an
/// Expression.
pub fn validate_result_expression_membership_owning_type(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::ResultExpressionMembership)
        || owned_by_function_or_expression(db, project, x)
}

/// KerML 8.3.4.7.8 validateReturnParameterMembershipOwningType: owned by a Function or an
/// Expression.
pub fn validate_return_parameter_membership_owning_type(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::ReturnParameterMembership)
        || owned_by_function_or_expression(db, project, x)
}

/// KerML 8.3.4.8.7 validateInstantiationExpressionInstantiatedType: `instantiatedType() <> null`.
pub fn validate_instantiation_expression_instantiated_type(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::InstantiationExpression)
        || d::instantiated_type(db, project, x).is_some()
}

/// KerML 8.3.4.8.7 validateInstantiationExpressionResult: `result.owningType = self`.
pub fn validate_instantiation_expression_result(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::InstantiationExpression)
        || d::result(db, project, x).is_some_and(|r| d::owning_type(db, project, r) == Some(x))
}

/// KerML 8.3.4.8.8 validateInvocationExpressionInstantiatedType: the instantiated type is a
/// Behavior, or a Feature with one type, a Behavior (the OCL's `type->size(1)`, read as
/// `type->size() = 1`).
pub fn validate_invocation_expression_instantiated_type(
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
    let types = || d::r#type(db, project, t);
    is(db, project, t, M::Behavior)
        || (is(db, project, t, M::Feature)
            && types().len() == 1
            && types().into_iter().any(|y| is(db, project, y, M::Behavior)))
}

/// KerML 8.3.4.8.8 validateInvocationExpressionOwnedFeatures: every owned feature other than the
/// result is an `in` parameter, or a named argument, which the grammar gives no direction. The
/// binding connector that checkInvocationExpressionBehaviorBindingConnector implies is not counted
/// (ADR 0023).
pub fn validate_invocation_expression_owned_features(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::InvocationExpression) {
        return true;
    }
    let result = d::result(db, project, x);
    written_features(db, project, x).into_iter().all(|f| {
        Some(f) == result
            || direction(db, project, f) == Some(Direction::In)
            || named_argument(db, project, x, f)
    })
}

/// The inputs of an invocation as the parameter-redefinition constraints read them: its
/// arguments, the owned features with direction `in` or `inout` and the named arguments. `input`
/// would also take the inputs it inherits from the instantiated type, which redefine nothing, as
/// an argument left to its default does (ADR 0023).
fn owned_inputs(db: &dyn salsa::Database, project: Project, x: Global) -> Vec<Global> {
    written_features(db, project, x)
        .into_iter()
        .filter(|f| {
            matches!(
                direction(db, project, *f),
                Some(Direction::In | Direction::Inout)
            ) || named_argument(db, project, x, *f)
        })
        .collect()
}

/// Whether an owned feature of an invocation is a named argument (KerML 8.2.5.8.3
/// NamedArgument): one without a direction, which the grammar does not give it, that redefines an
/// input of the instantiated type. It is read as an input (ADR 0023).
fn named_argument(db: &dyn salsa::Database, project: Project, x: Global, f: Global) -> bool {
    direction(db, project, f).is_none()
        && d::instantiated_type(db, project, x).is_some_and(|t| {
            let inputs = set(d::input(db, project, t));
            redefined(db, project, f).iter().any(|r| inputs.contains(r))
        })
}

/// KerML 8.3.4.8.8 validateInvocationExpressionParameterRedefinition: each input redefines exactly
/// one input of the instantiated type.
pub fn validate_invocation_expression_parameter_redefinition(
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
    let parameters = set(d::input(db, project, t));
    owned_inputs(db, project, x).into_iter().all(|inp| {
        set(redefined(db, project, inp))
            .intersection(&parameters)
            .count()
            == 1
    })
}

/// No two features redefine a common feature of `features`.
fn no_common_redefinition(
    db: &dyn salsa::Database,
    project: Project,
    xs: &[Global],
    features: &HashSet<Global>,
) -> bool {
    let mut seen: HashSet<Global> = HashSet::new();
    xs.iter().all(|f| {
        set(redefined(db, project, *f))
            .intersection(features)
            .all(|r| seen.insert(*r))
    })
}

/// KerML 8.3.4.8.8 validateInvocationExpressionNoDuplicateParameterRedefinition: no two inputs
/// redefine a common feature of the instantiated type.
pub fn validate_invocation_expression_no_duplicate_parameter_redefinition(
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
    let features = set(d::feature(db, project, t));
    no_common_redefinition(db, project, &owned_inputs(db, project, x), &features)
}

/// KerML 8.3.4.8.3 validateConstructorExpressionOwnedFeatures: no owned feature but the result
/// (the OCL's `ownedFeatures` read as `ownedFeature`).
pub fn validate_constructor_expression_owned_features(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::ConstructorExpression) {
        return true;
    }
    let result = d::result(db, project, x);
    written_features(db, project, x)
        .into_iter()
        .all(|f| Some(f) == result)
}

/// KerML 8.3.4.8.3 validateConstructorExpressionNoDuplicateFeatureRedefinition: no two features of
/// the result redefine a common public feature of the instantiated type.
pub fn validate_constructor_expression_no_duplicate_feature_redefinition(
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
    let features: HashSet<Global> = d::feature(db, project, t)
        .into_iter()
        .filter(|f| {
            d::owning_membership(db, project, *f).is_some_and(|m| {
                crate::view(db, project, m.file).visibility(m.ix()) == crate::Visibility::Public
            })
        })
        .collect();
    no_common_redefinition(db, project, &d::owned_feature(db, project, r), &features)
}

/// KerML 8.3.4.8.5 validateFeatureReferenceExpressionReferentIsFeature: the member element of the
/// first owned membership that is not a ParameterMembership is a Feature.
pub fn validate_feature_reference_expression_referent_is_feature(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::FeatureReferenceExpression) {
        return true;
    }
    d::owned_membership(db, project, x)
        .into_iter()
        .find(|m| !is(db, project, *m, M::ParameterMembership))
        .and_then(|m| crate::derived::membership_element(db, project, m))
        .is_some_and(|e| is(db, project, e, M::Feature))
}

/// KerML 8.3.4.8.5 validateFeatureReferenceExpressionResult: `result.owningType = self`.
pub fn validate_feature_reference_expression_result(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::FeatureReferenceExpression)
        || d::result(db, project, x).is_some_and(|r| d::owning_type(db, project, r) == Some(x))
}

/// KerML 8.3.4.8.4 validateFeatureChainExpressionOperator: `operator = '.'`.
pub fn validate_feature_chain_expression_operator(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::FeatureChainExpression) || operator(db, project, x) == Some(".")
}

/// KerML 8.3.4.8.4 validateFeatureChainExpressionConformance: the target feature is featured
/// within the result of the first argument.
pub fn validate_feature_chain_expression_conformance(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::FeatureChainExpression) {
        return true;
    }
    let Some(a) = d::argument(db, project, x).first().copied() else {
        return true;
    };
    match (
        d::chain_target_feature(db, project, x),
        d::result(db, project, a),
    ) {
        (Some(t), Some(r)) => d::is_featured_within(db, project, t, Some(r)),
        _ => true,
    }
}

/// KerML 8.3.4.8.18 validateSelectExpressionOperator: `operator = 'select'`.
pub fn validate_select_expression_operator(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::SelectExpression) || operator(db, project, x) == Some("select")
}

/// KerML 8.3.4.8.2 validateCollectExpressionOperator: `operator = 'collect'`.
pub fn validate_collect_expression_operator(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::CollectExpression) || operator(db, project, x) == Some("collect")
}

/// KerML 8.3.4.8.6 validateIndexExpressionOperator: `operator = '#'`.
pub fn validate_index_expression_operator(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::IndexExpression) || operator(db, project, x) == Some("#")
}

/// KerML 8.3.4.8.15 validateMetadataAccessExpressionReferencedElement: an owned membership that is
/// not a FeatureMembership.
pub fn validate_metadata_access_expression_referenced_element(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::MetadataAccessExpression)
        || d::owned_membership(db, project, x)
            .into_iter()
            .any(|m| !is(db, project, m, M::FeatureMembership))
}

/// KerML 8.3.4.9.2 validateFlowPayloadFeature: at most one owned PayloadFeature.
pub fn validate_flow_payload_feature(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Flow)
        || d::owned_feature(db, project, x)
            .into_iter()
            .filter(|f| is(db, project, *f, M::PayloadFeature))
            .count()
            <= 1
}

/// KerML 8.3.4.9.3 validateFlowEndIsEnd: `isEnd`.
pub fn validate_flow_end_is_end(db: &dyn salsa::Database, project: Project, x: Global) -> bool {
    !is(db, project, x, M::FlowEnd) || flag(db, project, x, Attr::IsEnd)
}

/// KerML 8.3.4.9.3 validateFlowEndNestedFeature: exactly one owned feature.
pub fn validate_flow_end_nested_feature(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::FlowEnd) || d::owned_feature(db, project, x).len() == 1
}

/// KerML 8.3.4.9.3 validateFlowEndOwningType: the owning type is a Flow.
pub fn validate_flow_end_owning_type(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::FlowEnd)
        || d::owning_type(db, project, x).is_some_and(|t| is(db, project, t, M::Flow))
}

/// `featureWithValue` of a FeatureValue: its owning related element.
fn feature_with_value(db: &dyn salsa::Database, project: Project, fv: Global) -> Option<Global> {
    let v = crate::view(db, project, fv.file);
    v.owning_related_element(fv.ix()).map(|e| Global {
        file: fv.file,
        index: e.index() as u32,
    })
}

/// KerML 8.3.4.10.2 validateFeatureValueIsInitial: an initial value is of a variable feature.
pub fn validate_feature_value_is_initial(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::FeatureValue)
        || !flag(db, project, x, Attr::IsInitial)
        || feature_with_value(db, project, x)
            .is_some_and(|f| flag(db, project, f, Attr::IsVariable))
}

/// KerML 8.3.4.10.2 validateFeatureValueOverriding: every feature that the feature with the value
/// redefines, directly or indirectly, has only default values (the OCL's `redefinition`, read as
/// `ownedRedefinition`).
pub fn validate_feature_value_overriding(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::FeatureValue) {
        return true;
    }
    let Some(f) = feature_with_value(db, project, x) else {
        return true;
    };
    let mut seen: HashSet<Global> = HashSet::new();
    let mut stack = redefined(db, project, f);
    while let Some(r) = stack.pop() {
        if !seen.insert(r) {
            continue;
        }
        let v = crate::view(db, project, r.file);
        let non_default = v
            .all_owned_relationships(r.ix())
            .any(|fv| v.metaclass(fv).specializes(M::FeatureValue) && !v.flag(fv, Attr::IsDefault));
        if non_default {
            return false;
        }
        stack.extend(redefined(db, project, r));
    }
    true
}

/// KerML 8.3.4.11.2 validateMultiplicityRangeBounds: the owned members begin with the lower bound,
/// if any, then the upper bound.
pub fn validate_multiplicity_range_bounds(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::MultiplicityRange) {
        return true;
    }
    let members = d::owned_member(db, project, x);
    let upper = d::upper_bound(db, project, x);
    match d::lower_bound(db, project, x) {
        None => !members.is_empty() && Some(members[0]) == upper,
        Some(l) => members.len() > 1 && members[0] == l && Some(members[1]) == upper,
    }
}

/// KerML 8.3.4.11.2 validateMultiplicityRangeBoundResultTypes: each bound's result is an Integer,
/// and not negative. The OCL takes the value as an UnlimitedNatural, which a negative number is
/// not, so the value the bound evaluates to is read instead (ADR 0023).
pub fn validate_multiplicity_range_bound_result_types(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::MultiplicityRange) {
        return true;
    }
    d::bound(db, project, x).into_iter().all(|b| {
        d::result(db, project, b)
            .is_some_and(|r| d::specializes_from_library(db, project, r, "ScalarValues::Integer"))
            && !matches!(d::evaluate(db, project, b, None)[..], [d::Item::Integer(n)] if n < 0)
    })
}

fn condition(db: &dyn salsa::Database, project: Project, m: Global) -> Option<Global> {
    owned_related(db, project, m)
}

/// KerML 8.3.4.13.2 validateElementFilterMembershipConditionIsBoolean: the condition's result is a
/// Boolean.
pub fn validate_element_filter_membership_condition_is_boolean(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::ElementFilterMembership)
        || condition(db, project, x).is_some_and(|c| {
            d::result(db, project, c).is_some_and(|r| {
                d::specializes_from_library(db, project, r, "ScalarValues::Boolean")
            })
        })
}

/// KerML 8.3.4.13.2 validateElementFilterMembershipConditionIsModelLevelEvaluable: the condition is
/// model-level evaluable.
pub fn validate_element_filter_membership_condition_is_model_level_evaluable(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::ElementFilterMembership)
        || condition(db, project, x).is_some_and(|c| d::is_model_level_evaluable(db, project, c))
}

/// KerML 8.3.4.1.2 validateDataTypeSpecialization: a data type specializes no Class or
/// Association.
pub fn validate_data_type_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::DataType)
        || generals(db, project, x)
            .into_iter()
            .all(|g| !is(db, project, g, M::Class) && !is(db, project, g, M::Association))
}

/// KerML 8.3.4.2.2 validateClassSpecialization: a class specializes no DataType and, unless it is
/// an Association, no Association.
pub fn validate_class_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::Class) {
        return true;
    }
    let association = is(db, project, x, M::Association);
    generals(db, project, x).into_iter().all(|g| {
        !is(db, project, g, M::DataType) && (association || !is(db, project, g, M::Association))
    })
}

/// KerML 8.3.4.3.2 validateStructureSpecialization: a structure specializes no Behavior.
pub fn validate_structure_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Structure)
        || generals(db, project, x)
            .into_iter()
            .all(|g| !is(db, project, g, M::Behavior))
}

/// KerML 8.3.4.4.2 validateAssociationStructureIntersection: an association is a Structure only as
/// an AssociationStructure.
pub fn validate_association_structure_intersection(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Association)
        || is(db, project, x, M::Structure) == is(db, project, x, M::AssociationStructure)
}

/// KerML 8.3.4.4.2 validateAssociationBinarySpecialization: one with more than two ends does not
/// specialize `Links::BinaryLink`.
pub fn validate_association_binary_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Association)
        || d::end_feature(db, project, x).len() <= 2
        || !d::specializes_from_library(db, project, x, "Links::BinaryLink")
}

/// KerML 8.3.4.4.2 validateAssociationRelatedTypes: a concrete association relates two types or
/// more.
pub fn validate_association_related_types(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Association)
        || flag(db, project, x, Attr::IsAbstract)
        || d::related_type(db, project, x).len() >= 2
}

/// KerML 8.3.4.4.2 validateAssociationEndTypes: each owned end has exactly one type.
pub fn validate_association_end_types(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Association)
        || d::owned_end_feature(db, project, x)
            .into_iter()
            .all(|e| d::r#type(db, project, e).len() == 1)
}

/// KerML 8.3.4.5.3 validateConnectorBinarySpecialization: one with more than two ends does not
/// specialize `Links::BinaryLink`.
pub fn validate_connector_binary_specialization(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Connector)
        || d::end_feature(db, project, x).len() <= 2
        || !d::specializes_from_library(db, project, x, "Links::BinaryLink")
}

/// KerML 8.3.4.5.3 validateConnectorRelatedFeatures: a concrete connector relates two features or
/// more.
pub fn validate_connector_related_features(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::Connector)
        || flag(db, project, x, Attr::IsAbstract)
        || d::related_feature(db, project, x).len() >= 2
}

/// KerML 8.3.4.5.2 validateBindingConnectorIsBinary: exactly two related features.
pub fn validate_binding_connector_is_binary(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::BindingConnector) || d::related_feature(db, project, x).len() == 2
}

/// KerML 8.3.4.12.3 validateMetadataFeatureMetaclass: exactly one of its types is a Metaclass
/// (the OCL's `.size()`, read as `->size()`).
pub fn validate_metadata_feature_metaclass(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::MetadataFeature)
        || d::r#type(db, project, x)
            .into_iter()
            .filter(|t| is(db, project, *t, M::Metaclass))
            .count()
            == 1
}

/// KerML 8.3.4.12.3 validateMetadataFeatureMetaclassNotAbstract: its metaclass is not abstract.
pub fn validate_metadata_feature_metaclass_not_abstract(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    !is(db, project, x, M::MetadataFeature)
        || d::metaclass_of(db, project, x).is_none_or(|m| !flag(db, project, m, Attr::IsAbstract))
}

/// KerML 8.3.4.12.3 validateMetadataFeatureBody: each feature in the body, at any depth, has no
/// declared name, redefines one feature, and has no value or one that is model-level evaluable.
/// The text is followed where the OCL's precedence would make the names imply the rest (ADR 0023).
pub fn validate_metadata_feature_body(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::MetadataFeature) {
        return true;
    }
    let mut stack = d::owned_feature(db, project, x);
    let mut seen: HashSet<Global> = HashSet::new();
    while let Some(f) = stack.pop() {
        if !seen.insert(f) {
            continue;
        }
        let v = crate::view(db, project, f.file);
        if v.is_implied(f.ix()) {
            continue;
        }
        let named = v.string(f.ix(), Attr::DeclaredName).is_some()
            || v.string(f.ix(), Attr::DeclaredShortName).is_some();
        let value_ok = d::valuation(db, project, f)
            .is_none_or(|value| d::is_model_level_evaluable(db, project, value));
        if named || !value_ok || set(redefined(db, project, f)).len() != 1 {
            return false;
        }
        stack.extend(d::owned_feature(db, project, f));
    }
    true
}

/// KerML 8.3.4.12.3 validateMetadataFeatureAnnotatedElement: if the metadata has features that
/// specialize `Metaobjects::Metaobject::annotatedElement`, the metaclass of each annotated element
/// specializes one of their types.
pub fn validate_metadata_feature_annotated_element(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> bool {
    if !is(db, project, x, M::MetadataFeature) {
        return true;
    }
    let Some(base) = d::library(db, project, "Metaobjects::Metaobject::annotatedElement") else {
        return true;
    };
    let features: Vec<Global> = d::feature(db, project, x)
        .into_iter()
        .filter(|f| *f != base && d::specializes(db, project, *f, base))
        .collect();
    if features.is_empty() {
        return true;
    }
    let types: Vec<Global> = features
        .into_iter()
        .flat_map(|f| d::owned_typing(db, project, f))
        .filter_map(|r| end(db, project, r, Slot::Target, 0))
        .collect();
    d::annotated_element(db, project, x).into_iter().all(|e| {
        crate::derived::reflective_metaclass(db, project, e)
            .is_none_or(|m| types.iter().any(|t| d::specializes(db, project, m, *t)))
    })
}
