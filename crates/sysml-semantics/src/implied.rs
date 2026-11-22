// SPDX-License-Identifier: Apache-2.0
//! The implied specializations and redefinitions that name resolution needs (KerML 8.4.2, with
//! the constraints of Tables 8–11 in 8.4.3.1 and 8.4.4.1), as queries (ADR 0019, ADR 0020).
//!
//! A type's general types are its owned specializations' and these implied ones, so inheritance
//! (KerML 8.3.3.1.10) sees `Base::things`, `Performances::Performance` and the other library
//! types the semantic constraints require, and the positional redefinitions of ends, parameters
//! and results. They are computed, not added to the fragments: the overlay of each file
//! (`materialize.rs`, ADR 0022) materializes them from these queries.

use sysml_model::{Attr, ElementIx, Graph, Metaclass as M};

use crate::resolve::{
    generals, global_namespace, member_element, owned_generals, owned_memberships, resolve_end,
    structure, Global, Project, SourceFile,
};

/// An implied relationship of a type: its kind and its general (or redefined) type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Implied {
    /// Subclassification, FeatureTyping, Subsetting, Specialization or Redefinition.
    pub kind: M,
    /// The target.
    pub general: Global,
}

fn ix(i: u32) -> ElementIx {
    ElementIx::from_index(i as usize)
}

fn at(file: SourceFile, e: ElementIx) -> Global {
    Global {
        file,
        index: e.index() as u32,
    }
}

/// A library element by its qualified name (KerML 8.4.2 `specializesFromLibrary`, which resolves
/// it globally): the first segment in the global namespace, then each further one among the owned
/// memberships of the element before, which is where the library declares every element the
/// constraints name. Every root namespace with the first segment's name is tried, in order, so a
/// user package of the same name, such as `Links`, does not hide the library's (ADR 0020).
/// `None` when the library is not part of the project.
#[salsa::tracked(cycle_initial = no_library, cycle_fn = settle_global)]
pub fn library(db: &dyn salsa::Database, project: Project, name: &'static str) -> Option<Global> {
    let segments: Vec<&str> = name.split("::").collect();
    qualified(db, project, &segments)
}

fn qualified(db: &dyn salsa::Database, project: Project, segments: &[&str]) -> Option<Global> {
    let (first, rest) = segments.split_first()?;
    global_namespace(db, project)
        .iter()
        .filter(|e| e.named(first))
        .find_map(|root| {
            let mut entry = root.clone();
            for s in rest {
                let ns = member_element(db, project, &entry)?;
                entry = owned_memberships(db, project, ns.file, ns.index)
                    .iter()
                    .find(|e| e.named(s))?
                    .clone();
            }
            member_element(db, project, &entry)
        })
}

settle!(settle_global, Option<Global>, (Project, &'static str));
settle!(settle_globals, Vec<Global>, (Project, SourceFile, u32));
settle!(settle_result, Option<Global>, (Project, SourceFile, u32));
settle!(settle_operator, Option<Global>, (Project, String));
settle!(settle_implied, Vec<Implied>, (Project, SourceFile, u32));

fn no_library(
    _db: &dyn salsa::Database,
    _id: salsa::Id,
    _project: Project,
    _name: &'static str,
) -> Option<Global> {
    None
}

/// Every direct or indirect general type of a type (KerML 8.3.3.1.10 `allSupertypes`, without
/// the type itself unless specializations are circular): a fixpoint from the empty set.
#[salsa::tracked(returns(ref), cycle_initial = no_globals, cycle_fn = settle_globals)]
pub fn supertypes(
    db: &dyn salsa::Database,
    project: Project,
    file: SourceFile,
    ty: u32,
) -> Vec<Global> {
    let mut out: Vec<Global> = Vec::new();
    for general in generals(db, project, Global { file, index: ty })
        .iter()
        .copied()
    {
        if !out.contains(&general) {
            out.push(general);
        }
        for s in supertypes(db, project, general.file, general.index) {
            if !out.contains(s) {
                out.push(*s);
            }
        }
    }
    out
}

fn no_globals(
    _db: &dyn salsa::Database,
    _id: salsa::Id,
    _project: Project,
    _file: SourceFile,
    _ty: u32,
) -> Vec<Global> {
    Vec::new()
}

/// `owningType.oclIsKindOf(kind)`, or the owning type is a Feature one of whose `type`s is
/// (KerML 8.3.3.3.4, as in checkFeatureSubobjectSpecialization).
fn owner_of_kind(db: &dyn salsa::Database, project: Project, t: Global, kind: M) -> bool {
    let m = metaclass(db, t);
    m.specializes(kind)
        || (m.specializes(M::Feature)
            && feature_types(db, project, t)
                .iter()
                .any(|x| metaclass(db, *x).specializes(kind)))
}

/// A feature's `type`s (KerML 8.3.3.3.4): the types of its typings, and those of the features it
/// subsets, redefines or chains, directly or indirectly.
pub(crate) fn feature_types(db: &dyn salsa::Database, project: Project, f: Global) -> Vec<Global> {
    feature_types_except(db, project, f, None)
}

/// [`feature_types`], without following one feature: an end's owned cross feature, whose own
/// types are the end's (`checkFeatureOwnedCrossFeatureSpecialization`).
fn feature_types_except(
    db: &dyn salsa::Database,
    project: Project,
    f: Global,
    skip: Option<Global>,
) -> Vec<Global> {
    let mut types = Vec::new();
    let mut seen: Vec<Global> = std::iter::once(f).chain(skip).collect();
    let mut stack = vec![f];
    while let Some(x) = stack.pop() {
        for general in generals(db, project, x).iter().copied() {
            let m = metaclass(db, general);
            if m.specializes(M::Feature) {
                if !seen.contains(&general) {
                    seen.push(general);
                    stack.push(general);
                }
            } else if m.specializes(M::Type) && !types.contains(&general) {
                types.push(general);
            }
        }
    }
    types
}

/// `owningType` (KerML 8.3.3.3.4): the type that owns a feature through a FeatureMembership.
pub(crate) fn owning_type(g: &Graph, f: ElementIx) -> Option<ElementIx> {
    let r = g.owning_relationship(f)?;
    if !g.metaclass(r).specializes(M::FeatureMembership) {
        return None;
    }
    g.owning_related_element(r)
        .filter(|t| g.metaclass(*t).specializes(M::Type))
}

/// `ownedFeature` (KerML 8.3.3.1.10): the features a type owns through FeatureMemberships.
fn owned_features(g: &Graph, t: ElementIx) -> impl Iterator<Item = ElementIx> + '_ {
    g.owned_relationships(t)
        .iter()
        .filter(|r| g.metaclass(**r).specializes(M::FeatureMembership))
        .filter_map(|r| g.owned_related_elements(*r).first().copied())
}

/// The element at an end of a relationship of the fragment: written there, or the member element
/// of the membership its name resolves to (KerML 8.2.3.5).
fn end_of(
    db: &dyn salsa::Database,
    project: Project,
    rel: Global,
    slot: sysml_model::Slot,
) -> Option<Global> {
    resolve_end(db, project, rel, slot, 0)
        .as_ref()
        .and_then(|e| member_element(db, project, e))
}

/// The kind of an implied specialization of a feature by a type: a FeatureTyping by a
/// Classifier, a Subsetting of a Feature, else a Specialization (KerML 8.3.3.1, 8.3.3.3).
fn feature_specialization(db: &dyn salsa::Database, t: Global) -> M {
    let m = metaclass(db, t);
    if m.specializes(M::Classifier) {
        M::FeatureTyping
    } else if m.specializes(M::Feature) {
        M::Subsetting
    } else {
        M::Specialization
    }
}

/// The kind of an implied specialization of a type by another, for either kind of type.
pub(crate) fn specialization_kind(db: &dyn salsa::Database, special: Global, general: Global) -> M {
    let (s, t) = (metaclass(db, special), metaclass(db, general));
    if s.specializes(M::Feature) {
        feature_specialization(db, general)
    } else if s.specializes(M::Classifier) && t.specializes(M::Classifier) {
        M::Subclassification
    } else {
        M::Specialization
    }
}

/// Whether a type owns a written Specialization of itself (KerML 8.3.3.1.10
/// `ownedSpecialization`, which the fragment holds only as written).
fn has_owned_specialization(g: &Graph, t: ElementIx) -> bool {
    g.owned_relationships(t).iter().any(|r| {
        g.metaclass(*r).specializes(M::Specialization)
            && match g.source(*r).first() {
                None => true,
                Some(sysml_model::Endpoint::Element(e)) => *e == t,
                Some(sysml_model::Endpoint::Name(_)) => false,
            }
    })
}

/// The member element of the first owned membership of a namespace that is not of a kind
/// (KerML 8.3.4.8.5 `referent`, with ParameterMembership).
fn first_member_not(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
    not: M,
) -> Option<Global> {
    let l = structure(db, x.file);
    let g = l.graph();
    let r = g.owned_relationships(x.ix()).iter().find(|r| {
        let m = g.metaclass(**r);
        m.specializes(M::Membership) && !m.specializes(not)
    })?;
    // An OwningMembership's member element is the element it owns (KerML 8.3.2.4.6).
    if g.metaclass(*r).specializes(M::OwningMembership) {
        return g.owned_related_elements(*r).first().map(|e| at(x.file, *e));
    }
    end_of(db, project, at(x.file, *r), sysml_model::Slot::Target)
}

/// The first argument of an invocation (KerML 8.3.4.8.8 `argument->first()`): the value of the
/// FeatureValue of its first owned parameter, which redefines the first input of the
/// instantiated type (`checkFeatureParameterRedefinition`).
fn first_argument(g: &Graph, file: SourceFile, x: ElementIx) -> Option<Global> {
    let p = *owned_parameters(g, x).first()?;
    g.owned_relationships(p)
        .iter()
        .find(|r| g.metaclass(**r).specializes(M::FeatureValue))
        .and_then(|r| g.owned_related_elements(*r).first())
        .map(|e| at(file, *e))
}

/// The specializations of a feature that its typing implies, for an implied feature typed by
/// `types` (KerML 8.3.3.3.4 checkFeatureObjectSpecialization, checkFeatureOccurrenceSpecialization,
/// checkFeatureDataValueSpecialization, checkFeatureSpecialization), without those KerML 8.4.2
/// leaves out: one that a type or another of them already implies.
pub(crate) fn typed_feature_generals(
    db: &dyn salsa::Database,
    project: Project,
    types: &[Global],
) -> Vec<Global> {
    let typed = |k: M| types.iter().any(|t| metaclass(db, *t).specializes(k));
    let mut candidates = Vec::new();
    for (k, n) in [
        (M::Structure, "Objects::objects"),
        (M::Class, "Occurrences::occurrences"),
        (M::DataType, "Base::dataValues"),
    ] {
        if typed(k) {
            candidates.extend(*library(db, project, n));
        }
    }
    candidates.extend(*library(db, project, "Base::things"));
    let all: Vec<Global> = types.iter().chain(&candidates).copied().collect();
    let mut kept: Vec<Global> = Vec::new();
    for c in candidates {
        let redundant = all
            .iter()
            .any(|o| *o != c && supertypes(db, project, o.file, o.index).contains(&c));
        if !redundant && !kept.contains(&c) {
            kept.push(c);
        }
    }
    kept
}

/// `ownedCrossFeature()` (KerML 8.3.3.3.4): for an end feature with an owning type, its first
/// owned member that is a Feature, not a Multiplicity or a MetadataFeature, and not owned through
/// a FeatureMembership.
pub(crate) fn owned_cross_feature(g: &Graph, f: ElementIx) -> Option<ElementIx> {
    if !g.flag(f, Attr::IsEnd) || owning_type(g, f).is_none() {
        return None;
    }
    g.owned_relationships(f)
        .iter()
        .filter(|r| {
            let m = g.metaclass(**r);
            m.specializes(M::OwningMembership) && !m.specializes(M::FeatureMembership)
        })
        .filter_map(|r| g.owned_related_elements(*r).first().copied())
        .find(|e| {
            let m = g.metaclass(*e);
            m.specializes(M::Feature)
                && !m.specializes(M::Multiplicity)
                && !m.specializes(M::MetadataFeature)
        })
}

/// The end feature whose owned cross feature a feature is (KerML 8.3.3.3.4
/// `isOwnedCrossFeature()`).
fn crossed_end(g: &Graph, f: ElementIx) -> Option<ElementIx> {
    let e = g.owner(f)?;
    (g.metaclass(e).specializes(M::Feature) && owned_cross_feature(g, e) == Some(f)).then_some(e)
}

/// `crossFeature` (KerML 8.3.3.3.4 deriveFeatureCrossFeature) of a feature: the second chaining
/// feature of the feature its first owned CrossSubsetting crosses; for an end that has an owned
/// cross feature and no CrossSubsetting, that feature, which its implied CrossSubsetting crosses
/// (`checkFeatureCrossingSpecialization`).
fn cross_feature(db: &dyn salsa::Database, project: Project, f: Global) -> Option<Global> {
    let l = structure(db, f.file);
    let g = l.graph();
    let Some(r) = g
        .owned_relationships(f.ix())
        .iter()
        .find(|r| g.metaclass(**r) == M::CrossSubsetting)
    else {
        return owned_cross_feature(g, f.ix()).map(|c| at(f.file, c));
    };
    let crossed = end_of(db, project, at(f.file, *r), sysml_model::Slot::Target)?;
    let lc = structure(db, crossed.file);
    let gc = lc.graph();
    let chaining = *gc
        .owned_relationships(crossed.ix())
        .iter()
        .filter(|r| gc.metaclass(**r) == M::FeatureChaining)
        .nth(1)?;
    end_of(
        db,
        project,
        at(crossed.file, chaining),
        sysml_model::Slot::Target,
    )
}

fn is_result(g: &Graph, f: ElementIx) -> bool {
    g.owning_relationship(f)
        .is_some_and(|r| g.metaclass(r).specializes(M::ReturnParameterMembership))
}

/// The owned parameters of a type that are not its result, in order (KerML 8.3.3.3.4
/// `checkFeatureParameterRedefinition`): owned features with a direction.
fn owned_parameters(g: &Graph, t: ElementIx) -> Vec<ElementIx> {
    owned_features(g, t)
        .filter(|f| g.values(*f, Attr::Direction).next().is_some() && !is_result(g, *f))
        .collect()
}

/// The end features of a type (KerML 8.3.3.1.10 `endFeature`): its owned ends or, with none, those
/// of its first owned general that has ends. Implied generals are not consulted, which keeps this
/// out of a cycle with them; every library type the constraints name owns its ends (ADR 0020).
#[salsa::tracked(returns(ref), cycle_initial = no_globals, cycle_fn = settle_globals)]
pub fn end_features(
    db: &dyn salsa::Database,
    project: Project,
    file: SourceFile,
    ty: u32,
) -> Vec<Global> {
    // An implied element (ADR 0022) has no names, memberships or generals of its own here.
    if !crate::resolve::in_fragment(db, file, ty) {
        return Default::default();
    }
    let l = structure(db, file);
    let g = &l.graph();
    let owned: Vec<Global> = owned_features(g, ix(ty))
        .filter(|f| g.flag(*f, Attr::IsEnd))
        .map(|f| at(file, f))
        .collect();
    if !owned.is_empty() {
        return owned;
    }
    for general in owned_generals(db, project, Global { file, index: ty }).iter() {
        if *general == (Global { file, index: ty }) {
            continue;
        }
        let ends = end_features(db, project, general.file, general.index);
        if !ends.is_empty() {
            return ends.clone();
        }
    }
    Vec::new()
}

/// The result parameter of a function or expression (KerML 8.3.4.7.3, 8.3.4.7.4 `result`): its own, or the
/// first one its generals have. A fixpoint from none.
#[salsa::tracked(cycle_initial = no_result, cycle_fn = settle_result)]
pub fn result_parameter(
    db: &dyn salsa::Database,
    project: Project,
    file: SourceFile,
    ty: u32,
) -> Option<Global> {
    // An implied element (ADR 0022) has no names, memberships or generals of its own here.
    if !crate::resolve::in_fragment(db, file, ty) {
        return Default::default();
    }
    let l = structure(db, file);
    let g = &l.graph();
    if let Some(r) = owned_features(g, ix(ty)).find(|f| is_result(g, *f)) {
        return Some(at(file, r));
    }
    let here = Global { file, index: ty };
    generals(db, project, here)
        .iter()
        .filter(|s| **s != here && is_function_or_expression(db, **s))
        .find_map(|s| *result_parameter(db, project, s.file, s.index))
}

fn no_result(
    _db: &dyn salsa::Database,
    _id: salsa::Id,
    _project: Project,
    _file: SourceFile,
    _ty: u32,
) -> Option<Global> {
    None
}

fn metaclass(db: &dyn salsa::Database, x: Global) -> M {
    structure(db, x.file).graph().metaclass(x.ix())
}

fn is_function_or_expression(db: &dyn salsa::Database, x: Global) -> bool {
    let m = metaclass(db, x);
    m.specializes(M::Function) || m.specializes(M::Expression)
}

fn is_behavior_or_step(db: &dyn salsa::Database, x: Global) -> bool {
    let m = metaclass(db, x);
    m.specializes(M::Behavior) || m.specializes(M::Step)
}

/// The targets of a feature's owned FeatureTypings (`ownedTyping.type`).
fn owned_types(db: &dyn salsa::Database, project: Project, f: Global) -> Vec<Global> {
    let l = structure(db, f.file);
    let g = &l.graph();
    g.owned_relationships(f.ix())
        .iter()
        .filter(|r| g.metaclass(**r) == M::FeatureTyping)
        .filter_map(|r| {
            resolve_end(db, project, at(f.file, *r), sysml_model::Slot::Target, 0)
                .as_ref()
                .and_then(|e| member_element(db, project, e))
        })
        .collect()
}

/// `instantiatedType` (KerML 8.3.4.8.7, 8.3.4.8.17): the member element of the first owned
/// membership that is not a FeatureMembership or, for an OperatorExpression, its operator in
/// BaseFunctions, DataFunctions or ControlFunctions.
pub(crate) fn instantiated_type(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> Option<Global> {
    let l = structure(db, x.file);
    let g = &l.graph();
    if g.metaclass(x.ix()).specializes(M::OperatorExpression) {
        let op = g.string(x.ix(), Attr::Operator)?;
        return *operator_function(db, project, op.to_owned());
    }
    let r = g.owned_relationships(x.ix()).iter().find(|r| {
        let m = g.metaclass(**r);
        m.specializes(M::Membership) && !m.specializes(M::FeatureMembership)
    })?;
    let e = resolve_end(db, project, at(x.file, *r), sysml_model::Slot::Target, 0)
        .as_ref()
        .and_then(|e| member_element(db, project, e))?;
    metaclass(db, e).specializes(M::Type).then_some(e)
}

/// The library function of an operator (KerML 8.3.4.8.17).
#[salsa::tracked(cycle_initial = no_operator, cycle_fn = settle_operator)]
fn operator_function(db: &dyn salsa::Database, project: Project, op: String) -> Option<Global> {
    ["BaseFunctions", "DataFunctions", "ControlFunctions"]
        .iter()
        .find_map(|p| qualified(db, project, &[p, &op]))
}

fn no_operator(
    _db: &dyn salsa::Database,
    _id: salsa::Id,
    _project: Project,
    _op: String,
) -> Option<Global> {
    None
}

/// The implied relationships of a type that name resolution needs, specializations first.
///
/// Specializations come from the specialization constraints (KerML 8.3.3, 8.3.4). Of those, the
/// rules of KerML 8.4.2 keep only the ones not already implied: none whose general is the type
/// itself, a general of an owned specialization, or a strict supertype of an owned general or of
/// another implied one; and one per general. Redefinitions come from the redefinition
/// constraints, which those rules do not filter: ends (`checkFeatureEndRedefinition`), parameters
/// (`checkFeatureParameterRedefinition`), results (`checkFeatureResultRedefinition`), flow end
/// features (`checkFeatureFlowFeatureRedefinition`) and payloads
/// (`checkPayloadFeatureRedefinition`), each unless the feature already redefines its target.
/// Implied relationships of the library build on the library's own: a fixpoint from none.
#[salsa::tracked(returns(ref), cycle_initial = no_implied, cycle_fn = settle_implied)]
pub fn implied_generals(
    db: &dyn salsa::Database,
    project: Project,
    file: SourceFile,
    ty: u32,
) -> Vec<Implied> {
    // An implied element (ADR 0022) has no names, memberships or generals of its own here.
    if !crate::resolve::in_fragment(db, file, ty) {
        return Default::default();
    }
    let here = Global { file, index: ty };
    let l = structure(db, file);
    let g = &l.graph();
    let x = ix(ty);
    let m = g.metaclass(x);
    if !m.specializes(M::Type) {
        return Vec::new();
    }
    let lib = |n: &'static str| *library(db, project, n);
    let owned = owned_generals(db, project, here).clone();

    // ---- specialization constraints, most specific first ----
    let candidates: std::cell::RefCell<Vec<(M, Option<Global>)>> = Default::default();
    let push = |kind: M, general: Option<Global>| candidates.borrow_mut().push((kind, general));
    if m.specializes(M::Classifier) {
        let sub = M::Subclassification;
        let binary = || end_features(db, project, file, ty).len() == 2;
        // KerML 8.3.4.12.2 checkMetaclassSpecialization
        if m.specializes(M::Metaclass) {
            push(sub, lib("Metaobjects::Metaobject"));
        }
        // KerML 8.3.4.7.6 checkPredicateSpecialization, 8.3.4.7.4 checkFunctionSpecialization,
        // 8.3.4.6.2 checkBehaviorSpecialization
        if m.specializes(M::Predicate) {
            push(sub, lib("Performances::BooleanEvaluation"));
        }
        if m.specializes(M::Function) {
            push(sub, lib("Performances::Evaluation"));
        }
        if m.specializes(M::Behavior) {
            push(sub, lib("Performances::Performance"));
        }
        // KerML 8.3.4.4.3 checkAssociationStructure(Binary)Specialization
        if m.specializes(M::AssociationStructure) {
            if binary() {
                push(sub, lib("Objects::BinaryLinkObject"));
            }
            push(sub, lib("Objects::LinkObject"));
        }
        // KerML 8.3.4.4.2 checkAssociation(Binary)Specialization
        if m.specializes(M::Association) {
            if binary() {
                push(sub, lib("Links::BinaryLink"));
            }
            push(sub, lib("Links::Link"));
        }
        // KerML 8.3.4.3.2 checkStructureSpecialization, 8.3.4.2.2 checkClassSpecialization,
        // 8.3.4.1.2 checkDataTypeSpecialization
        if m.specializes(M::Structure) {
            push(sub, lib("Objects::Object"));
        }
        if m.specializes(M::Class) {
            push(sub, lib("Occurrences::Occurrence"));
        }
        if m.specializes(M::DataType) {
            push(sub, lib("Base::DataValue"));
        }
        // KerML 8.3.3.1.10 checkTypeSpecialization: implied for Classifiers only (Table 8 Note 1)
        push(sub, lib("Base::Anything"));
    }
    if m.specializes(M::Feature) {
        let subset = M::Subsetting;
        let owner = owning_type(g, x).map(|t| at(file, t));
        let owner_is = |kind: M| owner.is_some_and(|t| owner_of_kind(db, project, t, kind));
        let types = owned_types(db, project, here);
        // `ownedTyping.type`: the written typings and the implied ones so far, as an owned cross
        // feature's or a result's (KerML 8.3.3.3.4 `ownedTyping`, implied ones included).
        let typed = |kind: M| {
            let implied: Vec<Global> = candidates
                .borrow()
                .iter()
                .filter(|c| c.0 == M::FeatureTyping)
                .filter_map(|c| c.1)
                .collect();
            types
                .iter()
                .chain(&implied)
                .any(|t| metaclass(db, *t).specializes(kind))
        };
        let composite = g.flag(x, Attr::IsComposite);
        // KerML 8.3.4.8.9–8.3.4.8.14 checkLiteral*Specialization, 8.3.4.8.16
        // checkNullExpression…, 8.3.4.8.15 checkMetadataAccessExpression…, 8.3.4.8.3
        // checkConstructorExpression…, 8.3.4.7.5 checkInvariantSpecialization, 8.3.4.7.2
        // checkBooleanExpression…, 8.3.4.8.8 checkInvocationExpressionSpecialization, 8.3.4.7.3
        // checkExpressionSpecialization
        let literal = [
            (M::LiteralBoolean, "Performances::literalBooleanEvaluations"),
            (M::LiteralInteger, "Performances::literalIntegerEvaluations"),
            (
                M::LiteralInfinity,
                "Performances::literalIntegerEvaluations",
            ),
            (
                M::LiteralRational,
                "Performances::literalRationalEvaluations",
            ),
            (M::LiteralString, "Performances::literalStringEvaluations"),
        ];
        for (k, n) in literal {
            if m.specializes(k) {
                push(subset, lib(n));
            }
        }
        if m.specializes(M::LiteralExpression) {
            push(subset, lib("Performances::literalEvaluations"));
        }
        if m.specializes(M::NullExpression) {
            push(subset, lib("Performances::nullEvaluations"));
        }
        if m.specializes(M::MetadataAccessExpression) {
            push(subset, lib("Performances::metadataAccessEvaluations"));
        }
        if m.specializes(M::ConstructorExpression) {
            push(subset, lib("Performances::constructorEvaluations"));
        }
        if m.specializes(M::Invariant) {
            if g.flag(x, Attr::IsNegated) {
                push(subset, lib("Performances::falseEvaluations"));
            } else {
                push(subset, lib("Performances::trueEvaluations"));
            }
        }
        if m.specializes(M::BooleanExpression) {
            push(subset, lib("Performances::booleanEvaluations"));
        }
        if m.specializes(M::InvocationExpression) {
            if let Some(t) = instantiated_type(db, project, here).filter(|t| *t != here) {
                push(feature_specialization(db, t), Some(t));
            }
        }
        if m.specializes(M::Expression) {
            push(subset, lib("Performances::evaluations"));
        }
        // KerML 8.3.4.8.3 checkConstructorExpressionResultSpecialization: the result of a
        // ConstructorExpression, which the grammar writes (8.2.5.8.3), specializes the
        // instantiated type; so the features of `new T(a = 1)` redefine those of T (ADR 0021).
        if is_result(g, x) {
            if let Some(c) =
                owning_type(g, x).filter(|c| g.metaclass(*c).specializes(M::ConstructorExpression))
            {
                if let Some(t) = instantiated_type(db, project, at(file, c)) {
                    push(feature_specialization(db, t), Some(t));
                }
            }
            if let Some(t) = owning_type(g, x) {
                let tg = at(file, t);
                let tm = g.metaclass(t);
                // KerML 8.3.4.8.5 checkFeatureReferenceExpressionResultSpecialization: the result
                // subsets the referent.
                if tm.specializes(M::FeatureReferenceExpression) {
                    let referent = first_member_not(db, project, tg, M::ParameterMembership)
                        .filter(|r| metaclass(db, *r).specializes(M::Feature));
                    push(subset, referent);
                }
                // KerML 8.3.4.8.8 checkInvocationExpressionBehaviorResultSpecialization: the
                // result of an invocation of a type that is not a Function, nor a Feature typed by
                // one, specializes it.
                if tm.specializes(M::InvocationExpression) {
                    if let Some(i) = instantiated_type(db, project, tg).filter(|i| *i != tg) {
                        let function = |y: &Global| metaclass(db, *y).specializes(M::Function);
                        let invokes_function = function(&i)
                            || (metaclass(db, i).specializes(M::Feature)
                                && feature_types(db, project, i).iter().any(function));
                        if !invokes_function {
                            push(feature_specialization(db, i), Some(i));
                        }
                    }
                }
                // KerML 8.3.4.8.18 checkSelectExpressionResultSpecialization, 8.3.4.8.6
                // checkIndexExpressionResultSpecialization: the result subsets the first
                // argument's result, for an index unless that result is a `Collections::Array`.
                let index = tm.specializes(M::IndexExpression);
                if tm.specializes(M::SelectExpression) || index {
                    if let Some(r) = first_argument(g, file, t)
                        .and_then(|a| *result_parameter(db, project, a.file, a.index))
                    {
                        let array = index
                            && lib("Collections::Array").is_some_and(|a| {
                                r == a || supertypes(db, project, r.file, r.index).contains(&a)
                            });
                        if !array {
                            push(subset, Some(r));
                        }
                    }
                }
            }
        }
        // KerML 8.3.3.3.4 checkFeatureValuationSpecialization: a feature without a direction or a
        // written specialization subsets the result of the value of each of its FeatureValues.
        if g.values(x, Attr::Direction).next().is_none() && !has_owned_specialization(g, x) {
            for r in g.owned_relationships(x) {
                if g.metaclass(*r).specializes(M::FeatureValue) {
                    if let Some(v) = g.owned_related_elements(*r).first() {
                        push(
                            subset,
                            *result_parameter(db, project, file, v.index() as u32),
                        );
                    }
                }
            }
        }
        // KerML 8.3.3.3.4 checkFeatureOwnedCrossFeatureSpecialization: an owned cross feature
        // specializes the types of its end.
        if let Some(e) = crossed_end(g, x) {
            for t in feature_types_except(db, project, at(file, e), Some(here)) {
                push(feature_specialization(db, t), Some(t));
            }
        }
        // KerML 8.3.4.9.6 checkSuccessionFlowSpecialization, 8.3.4.9.2 checkFlowWithEnds…,
        // checkFlowSpecialization
        if m.specializes(M::SuccessionFlow) {
            push(subset, lib("Transfers::flowTransfersBefore"));
        }
        if m.specializes(M::Flow) {
            // `ownedEndFeatures` read as the flow's FlowEnds, those of `from … to …` (KerML
            // 8.4.4.10.2), not the plain end features of the library's own flows (ADR 0020).
            if owned_features(g, x).any(|f| g.metaclass(f) == M::FlowEnd) {
                push(subset, lib("Transfers::flowTransfers"));
            }
            push(subset, lib("Transfers::transfers"));
        }
        // KerML 8.3.4.6.3 checkStep*Specialization
        if m.specializes(M::Step) {
            if composite && (owner_is(M::Structure)) {
                push(subset, lib("Objects::Object::ownedPerformances"));
            }
            if owner.is_some_and(|t| is_behavior_or_step(db, t)) {
                if composite {
                    push(subset, lib("Performances::Performance::subperformances"));
                }
                push(
                    subset,
                    lib("Performances::Performance::enclosedPerformances"),
                );
            }
            push(subset, lib("Performances::performances"));
        }
        // KerML 8.3.4.5.4 checkSuccession…, 8.3.4.5.2 checkBindingConnector…, 8.3.4.5.3
        // checkConnector*Specialization
        if m.specializes(M::Succession) {
            push(subset, lib("Occurrences::happensBeforeLinks"));
        }
        if m.specializes(M::BindingConnector) {
            push(subset, lib("Links::selfLinks"));
        }
        if m.specializes(M::Connector) {
            // `connectorEnd` is the connector's `endFeature`, owned or inherited.
            let binary = end_features(db, project, file, ty).len() == 2;
            let object = typed(M::AssociationStructure);
            if binary && object {
                push(subset, lib("Objects::binaryLinkObjects"));
            }
            if object {
                push(subset, lib("Objects::linkObjects"));
            }
            if binary {
                push(subset, lib("Links::binaryLinks"));
            }
            push(subset, lib("Links::links"));
        }
        // KerML 8.3.4.12.3 checkMetadataFeatureSpecialization, 8.3.3.1.9
        // checkMultiplicitySpecialization
        if m.specializes(M::MetadataFeature) {
            push(subset, lib("Metaobjects::metaobjects"));
        }
        if m.specializes(M::Multiplicity) {
            push(subset, lib("Base::naturals"));
        }
        // KerML 8.3.3.3.4: checkFeatureEndSpecialization, checkFeaturePortion…,
        // checkFeatureSubobject… (Table 9 names Objects::Object::subobjects; ADR 0020),
        // checkFeatureSuboccurrence…, checkFeatureObject…, checkFeatureOccurrence…,
        // checkFeatureDataValue…, checkFeatureSpecialization
        if g.flag(x, Attr::IsEnd)
            && owner.is_some_and(|t| {
                let k = metaclass(db, t);
                k.specializes(M::Association) || k.specializes(M::Connector)
            })
        {
            push(subset, lib("Links::Link::participant"));
        }
        if g.flag(x, Attr::IsPortion) && typed(M::Class) && owner_is(M::Class) {
            push(subset, lib("Occurrences::Occurrence::portions"));
        }
        if composite && typed(M::Structure) && owner_is(M::Structure) {
            push(subset, lib("Objects::Object::subobjects"));
        }
        if composite && typed(M::Class) && owner_is(M::Class) {
            push(subset, lib("Occurrences::Occurrence::suboccurrences"));
        }
        if typed(M::Structure) {
            push(subset, lib("Objects::objects"));
        }
        if typed(M::Class) {
            push(subset, lib("Occurrences::occurrences"));
        }
        if typed(M::DataType) {
            push(subset, lib("Base::dataValues"));
        }
        push(subset, lib("Base::things"));
    }

    // Specializations that a constraint requires to be owned, not only implied by the type's
    // supertypes, which the rules of KerML 8.4.2 therefore do not filter (ADR 0020).
    let mut direct: Vec<Implied> = Vec::new();
    if m.specializes(M::Feature) {
        // KerML 8.3.3.3.4 checkFeatureCrossingSpecialization: an end with an owned cross feature
        // and no CrossSubsetting crosses it, through a chain the overlay builds (Table 9 Note 2).
        if let Some(c) = owned_cross_feature(g, x) {
            if !g
                .owned_relationships(x)
                .iter()
                .any(|r| g.metaclass(*r) == M::CrossSubsetting)
            {
                direct.push(Implied {
                    kind: M::CrossSubsetting,
                    general: at(file, c),
                });
            }
        }
        // KerML 8.3.3.3.4 checkFeatureOwnedCrossFeatureRedefinitionSpecialization: an owned cross
        // feature subsets the cross features of the features its end redefines.
        if let Some(e) = crossed_end(g, x) {
            let eg = at(file, e);
            let written: Vec<Global> = g
                .owned_relationships(e)
                .iter()
                .filter(|r| g.metaclass(**r) == M::Redefinition)
                .filter_map(|r| end_of(db, project, at(file, *r), sysml_model::Slot::Target))
                .collect();
            let implied = implied_redefinitions(db, project, file, e.index() as u32);
            let subsetted: Vec<Global> = g
                .owned_relationships(x)
                .iter()
                .filter(|r| g.metaclass(**r) == M::Subsetting)
                .filter_map(|r| end_of(db, project, at(file, *r), sysml_model::Slot::Target))
                .collect();
            for r in written.into_iter().chain(implied.iter().map(|i| i.general)) {
                if r == eg {
                    continue;
                }
                if let Some(c) = cross_feature(db, project, r).filter(|c| *c != here) {
                    if !subsetted.contains(&c) && !direct.iter().any(|d| d.general == c) {
                        direct.push(Implied {
                            kind: M::Subsetting,
                            general: c,
                        });
                    }
                }
            }
        }
    }

    let redefinitions = implied_redefinitions(db, project, file, ty).clone();
    // KerML 8.4.2: drop the specializations already implied, by an owned specialization, another
    // implied specialization, or an implied redefinition (which itself is never dropped).
    let mut kept: Vec<Implied> = Vec::new();
    let candidates = candidates.into_inner();
    let generals_so_far: Vec<Global> = candidates.iter().filter_map(|c| c.1).collect();
    for (kind, general) in candidates {
        let Some(general) = general else { continue };
        if general == here || owned.contains(&general) || kept.iter().any(|k| k.general == general)
        {
            continue;
        }
        let redundant = owned
            .iter()
            .chain(&generals_so_far)
            .chain(redefinitions.iter().map(|r| &r.general))
            .any(|o| {
                *o != general
                    && *o != here
                    && supertypes(db, project, o.file, o.index).contains(&general)
            });
        if !redundant {
            kept.push(Implied { kind, general });
        }
    }

    kept.extend(direct);
    kept.extend(redefinitions);
    kept
}

/// The implied redefinitions of a feature, from the redefinition constraints (KerML 8.3.3.3.4):
/// ends (`checkFeatureEndRedefinition`), parameters (`checkFeatureParameterRedefinition`),
/// results (`checkFeatureResultRedefinition`), flow end features
/// (`checkFeatureFlowFeatureRedefinition`) and payloads (`checkPayloadFeatureRedefinition`), each
/// unless the feature already redefines its target. Apart from [`implied_generals`], so that
/// following redefinitions does not compute the implied specializations. A fixpoint from none.
#[salsa::tracked(returns(ref), cycle_initial = no_implied, cycle_fn = settle_implied)]
pub fn implied_redefinitions(
    db: &dyn salsa::Database,
    project: Project,
    file: SourceFile,
    ty: u32,
) -> Vec<Implied> {
    // An implied element (ADR 0022) has no names, memberships or generals of its own here.
    if !crate::resolve::in_fragment(db, file, ty) {
        return Default::default();
    }
    let here = Global { file, index: ty };
    let l = structure(db, file);
    let g = l.graph();
    let x = ix(ty);
    let m = g.metaclass(x);
    let lib = |n: &'static str| *library(db, project, n);
    let mut redefinitions: Vec<Implied> = Vec::new();
    if m.specializes(M::Feature) {
        // A redefinition is implied unless the feature already redefines its target itself (the
        // OCL's `redefines`), whatever its other generals are.
        let redefined: Vec<Global> = g
            .owned_relationships(x)
            .iter()
            .filter(|r| g.metaclass(**r) == M::Redefinition)
            .filter_map(|r| {
                resolve_end(db, project, at(file, *r), sysml_model::Slot::Target, 0)
                    .as_ref()
                    .and_then(|e| member_element(db, project, e))
            })
            .collect();
        let mut redefine = |target: Option<Global>| {
            if let Some(t) = target {
                if t != here
                    && !redefined.contains(&t)
                    && !redefinitions.iter().any(|k| k.general == t)
                {
                    redefinitions.push(Implied {
                        kind: M::Redefinition,
                        general: t,
                    });
                }
            }
        };
        if let Some(t) = owning_type(g, x) {
            let tg = at(file, t);
            let supers = generals(db, project, tg).clone();
            // KerML 8.3.3.3.4 checkFeatureEndRedefinition
            if g.flag(x, Attr::IsEnd) {
                let ends: Vec<ElementIx> = owned_features(g, t)
                    .filter(|f| g.flag(*f, Attr::IsEnd))
                    .collect();
                if let Some(i) = ends.iter().position(|f| *f == x) {
                    for s in supers.iter().filter(|s| **s != tg) {
                        redefine(end_features(db, project, s.file, s.index).get(i).copied());
                    }
                }
            }
            // KerML 8.3.3.3.4 checkFeatureParameterRedefinition
            let invocation = g.metaclass(t).specializes(M::InvocationExpression);
            let explicit_redefinition = g
                .owned_relationships(x)
                .iter()
                .any(|r| g.metaclass(*r) == M::Redefinition);
            if g.values(x, Attr::Direction).next().is_some()
                && !is_result(g, x)
                && (g.metaclass(t).specializes(M::Behavior) || g.metaclass(t).specializes(M::Step))
                && !(invocation && explicit_redefinition)
            {
                if let Some(i) = owned_parameters(g, t).iter().position(|f| *f == x) {
                    for s in supers
                        .iter()
                        .filter(|s| **s != tg && is_behavior_or_step(db, **s))
                    {
                        let sl = structure(db, s.file);
                        let p = owned_parameters(sl.graph(), s.ix()).get(i).copied();
                        redefine(p.map(|p| at(s.file, p)));
                    }
                }
            }
            // KerML 8.3.3.3.4 checkFeatureResultRedefinition
            if is_result(g, x) && is_function_or_expression(db, tg) {
                for s in supers
                    .iter()
                    .filter(|s| **s != tg && is_function_or_expression(db, **s))
                {
                    redefine(*result_parameter(db, project, s.file, s.index));
                }
            }
            // KerML 8.3.3.3.4 checkFeatureFlowFeatureRedefinition (the OCL's second target,
            // `source::targetInput`, reads `target::targetInput`; the position is among the flow's
            // FlowEnds, after any payload feature; ADR 0020)
            if g.metaclass(t) == M::FlowEnd && owned_features(g, t).next() == Some(x) {
                if let Some(flow) = owning_type(g, t) {
                    match owned_features(g, flow)
                        .filter(|f| g.metaclass(*f) == M::FlowEnd)
                        .position(|f| f == t)
                    {
                        Some(0) => redefine(lib("Transfers::Transfer::source::sourceOutput")),
                        Some(1) => redefine(lib("Transfers::Transfer::target::targetInput")),
                        _ => {}
                    }
                }
            }
        }
        // KerML 8.3.4.9.5 checkPayloadFeatureRedefinition
        if m == M::PayloadFeature {
            redefine(lib("Transfers::Transfer::payload"));
        }
    }
    redefinitions
}

fn no_implied(
    _db: &dyn salsa::Database,
    _id: salsa::Id,
    _project: Project,
    _file: SourceFile,
    _ty: u32,
) -> Vec<Implied> {
    Vec::new()
}
