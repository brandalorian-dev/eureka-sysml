// SPDX-License-Identifier: Apache-2.0
//! Derived properties and operations (step 1a.8, ADR 0021): one test per `derive…` rule and
//! operation, from its OCL in KerML 8.3, named after it (`tests/derived_inventory.rs` checks).
//!
//! Elements are found, and shown, by their declared names through the graph, independently of
//! the functions under test.

use sysml_model::{Attr, ElementIx, FileId, Metaclass};
use sysml_semantics::derived as d;
use sysml_semantics::{lowered, Database, Global, Project, SourceFile, Visibility};
use sysml_syntax::Language;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

struct Model {
    db: Database,
    project: Project,
    file: SourceFile,
}

fn model(text: &str) -> Model {
    let db = Database::default();
    let file = SourceFile::new(
        &db,
        FileId(0),
        "test.kerml".into(),
        Language::KerML,
        text.into(),
    );
    let project = Project::new(&db, vec![file]);
    Model { db, project, file }
}

impl Model {
    fn graph(&self) -> std::sync::Arc<sysml_semantics::Lowered> {
        lowered(&self.db, self.file).clone()
    }

    fn at(&self, e: ElementIx) -> Global {
        Global {
            file: self.file,
            index: e.index() as u32,
        }
    }

    fn root(&self) -> Global {
        self.at(self.graph().root)
    }

    /// The element at a path of declared names (or short names); `#n` is the n-th (from 1) owned
    /// element with the metaclass name before it, for example `Comment#1`.
    fn get(&self, path: &str) -> Global {
        let l = self.graph();
        let g = &l.graph;
        let mut x = l.root;
        for segment in path.split("::") {
            let owned: Vec<ElementIx> = g.owned_elements(x).collect();
            x = match segment.split_once('#') {
                Some((m, n)) => *owned
                    .iter()
                    .filter(|e| g.metaclass(**e).name() == m)
                    .nth(n.parse::<usize>().unwrap() - 1)
                    .unwrap_or_else(|| panic!("no {segment} in {path}")),
                None => *owned
                    .iter()
                    .find(|e| {
                        g.string(**e, Attr::DeclaredName) == Some(segment)
                            || g.string(**e, Attr::DeclaredShortName) == Some(segment)
                    })
                    .unwrap_or_else(|| panic!("no {segment} in {path}")),
            };
        }
        self.at(x)
    }

    /// The owning relationship of the element at a path.
    fn membership(&self, path: &str) -> Global {
        let l = self.graph();
        self.at(l.graph.owning_relationship(self.get(path).ix()).unwrap())
    }

    /// The n-th (from 1) owned relationship of a metaclass of the element at a path.
    fn relationship(&self, path: &str, m: Metaclass, n: usize) -> Global {
        let l = self.graph();
        let owner = if path.is_empty() {
            l.root
        } else {
            self.get(path).ix()
        };
        let r = l
            .graph
            .owned_relationships(owner)
            .iter()
            .filter(|r| l.graph.metaclass(**r) == m)
            .nth(n - 1)
            .copied()
            .unwrap_or_else(|| panic!("no {m:?} {n} in {path}"));
        self.at(r)
    }

    /// An element as its declared names, through its owners; a relationship as its metaclass and
    /// what it owns or names.
    fn label(&self, x: Global) -> String {
        let l = self.graph();
        let g = &l.graph;
        let m = g.metaclass(x.ix());
        if m.specializes(Metaclass::Relationship) && g.string(x.ix(), Attr::DeclaredName).is_none()
        {
            let owned = g.owned_related_elements(x.ix()).first();
            return match (owned, g.string(x.ix(), Attr::MemberName)) {
                (Some(e), _) => format!("{}({})", m.name(), self.label(self.at(*e))),
                (None, Some(n)) => format!("{}({n})", m.name()),
                (None, None) => m.name().to_owned(),
            };
        }
        let mut names = Vec::new();
        let mut e = Some(x.ix());
        while let Some(i) = e {
            match g
                .string(i, Attr::DeclaredName)
                .or(g.string(i, Attr::DeclaredShortName))
            {
                Some(n) => names.push(n.to_owned()),
                None if i == x.ix() => names.push(format!("({})", g.metaclass(i).name())),
                None => {}
            }
            e = g.owner(i);
        }
        names.reverse();
        names.join("::")
    }

    fn labels(&self, xs: impl IntoIterator<Item = Global>) -> Vec<String> {
        xs.into_iter().map(|x| self.label(x)).collect()
    }

    fn label_of(&self, x: Option<Global>) -> Option<String> {
        x.map(|x| self.label(x))
    }
}

fn strings(xs: Vec<std::sync::Arc<str>>) -> Vec<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

// ---- Element (KerML 8.3.2.1.2) ----

#[test]
fn derive_element_owner() {
    // owner = owningRelationship.owningRelatedElement
    let m = model("package P { class A; }");
    let (db, p) = (&m.db, m.project);
    assert_eq!(m.label_of(d::owner(db, p, m.get("P::A"))), Some("P".into()));
    assert_eq!(
        m.label_of(d::owner(db, p, m.get("P"))),
        Some("(Namespace)".into())
    );
    assert_eq!(d::owner(db, p, m.root()), None);
    // A relationship owned through ownedRelationship has no owningRelationship.
    assert_eq!(d::owner(db, p, m.membership("P::A")), None);
}

#[test]
fn derive_element_owned_element() {
    // ownedElement = ownedRelationship.ownedRelatedElement
    let m = model("package P { class A { feature f; } class B; }");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.labels(d::owned_element(db, p, m.get("P"))),
        ["P::A", "P::B"]
    );
    assert_eq!(
        m.labels(d::owned_element(db, p, m.get("P::A"))),
        ["P::A::f"]
    );
    assert!(d::owned_element(db, p, m.get("P::B")).is_empty());
}

#[test]
fn derive_owning_namespace() {
    // owningNamespace = owningMembership.membershipOwningNamespace, if any
    let m = model("package P { class A { feature f; } } metaclass M; classifier A0; classifier B0; #M dependency X from A0 to B0;");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.label_of(d::owning_namespace(db, p, m.get("P::A::f"))),
        Some("P::A".into())
    );
    assert_eq!(
        m.label_of(d::owning_namespace(db, p, m.get("P"))),
        Some("(Namespace)".into())
    );
    assert_eq!(d::owning_namespace(db, p, m.root()), None);
    // On a relationship, the metadata feature of `#M` is owned by an Annotation, not by a
    // membership (KerML 8.2.3.2, PrefixMetadataAnnotation).
    let metadata = m.get("X::MetadataFeature#1");
    assert_eq!(d::owning_namespace(db, p, metadata), None);
}

#[test]
fn derive_element_name() {
    // name = effectiveName(); for a feature without names, its naming feature's (8.3.3.3.4)
    let m =
        model("classifier A { feature x; } classifier B specializes A { feature redefines x; }");
    let (db, p) = (&m.db, m.project);
    assert_eq!(d::name(db, p, m.get("A")).as_deref(), Some("A"));
    assert_eq!(d::name(db, p, m.get("B::Feature#1")).as_deref(), Some("x"));
    assert_eq!(d::name(db, p, m.root()), None);
}

#[test]
fn derive_element_short_name() {
    // shortName = effectiveShortName()
    let m = model("classifier <a> A; classifier B;");
    let (db, p) = (&m.db, m.project);
    assert_eq!(d::short_name(db, p, m.get("A")).as_deref(), Some("a"));
    assert_eq!(d::short_name(db, p, m.get("B")), None);
}

#[test]
fn element_effective_name() {
    // effectiveName() = declaredName
    let m = model("class A; class <b>;");
    let (db, p) = (&m.db, m.project);
    assert_eq!(d::effective_name(db, p, m.get("A")).as_deref(), Some("A"));
    assert_eq!(d::effective_name(db, p, m.get("b")), None);
}

#[test]
fn element_effective_short_name() {
    // effectiveShortName() = declaredShortName
    let m = model("class <b> B; class A;");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        d::effective_short_name(db, p, m.get("B")).as_deref(),
        Some("b")
    );
    assert_eq!(d::effective_short_name(db, p, m.get("A")), None);
}

#[test]
fn element_escaped_name() {
    // escapedName(): the name, else the short name; a name that is not a basic name is written
    // as an unrestricted name (8.2.2.3)
    let m = model(r"class A; class 'my name'; class <'1.1'>; class 'it\'s'; class;");
    let (db, p) = (&m.db, m.project);
    let escaped = |path: &str| d::escaped_name(db, p, m.get(path));
    assert_eq!(escaped("A").as_deref(), Some("A"));
    assert_eq!(escaped("my name").as_deref(), Some("'my name'"));
    assert_eq!(escaped("1.1").as_deref(), Some("'1.1'"));
    assert_eq!(escaped("it's").as_deref(), Some(r"'it\'s'"));
    assert_eq!(escaped("Class#5"), None);
}

#[test]
fn derive_element_qualified_name() {
    // qualifiedName: null without an owning namespace, or after an earlier owned member with the
    // same name; the escaped name in a root namespace; else owningNamespace::escapedName
    let m = model("package P { class A; namespace N { class 'b c'; } class A; class; }");
    let (db, p) = (&m.db, m.project);
    let q = |x: Global| d::qualified_name(db, p, x).as_deref().map(str::to_owned);
    assert_eq!(q(m.get("P")).as_deref(), Some("P"));
    assert_eq!(q(m.get("P::A")).as_deref(), Some("P::A"));
    assert_eq!(q(m.get("P::N::b c")).as_deref(), Some("P::N::'b c'"));
    assert_eq!(q(m.get("P::Class#2")), None, "the second A");
    assert_eq!(q(m.get("P::Class#3")), None, "no name");
    assert_eq!(q(m.root()), None);
}

#[test]
fn element_path() {
    // path(): the qualified name, else the owning relationship's path, `/` and the position among
    // its owned related elements; for a root namespace, ''
    let m = model("package P { class A; class A; }");
    let (db, p) = (&m.db, m.project);
    assert_eq!(&**d::path(db, p, m.get("P::Class#1")), "P::A");
    // The second A has no qualified name: OwningMembership 2 of P, then its owned element 1.
    assert_eq!(&**d::path(db, p, m.get("P::Class#2")), "P/2/1");
    assert_eq!(&**d::path(db, p, m.root()), "");
}

#[test]
fn relationship_path() {
    // path() of a Relationship without an owning relationship: the owning related element's
    // path, `/` and the position among its owned relationships
    let m = model("package P { class A; feature f : A; }");
    let (db, p) = (&m.db, m.project);
    let typing = m.relationship("P::f", Metaclass::FeatureTyping, 1);
    assert_eq!(&**d::path(db, p, typing), "P::f/1");
    // The root namespace's membership of P: P is named, so it is P's OwningMembership path.
    let unnamed = model("class; class;");
    let second = unnamed.relationship("", Metaclass::OwningMembership, 2);
    assert_eq!(&**d::path(&unnamed.db, unnamed.project, second), "/2");
}

#[test]
fn owning_membership_path() {
    // path() of an OwningMembership: the owned element's qualified name and `/owningMembership`
    let m = model("package P { class A; class A; }");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        &**d::path(db, p, m.membership("P::Class#1")),
        "P::A/owningMembership"
    );
    assert_eq!(&**d::path(db, p, m.membership("P::Class#2")), "P/2");
}

#[test]
fn derive_element_owned_annotation() {
    // ownedAnnotation: the owned Annotations whose annotated element is the element itself
    let m = model("metaclass M; classifier A0; classifier B0; #M dependency X from A0 to B0; class A; comment C about A /* c */");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.labels(d::owned_annotation(db, p, m.get("X"))),
        ["Annotation(X::(MetadataFeature))"]
    );
    assert!(d::owned_annotation(db, p, m.get("C")).is_empty());
}

#[test]
fn derive_element_documentation() {
    // documentation = ownedElement->selectByKind(Documentation)
    let m = model("package P { doc /* d */ class A { doc /* e */ } class B; }");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.labels(d::documentation(db, p, m.get("P"))),
        ["P::(Documentation)"]
    );
    assert_eq!(
        m.labels(d::documentation(db, p, m.get("P::A"))),
        ["P::A::(Documentation)"]
    );
    assert!(d::documentation(db, p, m.get("P::B")).is_empty());
}

#[test]
fn derive_element_textual_representation() {
    // textualRepresentation = ownedElement->selectByKind(TextualRepresentation)
    let m = model(r#"class A { rep language "text" /* a */ } class B;"#);
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.labels(d::textual_representation(db, p, m.get("A"))),
        ["A::(TextualRepresentation)"]
    );
    assert!(d::textual_representation(db, p, m.get("B")).is_empty());
}

#[test]
fn element_library_namespace() {
    // libraryNamespace(): that of the owning relationship; a LibraryPackage is its own
    let m = model("library package L { class A { feature f; } } package Q { class B; }");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.label_of(d::library_namespace(db, p, m.get("L::A::f"))),
        Some("L".into())
    );
    assert_eq!(d::library_namespace(db, p, m.get("Q::B")), None);
}

#[test]
fn relationship_library_namespace() {
    // libraryNamespace() of a Relationship: that of its owning related element
    let m = model("library package L { class A; } package Q { class B; }");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.label_of(d::library_namespace(db, p, m.membership("L::A"))),
        Some("L".into())
    );
    assert_eq!(d::library_namespace(db, p, m.membership("Q::B")), None);
}

#[test]
fn library_package_library_namespace() {
    // libraryNamespace() of a LibraryPackage: itself (8.3.4.13.2)
    let m = model("library package L; standard library package S; package Q;");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.label_of(d::library_namespace(db, p, m.get("L"))),
        Some("L".into())
    );
    assert_eq!(
        m.label_of(d::library_namespace(db, p, m.get("S"))),
        Some("S".into())
    );
    assert_eq!(d::library_namespace(db, p, m.get("Q")), None);
}

#[test]
fn derive_element_is_library_element() {
    // isLibraryElement = libraryNamespace() <> null
    let m = model("library package L { class A; } package Q { class B; }");
    let (db, p) = (&m.db, m.project);
    assert!(d::is_library_element(db, p, m.get("L::A")));
    assert!(!d::is_library_element(db, p, m.get("Q::B")));
}

// ---- Relationship (KerML 8.3.2.1.3) ----

#[test]
fn derive_relationship_related_element() {
    // relatedElement = source->union(target)
    let m = model("classifier A; classifier B; specialization S subtype A specializes B;");
    let (db, p) = (&m.db, m.project);
    assert_eq!(m.labels(d::related_element(db, p, m.get("S"))), ["A", "B"]);
}

// ---- Annotations (KerML 8.3.2.3) ----

#[test]
fn derive_annotation_owned_annotating_element() {
    // ownedAnnotatingElement: the first owned related element that is an AnnotatingElement
    let m = model("metaclass M; classifier A0; classifier B0; #M dependency X from A0 to B0; class A; comment C about A /* c */");
    let (db, p) = (&m.db, m.project);
    let prefix = m.relationship("X", Metaclass::Annotation, 1);
    let about = m.relationship("C", Metaclass::Annotation, 1);
    assert_eq!(
        m.label_of(d::owned_annotating_element(db, p, prefix)),
        Some("X::(MetadataFeature)".into())
    );
    assert_eq!(d::owned_annotating_element(db, p, about), None);
}

#[test]
fn derive_annotation_annotating_element() {
    // annotatingElement: the owned annotating element, else the owning one
    let m = model("metaclass M; classifier A0; classifier B0; #M dependency X from A0 to B0; class A; comment C about A /* c */");
    let (db, p) = (&m.db, m.project);
    let prefix = m.relationship("X", Metaclass::Annotation, 1);
    let about = m.relationship("C", Metaclass::Annotation, 1);
    assert_eq!(
        m.label_of(d::annotating_element(db, p, prefix)),
        Some("X::(MetadataFeature)".into())
    );
    assert_eq!(
        m.label_of(d::annotating_element(db, p, about)),
        Some("C".into())
    );
}

#[test]
fn derive_annotating_element_owned_annotating_relationship() {
    // ownedAnnotatingRelationship: the owned Annotations whose annotated element is not self
    let m = model("metaclass M; classifier A0; classifier B0; #M dependency X from A0 to B0; class A; class B; comment C about A, B /* c */");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.labels(d::owned_annotating_relationship(db, p, m.get("C"))),
        ["Annotation", "Annotation"]
    );
    assert!(d::owned_annotating_relationship(db, p, m.get("X::MetadataFeature#1")).is_empty());
}

#[test]
fn derive_annotating_element_annotation() {
    // annotation: the owning annotating relationship, if any, then the owned ones
    let m = model("metaclass M; classifier A0; classifier B0; #M dependency X from A0 to B0; class A; comment C about A /* c */");
    let (db, p) = (&m.db, m.project);
    assert_eq!(m.labels(d::annotation(db, p, m.get("C"))), ["Annotation"]);
    assert_eq!(
        m.labels(d::annotation(db, p, m.get("X::MetadataFeature#1"))),
        ["Annotation(X::(MetadataFeature))"]
    );
}

#[test]
fn derive_annotating_element_annotated_element() {
    // annotatedElement: the annotations' annotated elements, else the owning namespace
    let m = model(
        "metaclass M; classifier A0; classifier B0; #M dependency X from A0 to B0; package P { class A; comment C about A /* c */ doc /* d */ }",
    );
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.labels(d::annotated_element(db, p, m.get("P::C"))),
        ["P::A"]
    );
    assert_eq!(
        m.labels(d::annotated_element(db, p, m.get("P::Documentation#1"))),
        ["P"]
    );
    assert_eq!(
        m.labels(d::annotated_element(db, p, m.get("X::MetadataFeature#1"))),
        ["X"]
    );
}

// ---- Namespaces (KerML 8.3.2.4) ----

const IMPORTS: &str = "package Q { class B; private class C; }
    package P { class A; private import Q::*; alias X for A; import Q::B; }";

#[test]
fn derive_namespace_owned_membership() {
    // ownedMembership = ownedRelationship->selectByKind(Membership)
    let m = model(IMPORTS);
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.labels(d::owned_membership(db, p, m.get("P"))),
        ["OwningMembership(P::A)", "Membership(X)"]
    );
}

#[test]
fn derive_namespace_owned_member() {
    // ownedMember = ownedMembership->selectByKind(OwningMembership).ownedMemberElement
    let m = model(IMPORTS);
    assert_eq!(
        m.labels(d::owned_member(&m.db, m.project, m.get("P"))),
        ["P::A"]
    );
}

#[test]
fn derive_namespace_owned_import() {
    // ownedImport = ownedRelationship->selectByKind(Import)
    let m = model(IMPORTS);
    assert_eq!(
        m.labels(d::owned_import(&m.db, m.project, m.get("P"))),
        ["NamespaceImport", "MembershipImport"]
    );
}

#[test]
fn derive_namespace_imported_membership() {
    // importedMembership = importedMemberships(Set{}): Q's public B, once
    let m = model(IMPORTS);
    assert_eq!(
        m.labels(d::imported_membership(&m.db, m.project, m.get("P"))),
        ["OwningMembership(Q::B)"]
    );
}

#[test]
fn namespace_imported_memberships() {
    // importedMemberships(excluded) = ownedImport.importedMemberships(excluded->including(self))
    let m = model(IMPORTS);
    let entries = d::namespace_imported_memberships(&m.db, m.project, m.get("P"));
    assert_eq!(
        m.labels(entries.iter().map(|e| e.membership)),
        ["OwningMembership(Q::B)"]
    );
}

#[test]
fn derive_namespace_members() {
    // member = membership.memberElement
    let m = model(IMPORTS);
    assert_eq!(
        m.labels(d::member(&m.db, m.project, m.get("P"))),
        ["P::A", "P::A", "Q::B"]
    );
}

#[test]
fn import_imported_memberships() {
    // Import::importedMemberships(excluded), as each kind of Import redefines it
    let m = model(IMPORTS);
    let (db, p) = (&m.db, m.project);
    let namespace = m.relationship("P", Metaclass::NamespaceImport, 1);
    let membership = m.relationship("P", Metaclass::MembershipImport, 1);
    assert_eq!(
        m.labels(d::import_imported_memberships(db, p, namespace)),
        ["OwningMembership(Q::B)"]
    );
    assert_eq!(
        m.labels(d::import_imported_memberships(db, p, membership)),
        ["OwningMembership(Q::B)"]
    );
}

#[test]
fn namespace_import_imported_memberships() {
    // NamespaceImport: importedNamespace.visibleMemberships(excluded, isRecursive, isImportAll)
    let m = model(
        "package Q { class B; private class C; package R { class D; } }
        package P { import Q::*; } package U { import all Q::*; } package V { import Q::*::**; }",
    );
    let (db, p) = (&m.db, m.project);
    let of = |path: &str| {
        let i = m.relationship(path, Metaclass::NamespaceImport, 1);
        m.labels(d::import_imported_memberships(db, p, i))
    };
    assert_eq!(
        of("P"),
        ["OwningMembership(Q::B)", "OwningMembership(Q::R)"]
    );
    assert_eq!(
        of("U"),
        [
            "OwningMembership(Q::B)",
            "OwningMembership(Q::C)",
            "OwningMembership(Q::R)"
        ]
    );
    assert_eq!(
        of("V"),
        [
            "OwningMembership(Q::B)",
            "OwningMembership(Q::R)",
            "OwningMembership(Q::R::D)"
        ]
    );
}

#[test]
fn membership_import_imported_memberships() {
    // MembershipImport: the importedMembership, and if recursive, the visible memberships of its
    // namespace, recursively
    let m = model("package Q { package R { class D; } } package P { import Q::R; } package U { import Q::R::**; }");
    let (db, p) = (&m.db, m.project);
    let of = |path: &str| {
        let i = m.relationship(path, Metaclass::MembershipImport, 1);
        m.labels(d::import_imported_memberships(db, p, i))
    };
    assert_eq!(of("P"), ["OwningMembership(Q::R)"]);
    assert_eq!(
        of("U"),
        ["OwningMembership(Q::R)", "OwningMembership(Q::R::D)"]
    );
}

#[test]
fn namespace_visible_memberships() {
    // visibleMemberships(excluded, isRecursive, includeAll)
    let m =
        model("package P { private class A; class B; package N { class C; private class D; } }");
    let (db, p) = (&m.db, m.project);
    let v = |r, a| m.labels(d::visible_memberships(db, p, m.get("P"), r, a));
    assert_eq!(
        v(false, false),
        ["OwningMembership(P::B)", "OwningMembership(P::N)"]
    );
    assert_eq!(
        v(false, true),
        [
            "OwningMembership(P::A)",
            "OwningMembership(P::B)",
            "OwningMembership(P::N)"
        ]
    );
    assert_eq!(
        v(true, false),
        [
            "OwningMembership(P::B)",
            "OwningMembership(P::N)",
            "OwningMembership(P::N::C)"
        ]
    );
}

#[test]
fn namespace_memberships_of_visibility() {
    // membershipsOfVisibility(visibility, excluded): owned memberships and imported ones of that
    // visibility; all for null
    let m = model(
        "package Q { class B; } package P { private class A; class C; private import Q::*; }",
    );
    let (db, p) = (&m.db, m.project);
    let of = |v| m.labels(d::memberships_of_visibility(db, p, m.get("P"), v));
    assert_eq!(
        of(Some(Visibility::Private)),
        ["OwningMembership(P::A)", "OwningMembership(Q::B)"]
    );
    assert_eq!(of(Some(Visibility::Public)), ["OwningMembership(P::C)"]);
    assert_eq!(
        of(None),
        [
            "OwningMembership(P::A)",
            "OwningMembership(P::C)",
            "OwningMembership(Q::B)"
        ]
    );
}

#[test]
fn namespace_names_of() {
    // namesOf(element): the short names and names of the memberships of that element
    let m = model("package P { class <b> B; alias X for B; class C; }");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        strings(d::names_of(db, p, m.get("P"), m.get("P::B"))),
        ["b", "B", "X"]
    );
    assert_eq!(
        strings(d::names_of(db, p, m.get("P"), m.get("P::C"))),
        ["C"]
    );
}

#[test]
fn namespace_visibility_of() {
    // visibilityOf(mem): the importing import's for an imported membership; the membership's own
    // for another; else private
    let m = model(
        "package Q { class B; class Z; } package P { protected class A; public import Q::B; }
        package O { class E; }",
    );
    let (db, p) = (&m.db, m.project);
    let ns = m.get("P");
    assert_eq!(
        d::visibility_of(db, p, ns, m.membership("Q::B")),
        Visibility::Public
    );
    assert_eq!(
        d::visibility_of(db, p, ns, m.membership("P::A")),
        Visibility::Protected
    );
    assert_eq!(
        d::visibility_of(db, p, ns, m.membership("O::E")),
        Visibility::Private
    );
}

#[test]
fn membership_is_distinguishable_from() {
    // isDistinguishableFrom(other): different names, or metaclasses that do not conform
    let m = model(
        "package A { class X; class Y; } package B { class X; feature Y; classifier <Y> Z; }",
    );
    let (db, p) = (&m.db, m.project);
    let dist =
        |a: &str, b: &str| d::is_distinguishable_from(db, p, m.membership(a), m.membership(b));
    assert!(!dist("A::X", "B::X"), "two classes named X");
    assert!(dist("A::X", "A::Y"), "different names");
    assert!(dist("A::Y", "B::Y"), "a class and a feature named Y");
    assert!(
        !dist("A::Y", "B::Z"),
        "a class Y and a classifier with the short name Y"
    );
}

#[test]
fn derive_owning_membership_owned_member_name() {
    // ownedMemberName = ownedMemberElement.name
    let m = model("package P { class <x> X; class; }");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        d::owned_member_name(db, p, m.membership("P::X")).as_deref(),
        Some("X")
    );
    assert_eq!(
        d::owned_member_name(db, p, m.membership("P::Class#2")),
        None
    );
}

#[test]
fn derive_owning_membership_owned_member_short_name() {
    // ownedMemberShortName = ownedMemberElement.shortName
    let m = model("package P { class <x> X; class Y; }");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        d::owned_member_short_name(db, p, m.membership("P::X")).as_deref(),
        Some("x")
    );
    assert_eq!(
        d::owned_member_short_name(db, p, m.membership("P::Y")),
        None
    );
}

#[test]
fn derive_namespace_import_imported_element() {
    // importedElement = importedNamespace
    let m = model("package Q { class B; } package P { import Q::*; import Q::B; }");
    let i = m.relationship("P", Metaclass::NamespaceImport, 1);
    assert_eq!(
        m.label_of(d::imported_element(&m.db, m.project, i)),
        Some("Q".into())
    );
}

#[test]
fn derive_membership_import_imported_element() {
    // importedElement = importedMembership.memberElement
    let m = model("package Q { class B; } package P { import Q::*; import Q::B; }");
    let i = m.relationship("P", Metaclass::MembershipImport, 1);
    assert_eq!(
        m.label_of(d::imported_element(&m.db, m.project, i)),
        Some("Q::B".into())
    );
}

// ---- string resolution (KerML 8.3.2.4.5) ----

const NAMES: &str = "package Q { class B; private class C; }
    package P { class A; namespace N { class D; } }";

#[test]
fn namespace_resolve_global() {
    // resolveGlobal(qualifiedName): in the global namespace (8.2.3.5.2)
    let m = model(NAMES);
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.label_of(d::resolve_global(db, p, "Q")),
        Some("OwningMembership(Q)".into())
    );
    assert_eq!(
        m.label_of(d::resolve_global(db, p, "Q::B")),
        Some("OwningMembership(Q::B)".into())
    );
    assert_eq!(d::resolve_global(db, p, "Q::C"), None);
}

#[test]
fn namespace_resolve_local() {
    // resolveLocal(name): a membership with the name, else in the owning namespace; for a root
    // namespace, the global namespace
    let m = model(NAMES);
    let (db, p) = (&m.db, m.project);
    let n = m.get("P::N");
    assert_eq!(
        m.label_of(d::resolve_local(db, p, n, "D")),
        Some("OwningMembership(P::N::D)".into())
    );
    assert_eq!(
        m.label_of(d::resolve_local(db, p, n, "A")),
        Some("OwningMembership(P::A)".into())
    );
    assert_eq!(
        m.label_of(d::resolve_local(db, p, n, "Q")),
        Some("OwningMembership(Q)".into())
    );
    assert_eq!(d::resolve_local(db, p, n, "B"), None);
}

#[test]
fn namespace_resolve_visible() {
    // resolveVisible(name): a visible membership with the name
    let m = model(NAMES);
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.label_of(d::resolve_visible(db, p, m.get("Q"), "B")),
        Some("OwningMembership(Q::B)".into())
    );
    assert_eq!(d::resolve_visible(db, p, m.get("Q"), "C"), None);
}

#[test]
fn namespace_resolve() {
    // resolve(qualifiedName): locally, globally after `$`, or visibly in the qualification
    let m = model(NAMES);
    let (db, p) = (&m.db, m.project);
    let n = m.get("P::N");
    assert_eq!(
        m.label_of(d::resolve(db, p, n, "A")),
        Some("OwningMembership(P::A)".into())
    );
    assert_eq!(
        m.label_of(d::resolve(db, p, n, "Q::B")),
        Some("OwningMembership(Q::B)".into())
    );
    assert_eq!(
        m.label_of(d::resolve(db, p, n, "$::P")),
        Some("OwningMembership(P)".into())
    );
    assert_eq!(d::resolve(db, p, n, "Q::C"), None, "private");
    assert_eq!(d::resolve(db, p, n, "P::A::x"), None, "A has no x");
}

#[test]
fn namespace_qualification_of() {
    // qualificationOf(qualifiedName): without the last segment
    assert_eq!(d::qualification_of("A::B::C").as_deref(), Some("A::B"));
    assert_eq!(d::qualification_of("'x y'::z").as_deref(), Some("'x y'"));
    assert_eq!(d::qualification_of("$::A").as_deref(), Some("$"));
    assert_eq!(d::qualification_of("A"), None);
}

#[test]
fn namespace_unqualified_name_of() {
    // unqualifiedNameOf(qualifiedName): the last segment, unescaped
    assert_eq!(d::unqualified_name_of("A::B::C").as_deref(), Some("C"));
    assert_eq!(
        d::unqualified_name_of(r"A::'it\'s'").as_deref(),
        Some("it's")
    );
    assert_eq!(d::unqualified_name_of("A").as_deref(), Some("A"));
    assert_eq!(d::unqualified_name_of("A::"), None);
}

// ---- regressions from review ----

#[test]
fn derive_relationship_related_element_with_derived_ends() {
    // relatedElement = source->union(target), with the ends each metaclass derives
    let m = model(
        "classifier A; classifier B; package Q; package P { class C; import Q::*; comment K about C /* k */ }
        metaclass M; #M dependency X from A to B;",
    );
    let (db, p) = (&m.db, m.project);
    let related = |x: Global| m.labels(d::related_element(db, p, x));
    // membershipOwningNamespace and ownedMemberElement.
    assert_eq!(related(m.membership("P::C")), ["P", "P::C"]);
    // importOwningNamespace and importedNamespace.
    assert_eq!(
        related(m.relationship("P", Metaclass::NamespaceImport, 1)),
        ["P", "Q"]
    );
    // An Annotation: its annotating element, owning or owned, and its annotated element.
    assert_eq!(
        related(m.relationship("P::K", Metaclass::Annotation, 1)),
        ["P::K", "P::C"]
    );
    assert_eq!(
        related(m.relationship("X", Metaclass::Annotation, 1)),
        ["X::(MetadataFeature)", "X"]
    );
}

#[test]
fn a_membership_two_imports_bring_is_one_membership() {
    // member = membership.memberElement, and importedMembership, once per membership
    let m = model("package Q { class X; } package P { public import Q::*; private import Q::*; }");
    let (db, p) = (&m.db, m.project);
    assert_eq!(m.labels(d::member(db, p, m.get("P"))), ["Q::X"]);
    assert_eq!(
        m.labels(d::imported_membership(db, p, m.get("P"))),
        ["OwningMembership(Q::X)"]
    );
}

#[test]
fn visibility_of_an_import_excluded_by_a_collision_is_private() {
    // visibilityOf(mem): `importedMembership->includes(mem)` is tested first; Q::X collides with
    // P's own X, so it is not imported (KerML 8.3.2.4.5; ADR 0020)
    let m = model("package Q { class X; } package P { class X; public import Q::*; }");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        d::visibility_of(db, p, m.get("P"), m.membership("Q::X")),
        Visibility::Private
    );
}

#[test]
fn memberships_of_visibility_take_each_imports_own_memberships() {
    // membershipsOfVisibility: `ownedImport->select(…).importedMemberships(…)`, which excludes no
    // collisions; the namespace's importedMembership excludes both X
    let m = model("package A { class X; } package B { class X; } package P { public import A::*; public import B::*; }");
    let (db, p) = (&m.db, m.project);
    assert_eq!(
        m.labels(d::memberships_of_visibility(
            db,
            p,
            m.get("P"),
            Some(Visibility::Public)
        )),
        ["OwningMembership(A::X)", "OwningMembership(B::X)"]
    );
    assert!(d::imported_membership(db, p, m.get("P")).is_empty());
}

#[test]
fn qualified_names_as_text_may_have_white_space_between_tokens() {
    // KerML 8.2.2.1: white space separates tokens; `::` itself is one token
    assert_eq!(d::qualification_of("A :: B").as_deref(), Some("A"));
    assert_eq!(d::unqualified_name_of("$ :: A").as_deref(), Some("A"));
    assert_eq!(
        d::qualification_of(" 'x y' ::\n z ").as_deref(),
        Some("'x y'")
    );
    assert_eq!(d::unqualified_name_of("A : : B"), None);
    let m = model("package Q { class B; } package P;");
    let found = d::resolve(&m.db, m.project, m.get("P"), "Q :: B");
    assert_eq!(m.label_of(found), Some("OwningMembership(Q::B)".into()));
}
