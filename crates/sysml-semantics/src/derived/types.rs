// SPDX-License-Identifier: Apache-2.0
//! The Core layer (KerML 8.3.3): types, classifiers and features.
//!
//! A relationship-valued property (`ownedSpecialization`, `ownedTyping` …) lists the relationships
//! of the view: those written, then the implied ones of the overlay (ADR 0022). A property or
//! operation over types (`supertypes`, `type`, `featuringType` …) therefore sees the implied
//! specializations, redefinitions and type featurings.

use std::collections::HashSet;
use sysml_model::{Attr, Metaclass as M, Slot, Value};

use super::root::{membership_element, owned_member, owning_namespace, resolve_global, source};
use super::{at, end, metaclass, owned_relationships_of, unique};
use crate::materialize::view;
use crate::resolve::{
    member_element, redefined_features, settle_all_supertypes, settle_compatible,
    settle_memberships, Entry, Global, Project,
};

// ---- helpers ----

fn kind_of(db: &dyn salsa::Database, project: Project, x: Global, m: M) -> bool {
    metaclass(db, project, x).specializes(m)
}

fn flag(db: &dyn salsa::Database, project: Project, x: Global, a: Attr) -> bool {
    view(db, project, x.file).flag(x.ix(), a)
}

fn first_end(db: &dyn salsa::Database, project: Project, rel: Global) -> Option<Global> {
    end(db, project, rel, Slot::Target, 0)
}

/// The library element with a qualified name: `resolveGlobal(name).memberElement`, as
/// `specializesFromLibrary` reads it (KerML 8.3.3.1.10).
pub fn library(db: &dyn salsa::Database, project: Project, name: &str) -> Option<Global> {
    let m = resolve_global(db, project, name)?;
    let s = view(db, project, m.file);
    let entry = Entry {
        name: None,
        short_name: None,
        membership: m,
        owned_element: s
            .owned_related_elements(m.ix())
            .first()
            .filter(|_| s.metaclass(m.ix()).specializes(M::OwningMembership))
            .map(|e| at(m.file, *e)),
        visibility: crate::Visibility::Public,
        hidden: false,
    };
    member_element(db, project, &entry)
}

/// The specializations of a type as (kind, general): its owned specializations in the view,
/// written then implied (ADR 0022), without the implied ones when `exclude_implied`.
fn specializations(
    db: &dyn salsa::Database,
    project: Project,
    ty: Global,
    exclude_implied: bool,
) -> Vec<(M, Global)> {
    // `ownedSpecialization->reject(isImplied)` when implied ones are excluded (KerML 8.3.3.1.10).
    let mut out: Vec<(M, Global)> = owned_specialization(db, project, ty)
        .into_iter()
        .filter(|r| !(exclude_implied && flag(db, project, *r, Attr::IsImplied)))
        .filter_map(|r| Some((metaclass(db, project, r), first_end(db, project, r)?)))
        .collect();
    // A written specialization and an implied one with the same general count once.
    let mut seen = HashSet::new();
    out.retain(|(_, g)| seen.insert(*g));
    out
}

// ---- Type (KerML 8.3.3.1.10) ----

/// `ownedSpecialization` (deriveTypeOwnedSpecialization): the owned Specializations whose
/// `special` (source) is this type.
pub fn owned_specialization(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    owned_relationships_of(db, project, ty, M::Specialization)
        .into_iter()
        .filter(|r| source(db, project, *r) == [ty])
        .collect()
}

/// `ownedConjugator` (deriveTypeOwnedConjugator): the first owned Conjugation.
pub fn owned_conjugator(db: &dyn salsa::Database, project: Project, ty: Global) -> Option<Global> {
    owned_relationships_of(db, project, ty, M::Conjugation)
        .into_iter()
        .next()
}

/// `isConjugated` (KerML 8.3.3.1.10): whether the type has an owned conjugator.
pub fn is_conjugated(db: &dyn salsa::Database, project: Project, ty: Global) -> bool {
    owned_conjugator(db, project, ty).is_some()
}

fn original_type(db: &dyn salsa::Database, project: Project, ty: Global) -> Option<Global> {
    first_end(db, project, owned_conjugator(db, project, ty)?)
}

/// `ownedUnioning` (deriveTypeOwnedUnioning).
pub fn owned_unioning(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    owned_relationships_of(db, project, ty, M::Unioning)
}

/// `ownedIntersecting` (deriveTypeOwnedIntersecting).
pub fn owned_intersecting(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    owned_relationships_of(db, project, ty, M::Intersecting)
}

/// `ownedDifferencing` (deriveTypeOwnedDifferencing).
pub fn owned_differencing(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    owned_relationships_of(db, project, ty, M::Differencing)
}

/// `ownedDisjoining` (deriveTypeOwnedDisjoining).
pub fn owned_disjoining(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    owned_relationships_of(db, project, ty, M::Disjoining)
}

/// `unioningType` (deriveTypeUnioningType): `ownedUnioning.unioningType`.
pub fn unioning_type(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    let rels = owned_unioning(db, project, ty);
    rels.into_iter()
        .filter_map(|r| first_end(db, project, r))
        .collect()
}

/// `intersectingType` (deriveTypeIntersectingType): `ownedIntersecting.intersectingType`.
pub fn intersecting_type(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    let rels = owned_intersecting(db, project, ty);
    rels.into_iter()
        .filter_map(|r| first_end(db, project, r))
        .collect()
}

/// `differencingType` (deriveTypeDifferencingType): `ownedDifferencing.differencingType`.
pub fn differencing_type(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    let rels = owned_differencing(db, project, ty);
    rels.into_iter()
        .filter_map(|r| first_end(db, project, r))
        .collect()
}

/// `ownedFeatureMembership` (deriveTypeOwnedFeatureMembership).
pub fn owned_feature_membership(
    db: &dyn salsa::Database,
    project: Project,
    ty: Global,
) -> Vec<Global> {
    owned_relationships_of(db, project, ty, M::FeatureMembership)
}

fn owned_member_of(db: &dyn salsa::Database, project: Project, m: Global) -> Option<Global> {
    let s = view(db, project, m.file);
    s.owned_related_elements(m.ix())
        .first()
        .map(|e| at(m.file, *e))
}

/// `ownedFeature` (deriveTypeOwnedFeature): `ownedFeatureMembership.ownedMemberFeature`.
pub fn owned_feature(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    let ms = owned_feature_membership(db, project, ty);
    ms.into_iter()
        .filter_map(|m| owned_member_of(db, project, m))
        .collect()
}

/// `inheritedMembership` (deriveTypeInheritedMembership): `inheritedMemberships(Set{}, Set{},
/// false)`.
pub fn inherited_membership(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    type_inherited_memberships(db, project, ty, false).clone()
}

/// `inheritedFeature` (deriveTypeInheritedFeature): the member features of the inherited
/// FeatureMemberships.
pub fn inherited_feature(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    inherited_membership(db, project, ty)
        .into_iter()
        .filter(|m| kind_of(db, project, *m, M::FeatureMembership))
        .filter_map(|m| owned_member_of(db, project, m))
        .collect()
}

/// `featureMembership` (deriveTypeFeatureMembership): the owned FeatureMemberships, then the
/// inherited ones.
pub fn feature_membership(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    let mut out = owned_feature_membership(db, project, ty);
    out.extend(
        inherited_membership(db, project, ty)
            .into_iter()
            .filter(|m| kind_of(db, project, *m, M::FeatureMembership)),
    );
    unique(out)
}

/// `feature` (deriveTypeFeature): `featureMembership.ownedMemberFeature`.
pub fn feature(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    let ms = feature_membership(db, project, ty);
    ms.into_iter()
        .filter_map(|m| owned_member_of(db, project, m))
        .collect()
}

/// `endFeature` (deriveTypeEndFeature): `feature->select(isEnd)`.
pub fn end_feature(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    let fs = feature(db, project, ty);
    fs.into_iter()
        .filter(|f| flag(db, project, *f, Attr::IsEnd))
        .collect()
}

/// `ownedEndFeature` (deriveTypeOwnedEndFeature): `ownedFeature->select(isEnd)`.
pub fn owned_end_feature(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    let fs = owned_feature(db, project, ty);
    fs.into_iter()
        .filter(|f| flag(db, project, *f, Attr::IsEnd))
        .collect()
}

/// `multiplicity` (deriveTypeMultiplicity): the first owned member that is a Multiplicity.
pub fn multiplicity(db: &dyn salsa::Database, project: Project, ty: Global) -> Option<Global> {
    owned_member(db, project, ty)
        .into_iter()
        .find(|m| kind_of(db, project, *m, M::Multiplicity))
}

/// A feature direction (KerML 8.3.3.3.5 FeatureDirectionKind).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    /// `in`
    In,
    /// `out`
    Out,
    /// `inout`
    Inout,
}

fn direction(db: &dyn salsa::Database, project: Project, f: Global) -> Option<Direction> {
    match view(db, project, f.file)
        .values(f.ix(), Attr::Direction)
        .next()
    {
        Some(Value::Enumeration("in")) => Some(Direction::In),
        Some(Value::Enumeration("out")) => Some(Direction::Out),
        Some(Value::Enumeration("inout")) => Some(Direction::Inout),
        _ => None,
    }
}

/// `owningType` (KerML 8.3.3.3.4): the type that owns the feature through a FeatureMembership.
pub fn owning_type(db: &dyn salsa::Database, project: Project, f: Global) -> Option<Global> {
    let s = view(db, project, f.file);
    let r = s.owning_relationship(f.ix())?;
    if !s.metaclass(r).specializes(M::FeatureMembership) {
        return None;
    }
    s.owning_related_element(r)
        .filter(|t| s.metaclass(*t).specializes(M::Type))
        .map(|t| at(f.file, t))
}

/// `directionOf(feature)` (KerML 8.3.3.1.10): `directionOfExcluding(feature, Set{})`.
pub fn direction_of(
    db: &dyn salsa::Database,
    project: Project,
    ty: Global,
    feature: Global,
) -> Option<Direction> {
    direction_of_excluding(db, project, ty, feature, &[])
}

/// `directionOfExcluding(feature, excluded)` (KerML 8.3.3.1.10): the feature's own direction in
/// its owning type; else the first direction its supertypes give, not visiting the excluded
/// types, reversed for a conjugated type.
pub fn direction_of_excluding(
    db: &dyn salsa::Database,
    project: Project,
    ty: Global,
    feature: Global,
    excluded: &[Global],
) -> Option<Direction> {
    let mut visited: HashSet<Global> = excluded.iter().copied().collect();
    direction_in(db, project, ty, feature, &mut visited)
}

/// `directionOfExcluding`, with one set of the types visited for the whole search: a type that
/// gave no direction on one path gives none on another, so each is visited once.
fn direction_in(
    db: &dyn salsa::Database,
    project: Project,
    ty: Global,
    feature: Global,
    visited: &mut HashSet<Global>,
) -> Option<Direction> {
    if owning_type(db, project, feature) == Some(ty) {
        return direction(db, project, feature);
    }
    visited.insert(ty);
    let mut found = None;
    for t in supertypes(db, project, ty, false) {
        if visited.contains(&t) {
            continue;
        }
        found = direction_in(db, project, t, feature, visited);
        if found.is_some() {
            break;
        }
    }
    let d = found?;
    if !is_conjugated(db, project, ty) {
        return Some(d);
    }
    Some(match d {
        Direction::In => Direction::Out,
        Direction::Out => Direction::In,
        Direction::Inout => Direction::Inout,
    })
}

/// `input` (deriveTypeInput): the features whose direction relative to the type is in or inout.
pub fn input(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    feature(db, project, ty)
        .into_iter()
        .filter(|f| {
            matches!(
                direction_of(db, project, ty, *f),
                Some(Direction::In | Direction::Inout)
            )
        })
        .collect()
}

/// `output` (deriveTypeOutput): the features whose direction relative to the type is out or
/// inout.
pub fn output(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    feature(db, project, ty)
        .into_iter()
        .filter(|f| {
            matches!(
                direction_of(db, project, ty, *f),
                Some(Direction::Out | Direction::Inout)
            )
        })
        .collect()
}

/// `directedFeature` (deriveTypeDirectedFeature): the features with a direction relative to the
/// type.
pub fn directed_feature(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    feature(db, project, ty)
        .into_iter()
        .filter(|f| direction_of(db, project, ty, *f).is_some())
        .collect()
}

/// `supertypes(excludeImplied)`: for a conjugated type its original type, else the generals of
/// its owned specializations, the implied ones unless excluded (KerML 8.3.3.1.10); for a feature
/// with chaining features, also its `featureTarget` (8.3.3.3.4).
pub fn supertypes(
    db: &dyn salsa::Database,
    project: Project,
    ty: Global,
    exclude_implied: bool,
) -> Vec<Global> {
    let mut out: Vec<Global> = if is_conjugated(db, project, ty) {
        original_type(db, project, ty).into_iter().collect()
    } else {
        let s = specializations(db, project, ty, exclude_implied);
        s.into_iter().map(|(_, g)| g).collect()
    };
    if kind_of(db, project, ty, M::Feature) {
        let target = feature_target(db, project, ty);
        if target != ty {
            out.push(target);
        }
    }
    out
}

/// `allSupertypes()` (KerML 8.3.3.1.10): the type and, depth first, its supertypes, each once.
/// Specializations may be circular: a fixpoint from none.
#[salsa::tracked(returns(ref), cycle_initial = no_supertypes, cycle_fn = settle_all_supertypes)]
pub fn all_supertypes(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    let mut out = vec![ty];
    let mut seen: HashSet<Global> = HashSet::from([ty]);
    for s in supertypes(db, project, ty, false) {
        if seen.insert(s) {
            out.push(s);
        }
        for x in all_supertypes(db, project, s) {
            if seen.insert(*x) {
                out.push(*x);
            }
        }
    }
    out
}

pub(crate) fn no_supertypes(
    _db: &dyn salsa::Database,
    _id: salsa::Id,
    _project: Project,
    _ty: Global,
) -> Vec<Global> {
    Vec::new()
}

/// `specializes(supertype)` (KerML 8.3.3.1.10).
pub fn specializes(
    db: &dyn salsa::Database,
    project: Project,
    ty: Global,
    supertype: Global,
) -> bool {
    // A conjugated type specializes what its original type does; conjugations may be circular.
    let mut x = ty;
    let mut seen: HashSet<Global> = HashSet::from([ty]);
    while let Some(original) = original_type(db, project, x) {
        if !seen.insert(original) {
            return false;
        }
        x = original;
    }
    all_supertypes(db, project, x).contains(&supertype)
}

/// `specializesFromLibrary(libraryTypeName)` (KerML 8.3.3.1.10).
pub fn specializes_from_library(
    db: &dyn salsa::Database,
    project: Project,
    ty: Global,
    library_type_name: &str,
) -> bool {
    library(db, project, library_type_name)
        .filter(|t| kind_of(db, project, *t, M::Type))
        .is_some_and(|t| specializes(db, project, ty, t))
}

/// `isCompatibleWith(otherType)` for a Type (KerML 8.3.3.1.10): `specializes(otherType)`; and
/// for a Feature (8.3.3.3.4), also two features without owned features that redefine a common
/// feature and of which one can access the other (ADR 0021).
///
/// Compatibility, access and featuring may be circular: a fixpoint from false.
#[salsa::tracked(returns(copy), cycle_initial = not_compatible, cycle_fn = settle_compatible)]
pub fn is_compatible_with(
    db: &dyn salsa::Database,
    project: Project,
    ty: Global,
    other: Global,
) -> bool {
    if specializes(db, project, ty, other) {
        return true;
    }
    if !kind_of(db, project, ty, M::Feature) || !kind_of(db, project, other, M::Feature) {
        return false;
    }
    owned_feature(db, project, ty).is_empty()
        && owned_feature(db, project, other).is_empty()
        && {
            let theirs = all_redefined_features(db, project, other);
            all_redefined_features(db, project, ty)
                .iter()
                .any(|f| theirs.contains(f))
        }
        && can_access(db, project, ty, other)
}

pub(crate) fn not_compatible(
    _db: &dyn salsa::Database,
    _id: salsa::Id,
    _project: Project,
    _ty: Global,
    _other: Global,
) -> bool {
    false
}

/// `multiplicities()` (KerML 8.3.3.1.10): the type's multiplicity, else the multiplicities of the
/// nearest generals that have one.
pub fn multiplicities(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    if let Some(m) = multiplicity(db, project, ty) {
        return vec![m];
    }
    let mut out = Vec::new();
    let mut seen: HashSet<Global> = HashSet::from([ty]);
    let mut stack: Vec<Global> = specializations(db, project, ty, false)
        .into_iter()
        .map(|(_, g)| g)
        .rev()
        .collect();
    while let Some(t) = stack.pop() {
        if !seen.insert(t) {
            continue;
        }
        match multiplicity(db, project, t) {
            Some(m) => out.push(m),
            None => stack.extend(
                specializations(db, project, t, false)
                    .into_iter()
                    .map(|(_, g)| g)
                    .rev(),
            ),
        }
    }
    unique(out)
}

/// `inheritedMemberships(excludedNamespaces, excludedTypes, excludeImplied)` (KerML
/// 8.3.3.1.10): `removeRedefinedFeatures(inheritableMemberships(…))`, in the OCL's order: for
/// each supertype its public memberships, its protected ones, then those it inherits. Circular
/// specializations converge by fixpoint iteration (ADR 0019); `inheritableMemberships` leaves
/// out the type's own memberships, as the OCL's excluded types do (ADR 0021).
#[salsa::tracked(returns(ref), cycle_initial = no_memberships, cycle_fn = settle_memberships)]
pub fn type_inherited_memberships(
    db: &dyn salsa::Database,
    project: Project,
    ty: Global,
    exclude_implied: bool,
) -> Vec<Global> {
    let inheritable = inheritable_memberships(db, project, ty, exclude_implied);
    remove_redefined_features(db, project, ty, &inheritable)
}

pub(crate) fn no_memberships(
    _db: &dyn salsa::Database,
    _id: salsa::Id,
    _project: Project,
    _ty: Global,
    _exclude_implied: bool,
) -> Vec<Global> {
    Vec::new()
}

/// `inheritableMemberships(…, excludeImplied)` (KerML 8.3.3.1.10): the non-private memberships of
/// the supertypes, before `removeRedefinedFeatures`.
pub fn inheritable_memberships(
    db: &dyn salsa::Database,
    project: Project,
    ty: Global,
    exclude_implied: bool,
) -> Vec<Global> {
    // `excludedTypes->including(self)`: around a circular specialization, the type's own owned
    // and imported memberships come back through its supertypes, and are left out.
    let own: HashSet<Global> = super::root::memberships_of_visibility(db, project, ty, None)
        .into_iter()
        .collect();
    let mut out = Vec::new();
    for t in supertypes(db, project, ty, exclude_implied) {
        if t != ty {
            out.extend(
                non_private_memberships(db, project, t, exclude_implied)
                    .into_iter()
                    .filter(|m| !own.contains(m)),
            );
        }
    }
    unique(out)
}

/// `nonPrivateMemberships(…, excludeImplied)` (KerML 8.3.3.1.10): the public and protected
/// memberships, and the inherited ones.
pub fn non_private_memberships(
    db: &dyn salsa::Database,
    project: Project,
    ty: Global,
    exclude_implied: bool,
) -> Vec<Global> {
    use super::root::memberships_of_visibility;
    use crate::Visibility;
    let mut out = memberships_of_visibility(db, project, ty, Some(Visibility::Public));
    out.extend(memberships_of_visibility(
        db,
        project,
        ty,
        Some(Visibility::Protected),
    ));
    out.extend(
        type_inherited_memberships(db, project, ty, exclude_implied)
            .iter()
            .copied(),
    );
    unique(out)
}

/// `allRedefinedFeaturesOf(membership)` (KerML 8.3.3.1.10): the member feature's
/// `allRedefinedFeatures()`, or none for another element.
pub fn all_redefined_features_of(
    db: &dyn salsa::Database,
    project: Project,
    membership: Global,
) -> Vec<Global> {
    match membership_element(db, project, membership) {
        Some(e) if kind_of(db, project, e, M::Feature) => all_redefined_features(db, project, e),
        _ => Vec::new(),
    }
}

/// `removeRedefinedFeatures(memberships)` (KerML 8.3.3.1.10): without the memberships whose
/// element another of them redefines, directly or indirectly, and without those that redefine,
/// or are, a feature an owned feature of the type redefines.
pub fn remove_redefined_features(
    db: &dyn salsa::Database,
    project: Project,
    ty: Global,
    memberships: &[Global],
) -> Vec<Global> {
    let redefined_by: Vec<Vec<Global>> = memberships
        .iter()
        .map(|m| all_redefined_features_of(db, project, *m))
        .collect();
    let reduced: Vec<(usize, Global)> = memberships
        .iter()
        .copied()
        .enumerate()
        .filter(|(i, m1)| {
            let element = membership_element(db, project, *m1);
            !memberships.iter().enumerate().any(|(j, m2)| {
                j != *i && m2 != m1 && element.is_some_and(|e| redefined_by[j].contains(&e))
            })
        })
        .collect();
    let owned_redefined: HashSet<Global> = owned_feature(db, project, ty)
        .into_iter()
        .flat_map(|f| direct_redefined(db, project, f))
        .collect();
    reduced
        .into_iter()
        .filter(|(i, _)| !redefined_by[*i].iter().any(|f| owned_redefined.contains(f)))
        .map(|(_, m)| m)
        .collect()
}

// ---- Classifier (KerML 8.3.3.2.2) ----

/// `ownedSubclassification` (deriveClassifierOwnedSubclassification).
pub fn owned_subclassification(
    db: &dyn salsa::Database,
    project: Project,
    c: Global,
) -> Vec<Global> {
    owned_specialization(db, project, c)
        .into_iter()
        .filter(|r| kind_of(db, project, *r, M::Subclassification))
        .collect()
}

// ---- Feature (KerML 8.3.3.3.4) ----

/// `ownedSubsetting` (deriveFeatureOwnedSubsetting): the owned specializations that are
/// Subsettings.
pub fn owned_subsetting(db: &dyn salsa::Database, project: Project, f: Global) -> Vec<Global> {
    owned_specialization(db, project, f)
        .into_iter()
        .filter(|r| kind_of(db, project, *r, M::Subsetting))
        .collect()
}

/// `ownedRedefinition` (deriveFeatureOwnedRedefinition).
pub fn owned_redefinition(db: &dyn salsa::Database, project: Project, f: Global) -> Vec<Global> {
    owned_subsetting(db, project, f)
        .into_iter()
        .filter(|r| kind_of(db, project, *r, M::Redefinition))
        .collect()
}

/// `ownedTyping` (deriveFeatureOwnedTyping): the owned specializations that are FeatureTypings.
pub fn owned_typing(db: &dyn salsa::Database, project: Project, f: Global) -> Vec<Global> {
    owned_specialization(db, project, f)
        .into_iter()
        .filter(|r| kind_of(db, project, *r, M::FeatureTyping))
        .collect()
}

/// `ownedReferenceSubsetting` (deriveFeatureOwnedReferenceSubsetting): the first owned
/// ReferenceSubsetting.
pub fn owned_reference_subsetting(
    db: &dyn salsa::Database,
    project: Project,
    f: Global,
) -> Option<Global> {
    owned_subsetting(db, project, f)
        .into_iter()
        .find(|r| kind_of(db, project, *r, M::ReferenceSubsetting))
}

/// `ownedCrossSubsetting` (deriveFeatureOwnedCrossSubsetting): the first owned CrossSubsetting.
pub fn owned_cross_subsetting(
    db: &dyn salsa::Database,
    project: Project,
    f: Global,
) -> Option<Global> {
    owned_subsetting(db, project, f)
        .into_iter()
        .find(|r| kind_of(db, project, *r, M::CrossSubsetting))
}

/// `ownedFeatureChaining` (deriveFeatureOwnedFeatureChaining).
pub fn owned_feature_chaining(
    db: &dyn salsa::Database,
    project: Project,
    f: Global,
) -> Vec<Global> {
    owned_relationships_of(db, project, f, M::FeatureChaining)
}

/// `chainingFeature` (deriveFeatureChainingFeature): `ownedFeatureChaining.chainingFeature`.
pub fn chaining_feature(db: &dyn salsa::Database, project: Project, f: Global) -> Vec<Global> {
    let rels = owned_feature_chaining(db, project, f);
    rels.into_iter()
        .filter_map(|r| first_end(db, project, r))
        .collect()
}

/// `featureTarget` (deriveFeatureFeatureTarget): the last chaining feature, else the feature.
pub fn feature_target(db: &dyn salsa::Database, project: Project, f: Global) -> Global {
    chaining_feature(db, project, f)
        .last()
        .copied()
        .unwrap_or(f)
}

/// `crossFeature` (deriveFeatureCrossFeature): the second chaining feature of the feature the
/// owned cross subsetting crosses, if it has two.
pub fn cross_feature(db: &dyn salsa::Database, project: Project, f: Global) -> Option<Global> {
    let crossed = first_end(db, project, owned_cross_subsetting(db, project, f)?)?;
    chaining_feature(db, project, crossed).get(1).copied()
}

/// `ownedTypeFeaturing` (deriveFeatureOwnedTypeFeaturing): the owned TypeFeaturings whose
/// `featureOfType` (source) is the feature.
pub fn owned_type_featuring(db: &dyn salsa::Database, project: Project, f: Global) -> Vec<Global> {
    owned_relationships_of(db, project, f, M::TypeFeaturing)
        .into_iter()
        .filter(|r| source(db, project, *r) == [f])
        .collect()
}

/// `ownedFeatureInverting` (deriveFeatureOwnedFeatureInverting): the owned FeatureInvertings
/// whose `featureInverted` (source) is the feature.
pub fn owned_feature_inverting(
    db: &dyn salsa::Database,
    project: Project,
    f: Global,
) -> Vec<Global> {
    owned_relationships_of(db, project, f, M::FeatureInverting)
        .into_iter()
        .filter(|r| source(db, project, *r) == [f])
        .collect()
}

/// `featuringType` (deriveFeatureFeaturingType): the types of the feature's owned
/// TypeFeaturings, written and implied (ADR 0022), and for a chained feature those of its first
/// chaining feature. The implied ones are those of the type-featuring constraints (KerML 8.4.2;
/// `implied_featurings`), such as the owning type of a feature owned through a FeatureMembership.
///
/// Chains and featurings may be circular: a fixpoint from none.
#[salsa::tracked(returns(ref), cycle_initial = no_featuring, cycle_fn = settle_all_supertypes)]
pub fn featuring_type(db: &dyn salsa::Database, project: Project, f: Global) -> Vec<Global> {
    let mut out: Vec<Global> = owned_type_featuring(db, project, f)
        .into_iter()
        .filter_map(|r| first_end(db, project, r))
        .collect();
    if let Some(first) = chaining_feature(db, project, f).first() {
        if *first != f {
            out.extend(featuring_type(db, project, *first).iter().copied());
        }
    }
    unique(out)
}

pub(crate) fn no_featuring(
    _db: &dyn salsa::Database,
    _id: salsa::Id,
    _project: Project,
    _f: Global,
) -> Vec<Global> {
    Vec::new()
}

/// `typingFeatures()` (KerML 8.3.3.3.4): the features a feature's types come from: those it
/// subsets (but not crosses), and its last chaining feature; for a conjugated feature, its
/// original type if that is a feature.
pub fn typing_features(db: &dyn salsa::Database, project: Project, f: Global) -> Vec<Global> {
    if is_conjugated(db, project, f) {
        return original_type(db, project, f)
            .filter(|t| kind_of(db, project, *t, M::Feature))
            .into_iter()
            .collect();
    }
    let mut out: Vec<Global> = specializations(db, project, f, false)
        .into_iter()
        .filter(|(k, _)| k.specializes(M::Subsetting) && !k.specializes(M::CrossSubsetting))
        .map(|(_, g)| g)
        .collect();
    if let Some(last) = chaining_feature(db, project, f).last() {
        if !out.contains(last) {
            out.push(*last);
        }
    }
    unique(out)
}

/// `type` (deriveFeatureType): the types of the typings of the feature and of all its typing
/// features, without those another of them specializes.
pub fn r#type(db: &dyn salsa::Database, project: Project, f: Global) -> Vec<Global> {
    let mut closure = vec![f];
    let mut seen: HashSet<Global> = HashSet::from([f]);
    let mut i = 0;
    while i < closure.len() {
        for t in typing_features(db, project, closure[i]) {
            if seen.insert(t) {
                closure.push(t);
            }
        }
        i += 1;
    }
    let types: Vec<Global> = unique(closure.into_iter().flat_map(|c| {
        specializations(db, project, c, false)
            .into_iter()
            .filter(|(k, _)| k.specializes(M::FeatureTyping))
            .map(|(_, g)| g)
            .collect::<Vec<_>>()
    }));
    types
        .iter()
        .copied()
        .filter(|t1| {
            !types
                .iter()
                .any(|t2| t2 != t1 && specializes(db, project, *t2, *t1))
        })
        .collect()
}

/// `directionFor(type)` (KerML 8.3.3.3.4): `type.directionOf(self)`.
pub fn direction_for(
    db: &dyn salsa::Database,
    project: Project,
    f: Global,
    ty: Global,
) -> Option<Direction> {
    direction_of(db, project, ty, f)
}

/// `namingFeature()` (KerML 8.3.3.3.4): the redefined feature of the first owned redefinition,
/// the written ones before the implied ones (ADR 0022).
pub fn naming_feature(db: &dyn salsa::Database, project: Project, f: Global) -> Option<Global> {
    first_end(db, project, *owned_redefinition(db, project, f).first()?)
}

/// The features a feature redefines directly: its owned redefinitions', written or implied.
fn direct_redefined(db: &dyn salsa::Database, project: Project, f: Global) -> Vec<Global> {
    unique(
        owned_redefinition(db, project, f)
            .into_iter()
            .filter_map(|r| first_end(db, project, r)),
    )
}

/// `redefines(redefinedFeature)` (KerML 8.3.3.3.4): whether an owned redefinition redefines it.
pub fn redefines(db: &dyn salsa::Database, project: Project, f: Global, redefined: Global) -> bool {
    direct_redefined(db, project, f).contains(&redefined)
}

/// `redefinesFromLibrary(libraryFeatureName)` (KerML 8.3.3.3.4).
pub fn redefines_from_library(
    db: &dyn salsa::Database,
    project: Project,
    f: Global,
    library_feature_name: &str,
) -> bool {
    library(db, project, library_feature_name)
        .filter(|x| kind_of(db, project, *x, M::Feature))
        .is_some_and(|x| redefines(db, project, f, x))
}

/// `allRedefinedFeatures()` (KerML 8.3.3.3.4): the feature, then the features it redefines,
/// directly or indirectly.
pub fn all_redefined_features(
    db: &dyn salsa::Database,
    project: Project,
    f: Global,
) -> Vec<Global> {
    let mut out = vec![f];
    out.extend(redefined_features(db, project, f).iter().copied());
    unique(out)
}

/// `subsetsChain(first, second)` (KerML 8.3.3.3.4): whether one of the feature's supertypes is a
/// chain whose last two chaining features are `first` and `second`.
pub fn subsets_chain(
    db: &dyn salsa::Database,
    project: Project,
    f: Global,
    first: Global,
    second: Global,
) -> bool {
    all_supertypes(db, project, f)
        .iter()
        .filter(|s| kind_of(db, project, **s, M::Feature))
        .any(|s| {
            let c = chaining_feature(db, project, *s);
            let n = c.len();
            n >= 2 && c[n - 2] == first && c[n - 1] == second
        })
}

/// `ownedCrossFeature()` (KerML 8.3.3.3.4): for an end feature with an owning type, its first
/// owned member feature that is not owned through a FeatureMembership or a FeatureValue and is
/// not a Multiplicity or a MetadataFeature.
pub fn owned_cross_feature(
    db: &dyn salsa::Database,
    project: Project,
    f: Global,
) -> Option<Global> {
    if !flag(db, project, f, Attr::IsEnd) || owning_type(db, project, f).is_none() {
        return None;
    }
    let g = view(db, project, f.file);
    owned_member(db, project, f).into_iter().find(|m| {
        let k = g.metaclass(m.ix());
        k.specializes(M::Feature)
            && !k.specializes(M::Multiplicity)
            && !k.specializes(M::MetadataFeature)
            // The OCL's `reject(oclIsKindOf(FeatureValue))`, which no Feature is, read as the value
            // of a FeatureValue (ADR 0020).
            && !g.owning_relationship(m.ix()).is_some_and(|r| {
                let rm = g.metaclass(r);
                rm.specializes(M::FeatureMembership) || rm.specializes(M::FeatureValue)
            })
    })
}

/// `isOwnedCrossFeature()` (KerML 8.3.3.3.4): whether the feature is the owned cross feature of
/// its owning namespace.
pub fn is_owned_cross_feature(db: &dyn salsa::Database, project: Project, f: Global) -> bool {
    owning_namespace(db, project, f)
        .filter(|n| kind_of(db, project, *n, M::Feature))
        .is_some_and(|n| owned_cross_feature(db, project, n) == Some(f))
}

/// `isCartesianProduct()` (KerML 8.3.3.3.4).
pub fn is_cartesian_product(db: &dyn salsa::Database, project: Project, f: Global) -> bool {
    let featuring = featuring_type(db, project, f).clone();
    r#type(db, project, f).len() == 1
        && featuring.len() == 1
        && (owner_of(db, project, featuring[0]) != Some(f)
            || (kind_of(db, project, featuring[0], M::Feature)
                && is_cartesian_product(db, project, featuring[0])))
}

fn owner_of(db: &dyn salsa::Database, project: Project, x: Global) -> Option<Global> {
    view(db, project, x.file)
        .owner(x.ix())
        .map(|o| at(x.file, o))
}

/// `asCartesianProduct()` (KerML 8.3.3.3.4): the featuring types not owned by the feature, those
/// of the owned featuring features as cartesian products, then the feature's types.
pub fn as_cartesian_product(db: &dyn salsa::Database, project: Project, f: Global) -> Vec<Global> {
    let featuring = featuring_type(db, project, f).clone();
    let mut out: Vec<Global> = featuring
        .iter()
        .copied()
        .filter(|t| owner_of(db, project, *t) != Some(f))
        .collect();
    for t in featuring {
        if owner_of(db, project, t) == Some(f) && kind_of(db, project, t, M::Feature) && t != f {
            out.extend(as_cartesian_product(db, project, t));
        }
    }
    out.extend(r#type(db, project, f));
    unique(out)
}

/// `isFeaturedWithin(type)` (KerML 8.3.3.3.4): for no type, whether every featuring type is
/// `Base::Anything`; else whether the type is compatible with every featuring type, or the
/// feature, or its first chaining feature, is variable and the type specializes its owning type.
pub fn is_featured_within(
    db: &dyn salsa::Database,
    project: Project,
    f: Global,
    ty: Option<Global>,
) -> bool {
    let featuring = featuring_type(db, project, f).clone();
    let Some(ty) = ty else {
        let anything = library(db, project, "Base::Anything");
        return featuring.iter().all(|t| Some(*t) == anything);
    };
    if featuring
        .iter()
        .all(|t| is_compatible_with(db, project, ty, *t))
    {
        return true;
    }
    let variable_in = |x: Global| {
        flag(db, project, x, Attr::IsVariable)
            && owning_type(db, project, x).is_some_and(|o| specializes(db, project, ty, o))
    };
    variable_in(f)
        || chaining_feature(db, project, f)
            .first()
            .is_some_and(|c| variable_in(*c))
}

/// `canAccess(feature)` (KerML 8.3.3.3.4): whether the other feature is featured within one of
/// this feature's featuring types or, transitively, theirs (`Base::Anything` for a feature
/// without featuring types; ADR 0021).
pub fn can_access(db: &dyn salsa::Database, project: Project, f: Global, other: Global) -> bool {
    let anything = library(db, project, "Base::Anything");
    let mut all = featuring_type(db, project, f).clone();
    let mut seen: HashSet<Global> = all.iter().copied().collect();
    let mut i = 0;
    while i < all.len() {
        let t = all[i];
        if kind_of(db, project, t, M::Feature) {
            let next = featuring_type(db, project, t).clone();
            let next = if next.is_empty() {
                anything.into_iter().collect()
            } else {
                next
            };
            for x in next {
                if seen.insert(x) {
                    all.push(x);
                }
            }
        }
        i += 1;
    }
    all.into_iter()
        .any(|t| is_featured_within(db, project, other, Some(t)))
}

/// `isFeaturingType(type)` (KerML 8.3.3.3.4): whether the type is one the FeatureMembership of
/// the feature implies as its featuring type: its owning type; for a variable feature, the
/// `snapshots` of `Occurrences::Occurrence`, or a feature featured by the owning type that
/// redefines them.
pub fn is_featuring_type(
    db: &dyn salsa::Database,
    project: Project,
    f: Global,
    ty: Global,
) -> bool {
    let Some(owner) = owning_type(db, project, f) else {
        return false;
    };
    if !flag(db, project, f, Attr::IsVariable) {
        return ty == owner;
    }
    if library(db, project, "Occurrences::Occurrence") == Some(owner) {
        return library(db, project, "Occurrences::Occurrence::snapshots") == Some(ty);
    }
    kind_of(db, project, ty, M::Feature)
        && featuring_type(db, project, ty).contains(&owner)
        && redefines_from_library(db, project, ty, "Occurrences::Occurrence::snapshots")
}
