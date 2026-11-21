// SPDX-License-Identifier: Apache-2.0
//! The element graph (ADR 0017): an arena of model elements addressed by dense indices, each
//! with its `DataIdentity` UUID, storing the canonical core of the KerML abstract syntax.
//!
//! Every element stores:
//! - its metaclass and its source span
//! - its ownership: `owningRelationship` and the ordered `ownedRelationship`s
//!   (KerML 8.3.2.1.3), with the implied relationships kept apart from the owned ones
//! - for a relationship, its `owningRelatedElement`, its ordered `ownedRelatedElement`s, and its
//!   `source` and `target` (KerML 8.3.2.1.5)
//! - its names, flags, enumeration values and literal values (`Attr`)
//!
//! Every other stored attribute is one of these under a redefined name (for example
//! `FeatureTyping::type` is `target[0]`), read through [`Metaclass::stored_attributes`].

use crate::{Attr, Metaclass, Slot, StoredAttribute};
use std::collections::HashMap;
use uuid::Uuid;

/// The index of an element in its [`Graph`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ElementIx(u32);

impl ElementIx {
    /// The position in the graph's arena.
    pub fn index(self) -> usize {
        self.0 as usize
    }

    /// The element at a position, as [`index`](Self::index) gives it (ADR 0019 addresses an
    /// element in the query graph by its file and this position).
    pub fn from_index(index: usize) -> ElementIx {
        ElementIx(index as u32)
    }
}

/// The index of an unresolved name in its [`Graph`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NameIx(u32);

/// The file a source span is in, numbered by whoever builds the graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileId(pub u32);

/// A byte range in a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    /// The file.
    pub file: FileId,
    /// The first byte.
    pub start: u32,
    /// The byte after the last.
    pub end: u32,
}

/// One end of a relationship: an element, or until name resolution (step 1a.7) the qualified
/// name the text gives for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Endpoint {
    /// A resolved element.
    Element(ElementIx),
    /// A name not resolved yet.
    Name(NameIx),
}

/// A qualified name as written (KerML 8.2.3.4.1), waiting for resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QualifiedName {
    /// `$::` at the start: the global namespace.
    pub global: bool,
    /// The segment names, unquoted (KerML 8.2.2.3).
    pub segments: Vec<Box<str>>,
    /// Where the name is written.
    pub span: Option<Span>,
}

/// The value of an attribute that is not a reference.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A Boolean.
    Boolean(bool),
    /// A String.
    String(Box<str>),
    /// An Integer.
    Integer(i64),
    /// A Real.
    Real(f64),
    /// A literal of an enumeration, as its name (`public`, `in` …).
    Enumeration(&'static str),
}

/// One element of the graph.
#[derive(Clone, Debug)]
pub struct Element {
    metaclass: Metaclass,
    span: Option<Span>,
    owning_relationship: Option<ElementIx>,
    owned_relationships: Vec<ElementIx>,
    implied_relationships: Vec<ElementIx>,
    owning_related_element: Option<ElementIx>,
    owned_related_elements: Vec<ElementIx>,
    source: Vec<Endpoint>,
    target: Vec<Endpoint>,
    values: Vec<(Attr, Value)>,
}

/// A stored attribute read through [`Graph::get`].
#[derive(Clone, Debug, PartialEq)]
pub enum Property<'g> {
    /// A reference attribute: its ends, for example the one `type` of a `FeatureTyping`.
    References(&'g [Endpoint]),
    /// The element on the other side of an ownership link, if any.
    Owner(Option<ElementIx>),
    /// The ownership links in order.
    Owned(&'g [ElementIx]),
    /// A value, or the default of the metamodel if it is not set; `None` if neither.
    Values(Vec<Value>),
}

/// An error building a graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphError {
    /// Two elements have the same `DataIdentity` id.
    DuplicateId(Uuid),
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::DuplicateId(id) => write!(f, "two elements have the id {id}"),
        }
    }
}

impl std::error::Error for GraphError {}

/// The element graph: elements, their UUIDs, and the names they refer to.
#[derive(Clone, Debug, Default)]
pub struct Graph {
    elements: Vec<Element>,
    ids: Vec<Uuid>,
    by_id: HashMap<Uuid, ElementIx>,
    names: Vec<QualifiedName>,
}

impl Graph {
    /// An empty graph.
    pub fn new() -> Graph {
        Graph::default()
    }

    /// The number of elements.
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Whether the graph has no elements.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Every element, in the order they were added.
    pub fn elements(&self) -> impl Iterator<Item = ElementIx> + '_ {
        (0..self.elements.len() as u32).map(ElementIx)
    }

    /// Add an element with its `DataIdentity` id.
    pub fn add(&mut self, metaclass: Metaclass, id: Uuid) -> Result<ElementIx, GraphError> {
        if self.by_id.contains_key(&id) {
            return Err(GraphError::DuplicateId(id));
        }
        let ix = ElementIx(self.elements.len() as u32);
        self.elements.push(Element {
            metaclass,
            span: None,
            owning_relationship: None,
            owned_relationships: Vec::new(),
            implied_relationships: Vec::new(),
            owning_related_element: None,
            owned_related_elements: Vec::new(),
            source: Vec::new(),
            target: Vec::new(),
            values: Vec::new(),
        });
        self.ids.push(id);
        self.by_id.insert(id, ix);
        Ok(ix)
    }

    fn e(&self, ix: ElementIx) -> &Element {
        &self.elements[ix.index()]
    }

    fn e_mut(&mut self, ix: ElementIx) -> &mut Element {
        &mut self.elements[ix.index()]
    }

    /// The metaclass of an element.
    pub fn metaclass(&self, ix: ElementIx) -> Metaclass {
        self.e(ix).metaclass
    }

    /// The `DataIdentity` id of an element.
    pub fn id(&self, ix: ElementIx) -> Uuid {
        self.ids[ix.index()]
    }

    /// The element with an id.
    pub fn by_id(&self, id: Uuid) -> Option<ElementIx> {
        self.by_id.get(&id).copied()
    }

    /// Whether two graphs are the same apart from where things are written: the same elements,
    /// ids, ownership, ends, values and names, ignoring every span. An edit of layout alone
    /// gives the same structure, which lets the queries that read only the structure stop there
    /// (early cutoff, ADR 0019).
    pub fn same_structure(&self, other: &Graph) -> bool {
        self.ids == other.ids
            && self.elements.len() == other.elements.len()
            && self.elements.iter().zip(&other.elements).all(|(a, b)| {
                a.metaclass == b.metaclass
                    && a.owning_relationship == b.owning_relationship
                    && a.owned_relationships == b.owned_relationships
                    && a.implied_relationships == b.implied_relationships
                    && a.owning_related_element == b.owning_related_element
                    && a.owned_related_elements == b.owned_related_elements
                    && a.source == b.source
                    && a.target == b.target
                    && a.values == b.values
            })
            && self.names.len() == other.names.len()
            && self
                .names
                .iter()
                .zip(&other.names)
                .all(|(a, b)| a.global == b.global && a.segments == b.segments)
    }

    /// Where an element is written, if it comes from text.
    pub fn span(&self, ix: ElementIx) -> Option<Span> {
        self.e(ix).span
    }

    /// Set where an element is written.
    pub fn set_span(&mut self, ix: ElementIx, span: Span) {
        self.e_mut(ix).span = Some(span);
    }

    // ---- ownership (KerML 8.3.2.1) ----

    /// Make `relationship` an `ownedRelationship` of `owner`, after the others.
    pub fn add_owned_relationship(&mut self, owner: ElementIx, relationship: ElementIx) {
        self.e_mut(owner).owned_relationships.push(relationship);
        self.e_mut(relationship).owning_related_element = Some(owner);
    }

    /// Make `element` an `ownedRelatedElement` of `relationship`, after the others.
    pub fn add_owned_related_element(&mut self, relationship: ElementIx, element: ElementIx) {
        self.e_mut(relationship)
            .owned_related_elements
            .push(element);
        self.e_mut(element).owning_relationship = Some(relationship);
    }

    /// Add an implied relationship to `owner` (KerML 8.3.2.1.5 `isImplied`): owned by `owner`,
    /// but kept apart from what a modeler wrote, so that it can be replaced or left out as a
    /// whole (ADR 0017).
    pub fn add_implied_relationship(&mut self, owner: ElementIx, relationship: ElementIx) {
        self.e_mut(owner).implied_relationships.push(relationship);
        self.e_mut(relationship).owning_related_element = Some(owner);
        self.set(relationship, Attr::IsImplied, Value::Boolean(true));
    }

    /// `owningRelationship`: the relationship that owns this element as a related element.
    pub fn owning_relationship(&self, ix: ElementIx) -> Option<ElementIx> {
        self.e(ix).owning_relationship
    }

    /// `owningRelatedElement`: the element that owns this relationship.
    pub fn owning_related_element(&self, ix: ElementIx) -> Option<ElementIx> {
        self.e(ix).owning_related_element
    }

    /// The `ownedRelationship`s a modeler wrote, in order, without the implied ones.
    pub fn owned_relationships(&self, ix: ElementIx) -> &[ElementIx] {
        &self.e(ix).owned_relationships
    }

    /// The implied relationships of an element, in order.
    pub fn implied_relationships(&self, ix: ElementIx) -> &[ElementIx] {
        &self.e(ix).implied_relationships
    }

    /// `ownedRelationship` as KerML 8.3.2.1.3 defines it: the owned relationships, followed by the
    /// implied ones if `isImpliedIncluded` is true (ADR 0017).
    pub fn all_owned_relationships(&self, ix: ElementIx) -> impl Iterator<Item = ElementIx> + '_ {
        let implied: &[ElementIx] = if self.flag(ix, Attr::IsImpliedIncluded) {
            &self.e(ix).implied_relationships
        } else {
            &[]
        };
        self.e(ix)
            .owned_relationships
            .iter()
            .chain(implied)
            .copied()
    }

    /// `ownedRelatedElement`: the elements a relationship owns, in order.
    pub fn owned_related_elements(&self, ix: ElementIx) -> &[ElementIx] {
        &self.e(ix).owned_related_elements
    }

    /// `ownedElement` (KerML 8.3.2.1.3, derived): the owned related elements of the owned
    /// relationships, in order.
    pub fn owned_elements(&self, ix: ElementIx) -> impl Iterator<Item = ElementIx> + '_ {
        self.e(ix)
            .owned_relationships
            .iter()
            .flat_map(|r| self.e(*r).owned_related_elements.iter().copied())
    }

    /// `owner` (derived): the owning related element of the owning relationship.
    pub fn owner(&self, ix: ElementIx) -> Option<ElementIx> {
        self.e(ix)
            .owning_relationship
            .and_then(|r| self.e(r).owning_related_element)
    }

    // ---- relationship ends (KerML 8.3.2.1.5) ----

    /// `source`, in order.
    pub fn source(&self, ix: ElementIx) -> &[Endpoint] {
        &self.e(ix).source
    }

    /// `target`, in order.
    pub fn target(&self, ix: ElementIx) -> &[Endpoint] {
        &self.e(ix).target
    }

    /// Add an end to `source`.
    pub fn add_source(&mut self, relationship: ElementIx, end: Endpoint) {
        self.e_mut(relationship).source.push(end);
    }

    /// Add an end to `target`.
    pub fn add_target(&mut self, relationship: ElementIx, end: Endpoint) {
        self.e_mut(relationship).target.push(end);
    }

    /// Replace an end, for example a name by the element it resolves to.
    pub fn set_end(&mut self, relationship: ElementIx, slot: Slot, position: usize, end: Endpoint) {
        let e = self.e_mut(relationship);
        match slot {
            Slot::Source => e.source[position] = end,
            Slot::Target => e.target[position] = end,
            _ => panic!("{slot:?} is not a relationship end"),
        }
    }

    // ---- names ----

    /// Record a qualified name to resolve, and return the end that refers to it.
    pub fn add_name(&mut self, name: QualifiedName) -> Endpoint {
        self.names.push(name);
        Endpoint::Name(NameIx(self.names.len() as u32 - 1))
    }

    /// A recorded name.
    pub fn name(&self, ix: NameIx) -> &QualifiedName {
        &self.names[ix.0 as usize]
    }

    // ---- values ----

    /// Set an attribute, replacing its value (for a single-valued attribute).
    pub fn set(&mut self, ix: ElementIx, attr: Attr, value: Value) {
        let values = &mut self.e_mut(ix).values;
        match values.iter_mut().find(|(a, _)| *a == attr) {
            Some(slot) => slot.1 = value,
            None => values.push((attr, value)),
        }
    }

    /// Add a value to a many-valued attribute (`aliasIds`).
    pub fn push(&mut self, ix: ElementIx, attr: Attr, value: Value) {
        self.e_mut(ix).values.push((attr, value));
    }

    /// The values set on an attribute, without defaults.
    pub fn values(&self, ix: ElementIx, attr: Attr) -> impl Iterator<Item = &Value> + '_ {
        self.e(ix)
            .values
            .iter()
            .filter(move |(a, _)| *a == attr)
            .map(|(_, v)| v)
    }

    /// A Boolean attribute, with its default.
    pub fn flag(&self, ix: ElementIx, attr: Attr) -> bool {
        match self.values(ix, attr).next() {
            Some(Value::Boolean(b)) => *b,
            _ => {
                self.stored_attribute(ix, attr.name())
                    .and_then(|a| a.default)
                    == Some("true")
            }
        }
    }

    /// A string attribute, such as `declaredName`.
    pub fn string(&self, ix: ElementIx, attr: Attr) -> Option<&str> {
        match self.values(ix, attr).next() {
            Some(Value::String(s)) => Some(s),
            _ => None,
        }
    }

    /// The stored attribute of this element's metaclass with a name (KerML 8.3).
    pub fn stored_attribute(&self, ix: ElementIx, name: &str) -> Option<&'static StoredAttribute> {
        self.metaclass(ix)
            .stored_attributes()
            .iter()
            .find(|a| a.name == name)
    }

    /// Read any stored attribute of an element by its name in the metamodel, through the slot it
    /// is a view of. `None` if the metaclass has no stored attribute of that name (it may be
    /// derived; derived properties are computed in step 1a.8).
    pub fn get(&self, ix: ElementIx, name: &str) -> Option<Property<'_>> {
        let a = self.stored_attribute(ix, name)?;
        let e = self.e(ix);
        Some(match a.slot {
            Slot::Source => Property::References(single(&e.source, a.many)),
            Slot::Target => Property::References(single(&e.target, a.many)),
            Slot::OwningRelationship => Property::Owner(e.owning_relationship),
            Slot::OwningRelatedElement => Property::Owner(e.owning_related_element),
            Slot::OwnedRelationship => Property::Owned(&e.owned_relationships),
            Slot::OwnedRelatedElement => Property::Owned(&e.owned_related_elements),
            Slot::Value(attr) => {
                let set: Vec<Value> = self.values(ix, attr).cloned().collect();
                if !set.is_empty() || a.many {
                    Property::Values(set)
                } else {
                    Property::Values(a.default.map(|d| default_value(a, d)).into_iter().collect())
                }
            }
        })
    }

    // ---- assembly ----

    /// Append another graph, a fragment lowered from one file (ADR 0017), shifting its indices.
    /// Returns the index its first element got.
    pub fn append(&mut self, other: Graph) -> Result<u32, GraphError> {
        let offset = self.elements.len() as u32;
        let names = self.names.len() as u32;
        for id in &other.ids {
            if self.by_id.contains_key(id) {
                return Err(GraphError::DuplicateId(*id));
            }
        }
        let shift = |ix: ElementIx| ElementIx(ix.0 + offset);
        let end = |end: Endpoint| match end {
            Endpoint::Element(ix) => Endpoint::Element(shift(ix)),
            Endpoint::Name(n) => Endpoint::Name(NameIx(n.0 + names)),
        };
        for (e, id) in other.elements.into_iter().zip(other.ids) {
            self.by_id.insert(id, ElementIx(self.elements.len() as u32));
            self.ids.push(id);
            self.elements.push(Element {
                owning_relationship: e.owning_relationship.map(shift),
                owned_relationships: e.owned_relationships.into_iter().map(shift).collect(),
                implied_relationships: e.implied_relationships.into_iter().map(shift).collect(),
                owning_related_element: e.owning_related_element.map(shift),
                owned_related_elements: e.owned_related_elements.into_iter().map(shift).collect(),
                source: e.source.into_iter().map(end).collect(),
                target: e.target.into_iter().map(end).collect(),
                ..e
            });
        }
        self.names.extend(other.names);
        Ok(offset)
    }
}

/// A single-valued view of an end list (`FeatureTyping::type` is `target[0]`).
fn single(ends: &[Endpoint], many: bool) -> &[Endpoint] {
    if many {
        ends
    } else {
        &ends[..ends.len().min(1)]
    }
}

/// The value of a metamodel default.
fn default_value(a: &StoredAttribute, text: &'static str) -> Value {
    match a.kind {
        crate::PropertyKind::Boolean => Value::Boolean(text == "true"),
        crate::PropertyKind::Integer => Value::Integer(text.parse().unwrap_or(0)),
        crate::PropertyKind::Real => Value::Real(text.parse().unwrap_or(0.0)),
        crate::PropertyKind::Enumeration { .. } => Value::Enumeration(text),
        _ => Value::String(text.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    /// `classifier C; feature f : C;` in a namespace, by hand.
    fn sample() -> (Graph, [ElementIx; 6]) {
        let mut g = Graph::new();
        let ns = g.add(Metaclass::Namespace, id(1)).unwrap();
        let m1 = g.add(Metaclass::OwningMembership, id(2)).unwrap();
        let c = g.add(Metaclass::Classifier, id(3)).unwrap();
        let m2 = g.add(Metaclass::OwningMembership, id(4)).unwrap();
        let f = g.add(Metaclass::Feature, id(5)).unwrap();
        let t = g.add(Metaclass::FeatureTyping, id(6)).unwrap();
        g.add_owned_relationship(ns, m1);
        g.add_owned_related_element(m1, c);
        g.add_owned_relationship(ns, m2);
        g.add_owned_related_element(m2, f);
        g.add_owned_relationship(f, t);
        g.add_source(t, Endpoint::Element(f));
        g.add_target(t, Endpoint::Element(c));
        g.set(c, Attr::DeclaredName, Value::String("C".into()));
        g.set(f, Attr::DeclaredName, Value::String("f".into()));
        (g, [ns, m1, c, m2, f, t])
    }

    #[test]
    fn the_structure_ignores_spans_only() {
        let (a, [_, _, c, ..]) = sample();
        let mut b = a.clone();
        b.set_span(
            c,
            Span {
                file: FileId(0),
                start: 4,
                end: 9,
            },
        );
        assert!(a.same_structure(&b));
        b.set(c, Attr::DeclaredName, Value::String("D".into()));
        assert!(!a.same_structure(&b));
    }

    #[test]
    fn walk_by_ownership() {
        let (g, [ns, m1, c, m2, f, t]) = sample();
        assert_eq!(g.owned_relationships(ns), [m1, m2]);
        assert_eq!(g.owned_elements(ns).collect::<Vec<_>>(), [c, f]);
        assert_eq!(g.owner(f), Some(ns));
        assert_eq!(g.owning_related_element(t), Some(f));
        assert_eq!(g.owner(ns), None);
    }

    #[test]
    fn walk_by_relationship_through_the_typed_views() {
        // KerML 8.3.3.3.4: `type` and `typedFeature` are views of `target` and `source`.
        let (g, [_, _, c, _, f, t]) = sample();
        assert_eq!(
            g.get(t, "type"),
            Some(Property::References(&[Endpoint::Element(c)]))
        );
        assert_eq!(
            g.get(t, "typedFeature"),
            Some(Property::References(&[Endpoint::Element(f)]))
        );
        assert_eq!(g.get(t, "general"), g.get(t, "type"));
        assert_eq!(
            g.get(t, "owningRelatedElement"),
            Some(Property::Owner(Some(f)))
        );
        assert_eq!(
            g.get(f, "owningRelationship"),
            g.get(f, "owningRelationship")
        );
        // A derived property is not stored.
        assert_eq!(g.get(t, "owningFeature"), None);
    }

    #[test]
    fn values_and_metamodel_defaults() {
        let (mut g, [_, m1, c, _, f, _]) = sample();
        assert_eq!(g.string(c, Attr::DeclaredName), Some("C"));
        assert_eq!(
            g.get(f, "isUnique"),
            Some(Property::Values(vec![Value::Boolean(true)]))
        );
        assert!(g.flag(f, Attr::IsUnique) && !g.flag(f, Attr::IsOrdered));
        g.set(f, Attr::IsOrdered, Value::Boolean(true));
        assert!(g.flag(f, Attr::IsOrdered));
        assert_eq!(
            g.get(m1, "visibility"),
            Some(Property::Values(vec![Value::Enumeration("public")]))
        );
        assert_eq!(g.get(f, "direction"), Some(Property::Values(vec![])));
    }

    #[test]
    fn implied_relationships_are_kept_apart() {
        // KerML 8.3.2.1.3: implied relationships are ownedRelationships only when
        // isImpliedIncluded is true (ADR 0017).
        let (mut g, [_, _, c, _, f, t]) = sample();
        let s = g.add(Metaclass::Subclassification, id(7)).unwrap();
        g.add_implied_relationship(c, s);
        assert!(g.flag(s, Attr::IsImplied));
        assert_eq!(g.owned_relationships(c), []);
        assert_eq!(g.all_owned_relationships(c).count(), 0);
        g.set(c, Attr::IsImpliedIncluded, Value::Boolean(true));
        assert_eq!(g.all_owned_relationships(c).collect::<Vec<_>>(), [s]);
        assert_eq!(g.all_owned_relationships(f).collect::<Vec<_>>(), [t]);
    }

    #[test]
    fn ids_are_unique_and_fragments_append() {
        let (mut g, [ns, ..]) = sample();
        assert_eq!(g.by_id(id(1)), Some(ns));
        assert_eq!(
            g.add(Metaclass::Package, id(1)),
            Err(GraphError::DuplicateId(id(1)))
        );
        let mut other = Graph::new();
        let p = other.add(Metaclass::Package, id(10)).unwrap();
        let m = other.add(Metaclass::OwningMembership, id(11)).unwrap();
        other.add_owned_relationship(p, m);
        let name = other.add_name(QualifiedName {
            global: false,
            segments: vec!["C".into()],
            span: None,
        });
        other.add_target(m, name);
        let offset = g.append(other).unwrap();
        assert_eq!(offset, 6);
        let p = g.by_id(id(10)).unwrap();
        let m = g.by_id(id(11)).unwrap();
        assert_eq!(g.owned_relationships(p), [m]);
        let Endpoint::Name(n) = g.target(m)[0] else {
            panic!()
        };
        assert_eq!(&*g.name(n).segments[0], "C");
        let (dup, _) = sample();
        assert!(g.append(dup).is_err());
    }
}
