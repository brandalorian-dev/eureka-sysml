// SPDX-License-Identifier: Apache-2.0
//! The Root layer (KerML 8.3.2): elements, relationships, annotations and namespaces.

use std::sync::Arc;
use sysml_model::{Attr, Metaclass as M, Slot};

use super::{at, end, ends, metaclass, owned_relationships_of, unique};
use crate::materialize::view;
use crate::resolve::{
    collide, effective_names, global_namespace, import_memberships, imported_memberships,
    member_element, member_metaclass, memberships, namespace_memberships, settle_path,
    settle_qualified_name, Entry, Global, Project, Visibility,
};

// ---- Element (KerML 8.3.2.1.2) ----

/// `owner` (deriveElementOwner): `owningRelationship.owningRelatedElement`.
pub fn owner(db: &dyn salsa::Database, project: Project, x: Global) -> Option<Global> {
    view(db, project, x.file)
        .owner(x.ix())
        .map(|o| at(x.file, o))
}

/// `ownedElement` (deriveElementOwnedElement): `ownedRelationship.ownedRelatedElement`. Like
/// every `ownedRelationship` here, it includes the implied relationships once `isImpliedIncluded`
/// is true (KerML 8.3.2.1.2; ADR 0017).
pub fn owned_element(db: &dyn salsa::Database, project: Project, x: Global) -> Vec<Global> {
    let g = view(db, project, x.file);
    g.all_owned_relationships(x.ix())
        .flat_map(|r| g.owned_related_elements(r))
        .map(|e| at(x.file, e))
        .collect()
}

/// `owningMembership` (KerML 8.3.2.1.2): the owning relationship, if it is an OwningMembership.
pub fn owning_membership(db: &dyn salsa::Database, project: Project, x: Global) -> Option<Global> {
    let g = view(db, project, x.file);
    g.owning_relationship(x.ix())
        .filter(|r| g.metaclass(*r).specializes(M::OwningMembership))
        .map(|r| at(x.file, r))
}

/// `owningNamespace` (deriveOwningNamespace): `owningMembership.membershipOwningNamespace`.
pub fn owning_namespace(db: &dyn salsa::Database, project: Project, x: Global) -> Option<Global> {
    let m = owning_membership(db, project, x)?;
    let s = view(db, project, x.file);
    s.owning_related_element(m.ix()).map(|n| at(x.file, n))
}

/// `name` (deriveElementName): `effectiveName()`.
pub fn name(db: &dyn salsa::Database, project: Project, x: Global) -> Option<Arc<str>> {
    effective_name(db, project, x)
}

/// `shortName` (deriveElementShortName): `effectiveShortName()`.
pub fn short_name(db: &dyn salsa::Database, project: Project, x: Global) -> Option<Arc<str>> {
    effective_short_name(db, project, x)
}

/// `effectiveName()` (KerML 8.3.2.1.2: `declaredName`; redefined for a Feature in 8.3.3.3.4: the
/// name of its naming feature, `namingFeature()`, when it declares no name).
pub fn effective_name(db: &dyn salsa::Database, project: Project, x: Global) -> Option<Arc<str>> {
    effective_names(db, project, x.file, x.index).0.clone()
}

/// `effectiveShortName()` (KerML 8.3.2.1.2, redefined for a Feature in 8.3.3.3.4).
pub fn effective_short_name(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> Option<Arc<str>> {
    effective_names(db, project, x.file, x.index).1.clone()
}

/// A name as KerML text (KerML 8.2.2.3): as it is if it is a basic name, else as an unrestricted
/// name, in single quotes with the escapes of Table 4.
pub fn escape(name: &str) -> String {
    let basic = name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if basic {
        return name.to_owned();
    }
    let mut out = String::from("'");
    for c in name.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

/// `escapedName()` (KerML 8.3.2.1.2): the name, else the short name, escaped as KerML text.
pub fn escaped_name(db: &dyn salsa::Database, project: Project, x: Global) -> Option<String> {
    name(db, project, x)
        .or_else(|| short_name(db, project, x))
        .map(|n| escape(&n))
}

/// `qualifiedName` (deriveElementQualifiedName). Null without an owning namespace, or when an
/// earlier owned member of that namespace has the same name; the escaped name in a root
/// namespace; else the owning namespace's qualified name, `::` and the escaped name.
#[salsa::tracked(returns(ref), cycle_initial = no_qualified_name, cycle_fn = settle_qualified_name)]
pub fn qualified_name(db: &dyn salsa::Database, project: Project, x: Global) -> Option<Arc<str>> {
    let ns = owning_namespace(db, project, x)?;
    if let Some(n) = name(db, project, x) {
        if first_owned_members(db, project, ns).get(&n) != Some(&x) {
            return None;
        }
    }
    let escaped = escaped_name(db, project, x)?;
    if owner(db, project, ns).is_none() {
        return Some(escaped.into());
    }
    let q = qualified_name(db, project, ns).clone()?;
    Some(format!("{q}::{escaped}").into())
}

/// The first owned member of a namespace with each name, for `qualifiedName`: computed once per
/// namespace.
#[salsa::tracked(returns(ref))]
fn first_owned_members(
    db: &dyn salsa::Database,
    project: Project,
    ns: Global,
) -> std::collections::HashMap<Arc<str>, Global> {
    let mut out = std::collections::HashMap::new();
    for m in owned_member(db, project, ns) {
        if let Some(n) = name(db, project, m) {
            out.entry(n).or_insert(m);
        }
    }
    out
}

pub(crate) fn no_qualified_name(
    _db: &dyn salsa::Database,
    _id: salsa::Id,
    _project: Project,
    _x: Global,
) -> Option<Arc<str>> {
    None
}

/// `path()` (KerML 8.3.2.1.2 for an Element, 8.3.2.1.3 for a Relationship, 8.3.2.4.8 for an
/// OwningMembership): a unique description of where an element is in its root namespace.
#[salsa::tracked(returns(ref), cycle_initial = no_path, cycle_fn = settle_path)]
pub fn path(db: &dyn salsa::Database, project: Project, x: Global) -> Arc<str> {
    let g = view(db, project, x.file);
    let m = g.metaclass(x.ix());
    if m.specializes(M::OwningMembership) {
        // 8.3.2.4.8: `ownedElement.qualifiedName + '/owningMembership'`, if that is not null.
        if let Some(e) = g.owned_related_elements(x.ix()).first() {
            if let Some(q) = qualified_name(db, project, at(x.file, *e)) {
                return format!("{q}/owningMembership").into();
            }
        }
    }
    if m.specializes(M::Relationship) && g.owning_relationship(x.ix()).is_none() {
        // 8.3.2.1.3: the position among the owning related element's owned relationships. An
        // implied relationship not included yet (`isImpliedIncluded`) is not among them.
        if let Some(o) = g.owning_related_element(x.ix()) {
            if let Some(i) = g.all_owned_relationships(o).position(|r| r == x.ix()) {
                return format!("{}/{}", path(db, project, at(x.file, o)), i + 1).into();
            }
        }
    }
    // 8.3.2.1.2.
    if let Some(q) = qualified_name(db, project, x) {
        return q.clone();
    }
    match g.owning_relationship(x.ix()) {
        Some(r) => {
            let i = g
                .owned_related_elements(r)
                .iter()
                .position(|e| *e == x.ix());
            let i = i.expect("an owned related element is among its owner's") + 1;
            format!("{}/{i}", path(db, project, at(x.file, r))).into()
        }
        None => "".into(),
    }
}

pub(crate) fn no_path(
    _db: &dyn salsa::Database,
    _id: salsa::Id,
    _project: Project,
    _x: Global,
) -> Arc<str> {
    "".into()
}

/// `ownedAnnotation` (deriveElementOwnedAnnotation): the owned Annotations whose annotated
/// element is this element.
pub fn owned_annotation(db: &dyn salsa::Database, project: Project, x: Global) -> Vec<Global> {
    owned_relationships_of(db, project, x, M::Annotation)
        .into_iter()
        .filter(|a| annotation_annotated_element(db, project, *a) == Some(x))
        .collect()
}

/// `documentation` (deriveElementDocumentation): the owned elements that are Documentation.
pub fn documentation(db: &dyn salsa::Database, project: Project, x: Global) -> Vec<Global> {
    owned_element(db, project, x)
        .into_iter()
        .filter(|e| metaclass(db, project, *e).specializes(M::Documentation))
        .collect()
}

/// `textualRepresentation` (deriveElementTextualRepresentation): the owned elements that are
/// TextualRepresentations.
pub fn textual_representation(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> Vec<Global> {
    owned_element(db, project, x)
        .into_iter()
        .filter(|e| metaclass(db, project, *e).specializes(M::TextualRepresentation))
        .collect()
}

/// `libraryNamespace()`: for an Element, that of its owning relationship (KerML 8.3.2.1.2); for a
/// Relationship, that of its owning related element, else of its owning relationship
/// (8.3.2.1.3); a LibraryPackage is its own (8.3.4.13.2).
pub fn library_namespace(db: &dyn salsa::Database, project: Project, x: Global) -> Option<Global> {
    let g = view(db, project, x.file);
    let mut e = x.ix();
    loop {
        let m = g.metaclass(e);
        if m.specializes(M::LibraryPackage) {
            return Some(at(x.file, e));
        }
        e = if m.specializes(M::Relationship) {
            g.owning_related_element(e).or(g.owning_relationship(e))?
        } else {
            g.owning_relationship(e)?
        };
    }
}

/// `isLibraryElement` (deriveElementIsLibraryElement): `libraryNamespace() <> null`.
pub fn is_library_element(db: &dyn salsa::Database, project: Project, x: Global) -> bool {
    library_namespace(db, project, x).is_some()
}

// ---- Relationship (KerML 8.3.2.1.3) ----

/// `source` (KerML 8.3.2.1.3) as each metaclass redefines it: the ends written as sources where
/// the metaclass stores them; otherwise the owning related element (`membershipOwningNamespace`,
/// `importOwningNamespace`, `featureChained`, `owningType` …). An Annotation's source is its
/// annotating element, owned or owning (8.3.2.3.3).
pub fn source(db: &dyn salsa::Database, project: Project, x: Global) -> Vec<Global> {
    let s = view(db, project, x.file);
    let m = s.metaclass(x.ix());
    if m.specializes(M::Annotation) {
        return annotating_element(db, project, x).into_iter().collect();
    }
    if m.stored_attributes().iter().any(|a| a.slot == Slot::Source) {
        return ends(db, project, x, Slot::Source);
    }
    s.owning_related_element(x.ix())
        .map(|o| at(x.file, o))
        .into_iter()
        .collect()
}

/// `target` (KerML 8.3.2.1.3) as each metaclass redefines it: the ends written as targets where
/// the metaclass stores them; otherwise the owned related elements (an OwningMembership's
/// `ownedMemberElement`). An Annotation's target is its annotated element (8.3.2.3.3).
pub fn target(db: &dyn salsa::Database, project: Project, x: Global) -> Vec<Global> {
    let g = view(db, project, x.file);
    let m = g.metaclass(x.ix());
    if m.specializes(M::Annotation) {
        return annotation_annotated_element(db, project, x)
            .into_iter()
            .collect();
    }
    if m.stored_attributes().iter().any(|a| a.slot == Slot::Target) {
        return ends(db, project, x, Slot::Target);
    }
    g.owned_related_elements(x.ix())
        .iter()
        .map(|e| at(x.file, *e))
        .collect()
}

/// `relatedElement` (deriveRelationshipRelatedElement): `source->union(target)`.
pub fn related_element(db: &dyn salsa::Database, project: Project, x: Global) -> Vec<Global> {
    let mut out = source(db, project, x);
    out.extend(target(db, project, x));
    out
}

// ---- Annotations (KerML 8.3.2.3) ----

/// `Annotation::annotatedElement`: the element written as its target or, for an Annotation that
/// owns its annotating element, its owner (`owningAnnotatedElement`, which subsets
/// `annotatedElement`; ADR 0021).
pub fn annotation_annotated_element(
    db: &dyn salsa::Database,
    project: Project,
    a: Global,
) -> Option<Global> {
    let s = view(db, project, a.file);
    if !s.ends(a.ix(), Slot::Target).is_empty() {
        return end(db, project, a, Slot::Target, 0);
    }
    owned_annotating_element(db, project, a)?;
    s.owning_related_element(a.ix()).map(|o| at(a.file, o))
}

/// `ownedAnnotatingElement` (deriveAnnotationOwnedAnnotatingElement): the first owned related
/// element that is an AnnotatingElement.
pub fn owned_annotating_element(
    db: &dyn salsa::Database,
    project: Project,
    a: Global,
) -> Option<Global> {
    let g = view(db, project, a.file);
    g.owned_related_elements(a.ix())
        .iter()
        .find(|e| g.metaclass(**e).specializes(M::AnnotatingElement))
        .map(|e| at(a.file, *e))
}

/// `owningAnnotatingElement` (KerML 8.3.2.3.3): the owning related element, if it is an
/// AnnotatingElement.
pub fn owning_annotating_element(
    db: &dyn salsa::Database,
    project: Project,
    a: Global,
) -> Option<Global> {
    let g = view(db, project, a.file);
    g.owning_related_element(a.ix())
        .filter(|o| g.metaclass(*o).specializes(M::AnnotatingElement))
        .map(|o| at(a.file, o))
}

/// `annotatingElement` (deriveAnnotationAnnotatingElement): the owned annotating element, else
/// the owning one.
pub fn annotating_element(db: &dyn salsa::Database, project: Project, a: Global) -> Option<Global> {
    owned_annotating_element(db, project, a).or_else(|| owning_annotating_element(db, project, a))
}

/// `ownedAnnotatingRelationship` (deriveAnnotatingElementOwnedAnnotatingRelationship): the owned
/// Annotations whose annotated element is not this element.
pub fn owned_annotating_relationship(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> Vec<Global> {
    owned_relationships_of(db, project, x, M::Annotation)
        .into_iter()
        .filter(|a| annotation_annotated_element(db, project, *a) != Some(x))
        .collect()
}

/// `owningAnnotatingRelationship` (KerML 8.3.2.3.2): the owning relationship, if it is an
/// Annotation.
pub fn owning_annotating_relationship(
    db: &dyn salsa::Database,
    project: Project,
    x: Global,
) -> Option<Global> {
    let g = view(db, project, x.file);
    g.owning_relationship(x.ix())
        .filter(|r| g.metaclass(*r).specializes(M::Annotation))
        .map(|r| at(x.file, r))
}

/// `annotation` (deriveAnnotatingElementAnnotation): the owned annotating relationships, after
/// the owning one if there is one.
pub fn annotation(db: &dyn salsa::Database, project: Project, x: Global) -> Vec<Global> {
    let mut out: Vec<Global> = owning_annotating_relationship(db, project, x)
        .into_iter()
        .collect();
    out.extend(owned_annotating_relationship(db, project, x));
    out
}

/// `annotatedElement` of an AnnotatingElement (deriveAnnotatingElementAnnotatedElement): those of
/// its annotations or, with none, its owning namespace.
pub fn annotated_element(db: &dyn salsa::Database, project: Project, x: Global) -> Vec<Global> {
    let annotations = annotation(db, project, x);
    if annotations.is_empty() {
        return owning_namespace(db, project, x).into_iter().collect();
    }
    annotations
        .into_iter()
        .filter_map(|a| annotation_annotated_element(db, project, a))
        .collect()
}

// ---- Namespaces (KerML 8.3.2.4) ----

/// `ownedMembership` (deriveNamespaceOwnedMembership): the owned relationships that are
/// Memberships.
pub fn owned_membership(db: &dyn salsa::Database, project: Project, ns: Global) -> Vec<Global> {
    owned_relationships_of(db, project, ns, M::Membership)
}

/// `ownedMember` (deriveNamespaceOwnedMember): the owned member elements of the owned
/// OwningMemberships.
pub fn owned_member(db: &dyn salsa::Database, project: Project, ns: Global) -> Vec<Global> {
    let g = view(db, project, ns.file);
    g.all_owned_relationships(ns.ix())
        .filter(|r| g.metaclass(*r).specializes(M::OwningMembership))
        .filter_map(|r| g.owned_related_elements(r).first().copied())
        .map(|e| at(ns.file, e))
        .collect()
}

/// `ownedImport` (deriveNamespaceOwnedImport): the owned relationships that are Imports.
pub fn owned_import(db: &dyn salsa::Database, project: Project, ns: Global) -> Vec<Global> {
    owned_relationships_of(db, project, ns, M::Import)
}

/// `importedMembership` (deriveNamespaceImportedMembership): `importedMemberships(Set{})`.
pub fn imported_membership(db: &dyn salsa::Database, project: Project, ns: Global) -> Vec<Global> {
    unique(
        namespace_imported_memberships(db, project, ns)
            .into_iter()
            .map(|e| e.membership),
    )
}

/// `membership` (KerML 8.3.2.4.5): the owned, imported and, for a type, inherited memberships,
/// without those a redefinition removes (8.3.3.1.10). Unnamed memberships count too, such as a
/// result expression's; name resolution's memberships hold only those with names.
pub fn membership(db: &dyn salsa::Database, project: Project, ns: Global) -> Vec<Global> {
    let mut out = owned_membership(db, project, ns);
    out.extend(imported_membership(db, project, ns));
    if view(db, project, ns.file)
        .metaclass(ns.ix())
        .specializes(M::Type)
    {
        out.extend(super::types::inherited_membership(db, project, ns));
    }
    unique(out)
}

/// `member` (deriveNamespaceMembers): `membership.memberElement`, for each membership once.
pub fn member(db: &dyn salsa::Database, project: Project, ns: Global) -> Vec<Global> {
    // `membership.memberElement`, unnamed memberships included (ADR 0021).
    membership(db, project, ns)
        .into_iter()
        .filter_map(|m| membership_element(db, project, m))
        .collect()
}

/// `Namespace::importedMemberships(excluded)` (KerML 8.3.2.4.5): the memberships of the owned
/// imports, without collisions. The excluded set is not needed: circular imports converge by
/// fixpoint iteration (ADR 0019).
pub fn namespace_imported_memberships(
    db: &dyn salsa::Database,
    project: Project,
    ns: Global,
) -> Vec<Entry> {
    imported_memberships(db, project, ns.file, ns.index).clone()
}

/// `Import::importedMemberships(excluded)`, as NamespaceImport (KerML 8.3.2.4.6) and
/// MembershipImport (8.3.2.4.4) redefine it: the memberships one import brings.
#[salsa::tracked(returns(ref))]
pub fn import_imported_memberships(
    db: &dyn salsa::Database,
    project: Project,
    import: Global,
) -> Vec<Global> {
    // The importing namespace is the import's owning related element (`importOwningNamespace`).
    let s = view(db, project, import.file);
    let Some(ns) = s.owning_related_element(import.ix()) else {
        return Vec::new();
    };
    let ns = at(import.file, ns);
    import_memberships(db, project, ns, import)
        .into_iter()
        .map(|e| e.membership)
        .collect()
}

/// `membershipsOfVisibility(visibility, excluded)` (KerML 8.3.2.4.5): the owned memberships with
/// the visibility, and the memberships of the owned imports with it; all of them for none.
pub fn memberships_of_visibility(
    db: &dyn salsa::Database,
    project: Project,
    ns: Global,
    visibility_kind: Option<Visibility>,
) -> Vec<Global> {
    let g = view(db, project, ns.file);
    let mut out: Vec<Global> = g
        .all_owned_relationships(ns.ix())
        .filter(|r| g.metaclass(*r).specializes(M::Membership))
        .filter(|r| visibility_kind.is_none_or(|v| g.visibility(*r) == v))
        .map(|r| at(ns.file, r))
        .collect();
    // `ownedImport->select(imp | …).importedMemberships(…)`: each import's own memberships,
    // without the namespace's exclusion of collisions.
    for import in owned_import(db, project, ns) {
        if visibility_kind.is_none_or(|v| g.visibility(import.ix()) == v) {
            out.extend(
                import_imported_memberships(db, project, import)
                    .iter()
                    .copied(),
            );
        }
    }
    unique(out)
}

/// `visibleMemberships(excluded, isRecursive, includeAll)` (KerML 8.3.2.4.5, and 8.3.3.1.10 for
/// a type).
pub fn visible_memberships(
    db: &dyn salsa::Database,
    project: Project,
    ns: Global,
    recursive: bool,
    include_all: bool,
) -> Vec<Global> {
    unique(
        namespace_memberships(db, project, ns, recursive, include_all)
            .into_iter()
            .filter(|e| !e.hidden)
            .map(|e| e.membership),
    )
}

/// `namesOf(element)` (KerML 8.3.2.4.5): the names and short names of the memberships with that
/// member element. The OCL selects those memberships and then takes the names of all memberships;
/// the description and the `let` say the selected ones (ADR 0021).
pub fn names_of(
    db: &dyn salsa::Database,
    project: Project,
    ns: Global,
    element: Global,
) -> Vec<Arc<str>> {
    let selected: Vec<Entry> = memberships(db, project, ns)
        .into_iter()
        .filter(|e| !e.hidden && member_element(db, project, e) == Some(element))
        .collect();
    // `memberShortName->union(memberName)->asSet()`: the short names, then the names, once each.
    let mut out: Vec<Arc<str>> = Vec::new();
    let short = selected.iter().filter_map(|e| e.short_name.clone());
    for n in short.chain(selected.iter().filter_map(|e| e.name.clone())) {
        if !out.contains(&n) {
            out.push(n);
        }
    }
    out
}

/// `visibilityOf(mem)` (KerML 8.3.2.4.5): for an imported membership, the visibility of the first
/// owned import that imports it; for another membership of the namespace, its own; else private.
pub fn visibility_of(
    db: &dyn salsa::Database,
    project: Project,
    ns: Global,
    mem: Global,
) -> Visibility {
    // `importedMembership->includes(mem)`: after the exclusion of collisions.
    if imported_membership(db, project, ns).contains(&mem) {
        for import in owned_import(db, project, ns) {
            if import_imported_memberships(db, project, import).contains(&mem) {
                let s = view(db, project, import.file);
                return s.visibility(import.ix());
            }
        }
    }
    if membership(db, project, ns).contains(&mem) {
        let s = view(db, project, mem.file);
        return s.visibility(mem.ix());
    }
    Visibility::Private
}

/// `isDistinguishableFrom(other)` (KerML 8.3.2.4.3): whether two memberships differ in their
/// names, or their member elements' metaclasses do not conform either way.
pub fn is_distinguishable_from(
    db: &dyn salsa::Database,
    project: Project,
    a: Global,
    b: Global,
) -> bool {
    let (ea, eb) = (
        membership_entry(db, project, a),
        membership_entry(db, project, b),
    );
    !collide(
        &ea,
        member_metaclass(db, project, &ea),
        &eb,
        member_metaclass(db, project, &eb),
    )
}

/// A membership as resolution sees it: its member names (an OwningMembership's are its owned
/// element's, `ownedMemberName` and `ownedMemberShortName`).
pub(crate) fn membership_entry(db: &dyn salsa::Database, project: Project, m: Global) -> Entry {
    let g = view(db, project, m.file);
    let owned = g
        .owned_related_elements(m.ix())
        .first()
        .filter(|_| g.metaclass(m.ix()).specializes(M::OwningMembership))
        .map(|e| at(m.file, *e));
    let (name, short_name) = match owned {
        Some(e) => (
            effective_name(db, project, e),
            effective_short_name(db, project, e),
        ),
        None => (
            g.string(m.ix(), Attr::MemberName).map(Arc::from),
            g.string(m.ix(), Attr::MemberShortName).map(Arc::from),
        ),
    };
    Entry {
        name,
        short_name,
        membership: m,
        owned_element: owned,
        visibility: g.visibility(m.ix()),
        hidden: false,
    }
}

/// `ownedMemberName` (deriveOwningMembershipOwnedMemberName): `ownedMemberElement.name`.
pub fn owned_member_name(
    db: &dyn salsa::Database,
    project: Project,
    m: Global,
) -> Option<Arc<str>> {
    membership_entry(db, project, m)
        .owned_element
        .and_then(|e| name(db, project, e))
}

/// `ownedMemberShortName` (deriveOwningMembershipOwnedMemberShortName):
/// `ownedMemberElement.shortName`.
pub fn owned_member_short_name(
    db: &dyn salsa::Database,
    project: Project,
    m: Global,
) -> Option<Arc<str>> {
    membership_entry(db, project, m)
        .owned_element
        .and_then(|e| short_name(db, project, e))
}

/// `importedElement`: `importedNamespace` for a NamespaceImport (deriveNamespaceImport-
/// ImportedElement), which with filter conditions is the FilterPackage (KerML 8.2.3.4.2);
/// `importedMembership.memberElement` for a MembershipImport
/// (deriveMembershipImportImportedElement).
pub fn imported_element(
    db: &dyn salsa::Database,
    project: Project,
    import: Global,
) -> Option<Global> {
    let imported = end(db, project, import, Slot::Target, 0)?;
    if metaclass(db, project, import).specializes(M::MembershipImport) {
        return membership_element(db, project, imported);
    }
    Some(imported)
}

// ---- string resolution (KerML 8.3.2.4.5) ----

/// The segments of a qualified name written as KerML text (KerML 8.2.3.4.1), unescaped, and
/// whether it starts with `$::`. White space may separate the tokens (8.2.2.1). `None` if it is
/// not a qualified name.
fn segments(text: &str) -> Option<(bool, Vec<String>)> {
    // KerML 8.2.2.1: space, tab, form feed and the line terminators.
    let white = |c: &char| matches!(c, ' ' | '\t' | '\u{c}' | '\n' | '\r');
    let mut chars = text.chars().peekable();
    let skip = |chars: &mut std::iter::Peekable<std::str::Chars>| {
        while chars.next_if(white).is_some() {}
    };
    // `::` is one token: its two colons are not separated.
    let separator = |chars: &mut std::iter::Peekable<std::str::Chars>| {
        chars.next() == Some(':') && chars.next() == Some(':')
    };
    let mut out = Vec::new();
    skip(&mut chars);
    let global = chars.next_if_eq(&'$').is_some();
    if global {
        skip(&mut chars);
        separator(&mut chars).then_some(())?;
    }
    loop {
        skip(&mut chars);
        let mut s = String::new();
        if chars.next_if_eq(&'\'').is_some() {
            // The text up to the closing quote, unescaped as the lowering does (KerML 8.2.2.3).
            let mut raw = String::new();
            loop {
                match chars.next()? {
                    '\'' => break,
                    '\\' => {
                        raw.push('\\');
                        raw.push(chars.next()?);
                    }
                    c => raw.push(c),
                }
            }
            s = crate::lower::unescape(&raw);
        } else {
            while let Some(c) = chars.next_if(|c| c.is_ascii_alphanumeric() || *c == '_') {
                s.push(c);
            }
            if s.is_empty() || s.starts_with(|c: char| c.is_ascii_digit()) {
                return None;
            }
        }
        out.push(s);
        skip(&mut chars);
        if chars.peek().is_none() {
            return Some((global, out));
        }
        separator(&mut chars).then_some(())?;
    }
}

/// `qualificationOf(qualifiedName)` (KerML 8.3.2.4.5): the qualified name without its last
/// segment, as KerML text; `$` for a global name of one segment; null for a simple name.
pub fn qualification_of(qualified_name: &str) -> Option<String> {
    let (global, s) = segments(qualified_name)?;
    match s.len() {
        1 if global => Some("$".to_owned()),
        1 => None,
        n => {
            let q: Vec<String> = s[..n - 1].iter().map(|x| escape(x)).collect();
            Some(format!(
                "{}{}",
                if global { "$::" } else { "" },
                q.join("::")
            ))
        }
    }
}

/// `unqualifiedNameOf(qualifiedName)` (KerML 8.3.2.4.5): the last segment, unescaped.
pub fn unqualified_name_of(qualified_name: &str) -> Option<String> {
    segments(qualified_name)?.1.pop()
}

fn named<'a>(entries: impl IntoIterator<Item = &'a Entry>, name: &str) -> Option<Global> {
    entries
        .into_iter()
        .find(|e| {
            !e.hidden && (e.name.as_deref() == Some(name) || e.short_name.as_deref() == Some(name))
        })
        .map(|e| e.membership)
}

/// `resolveGlobal(qualifiedName)` (KerML 8.3.2.4.5): the membership a name of one segment has in
/// the global namespace (8.2.3.5.2), or, for a qualified name, its resolution there.
pub fn resolve_global(
    db: &dyn salsa::Database,
    project: Project,
    qualified_name: &str,
) -> Option<Global> {
    let (_, s) = segments(qualified_name)?;
    let (first, rest) = s.split_first()?;
    let mut m = named(global_namespace(db, project), first)?;
    for name in rest {
        m = resolve_visible(db, project, membership_element(db, project, m)?, name)?;
    }
    Some(m)
}

/// The member element of a membership.
pub(crate) fn membership_element(
    db: &dyn salsa::Database,
    project: Project,
    m: Global,
) -> Option<Global> {
    member_element(db, project, &membership_entry(db, project, m))
}

/// `resolveLocal(name)` (KerML 8.3.2.4.5): a membership of the namespace with the name, else the
/// resolution in the owning namespace; for a root namespace, in the global namespace.
pub fn resolve_local(
    db: &dyn salsa::Database,
    project: Project,
    ns: Global,
    name: &str,
) -> Option<Global> {
    let mut ns = ns;
    loop {
        let Some(outer) = owning_namespace(db, project, ns) else {
            return resolve_global(db, project, &escape(name));
        };
        if let Some(m) = named(&memberships(db, project, ns), name) {
            return Some(m);
        }
        ns = outer;
    }
}

/// `resolveVisible(name)` (KerML 8.3.2.4.5): a visible membership of the namespace with the name.
pub fn resolve_visible(
    db: &dyn salsa::Database,
    project: Project,
    ns: Global,
    name: &str,
) -> Option<Global> {
    named(&namespace_memberships(db, project, ns, false, false), name)
}

/// `resolve(qualifiedName)` (KerML 8.3.2.4.5): a simple name locally, a global one globally, and
/// otherwise the last segment visibly in the namespace the qualification resolves to.
pub fn resolve(
    db: &dyn salsa::Database,
    project: Project,
    ns: Global,
    qualified_name: &str,
) -> Option<Global> {
    let name = unqualified_name_of(qualified_name)?;
    match qualification_of(qualified_name) {
        None => resolve_local(db, project, ns, &name),
        Some(q) if q == "$" => resolve_global(db, project, &escape(&name)),
        Some(q) => {
            let namespace = membership_element(db, project, resolve(db, project, ns, &q)?)?;
            if !metaclass(db, project, namespace).specializes(M::Namespace) {
                return None;
            }
            resolve_visible(db, project, namespace, &name)
        }
    }
}
