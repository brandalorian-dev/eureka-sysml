// SPDX-License-Identifier: Apache-2.0
//! Derived properties and operations of the KerML metaclasses (KerML 8.3; step 1a.8, ADR 0021),
//! computed on read from the queries of name resolution (ADR 0019, ADR 0020).
//!
//! Each function implements one `derive…` rule or one operation and cites it. An element is a
//! [`Global`]; a function takes the project, because most properties depend on resolution. The
//! inventory of every rule and operation, with its status, is `crates/sysml-semantics/derived.json`,
//! kept equal to `KerML.xmi` by `tests/derived_inventory.rs`.
//!
//! This module has the Root layer (KerML 8.3.2), and `path()` with what it needs.

use std::sync::Arc;
use sysml_model::{Attr, ElementIx, Endpoint, Metaclass as M, Slot};

use crate::resolve::{
    collide, effective_names, global_namespace, import_memberships, imported_memberships,
    member_element, member_metaclass, memberships, namespace_memberships, resolve_end, settle_path,
    settle_qualified_name, structure, visibility, Entry, Global, Project, Visibility,
};

fn at(file: crate::SourceFile, e: ElementIx) -> Global {
    Global {
        file,
        index: e.index() as u32,
    }
}

fn metaclass(db: &dyn salsa::Database, x: Global) -> M {
    structure(db, x.file).graph().metaclass(x.ix())
}

/// The element an end of a relationship identifies: the element written there, or the member
/// element of the membership its name resolves to (KerML 8.2.3.5).
fn end(
    db: &dyn salsa::Database,
    project: Project,
    rel: Global,
    slot: Slot,
    i: usize,
) -> Option<Global> {
    let s = structure(db, rel.file);
    let ends = match slot {
        Slot::Source => s.graph().source(rel.ix()),
        _ => s.graph().target(rel.ix()),
    };
    match ends.get(i)? {
        Endpoint::Element(e) => Some(at(rel.file, *e)),
        Endpoint::Name(_) => resolve_end(db, project, rel, slot, i as u32)
            .as_ref()
            .and_then(|e| member_element(db, project, e)),
    }
}

fn ends(db: &dyn salsa::Database, project: Project, rel: Global, slot: Slot) -> Vec<Global> {
    let s = structure(db, rel.file);
    let n = match slot {
        Slot::Source => s.graph().source(rel.ix()).len(),
        _ => s.graph().target(rel.ix()).len(),
    };
    (0..n)
        .filter_map(|i| end(db, project, rel, slot, i))
        .collect()
}

/// The owned relationships of an element that are of a metaclass, in order.
fn owned_relationships_of(db: &dyn salsa::Database, x: Global, kind: M) -> Vec<Global> {
    let s = structure(db, x.file);
    let g = s.graph();
    g.owned_relationships(x.ix())
        .iter()
        .filter(|r| g.metaclass(**r).specializes(kind))
        .map(|r| at(x.file, *r))
        .collect()
}

// ---- Element (KerML 8.3.2.1.2) ----

/// `owner` (deriveElementOwner): `owningRelationship.owningRelatedElement`.
pub fn owner(db: &dyn salsa::Database, _project: Project, x: Global) -> Option<Global> {
    structure(db, x.file)
        .graph()
        .owner(x.ix())
        .map(|o| at(x.file, o))
}

/// `ownedElement` (deriveElementOwnedElement): `ownedRelationship.ownedRelatedElement`.
pub fn owned_element(db: &dyn salsa::Database, _project: Project, x: Global) -> Vec<Global> {
    structure(db, x.file)
        .graph()
        .owned_elements(x.ix())
        .map(|e| at(x.file, e))
        .collect()
}

/// `owningMembership` (KerML 8.3.2.1.2): the owning relationship, if it is an OwningMembership.
pub fn owning_membership(db: &dyn salsa::Database, _project: Project, x: Global) -> Option<Global> {
    let s = structure(db, x.file);
    let g = s.graph();
    g.owning_relationship(x.ix())
        .filter(|r| g.metaclass(*r).specializes(M::OwningMembership))
        .map(|r| at(x.file, r))
}

/// `owningNamespace` (deriveOwningNamespace): `owningMembership.membershipOwningNamespace`.
pub fn owning_namespace(db: &dyn salsa::Database, project: Project, x: Global) -> Option<Global> {
    let m = owning_membership(db, project, x)?;
    let s = structure(db, x.file);
    s.graph()
        .owning_related_element(m.ix())
        .map(|n| at(x.file, n))
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
        let first = owned_member(db, project, ns)
            .into_iter()
            .find(|m| name(db, project, *m).as_deref() == Some(&*n));
        if first != Some(x) {
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
    let s = structure(db, x.file);
    let g = s.graph();
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
        // 8.3.2.1.3: the position among the owning related element's owned relationships.
        if let Some(o) = g.owning_related_element(x.ix()) {
            let i = g.owned_relationships(o).iter().position(|r| *r == x.ix());
            let i = i.expect("an owned relationship is among its owner's") + 1;
            return format!("{}/{i}", path(db, project, at(x.file, o))).into();
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
    owned_relationships_of(db, x, M::Annotation)
        .into_iter()
        .filter(|a| annotation_annotated_element(db, project, *a) == Some(x))
        .collect()
}

/// `documentation` (deriveElementDocumentation): the owned elements that are Documentation.
pub fn documentation(db: &dyn salsa::Database, project: Project, x: Global) -> Vec<Global> {
    owned_element(db, project, x)
        .into_iter()
        .filter(|e| metaclass(db, *e).specializes(M::Documentation))
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
        .filter(|e| metaclass(db, *e).specializes(M::TextualRepresentation))
        .collect()
}

/// `libraryNamespace()`: for an Element, that of its owning relationship (KerML 8.3.2.1.2); for a
/// Relationship, that of its owning related element, else of its owning relationship
/// (8.3.2.1.3); a LibraryPackage is its own (8.3.4.13.2).
pub fn library_namespace(db: &dyn salsa::Database, _project: Project, x: Global) -> Option<Global> {
    let s = structure(db, x.file);
    let g = s.graph();
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

/// `relatedElement` (deriveRelationshipRelatedElement): `source->union(target)`.
pub fn related_element(db: &dyn salsa::Database, project: Project, x: Global) -> Vec<Global> {
    let mut out = ends(db, project, x, Slot::Source);
    out.extend(ends(db, project, x, Slot::Target));
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
    let s = structure(db, a.file);
    if !s.graph().target(a.ix()).is_empty() {
        return end(db, project, a, Slot::Target, 0);
    }
    owned_annotating_element(db, project, a)?;
    s.graph()
        .owning_related_element(a.ix())
        .map(|o| at(a.file, o))
}

/// `ownedAnnotatingElement` (deriveAnnotationOwnedAnnotatingElement): the first owned related
/// element that is an AnnotatingElement.
pub fn owned_annotating_element(
    db: &dyn salsa::Database,
    _project: Project,
    a: Global,
) -> Option<Global> {
    let s = structure(db, a.file);
    let g = s.graph();
    g.owned_related_elements(a.ix())
        .iter()
        .find(|e| g.metaclass(**e).specializes(M::AnnotatingElement))
        .map(|e| at(a.file, *e))
}

/// `owningAnnotatingElement` (KerML 8.3.2.3.3): the owning related element, if it is an
/// AnnotatingElement.
pub fn owning_annotating_element(
    db: &dyn salsa::Database,
    _project: Project,
    a: Global,
) -> Option<Global> {
    let s = structure(db, a.file);
    let g = s.graph();
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
    owned_relationships_of(db, x, M::Annotation)
        .into_iter()
        .filter(|a| annotation_annotated_element(db, project, *a) != Some(x))
        .collect()
}

/// `owningAnnotatingRelationship` (KerML 8.3.2.3.2): the owning relationship, if it is an
/// Annotation.
pub fn owning_annotating_relationship(
    db: &dyn salsa::Database,
    _project: Project,
    x: Global,
) -> Option<Global> {
    let s = structure(db, x.file);
    let g = s.graph();
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
pub fn owned_membership(db: &dyn salsa::Database, _project: Project, ns: Global) -> Vec<Global> {
    owned_relationships_of(db, ns, M::Membership)
}

/// `ownedMember` (deriveNamespaceOwnedMember): the owned member elements of the owned
/// OwningMemberships.
pub fn owned_member(db: &dyn salsa::Database, _project: Project, ns: Global) -> Vec<Global> {
    let s = structure(db, ns.file);
    let g = s.graph();
    g.owned_relationships(ns.ix())
        .iter()
        .filter(|r| g.metaclass(**r).specializes(M::OwningMembership))
        .filter_map(|r| g.owned_related_elements(*r).first())
        .map(|e| at(ns.file, *e))
        .collect()
}

/// `ownedImport` (deriveNamespaceOwnedImport): the owned relationships that are Imports.
pub fn owned_import(db: &dyn salsa::Database, _project: Project, ns: Global) -> Vec<Global> {
    owned_relationships_of(db, ns, M::Import)
}

/// `importedMembership` (deriveNamespaceImportedMembership): `importedMemberships(Set{})`.
pub fn imported_membership(db: &dyn salsa::Database, project: Project, ns: Global) -> Vec<Global> {
    namespace_imported_memberships(db, project, ns)
        .into_iter()
        .map(|e| e.membership)
        .collect()
}

/// `membership` (KerML 8.3.2.4.5): the owned, imported and, for a type, inherited memberships,
/// without those a redefinition removes (8.3.3.1.10).
pub fn membership(db: &dyn salsa::Database, project: Project, ns: Global) -> Vec<Global> {
    let mut out: Vec<Global> = Vec::new();
    for e in memberships(db, project, ns) {
        if !e.hidden && !out.contains(&e.membership) {
            out.push(e.membership);
        }
    }
    out
}

/// `member` (deriveNamespaceMembers): `membership.memberElement`.
pub fn member(db: &dyn salsa::Database, project: Project, ns: Global) -> Vec<Global> {
    memberships(db, project, ns)
        .iter()
        .filter(|e| !e.hidden)
        .filter_map(|e| member_element(db, project, e))
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
pub fn import_imported_memberships(
    db: &dyn salsa::Database,
    project: Project,
    import: Global,
) -> Vec<Global> {
    // The importing namespace is the import's owning related element (`importOwningNamespace`).
    let s = structure(db, import.file);
    let Some(ns) = s.graph().owning_related_element(import.ix()) else {
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
    let s = structure(db, ns.file);
    let g = s.graph();
    let mut out: Vec<Global> = g
        .owned_relationships(ns.ix())
        .iter()
        .filter(|r| g.metaclass(**r).specializes(M::Membership))
        .filter(|r| visibility_kind.is_none_or(|v| visibility(g, **r) == v))
        .map(|r| at(ns.file, *r))
        .collect();
    for e in namespace_imported_memberships(db, project, ns) {
        if visibility_kind.is_none_or(|v| e.visibility == v) && !out.contains(&e.membership) {
            out.push(e.membership);
        }
    }
    out
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
    let mut out: Vec<Global> = Vec::new();
    for e in namespace_memberships(db, project, ns, recursive, include_all) {
        if !e.hidden && !out.contains(&e.membership) {
            out.push(e.membership);
        }
    }
    out
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
    for import in owned_import(db, project, ns) {
        if import_imported_memberships(db, project, import).contains(&mem) {
            let s = structure(db, import.file);
            return visibility(s.graph(), import.ix());
        }
    }
    if membership(db, project, ns).contains(&mem) {
        let s = structure(db, mem.file);
        return visibility(s.graph(), mem.ix());
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
fn membership_entry(db: &dyn salsa::Database, project: Project, m: Global) -> Entry {
    let s = structure(db, m.file);
    let g = s.graph();
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
        visibility: visibility(g, m.ix()),
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
/// ImportedElement), `importedMembership.memberElement` for a MembershipImport
/// (deriveMembershipImportImportedElement). With filter conditions, the namespace the
/// FilterPackage's own import names (ADR 0020).
pub fn imported_element(
    db: &dyn salsa::Database,
    project: Project,
    import: Global,
) -> Option<Global> {
    let s = structure(db, import.file);
    let g = s.graph();
    let decl = match g.target(import.ix()).first() {
        Some(Endpoint::Element(p)) => g
            .owned_relationships(*p)
            .iter()
            .find(|r| g.metaclass(**r).specializes(M::Import))
            .map(|r| at(import.file, *r))?,
        _ => import,
    };
    resolve_end(db, project, decl, Slot::Target, 0)
        .as_ref()
        .and_then(|e| member_element(db, project, e))
}

// ---- string resolution (KerML 8.3.2.4.5) ----

/// The segments of a qualified name written as KerML text (KerML 8.2.3.4.1), unescaped, and
/// whether it starts with `$::`. `None` if it is not a qualified name.
fn segments(text: &str) -> Option<(bool, Vec<String>)> {
    let mut out = Vec::new();
    let mut chars = text.trim().chars().peekable();
    let mut global = false;
    loop {
        let mut s = String::new();
        match chars.peek() {
            Some('\'') => {
                chars.next();
                loop {
                    match chars.next()? {
                        '\'' => break,
                        '\\' => s.push(match chars.next()? {
                            'b' => '\u{8}',
                            'f' => '\u{c}',
                            't' => '\t',
                            'n' => '\n',
                            c => c,
                        }),
                        c => s.push(c),
                    }
                }
            }
            Some('$') if out.is_empty() && !global => {
                chars.next();
                global = true;
                (chars.next()? == ':' && chars.next()? == ':').then_some(())?;
                continue;
            }
            _ => {
                while let Some(c) = chars
                    .peek()
                    .filter(|c| c.is_ascii_alphanumeric() || **c == '_')
                {
                    s.push(*c);
                    chars.next();
                }
                if s.is_empty() || s.starts_with(|c: char| c.is_ascii_digit()) {
                    return None;
                }
            }
        }
        out.push(s);
        match chars.next() {
            None => return Some((global, out)),
            Some(':') if chars.next() == Some(':') => {}
            _ => return None,
        }
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

fn membership_element(db: &dyn salsa::Database, project: Project, m: Global) -> Option<Global> {
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
            if !metaclass(db, namespace).specializes(M::Namespace) {
                return None;
            }
            resolve_visible(db, project, namespace, &name)
        }
    }
}
