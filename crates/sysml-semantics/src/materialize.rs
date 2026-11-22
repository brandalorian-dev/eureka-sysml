// SPDX-License-Identifier: Apache-2.0
//! The implied relationships of each file as an overlay on its fragment, and the view of the two
//! (KerML 8.4.2; step 1a.9, ADR 0022).
//!
//! A file's [`overlay`] holds its implied relationships, and the implied elements they own, as a
//! graph of their own. Each has an index after the lowered fragment's, so that it is a [`Global`]
//! of the file like any other element. Its owner is an element of the fragment, and its ends,
//! often in another file, are `Global`s. A [`View`] reads the fragment and the overlay as one: an
//! element's `ownedRelationship` is its owned relationships, then its implied ones, since all of
//! them are included (`isImpliedIncluded`, KerML 8.3.2.1.2).
//!
//! Resolution never reads the view: the overlay is built from resolution's queries (ADR 0020),
//! never the reverse.

use std::collections::HashMap;
use sysml_model::{Attr, ElementIx, Endpoint, Graph, Metaclass as M, Slot, Value};
use uuid::Uuid;

use crate::implied::implied_generals;
use crate::resolve::{structure, Global, Project, SourceFile, Structure};

/// The implied part of a file.
#[derive(Clone, Debug)]
pub struct Overlay {
    /// The number of elements of the lowered fragment: overlay element `k` is element
    /// `base + k` of the file.
    base: u32,
    /// The implied elements, at their overlay indices.
    graph: Graph,
    /// The implied relationships each element of the fragment owns, as file indices, in order.
    owned: HashMap<u32, Vec<u32>>,
    /// The owner of each implied relationship that an element of the fragment owns, by overlay
    /// index.
    owner: HashMap<u32, u32>,
    /// The ends of each implied relationship, by overlay index: its sources, then its targets.
    ends: HashMap<u32, (Vec<Global>, Vec<Global>)>,
}

impl PartialEq for Overlay {
    fn eq(&self, o: &Overlay) -> bool {
        self.base == o.base
            && self.owned == o.owned
            && self.owner == o.owner
            && self.ends == o.ends
            && self.graph.same_structure(&o.graph)
    }
}

impl Eq for Overlay {}

impl Overlay {
    /// The number of implied elements.
    pub fn len(&self) -> usize {
        self.graph.len()
    }

    /// Whether the file has no implied elements.
    pub fn is_empty(&self) -> bool {
        self.graph.is_empty()
    }
}

/// The overlay of a file: for each element of its fragment, in order, the specializations and
/// redefinitions that KerML 8.4.2 implies for it (ADR 0020), each an implied relationship with
/// the element as its source and the general as its target, then the specializations its
/// semantic metadata implies. The order is that of `implied_generals`, which the 1a.11 UUIDs
/// follow through `path()` (ADR 0022). An implied CrossSubsetting targets a feature chain that
/// the overlay builds as its owned related element (KerML 8.4.3.1 Table 9 Note 2).
///
/// Evaluating a filter condition, or the base type of semantic metadata, reads derived
/// properties, and so the view; those cycles converge by fixpoint iteration from an empty overlay
/// (ADR 0022).
#[salsa::tracked(returns(ref), cycle_initial = no_overlay, cycle_fn = settle_overlay)]
pub fn overlay(db: &dyn salsa::Database, project: Project, file: SourceFile) -> Overlay {
    let s = structure(db, file);
    let g = s.graph();
    let base = g.len() as u32;
    let mut b = Build {
        file,
        o: Overlay {
            base,
            graph: Graph::new(),
            owned: HashMap::new(),
            owner: HashMap::new(),
            ends: HashMap::new(),
        },
    };
    let semantic = semantic_specializations(db, project, file);
    for x in 0..base {
        let here = Global { file, index: x };
        for i in implied_generals(db, project, file, x) {
            let r = b.relationship(Owner::Fragment(x), i.kind, here, None);
            if i.kind == M::CrossSubsetting {
                b.crossing(db, project, x, r, i.general);
            } else if let Some(first) = i.chain {
                b.chain(r, &[first, i.general]);
            } else {
                b.end(r, Slot::Target, i.general);
            }
        }
        for (kind, general) in semantic.iter().filter(|s| s.0 == x).map(|s| (s.1, s.2)) {
            let r = b.relationship(Owner::Fragment(x), kind, here, None);
            b.end(r, Slot::Target, general);
        }
    }
    b.o
}

/// Where an implied relationship is owned: by an element of the fragment, or by an implied
/// element (by overlay index).
#[derive(Clone, Copy)]
enum Owner {
    Fragment(u32),
    Implied(u32),
}

/// Builds an overlay.
struct Build {
    file: SourceFile,
    o: Overlay,
}

impl Build {
    fn global(&self, local: u32) -> Global {
        Global {
            file: self.file,
            index: self.o.base + local,
        }
    }

    /// A new implied element. Its id is one inside the query graph only: DataIdentity ids are
    /// assigned outside it (ADR 0019), the library ones from `path()` in step 1a.11.
    fn element(&mut self, kind: M) -> u32 {
        let k = self.o.graph.len() as u128;
        let e = self
            .o
            .graph
            .add(kind, Uuid::from_u128(u128::from(self.o.base) + k + 1))
            .expect("fresh ids");
        e.index() as u32
    }

    /// A new relationship, owned by `owner`, with `source` as its source and, if given, an owned
    /// related element. The relationships an element of the fragment owns are implied; those of
    /// an implied element are part of it.
    fn relationship(&mut self, owner: Owner, kind: M, source: Global, owned: Option<u32>) -> u32 {
        let r = self.element(kind);
        match owner {
            Owner::Fragment(x) => {
                self.o.graph.set(
                    ElementIx::from_index(r as usize),
                    Attr::IsImplied,
                    Value::Boolean(true),
                );
                self.o.owned.entry(x).or_default().push(self.o.base + r);
                self.o.owner.insert(r, x);
            }
            Owner::Implied(e) => self.o.graph.add_owned_relationship(
                ElementIx::from_index(e as usize),
                ElementIx::from_index(r as usize),
            ),
        }
        if let Some(e) = owned {
            self.o.graph.add_owned_related_element(
                ElementIx::from_index(r as usize),
                ElementIx::from_index(e as usize),
            );
        }
        // The source is an end only where the metaclass stores one; otherwise it is the owning
        // related element, as in the fragment (`featureChained`, `membershipOwningNamespace`).
        let sources = if kind
            .stored_attributes()
            .iter()
            .any(|a| a.slot == Slot::Source)
        {
            vec![source]
        } else {
            Vec::new()
        };
        self.o.ends.insert(r, (sources, Vec::new()));
        r
    }

    fn end(&mut self, r: u32, slot: Slot, end: Global) {
        let ends = self.o.ends.entry(r).or_default();
        if slot == Slot::Source {
            ends.0.push(end);
        } else {
            ends.1.push(end);
        }
    }

    /// A feature chain of `features` as the owned related element and target of the implied
    /// relationship `r` (KerML 8.3.3.3.4 `subsetsChain`): a Feature with one FeatureChaining per
    /// feature. Its supertype is its last feature (`featureTarget`).
    fn chain(&mut self, r: u32, features: &[Global]) -> u32 {
        let chain = self.element(M::Feature);
        self.o.graph.add_owned_related_element(
            ElementIx::from_index(r as usize),
            ElementIx::from_index(chain as usize),
        );
        let chain_g = self.global(chain);
        self.end(r, Slot::Target, chain_g);
        for f in features {
            let ch = self.relationship(Owner::Implied(chain), M::FeatureChaining, chain_g, None);
            self.end(ch, Slot::Target, *f);
        }
        chain
    }

    /// The target of the implied CrossSubsetting `r` of the end `x` (KerML 8.3.3.3.4
    /// checkFeatureCrossingSpecialization; 8.4.3.1 Table 9 Note 2, 8.4.4.5.1): a chain of two
    /// features, owned by the CrossSubsetting. The first is owned by the chain, typed by the
    /// featuring type of the owned cross feature and featured by the end's owning type; the second
    /// is the owned cross feature. For a binary owning type that featuring type is the types of
    /// the other end (Table 9 Note 3); for more ends it is a Cartesian product, which the
    /// type-featuring constraints build, and the first feature is not typed here.
    fn crossing(
        &mut self,
        db: &dyn salsa::Database,
        project: Project,
        x: u32,
        r: u32,
        cross: Global,
    ) {
        let chain = self.element(M::Feature);
        self.o.graph.add_owned_related_element(
            ElementIx::from_index(r as usize),
            ElementIx::from_index(chain as usize),
        );
        let chain_g = self.global(chain);
        self.end(r, Slot::Target, chain_g);
        let first = self.element(M::Feature);
        let first_g = self.global(first);
        self.relationship(
            Owner::Implied(chain),
            M::OwningMembership,
            chain_g,
            Some(first),
        );
        for c in [first_g, cross] {
            let ch = self.relationship(Owner::Implied(chain), M::FeatureChaining, chain_g, None);
            self.end(ch, Slot::Target, c);
        }
        let end = Global {
            file: self.file,
            index: x,
        };
        let owning = crate::implied::owning_type(structure(db, self.file).graph(), end.ix());
        if let Some(t) = owning.map(|t| Global {
            file: self.file,
            index: t.index() as u32,
        }) {
            let ends = crate::implied::end_features(db, project, t.file, t.index);
            let others: Vec<Global> = ends.iter().copied().filter(|e| *e != end).collect();
            let types = match others[..] {
                [other] => crate::implied::feature_types(db, project, other),
                _ => Vec::new(),
            };
            for ty in &types {
                let typing =
                    self.relationship(Owner::Implied(first), M::FeatureTyping, first_g, None);
                self.end(typing, Slot::Target, *ty);
            }
            // The feature constraints on the first feature, which is no element of the fragment.
            for general in crate::implied::typed_feature_generals(db, project, &types) {
                let subsetting =
                    self.relationship(Owner::Implied(first), M::Subsetting, first_g, None);
                self.end(subsetting, Slot::Target, general);
            }
            let featuring =
                self.relationship(Owner::Implied(first), M::TypeFeaturing, first_g, None);
            self.end(featuring, Slot::Target, t);
        }
    }
}

/// The specializations that semantic metadata of the file implies (KerML 8.3.4.12.3
/// checkMetadataFeatureSemanticSpecialization), as (annotated type, kind, general), for the
/// annotated types of the file. The annotated type is the first Type that a semantic
/// MetadataFeature annotates, and its base type the syntax element of the first value of
/// `Metaobjects::SemanticMetadata::baseType`: a Classifier annotated with a Feature specializes the
/// feature's types, and any other type specializes the base type, unless it does already (KerML
/// 8.4.2). Name resolution does not see these (ADR 0022).
fn semantic_specializations(
    db: &dyn salsa::Database,
    project: Project,
    file: SourceFile,
) -> Vec<(u32, M, Global)> {
    let g = structure(db, file).graph();
    let metadata: Vec<Global> = g
        .elements()
        .filter(|e| g.metaclass(*e).specializes(M::MetadataFeature))
        .map(|e| Global {
            file,
            index: e.index() as u32,
        })
        .collect();
    if metadata.is_empty() {
        return Vec::new();
    }
    let Some(base_type) =
        crate::derived::library(db, project, "Metaobjects::SemanticMetadata::baseType")
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for mf in metadata {
        if !crate::derived::is_semantic(db, project, mf) {
            continue;
        }
        let Some(annotated) = crate::derived::annotated_element(db, project, mf)
            .into_iter()
            .find(|t| {
                view(db, project, t.file)
                    .metaclass(t.ix())
                    .specializes(M::Type)
            })
        else {
            continue;
        };
        if annotated.file != file {
            continue;
        }
        let Some(base) = crate::derived::evaluate_feature(db, project, mf, base_type)
            .first()
            .and_then(crate::derived::syntax_element)
        else {
            continue;
        };
        let kind_of = |x: Global, k: M| view(db, project, x.file).metaclass(x.ix()).specializes(k);
        let generals: Vec<Global> =
            if kind_of(annotated, M::Classifier) && kind_of(base, M::Feature) {
                crate::derived::r#type(db, project, base)
            } else if kind_of(base, M::Type) {
                vec![base]
            } else {
                Vec::new()
            };
        let supertypes = crate::implied::supertypes(db, project, annotated.file, annotated.index);
        for t in generals {
            if t != annotated
                && !supertypes.contains(&t)
                && !out
                    .iter()
                    .any(|o: &(u32, M, Global)| o.0 == annotated.index && o.2 == t)
            {
                out.push((
                    annotated.index,
                    crate::implied::specialization_kind(db, annotated, t),
                    t,
                ));
            }
        }
    }
    out
}

fn no_overlay(
    db: &dyn salsa::Database,
    _id: salsa::Id,
    _project: Project,
    file: SourceFile,
) -> Overlay {
    Overlay {
        base: structure(db, file).graph().len() as u32,
        graph: Graph::new(),
        owned: HashMap::new(),
        owner: HashMap::new(),
        ends: HashMap::new(),
    }
}

settle!(settle_overlay, Overlay, (Project, SourceFile));

/// An end of a relationship in the view: written in the fragment, or a `Global` of an implied
/// relationship.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewEnd {
    /// An end of a relationship of the fragment, as written (ADR 0017).
    Written(Endpoint),
    /// An end of an implied relationship.
    Implied(Global),
}

/// A file's fragment and its overlay, read as one (ADR 0022).
#[derive(Clone, Copy)]
pub struct View<'db> {
    file: SourceFile,
    base: &'db Structure,
    overlay: &'db Overlay,
}

/// The view of a file.
pub fn view(db: &dyn salsa::Database, project: Project, file: SourceFile) -> View<'_> {
    View {
        file,
        base: structure(db, file),
        overlay: overlay(db, project, file),
    }
}

impl<'db> View<'db> {
    fn base(&self) -> &'db Graph {
        self.base.graph()
    }

    /// The overlay index of an implied element, or `None` for an element of the fragment.
    fn implied(&self, ix: ElementIx) -> Option<ElementIx> {
        let i = ix.index() as u32;
        (i >= self.overlay.base).then(|| ElementIx::from_index((i - self.overlay.base) as usize))
    }

    fn global(&self, local: ElementIx) -> ElementIx {
        ElementIx::from_index(self.overlay.base as usize + local.index())
    }

    /// The number of elements, the implied ones included.
    pub fn len(&self) -> usize {
        self.overlay.base as usize + self.overlay.len()
    }

    /// Whether the view has no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The root namespace.
    pub fn root(&self) -> ElementIx {
        self.base.root()
    }

    /// Whether an element is implied.
    pub fn is_implied(&self, ix: ElementIx) -> bool {
        self.implied(ix).is_some()
    }

    /// The element's metaclass.
    pub fn metaclass(&self, ix: ElementIx) -> M {
        match self.implied(ix) {
            Some(l) => self.overlay.graph.metaclass(l),
            None => self.base().metaclass(ix),
        }
    }

    /// `ownedRelationship` (KerML 8.3.2.1.2): the owned relationships, then the implied ones.
    pub fn all_owned_relationships(&self, ix: ElementIx) -> std::vec::IntoIter<ElementIx> {
        self.owned_relationship_list(ix).into_iter()
    }

    fn owned_relationship_list(&self, ix: ElementIx) -> Vec<ElementIx> {
        match self.implied(ix) {
            Some(l) => self
                .overlay
                .graph
                .owned_relationships(l)
                .iter()
                .map(|r| self.global(*r))
                .collect(),
            None => {
                let mut out: Vec<ElementIx> = self.base().all_owned_relationships(ix).collect();
                if let Some(implied) = self.overlay.owned.get(&(ix.index() as u32)) {
                    out.extend(implied.iter().map(|i| ElementIx::from_index(*i as usize)));
                }
                out
            }
        }
    }

    /// `ownedRelatedElement`.
    pub fn owned_related_elements(&self, ix: ElementIx) -> Vec<ElementIx> {
        match self.implied(ix) {
            Some(l) => self
                .overlay
                .graph
                .owned_related_elements(l)
                .iter()
                .map(|e| self.global(*e))
                .collect(),
            None => self.base().owned_related_elements(ix).to_vec(),
        }
    }

    /// `owningRelationship`.
    pub fn owning_relationship(&self, ix: ElementIx) -> Option<ElementIx> {
        match self.implied(ix) {
            Some(l) => self
                .overlay
                .graph
                .owning_relationship(l)
                .map(|r| self.global(r)),
            None => self.base().owning_relationship(ix),
        }
    }

    /// `owningRelatedElement`: for an implied relationship of an element of the fragment, that
    /// element.
    pub fn owning_related_element(&self, ix: ElementIx) -> Option<ElementIx> {
        match self.implied(ix) {
            Some(l) => match self.overlay.owner.get(&(l.index() as u32)) {
                Some(o) => Some(ElementIx::from_index(*o as usize)),
                None => self
                    .overlay
                    .graph
                    .owning_related_element(l)
                    .map(|e| self.global(e)),
            },
            None => self.base().owning_related_element(ix),
        }
    }

    /// `owner`: the owning related element of the owning relationship.
    pub fn owner(&self, ix: ElementIx) -> Option<ElementIx> {
        self.owning_relationship(ix)
            .and_then(|r| self.owning_related_element(r))
    }

    /// `ownedElement`: the owned related elements of the owned relationships, implied ones
    /// included.
    pub fn owned_elements(&self, ix: ElementIx) -> std::vec::IntoIter<ElementIx> {
        let v: Vec<ElementIx> = self
            .all_owned_relationships(ix)
            .flat_map(|r| self.owned_related_elements(r))
            .collect();
        v.into_iter()
    }

    /// The fragment's graph, for what implied elements never are (memberships and imports,
    /// names to resolve).
    pub fn lowered(&self) -> &'db Graph {
        self.base.graph()
    }

    /// The ends of a relationship at a slot: as written, or the `Global`s of an implied one.
    pub fn ends(&self, ix: ElementIx, slot: Slot) -> Vec<ViewEnd> {
        match self.implied(ix) {
            Some(l) => {
                let Some((sources, targets)) = self.overlay.ends.get(&(l.index() as u32)) else {
                    return Vec::new();
                };
                let ends = if slot == Slot::Source {
                    sources
                } else {
                    targets
                };
                ends.iter().map(|g| ViewEnd::Implied(*g)).collect()
            }
            None => {
                let ends = if slot == Slot::Source {
                    self.base().source(ix)
                } else {
                    self.base().target(ix)
                };
                ends.iter().map(|e| ViewEnd::Written(*e)).collect()
            }
        }
    }

    /// The values of an attribute.
    pub fn values(&self, ix: ElementIx, attr: Attr) -> std::vec::IntoIter<&'db Value> {
        let v: Vec<&'db Value> = match self.implied(ix) {
            Some(l) => self.overlay.graph.values(l, attr).collect(),
            None => self.base().values(ix, attr).collect(),
        };
        v.into_iter()
    }

    /// A Boolean attribute, or the metamodel's default.
    pub fn flag(&self, ix: ElementIx, attr: Attr) -> bool {
        match self.implied(ix) {
            Some(l) => self.overlay.graph.flag(l, attr),
            None => self.base().flag(ix, attr),
        }
    }

    /// The visibility of a membership or an import (KerML 8.3.2.4), or the metamodel default for
    /// an implied one, which sets none.
    pub fn visibility(&self, ix: ElementIx) -> crate::Visibility {
        match self.implied(ix) {
            Some(l) => crate::resolve::visibility(&self.overlay.graph, l),
            None => crate::resolve::visibility(self.base(), ix),
        }
    }

    /// A String attribute.
    pub fn string(&self, ix: ElementIx, attr: Attr) -> Option<&'db str> {
        match self.implied(ix) {
            Some(l) => self.overlay.graph.string(l, attr),
            None => self.base().string(ix, attr),
        }
    }

    /// The file.
    pub fn file(&self) -> SourceFile {
        self.file
    }
}
