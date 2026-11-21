// SPDX-License-Identifier: Apache-2.0
//! Name resolution (KerML 8.2.3.5) as incremental queries (ADR 0019).
//!
//! The inputs are the source files and the project that groups them. Each file is parsed and
//! lowered into its own fragment (ADR 0017, ADR 0018); an element is a [`Global`]: its file and
//! its index in that file's fragment. The membership tables of each namespace (owned, imported,
//! inherited and visible, KerML 8.2.3.5.3) are queries, and so is the resolution of each name.
//! Imports and specializations may form cycles (KerML 8.2.3.5.1), so the queries that can meet
//! one iterate to a fixpoint from the empty set.

use std::sync::Arc;
use sysml_model::{Attr, ElementIx, Endpoint, FileId, Graph, Metaclass as M, Slot, Value};
use sysml_syntax::{Language, Parse};
use uuid::Uuid;

use crate::implied::{implied_generals, implied_redefinitions};
use crate::lower::{lower, Lowered};

/// A source file: an input of the query graph.
#[salsa::input(debug)]
pub struct SourceFile {
    /// The id its spans carry.
    pub file_id: FileId,
    /// Where it comes from, for messages.
    #[returns(ref)]
    pub path: String,
    /// The language it is in.
    pub language: Language,
    /// Its text.
    #[returns(ref)]
    pub text: String,
}

/// The files that resolve against one another: one global namespace (KerML 8.2.3.5.2). The three
/// KerML libraries are one project, whose files import each other in circles.
#[salsa::input(debug)]
pub struct Project {
    /// The files.
    #[returns(ref)]
    pub files: Vec<SourceFile>,
}

/// An element: its file and its index in that file's fragment (ADR 0019).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Global {
    /// The file.
    pub file: SourceFile,
    /// The element's index in the file's fragment.
    pub index: u32,
}

impl Global {
    /// The element's index in its file's fragment.
    pub fn ix(self) -> ElementIx {
        ix(self.index)
    }
}

fn ix(i: u32) -> ElementIx {
    ElementIx::from_index(i as usize)
}

/// The parse of a file (ADR 0011).
#[salsa::tracked(returns(ref))]
pub fn parsed(db: &dyn salsa::Database, file: SourceFile) -> Parse {
    sysml_syntax::parse(*file.language(db), file.text(db))
}

/// The lowered fragment of a file (ADR 0018). Its ids are the file-local indices as v5-free
/// placeholders: the `DataIdentity` ids are mapped outside the query graph (ADR 0019). A graph has
/// no equality, so this query does not backdate; the projections below do.
#[salsa::tracked(returns(ref), no_eq)]
pub fn lowered(db: &dyn salsa::Database, file: SourceFile) -> Arc<Lowered> {
    let mut n = 0u128;
    Arc::new(lower(
        *file.file_id(db),
        &parsed(db, file).syntax(),
        &mut || {
            n += 1;
            Uuid::from_u128(n)
        },
    ))
}

/// A file's lowered fragment, as resolution reads it: its graph and root, equal to the last ones
/// when only spans changed (`Graph::same_structure`), so an edit of layout stops here (early
/// cutoff, ADR 0019). It has no diagnostics, whose spans it does not compare: those are read
/// through [`lowered`].
#[derive(Clone)]
pub struct Structure(Arc<Lowered>);

impl Structure {
    /// The fragment's graph. Its spans may be those of an earlier edit of layout.
    pub fn graph(&self) -> &Graph {
        &self.0.graph
    }

    /// The root namespace.
    pub fn root(&self) -> ElementIx {
        self.0.root
    }
}

impl PartialEq for Structure {
    fn eq(&self, other: &Structure) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
            || (self.0.root == other.0.root && self.0.graph.same_structure(&other.0.graph))
    }
}

// The values compared are never NaN: a KerML literal cannot denote one (KerML 8.2.2.3).
impl Eq for Structure {}

/// The structure of a file's fragment, without its spans.
#[salsa::tracked(returns(ref))]
pub fn structure(db: &dyn salsa::Database, file: SourceFile) -> Structure {
    Structure(lowered(db, file).clone())
}

/// A membership's visibility (KerML 8.3.2.4.5 VisibilityKind).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Visibility {
    /// `public`
    Public,
    /// `protected`
    Protected,
    /// `private`
    Private,
}

/// A membership as a name lookup sees it (KerML 8.2.3.5.3): its names, the membership, its member
/// element when the membership owns it (an alias's is a name to resolve), and its visibility
/// where it is found (an imported membership has its import's).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Entry {
    /// `memberName`
    pub name: Option<Arc<str>>,
    /// `memberShortName`
    pub short_name: Option<Arc<str>>,
    /// The membership.
    pub membership: Global,
    /// The owned member element; `None` for an alias, whose element is resolved.
    pub owned_element: Option<Global>,
    /// The visibility where the membership is found.
    pub visibility: Visibility,
    /// Inherited, but its feature is redefined (KerML 8.3.3.1.10 `removeRedefinedFeatures`):
    /// consulted only when a name resolves nowhere else (ADR 0020).
    pub hidden: bool,
}

impl Entry {
    pub(crate) fn named(&self, name: &str) -> bool {
        self.name.as_deref() == Some(name) || self.short_name.as_deref() == Some(name)
    }
}

pub(crate) fn visibility(g: &Graph, x: ElementIx) -> Visibility {
    match g.values(x, Attr::Visibility).next() {
        Some(Value::Enumeration("private")) => Visibility::Private,
        Some(Value::Enumeration("protected")) => Visibility::Protected,
        Some(_) => Visibility::Public,
        // The metamodel default: public for a Membership, private for an Import (KerML 8.3.2).
        None => match g.stored_attribute(x, "visibility").and_then(|a| a.default) {
            Some("private") => Visibility::Private,
            Some("protected") => Visibility::Protected,
            _ => Visibility::Public,
        },
    }
}

/// The effective name and short name of an element (KerML 8.3.3.3.4 `effectiveName`,
/// `effectiveShortName`): its declared names or, for a feature with neither, those of its naming
/// feature, the redefined feature of its first owned redefinition (`namingFeature`). Naming
/// features may form a cycle: a fixpoint from no name.
#[salsa::tracked(returns(ref), cycle_initial = no_names, cycle_fn = settle_names)]
pub fn effective_names(
    db: &dyn salsa::Database,
    project: Project,
    file: SourceFile,
    element: u32,
) -> (Option<Arc<str>>, Option<Arc<str>>) {
    let l = structure(db, file);
    let g = &l.graph();
    let e = ix(element);
    let name = g.string(e, Attr::DeclaredName).map(Arc::from);
    let short = g.string(e, Attr::DeclaredShortName).map(Arc::from);
    if name.is_some() || short.is_some() || !g.metaclass(e).specializes(M::Feature) {
        return (name, short);
    }
    // The first owned redefinition is a written one if there is any, else an implied one
    // (KerML 8.4.2; ADR 0020).
    let naming = match g
        .owned_relationships(e)
        .iter()
        .find(|r| g.metaclass(**r) == M::Redefinition)
    {
        Some(r) => {
            let at = Global {
                file,
                index: r.index() as u32,
            };
            resolve_end(db, project, at, Slot::Target, 0)
                .as_ref()
                .and_then(|x| member_element(db, project, x))
        }
        None => implied_redefinitions(db, project, file, element)
            .first()
            .map(|i| i.general),
    };
    match naming {
        Some(naming)
            if naming
                != (Global {
                    file,
                    index: element,
                }) =>
        {
            effective_names(db, project, naming.file, naming.index).clone()
        }
        _ => (None, None),
    }
}

settle!(
    settle_names,
    (Option<Arc<str>>, Option<Arc<str>>),
    (Project, SourceFile, u32)
);
settle!(settle_entries, Vec<Entry>, (Project, SourceFile, u32));
settle!(
    pub(crate) settle_qualified_name,
    Option<Arc<str>>,
    (Project, Global)
);
settle!(pub(crate) settle_path, Arc<str>, (Project, Global));
settle!(
    pub(crate) settle_memberships,
    Vec<Global>,
    (Project, Global, bool)
);
settle!(
    pub(crate) settle_all_supertypes,
    Vec<Global>,
    (Project, Global)
);
settle!(settle_generals, Vec<Global>, (Project, Global));
// A name whose resolution does not converge is unresolved.
settle!(
    settle_resolution,
    Option<Entry>,
    (Project, Global, Slot, u32),
    |_: &Option<Entry>| None
);

fn no_names(
    _db: &dyn salsa::Database,
    _id: salsa::Id,
    _project: Project,
    _file: SourceFile,
    _element: u32,
) -> (Option<Arc<str>>, Option<Arc<str>>) {
    (None, None)
}

/// The owned memberships of a namespace (KerML 8.3.2.4.5 `ownedMembership`), named by their
/// member's effective names. A span-free projection that backdates (ADR 0019). An effective name
/// may need resolution, which may come back here: a fixpoint from the empty set.
#[salsa::tracked(returns(ref), cycle_initial = no_entries, cycle_fn = settle_entries)]
pub fn owned_memberships(
    db: &dyn salsa::Database,
    project: Project,
    file: SourceFile,
    ns: u32,
) -> Vec<Entry> {
    let l = structure(db, file);
    let g = &l.graph();
    let mut out = Vec::new();
    for r in g.owned_relationships(ix(ns)) {
        let m = g.metaclass(*r);
        if !m.specializes(M::Membership) {
            continue;
        }
        let at = |i: ElementIx| Global {
            file,
            index: i.index() as u32,
        };
        let (name, short_name, owned) = if m.specializes(M::OwningMembership) {
            // `memberName` of an OwningMembership is its owned member element's (effective) name.
            let Some(e) = g.owned_related_elements(*r).first().copied() else {
                continue;
            };
            let (name, short) = effective_names(db, project, file, e.index() as u32).clone();
            (name, short, Some(at(e)))
        } else {
            (
                g.string(*r, Attr::MemberName).map(Arc::from),
                g.string(*r, Attr::MemberShortName).map(Arc::from),
                None,
            )
        };
        if name.is_none() && short_name.is_none() {
            continue;
        }
        out.push(Entry {
            name,
            short_name,
            membership: at(*r),
            owned_element: owned,
            visibility: visibility(g, *r),
            hidden: false,
        });
    }
    out
}

/// An import of a namespace: what resolving it needs, without spans.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ImportDecl {
    import: u32,
    namespace: bool,
    recursive: bool,
    all: bool,
    visibility: Visibility,
    /// The FilterPackage's own import, when the import has filter conditions (KerML 8.2.3.4.2).
    filtered: Option<u32>,
}

#[salsa::tracked(returns(ref))]
fn imports(db: &dyn salsa::Database, file: SourceFile, ns: u32) -> Vec<ImportDecl> {
    let l = structure(db, file);
    let g = &l.graph();
    let mut out = Vec::new();
    for r in g.owned_relationships(ix(ns)) {
        let m = g.metaclass(*r);
        if !m.specializes(M::Import) {
            continue;
        }
        // A FilterPackage (KerML 8.2.3.4.2): the import's target is an owned package whose own
        // import says what is imported; its filter conditions are not evaluated yet. `all` is on
        // the outer import, where it governs the FilterPackage; the inner import, which the
        // grammar gives no `all`, imports the visible memberships of its namespace (ADR 0020).
        let filtered = match g.target(*r).first() {
            Some(Endpoint::Element(p)) => g
                .owned_relationships(*p)
                .iter()
                .find(|x| g.metaclass(**x).specializes(M::Import))
                .map(|x| x.index() as u32),
            _ => None,
        };
        let decl = filtered.map_or(*r, ix);
        out.push(ImportDecl {
            import: r.index() as u32,
            namespace: g.metaclass(decl) == M::NamespaceImport,
            recursive: g.flag(decl, Attr::IsRecursive),
            all: g.flag(decl, Attr::IsImportAll),
            visibility: visibility(g, *r),
            filtered,
        });
    }
    out
}

/// The memberships a namespace imports (KerML 8.3.2.4.5 `importedMembership`, 8.2.3.4.2), each
/// with its import's visibility. Imports may be circular: a fixpoint from the empty set.
#[salsa::tracked(returns(ref), cycle_initial = no_entries, cycle_fn = settle_entries)]
pub fn imported_memberships(
    db: &dyn salsa::Database,
    project: Project,
    file: SourceFile,
    ns: u32,
) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for decl in imports(db, file, ns) {
        let found = decl_memberships(db, project, file, decl);
        for mut e in found {
            e.visibility = decl.visibility;
            if !out
                .iter()
                .any(|o| o.membership == e.membership && o.visibility == e.visibility)
            {
                out.push(e);
            }
        }
    }
    // KerML 8.3.2.4.5 `importedMemberships`: memberships that collide with each other or with an
    // owned membership in distinguishability are excluded (ADR 0020).
    let owned = owned_memberships(db, project, file, ns);
    let kinds: Vec<Option<M>> = out
        .iter()
        .map(|e| member_metaclass(db, project, e))
        .collect();
    let owned_kinds: Vec<Option<M>> = owned
        .iter()
        .map(|e| member_metaclass(db, project, e))
        .collect();
    // Only entries that share a name can collide: index them by name.
    let mut by_name: std::collections::HashMap<&str, Vec<usize>> = Default::default();
    for (i, e) in out.iter().enumerate() {
        for n in [&e.name, &e.short_name].into_iter().flatten() {
            by_name.entry(n).or_default().push(i);
        }
    }
    let keep: Vec<bool> = (0..out.len())
        .map(|i| {
            let collides_owned = owned
                .iter()
                .zip(&owned_kinds)
                .filter(|(o, _)| {
                    [&o.name, &o.short_name]
                        .into_iter()
                        .flatten()
                        .any(|n| by_name.get(&**n).is_some_and(|v| v.contains(&i)))
                })
                .any(|(o, k)| collide(&out[i], kinds[i], o, *k));
            let collides_imported = [&out[i].name, &out[i].short_name]
                .into_iter()
                .flatten()
                .flat_map(|n| by_name.get(&**n).into_iter().flatten())
                .any(|&j| {
                    out[j].membership != out[i].membership
                        && collide(&out[i], kinds[i], &out[j], kinds[j])
                });
            !collides_owned && !collides_imported
        })
        .collect();
    out.into_iter()
        .zip(keep)
        .filter_map(|(e, k)| k.then_some(e))
        .collect()
}

pub(crate) fn member_metaclass(db: &dyn salsa::Database, project: Project, e: &Entry) -> Option<M> {
    member_element(db, project, e).map(|x| structure(db, x.file).graph().metaclass(x.ix()))
}

/// Two memberships are not distinguishable (KerML 8.3.2.4.3 `isDistinguishableFrom`) when a name
/// or short name of one is a name or short name of the other, and the metaclass of one member
/// element conforms to the other's. An element not resolved yet collides with nothing.
pub(crate) fn collide(a: &Entry, ka: Option<M>, b: &Entry, kb: Option<M>) -> bool {
    let (Some(ka), Some(kb)) = (ka, kb) else {
        return false;
    };
    let names = |e: &Entry| [e.name.clone(), e.short_name.clone()];
    let shared = names(a)
        .iter()
        .flatten()
        .any(|n| names(b).iter().flatten().any(|m| m == n));
    shared && (ka.specializes(kb) || kb.specializes(ka))
}

/// The memberships one import brings (KerML 8.3.2.4.2 `Import::importedMemberships`), before the
/// exclusion of collisions and with the visibilities they have where they are found.
fn decl_memberships(
    db: &dyn salsa::Database,
    project: Project,
    file: SourceFile,
    decl: &ImportDecl,
) -> Vec<Entry> {
    let at = Global {
        file,
        index: decl.filtered.unwrap_or(decl.import),
    };
    let Some(target) = resolve_end(db, project, at, Slot::Target, 0).clone() else {
        return Vec::new();
    };
    let mut found: Vec<Entry> = Vec::new();
    if decl.namespace {
        // NamespaceImport (KerML 8.3.2.4.6): the namespace's visible memberships, with
        // `isRecursive` and `isImportAll` passed on to `visibleMemberships`.
        let Some(target) = member_element(db, project, &target) else {
            return Vec::new();
        };
        found = namespace_memberships(db, project, target, decl.recursive, decl.all);
    } else {
        // MembershipImport (KerML 8.3.2.4.4): the membership itself (8.2.3.4.2, Note 1) and,
        // if recursive and its element is a namespace, that namespace's memberships.
        let element = member_element(db, project, &target);
        found.push(target);
        if let Some(e) = element.filter(|_| decl.recursive) {
            if structure(db, e.file)
                .graph()
                .metaclass(e.ix())
                .specializes(M::Namespace)
            {
                for x in namespace_memberships(db, project, e, true, decl.all) {
                    if !found.iter().any(|f| f.membership == x.membership) {
                        found.push(x);
                    }
                }
            }
        }
    }
    found
}

/// The memberships the import `import` of the namespace `ns` brings (KerML 8.3.2.4.2), or none
/// if `import` is not one of its imports.
pub(crate) fn import_memberships(
    db: &dyn salsa::Database,
    project: Project,
    ns: Global,
    import: Global,
) -> Vec<Entry> {
    imports(db, ns.file, ns.index)
        .iter()
        .find(|d| d.import == import.index)
        .map(|d| decl_memberships(db, project, ns.file, d))
        .unwrap_or_default()
}

/// `visibleMemberships(excluded, isRecursive, includeAll)` (KerML 8.3.2.4.5, and 8.3.3.1.10 for
/// a type): all the memberships with `all`, else the public ones; if recursive, also those of
/// each owned namespace, through a public owning membership unless `all`. The excluded set is
/// not needed: recursion follows ownership, which has no cycles, and the memberships of each
/// namespace come from queries that iterate to a fixpoint.
pub(crate) fn namespace_memberships(
    db: &dyn salsa::Database,
    project: Project,
    ns: Global,
    recursive: bool,
    all: bool,
) -> Vec<Entry> {
    // A recursive import inherits through the written specializations only: `visibleMemberships`
    // of a Type passes `isRecursive` as `excludeImplied` (KerML 8.3.3.1.10; ADR 0021).
    let mut out = if !recursive && !all {
        visible_memberships(db, project, ns.file, ns.index).clone()
    } else {
        memberships_with(db, project, ns, recursive)
            .into_iter()
            .filter(|e| all || e.visibility == Visibility::Public)
            .collect()
    };
    if recursive {
        let l = structure(db, ns.file);
        let g = &l.graph();
        for r in g.owned_relationships(ns.ix()) {
            if !g.metaclass(*r).specializes(M::OwningMembership)
                || !(all || visibility(g, *r) == Visibility::Public)
            {
                continue;
            }
            for e in g.owned_related_elements(*r) {
                if g.metaclass(*e).specializes(M::Namespace) {
                    let nested = Global {
                        file: ns.file,
                        index: e.index() as u32,
                    };
                    for x in namespace_memberships(db, project, nested, true, all) {
                        match out.iter_mut().find(|o| o.membership == x.membership) {
                            Some(o) => o.hidden &= x.hidden,
                            None => out.push(x),
                        }
                    }
                }
            }
        }
    }
    out
}

fn no_entries(
    _db: &dyn salsa::Database,
    _id: salsa::Id,
    _project: Project,
    _file: SourceFile,
    _ns: u32,
) -> Vec<Entry> {
    Vec::new()
}

/// The general types a type declares: those of its owned specializations (KerML 8.3.3.1.8, including
/// FeatureTyping, Subsetting and Redefinition) and its conjugation (8.3.3.1.2); for a feature
/// with chainings, its last chaining feature (8.3.3.3.5).
#[salsa::tracked(returns(ref), cycle_initial = no_generals, cycle_fn = settle_generals)]
pub(crate) fn owned_generals(
    db: &dyn salsa::Database,
    project: Project,
    ty: Global,
) -> Vec<Global> {
    let l = structure(db, ty.file);
    let g = &l.graph();
    let mut out = Vec::new();
    let mut last_chaining = None;
    for r in g.owned_relationships(ty.ix()) {
        let m = g.metaclass(*r);
        let at = Global {
            file: ty.file,
            index: r.index() as u32,
        };
        if m.specializes(M::Specialization) || m == M::Conjugation {
            if let Some(e) = resolve_end(db, project, at, Slot::Target, 0)
                .as_ref()
                .and_then(|e| member_element(db, project, e))
            {
                out.push(e);
            }
        } else if m == M::FeatureChaining {
            last_chaining = Some(at);
        }
    }
    if let Some(at) = last_chaining {
        if let Some(e) = resolve_end(db, project, at, Slot::Target, 0)
            .as_ref()
            .and_then(|e| member_element(db, project, e))
        {
            out.push(e);
        }
    }
    out
}

/// The general types of a type: its owned ones, then its implied ones (KerML 8.4.2; ADR 0020).
#[salsa::tracked(returns(ref), cycle_initial = no_generals, cycle_fn = settle_generals)]
pub(crate) fn generals(db: &dyn salsa::Database, project: Project, ty: Global) -> Vec<Global> {
    let mut out = owned_generals(db, project, ty).clone();
    for i in implied_generals(db, project, ty.file, ty.index) {
        if !out.contains(&i.general) {
            out.push(i.general);
        }
    }
    out
}

fn no_generals(
    _db: &dyn salsa::Database,
    _id: salsa::Id,
    _project: Project,
    _ty: Global,
) -> Vec<Global> {
    Vec::new()
}

/// The memberships a type inherits (KerML 8.3.3.1.10 `inheritedMemberships`): the non-private
/// memberships of its general types, with `removeRedefinedFeatures` applied. A membership that
/// arrives through several generals is hidden only if it is hidden on every path. Then it is
/// hidden if its feature is redefined by another inherited membership's feature, directly or
/// indirectly, or if it or a feature it redefines is directly redefined by an owned feature of
/// this type. Hidden memberships are kept, marked, for decision 3 of ADR 0020. Specializations
/// may be circular: a fixpoint from the empty set.
///
/// With `exclude_implied` (the operation's `excludeImplied`), only the written specializations
/// are followed, and the generals' memberships are computed the same way; a recursive import uses
/// it (KerML 8.3.3.1.10 `visibleMemberships`). Hiding by redefinition sees every redefinition, as
/// `removeRedefinedFeatures` does.
#[salsa::tracked(returns(ref), cycle_initial = no_inherited, cycle_fn = settle_inherited)]
pub fn inherited_memberships(
    db: &dyn salsa::Database,
    project: Project,
    file: SourceFile,
    ty: u32,
    exclude_implied: bool,
) -> Vec<Entry> {
    let here = Global { file, index: ty };
    let l = structure(db, file);
    let g = l.graph();
    if !g.metaclass(here.ix()).specializes(M::Type) {
        return Vec::new();
    }
    // The features this type's owned features directly redefine, as written or by implication
    // (KerML 8.4.2; ADR 0020).
    let mut redefined: Vec<Global> = Vec::new();
    for f in g.owned_elements(here.ix()) {
        redefined.extend(direct_redefinitions(
            db,
            project,
            Global {
                file,
                index: f.index() as u32,
            },
        ));
    }
    let generals = if exclude_implied {
        owned_generals(db, project, here)
    } else {
        generals(db, project, here)
    };
    let mut out: Vec<Entry> = Vec::new();
    for general in generals.iter().copied() {
        if general == here {
            continue;
        }
        for e in memberships_with(db, project, general, exclude_implied) {
            if e.visibility == Visibility::Private {
                continue;
            }
            match out.iter_mut().find(|o| o.membership == e.membership) {
                Some(o) => o.hidden &= e.hidden,
                None => out.push(e),
            }
        }
    }
    // Condition 1: what the inherited features redefine, from each general's memoized set.
    let redefined_by_members = |x: Global| {
        generals.iter().any(|general| {
            *general != here && redefined_in(db, project, *general, exclude_implied).contains(&x)
        })
    };
    for e in out.iter_mut() {
        let Some(x) = member_element(db, project, e) else {
            continue;
        };
        // Condition 2 needs what `x` redefines only where owned features redefine something.
        if redefined_by_members(x)
            || redefined.contains(&x)
            || (!redefined.is_empty()
                && redefined_features(db, project, x)
                    .iter()
                    .any(|c| redefined.contains(c)))
        {
            e.hidden = true;
        }
    }
    out
}

/// What the features a type passes on to its subtypes redefine, directly or indirectly: the
/// features its non-private owned features redefine, and those in its generals' sets (KerML
/// 8.3.3.1.10, condition 1 of `removeRedefinedFeatures` in a subtype). Memoized per type, so each
/// feature's redefinitions are followed once. A fixpoint from the empty set.
#[salsa::tracked(returns(ref), cycle_initial = no_redefined, cycle_fn = settle_redefined)]
pub fn redefined_in(
    db: &dyn salsa::Database,
    project: Project,
    ty: Global,
    exclude_implied: bool,
) -> std::collections::HashSet<Global> {
    let l = structure(db, ty.file);
    let g = l.graph();
    let mut out = std::collections::HashSet::new();
    if !g.metaclass(ty.ix()).specializes(M::Type) {
        return out;
    }
    for r in g.owned_relationships(ty.ix()) {
        if !g.metaclass(*r).specializes(M::FeatureMembership)
            || visibility(g, *r) == Visibility::Private
        {
            continue;
        }
        for f in g.owned_related_elements(*r) {
            let f = Global {
                file: ty.file,
                index: f.index() as u32,
            };
            out.extend(redefined_features(db, project, f).iter().copied());
        }
    }
    let generals = if exclude_implied {
        owned_generals(db, project, ty)
    } else {
        generals(db, project, ty)
    };
    for general in generals {
        if *general != ty {
            out.extend(
                redefined_in(db, project, *general, exclude_implied)
                    .iter()
                    .copied(),
            );
        }
    }
    out
}

fn no_redefined(
    _db: &dyn salsa::Database,
    _id: salsa::Id,
    _project: Project,
    _ty: Global,
    _exclude_implied: bool,
) -> std::collections::HashSet<Global> {
    std::collections::HashSet::new()
}

fn no_inherited(
    _db: &dyn salsa::Database,
    _id: salsa::Id,
    _project: Project,
    _file: SourceFile,
    _ty: u32,
    _exclude_implied: bool,
) -> Vec<Entry> {
    Vec::new()
}

settle!(
    settle_inherited,
    Vec<Entry>,
    (Project, SourceFile, u32, bool)
);

settle!(
    settle_redefined,
    std::collections::HashSet<Global>,
    (Project, Global, bool)
);

/// The features a feature redefines directly: its owned Redefinitions' targets and its implied
/// redefinitions (KerML 8.4.2).
fn direct_redefinitions(db: &dyn salsa::Database, project: Project, f: Global) -> Vec<Global> {
    let l = structure(db, f.file);
    let g = l.graph();
    if !g.metaclass(f.ix()).specializes(M::Feature) {
        return Vec::new();
    }
    let mut out: Vec<Global> = g
        .owned_relationships(f.ix())
        .iter()
        .filter(|r| g.metaclass(**r) == M::Redefinition)
        .filter_map(|r| {
            let at = Global {
                file: f.file,
                index: r.index() as u32,
            };
            resolve_end(db, project, at, Slot::Target, 0)
                .as_ref()
                .and_then(|e| member_element(db, project, e))
        })
        .collect();
    for i in implied_redefinitions(db, project, f.file, f.index) {
        if !out.contains(&i.general) {
            out.push(i.general);
        }
    }
    out
}

/// The features a feature redefines, directly or indirectly (KerML 8.3.3.1.10
/// `allRedefinedFeaturesOf`, without the feature itself). A fixpoint from none.
#[salsa::tracked(returns(ref), cycle_initial = no_generals, cycle_fn = settle_generals)]
pub fn redefined_features(db: &dyn salsa::Database, project: Project, f: Global) -> Vec<Global> {
    let mut out: Vec<Global> = Vec::new();
    for r in direct_redefinitions(db, project, f) {
        if r == f {
            continue;
        }
        if !out.contains(&r) {
            out.push(r);
        }
        for x in redefined_features(db, project, r) {
            if *x != f && !out.contains(x) {
                out.push(*x);
            }
        }
    }
    out
}

/// Every membership of a namespace: owned, imported and, for a type, inherited (KerML 8.2.3.5.3,
/// local resolution).
pub fn memberships(db: &dyn salsa::Database, project: Project, ns: Global) -> Vec<Entry> {
    memberships_with(db, project, ns, false)
}

/// The memberships of a namespace, inheriting through the written specializations only when
/// `exclude_implied` (KerML 8.3.3.1.10 `excludeImplied`).
pub(crate) fn memberships_with(
    db: &dyn salsa::Database,
    project: Project,
    ns: Global,
    exclude_implied: bool,
) -> Vec<Entry> {
    let mut out = owned_memberships(db, project, ns.file, ns.index).clone();
    out.extend(
        imported_memberships(db, project, ns.file, ns.index)
            .iter()
            .cloned(),
    );
    out.extend(
        inherited_memberships(db, project, ns.file, ns.index, exclude_implied)
            .iter()
            .cloned(),
    );
    out
}

/// The visible memberships of a namespace (KerML 8.2.3.5.3): the public owned ones, those of its
/// public imports, and the public inherited ones.
#[salsa::tracked(returns(ref), cycle_initial = no_entries, cycle_fn = settle_entries)]
pub fn visible_memberships(
    db: &dyn salsa::Database,
    project: Project,
    file: SourceFile,
    ns: u32,
) -> Vec<Entry> {
    memberships(db, project, Global { file, index: ns })
        .into_iter()
        .filter(|e| e.visibility == Visibility::Public)
        .collect()
}

/// The global namespace (KerML 8.2.3.5.2): the visible memberships of every root namespace.
#[salsa::tracked(returns(ref))]
pub fn global_namespace(db: &dyn salsa::Database, project: Project) -> Vec<Entry> {
    let mut out = Vec::new();
    for f in project.files(db) {
        let root = structure(db, *f).root().index() as u32;
        out.extend(visible_memberships(db, project, *f, root).iter().cloned());
    }
    out
}

/// The member element of a membership found by a lookup: the element it owns, or for an alias
/// the element its name resolves to, through any further aliases. Aliases that name each other
/// have no member element.
pub fn member_element(db: &dyn salsa::Database, project: Project, e: &Entry) -> Option<Global> {
    let mut entry = e.clone();
    let mut seen: Vec<Global> = Vec::new();
    loop {
        if let Some(owned) = entry.owned_element {
            return Some(owned);
        }
        if seen.contains(&entry.membership) {
            return None;
        }
        seen.push(entry.membership);
        entry = resolve_end(db, project, entry.membership, Slot::Target, 0).clone()?;
    }
}

// ---- local namespaces (KerML 8.2.3.5.2) ----

/// Where a name is resolved from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Context {
    /// Full resolution from a namespace.
    Namespace(Global),
    /// The redefined feature of a Redefinition whose owning feature has an owning type: each
    /// general of that type in turn (KerML 8.2.3.5.1).
    Generals(Global),
    /// A chaining after the first: the previous chaining feature (KerML 8.2.3.5.2).
    PreviousChaining(Global),
    /// The member of a FeatureChainExpression: the result of its argument (KerML 8.2.3.5.2).
    ChainArgument(Global),
}

fn is_instantiation(m: M) -> bool {
    m.specializes(M::InstantiationExpression)
}

/// The owning namespace of an element (KerML 8.3.2.1.3): the namespace of its owning membership.
/// An element owned through another relationship (a feature chain owned by a specialization)
/// climbs to that relationship's owner.
fn owning_namespace(g: &Graph, e: ElementIx) -> Option<ElementIx> {
    let r = g.owning_relationship(e)?;
    let owner = g.owning_related_element(r)?;
    if g.metaclass(r).specializes(M::Membership) {
        Some(owner)
    } else {
        Some(owner)
            .filter(|o| g.metaclass(*o).specializes(M::Namespace))
            .or_else(|| owning_namespace(g, owner))
    }
}

/// The non-invocation namespace (KerML 8.2.3.5.2): the nearest containing namespace that is not a
/// FeatureReferenceExpression, an InstantiationExpression, an owned feature of an
/// InstantiationExpression, or an owned feature of the result of a ConstructorExpression.
fn non_invocation(g: &Graph, mut x: ElementIx) -> Option<ElementIx> {
    loop {
        let m = g.metaclass(x);
        let parent = owning_namespace(g, x);
        let skip = m == M::FeatureReferenceExpression
            || is_instantiation(m)
            || parent
                .is_some_and(|p| is_instantiation(g.metaclass(p)) && m.specializes(M::Feature))
            || parent.is_some_and(|p| {
                owning_namespace(g, p).is_some_and(|c| g.metaclass(c) == M::ConstructorExpression)
                    && g.owning_relationship(p)
                        .is_some_and(|r| g.metaclass(r) == M::ReturnParameterMembership)
            });
        if !skip {
            return Some(x);
        }
        x = parent?;
    }
}

/// The local namespace of an end of a relationship (KerML 8.2.3.5.2).
fn context(g: &Graph, file: SourceFile, rel: ElementIx) -> Option<Context> {
    let at = |i: ElementIx| Global {
        file,
        index: i.index() as u32,
    };
    let m = g.metaclass(rel);
    let owner = g.owning_related_element(rel);
    if m.specializes(M::Import) {
        // The importOwningNamespace.
        return Some(Context::Namespace(at(owner?)));
    }
    if m == M::FeatureChaining {
        let chained = owner?;
        let chainings: Vec<ElementIx> = g
            .owned_relationships(chained)
            .iter()
            .copied()
            .filter(|r| g.metaclass(*r) == M::FeatureChaining)
            .collect();
        let position = chainings.iter().position(|r| *r == rel)?;
        if position > 0 {
            return Some(Context::PreviousChaining(at(chainings[position - 1])));
        }
        // The first: as if the owning relationship of the chained feature were the context.
        return match g.owning_relationship(chained) {
            Some(r) => context(g, file, r),
            None => Some(Context::Namespace(at(chained))),
        };
    }
    if m.specializes(M::Membership) {
        let ns = owner?;
        let om = g.metaclass(ns);
        if om == M::FeatureChainExpression && !m.specializes(M::FeatureMembership) {
            return Some(Context::ChainArgument(at(ns)));
        }
        if om == M::FeatureReferenceExpression
            || (is_instantiation(om) && !m.specializes(M::FeatureMembership))
        {
            return non_invocation(g, ns).map(|n| Context::Namespace(at(n)));
        }
        return Some(Context::Namespace(at(ns)));
    }
    if m.specializes(M::Specialization) || m == M::Conjugation {
        let owning_type = owner.filter(|o| g.metaclass(*o).specializes(M::Type));
        if let Some(t) = owning_type {
            // A Redefinition of a feature with an owning type: the generals of that type.
            if m == M::Redefinition {
                if let Some(ot) =
                    owning_namespace(g, t).filter(|o| g.metaclass(*o).specializes(M::Type))
                {
                    if g.owning_relationship(t)
                        .is_some_and(|r| g.metaclass(r).specializes(M::FeatureMembership))
                    {
                        return Some(Context::Generals(at(ot)));
                    }
                }
            }
            // A ReferenceSubsetting of a connector end: the connector's namespace.
            if m == M::ReferenceSubsetting && g.flag(t, Attr::IsEnd) {
                if let Some(c) =
                    owning_namespace(g, t).filter(|c| g.metaclass(*c).specializes(M::Connector))
                {
                    return owning_namespace(g, c).map(|n| Context::Namespace(at(n)));
                }
            }
            return owning_namespace(g, t).map(|n| Context::Namespace(at(n)));
        }
        return owning_namespace(g, rel).map(|n| Context::Namespace(at(n)));
    }
    // Other relationships (Annotation, Dependency, TypeFeaturing, Disjoining …): their owner's
    // namespace, or the relationship's own.
    let base = owner.unwrap_or(rel);
    if g.metaclass(base).specializes(M::Namespace) {
        return owning_namespace(g, base)
            .or(Some(base))
            .map(|n| Context::Namespace(at(n)));
    }
    owning_namespace(g, base).map(|n| Context::Namespace(at(n)))
}

// ---- resolution (KerML 8.2.3.5.1, 8.2.3.5.4) ----

/// The resolution of one end of a relationship: the Membership its qualified name identifies, if
/// the end is a name. A name may be part of a cycle (an alias of an alias): a fixpoint from none.
#[salsa::tracked(returns(ref), cycle_initial = no_resolution, cycle_fn = settle_resolution)]
pub fn resolve_end(
    db: &dyn salsa::Database,
    project: Project,
    rel: Global,
    slot: Slot,
    position: u32,
) -> Option<Entry> {
    let l = structure(db, rel.file);
    let g = &l.graph();
    let ends = match slot {
        Slot::Source => g.source(rel.ix()),
        _ => g.target(rel.ix()),
    };
    let name = match ends.get(position as usize)? {
        // An owned feature chain: its own element.
        Endpoint::Element(e) => {
            return Some(Entry {
                name: None,
                short_name: None,
                membership: rel,
                owned_element: Some(Global {
                    file: rel.file,
                    index: e.index() as u32,
                }),
                visibility: Visibility::Public,
                hidden: false,
            })
        }
        Endpoint::Name(n) => g.name(*n).clone(),
    };
    let ctx = context(g, rel.file, rel.ix())?;
    let segments: Vec<&str> = name.segments.iter().map(|s| &**s).collect();
    let entry = resolve_qualified(db, project, ctx, name.global, &segments)?;
    // KerML 8.2.3.5.1: "if the resulting Element does not have the proper type for its context,
    // then the qualified name has no resolution". The proper type is the metaclass of the
    // stored attribute the end is a view of: a Membership for a MembershipImport, whose
    // resolution is the membership itself (8.2.3.4.2 Note 1), else the member element's.
    let expected = g
        .metaclass(rel.ix())
        .stored_attributes()
        .iter()
        .find(|a| a.slot == slot)
        .and_then(|a| match a.kind {
            sysml_model::PropertyKind::Reference { target } => {
                M::ALL.iter().copied().find(|m| m.name() == target)
            }
            _ => None,
        });
    let Some(expected) = expected else {
        return Some(entry);
    };
    let actual = if expected.specializes(M::Membership) {
        Some(
            structure(db, entry.membership.file)
                .graph()
                .metaclass(entry.membership.ix()),
        )
    } else {
        member_element(db, project, &entry).map(|x| structure(db, x.file).graph().metaclass(x.ix()))
    };
    // An alias whose own target is not resolved yet is not rejected here: that name is reported.
    match actual {
        Some(m) if !m.specializes(expected) => None,
        _ => Some(entry),
    }
}

fn no_resolution(
    _db: &dyn salsa::Database,
    _id: salsa::Id,
    _project: Project,
    _rel: Global,
    _slot: Slot,
    _position: u32,
) -> Option<Entry> {
    None
}

/// KerML 8.2.3.5.1: one segment is fully resolved from the local namespace; `$::` and one segment
/// is resolved in the global namespace; otherwise the qualification is resolved, must be a
/// namespace, and the last segment is resolved visibly in it.
fn resolve_qualified(
    db: &dyn salsa::Database,
    project: Project,
    ctx: Context,
    global: bool,
    segments: &[&str],
) -> Option<Entry> {
    // KerML 8.2.3.5.1: a qualified redefined feature is resolved whole with each general of the
    // owning type in turn as the local namespace, then from the owning type's namespace (ADR 0020
    // decision 2).
    if let (Context::Generals(ty), false, true) = (ctx, global, segments.len() > 1) {
        let generals = generals(db, project, ty);
        let outer = owning_namespace(structure(db, ty.file).graph(), ty.ix()).map(|o| Global {
            file: ty.file,
            index: o.index() as u32,
        });
        return generals
            .iter()
            .copied()
            .filter(|general| *general != ty)
            .chain(outer)
            .find_map(|ns| {
                resolve_qualified(db, project, Context::Namespace(ns), false, segments)
            });
    }
    match segments {
        [] => None,
        [one] if global => lookup(global_namespace(db, project), one, false),
        // A hidden membership only where the name resolves nowhere else (ADR 0020).
        [one] => resolve_simple(db, project, ctx, one, false)
            .or_else(|| resolve_simple(db, project, ctx, one, true)),
        [qualification @ .., last] => {
            let q = resolve_qualified(db, project, ctx, global, qualification)?;
            let ns = member_element(db, project, &q)?;
            let l = structure(db, ns.file);
            if !l.graph().metaclass(ns.ix()).specializes(M::Namespace) {
                return None;
            }
            let visible = visible_memberships(db, project, ns.file, ns.index);
            lookup(visible, last, false).or_else(|| lookup(visible, last, true))
        }
    }
}

/// The first membership with the name; with `hidden`, those hidden by a redefinition too.
fn lookup(entries: &[Entry], name: &str, hidden: bool) -> Option<Entry> {
    entries
        .iter()
        .find(|e| e.named(name) && (hidden || !e.hidden))
        .cloned()
}

fn resolve_simple(
    db: &dyn salsa::Database,
    project: Project,
    ctx: Context,
    name: &str,
    hidden: bool,
) -> Option<Entry> {
    match ctx {
        Context::Namespace(ns) => full_resolution(db, project, ns, name, hidden),
        // KerML 8.2.3.5.1: the generals of the owning type in turn. Each general's own memberships
        // are looked up first, across all of them, so that the general declaring the name wins
        // over a namespace that contains an earlier general; then full resolution from each.
        // Where none of them has the name, the normative libraries still expect a resolution
        // (for example FeatureReferencingPerformances: `feature redefines monitoredFeature`), so
        // resolution continues from the namespace of the owning type (ADR 0020). The owning type
        // itself is skipped: its member named by this very redefinition would otherwise resolve to
        // itself.
        Context::Generals(ty) => {
            let generals: Vec<Global> = generals(db, project, ty)
                .iter()
                .copied()
                .filter(|general| *general != ty)
                .collect();
            generals
                .iter()
                .find_map(|general| lookup(&memberships(db, project, *general), name, hidden))
                .or_else(|| {
                    generals
                        .iter()
                        .find_map(|general| full_resolution(db, project, *general, name, hidden))
                })
                .or_else(|| {
                    let l = structure(db, ty.file);
                    let outer = owning_namespace(l.graph(), ty.ix())?;
                    full_resolution(
                        db,
                        project,
                        Global {
                            file: ty.file,
                            index: outer.index() as u32,
                        },
                        name,
                        hidden,
                    )
                })
        }
        Context::PreviousChaining(previous) => {
            let feature = resolve_end(db, project, previous, Slot::Target, 0)
                .as_ref()
                .and_then(|e| member_element(db, project, e))?;
            lookup(&memberships(db, project, feature), name, hidden)
        }
        Context::ChainArgument(expression) => {
            let l = structure(db, expression.file);
            let fallback = non_invocation(l.graph(), expression.ix()).map(|n| Global {
                file: expression.file,
                index: n.index() as u32,
            });
            // KerML 8.2.3.5.2: the features of the argument's result. Only where that result
            // cannot be determined yet (an argument whose own result is an implied element, step
            // 1a.9) is the name resolved from the expression's namespace (ADR 0020).
            // A reference or chain whose own name is unresolved has no result to fall back from.
            let determinable = chain_argument(db, expression).is_some_and(|a| {
                let m = structure(db, a.file).graph().metaclass(a.ix());
                m == M::FeatureReferenceExpression || m == M::FeatureChainExpression
            });
            match result_scope(db, project, expression) {
                Some(scope) => lookup(&memberships(db, project, scope), name, hidden),
                None if determinable => None,
                None => fallback.and_then(|n| full_resolution(db, project, n, name, hidden)),
            }
        }
    }
}

/// The element whose features the result of a FeatureChainExpression's argument has: the
/// feature a reference names, the target of a chain, or the result parameter of an invoked
/// function (KerML 8.4.4.9).
fn result_scope(db: &dyn salsa::Database, project: Project, chain: Global) -> Option<Global> {
    expression_target(db, project, chain_argument(db, chain)?)
}

/// The argument of a FeatureChainExpression: its first parameter's FeatureValue's expression.
fn chain_argument(db: &dyn salsa::Database, chain: Global) -> Option<Global> {
    let l = structure(db, chain.file);
    let g = &l.graph();
    let at = |i: ElementIx| Global {
        file: chain.file,
        index: i.index() as u32,
    };
    // The argument: the first parameter's FeatureValue's expression.
    let param = g
        .owned_relationships(chain.ix())
        .iter()
        .find(|r| g.metaclass(**r) == M::ParameterMembership)
        .and_then(|r| g.owned_related_elements(*r).first().copied())?;
    let value = g
        .owned_relationships(param)
        .iter()
        .find(|r| g.metaclass(**r) == M::FeatureValue)?;
    let argument = *g.owned_related_elements(*value).first()?;
    Some(at(argument))
}

fn expression_target(db: &dyn salsa::Database, project: Project, x: Global) -> Option<Global> {
    let l = structure(db, x.file);
    let g = &l.graph();
    let at = |i: ElementIx| Global {
        file: x.file,
        index: i.index() as u32,
    };
    let membership = g
        .owned_relationships(x.ix())
        .iter()
        .copied()
        .find(|r| g.metaclass(*r) == M::Membership);
    match g.metaclass(x.ix()) {
        M::FeatureReferenceExpression | M::FeatureChainExpression => {
            let r = membership.map(at);
            if let Some(r) = r {
                return resolve_end(db, project, r, Slot::Target, 0)
                    .as_ref()
                    .and_then(|e| member_element(db, project, e));
            }
            // A chain target owned by an OwningMembership.
            g.owned_relationships(x.ix())
                .iter()
                .find(|r| g.metaclass(**r) == M::OwningMembership)
                .and_then(|r| g.owned_related_elements(*r).first())
                .map(|e| at(*e))
        }
        m if m.specializes(M::InvocationExpression) => {
            // Its own result parameter, as a cast has (KerML 8.2.5.8.1 TypeResultMember), else
            // the invoked function's.
            if let Some(r) = g
                .owned_relationships(x.ix())
                .iter()
                .find(|r| g.metaclass(**r).specializes(M::ReturnParameterMembership))
                .and_then(|r| g.owned_related_elements(*r).first())
            {
                return Some(at(*r));
            }
            let function = resolve_end(db, project, at(membership?), Slot::Target, 0)
                .as_ref()
                .and_then(|e| member_element(db, project, e))?;
            let fl = structure(db, function.file);
            let fg = &fl.graph();
            fg.owned_relationships(function.ix())
                .iter()
                .find(|r| fg.metaclass(**r) == M::ReturnParameterMembership)
                .and_then(|r| fg.owned_related_elements(*r).first())
                .map(|e| Global {
                    file: function.file,
                    index: e.index() as u32,
                })
        }
        _ => None,
    }
}

/// Full resolution (KerML 8.2.3.5.4): the local resolution in the namespace, then in each
/// containing namespace, then in the global namespace.
fn full_resolution(
    db: &dyn salsa::Database,
    project: Project,
    mut ns: Global,
    name: &str,
    hidden: bool,
) -> Option<Entry> {
    loop {
        if let Some(e) = lookup(&memberships(db, project, ns), name, hidden) {
            return Some(e);
        }
        let l = structure(db, ns.file);
        if ns.ix() == l.root() {
            return lookup(global_namespace(db, project), name, false);
        }
        match owning_namespace(l.graph(), ns.ix()) {
            Some(parent) => {
                ns = Global {
                    file: ns.file,
                    index: parent.index() as u32,
                }
            }
            None => return lookup(global_namespace(db, project), name, false),
        }
    }
}

/// The resolution of every name of a file, in the order of the fragment's names: the element each
/// identifies (the membership itself for a MembershipImport, KerML 8.2.3.4.2 Note 1), or `None`.
#[salsa::tracked(returns(ref))]
pub fn resolved_names(db: &dyn salsa::Database, project: Project, file: SourceFile) -> Vec<Name> {
    let l = structure(db, file);
    let g = &l.graph();
    let mut out = Vec::new();
    for x in g.elements() {
        for (slot, ends) in [(Slot::Source, g.source(x)), (Slot::Target, g.target(x))] {
            for (position, end) in ends.iter().enumerate() {
                if !matches!(end, Endpoint::Name(_)) {
                    continue;
                }
                let rel = Global {
                    file,
                    index: x.index() as u32,
                };
                let entry = resolve_end(db, project, rel, slot, position as u32).clone();
                let target = entry.as_ref().and_then(|e| {
                    if g.metaclass(x) == M::MembershipImport {
                        Some(e.membership)
                    } else {
                        member_element(db, project, e)
                    }
                });
                out.push(Name {
                    relationship: x.index() as u32,
                    slot,
                    position: position as u32,
                    target,
                });
            }
        }
    }
    out
}

/// One name of a file and what it resolves to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    /// The relationship whose end the name is.
    pub relationship: u32,
    /// The end.
    pub slot: Slot,
    /// The position in the end.
    pub position: u32,
    /// The element it identifies, if it resolves.
    pub target: Option<Global>,
}

/// The query database (ADR 0019), optionally recording every query execution.
#[salsa::db]
#[derive(Clone)]
pub struct Database {
    storage: salsa::Storage<Self>,
}

#[salsa::db]
impl salsa::Database for Database {}

impl Default for Database {
    fn default() -> Self {
        Database {
            storage: salsa::Storage::new(None),
        }
    }
}

impl Database {
    /// A database that calls `log` with the name of every query it executes (`salsa`'s
    /// `WillExecute` event): what a test reads to check incrementality.
    pub fn with_log(log: impl Fn(String) + Send + Sync + 'static) -> Database {
        Database {
            storage: salsa::Storage::new(Some(Box::new(move |e: salsa::Event| {
                if let salsa::EventKind::WillExecute { database_key } = e.kind {
                    log(format!("{database_key:?}"));
                }
            }))),
        }
    }
}
