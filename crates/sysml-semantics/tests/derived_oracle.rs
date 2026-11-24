// SPDX-License-Identifier: Apache-2.0
//! The library element ids (KerML 9.1; `sysml_semantics::identity`, step 1a.11), and through them
//! `path()`, `qualifiedName` and `escapedName` (KerML 8.3.2), on the KerML libraries, against the
//! element ids of the OMG pilot implementation's XMI (ADR 0009): KerML 9.1 makes the id of each
//! element of a library package a v5 UUID of the package's id and the element's `path()`, and that
//! of the package a v5 UUID of its URL. So if our ids are right, so are our paths, for every
//! element, including those named only by an implied redefinition (ADR 0020).
//!
//! Elements are matched by their position in the ownership walk of their file, on both sides
//! (`tests/oracle.rs` shows the trees are equal). Root namespaces are outside KerML 9.1.

#[path = "../../sysml-model/xmi.rs"]
#[allow(dead_code)]
mod xmi;

use sysml_model::{ElementIx, FileId, Graph};
use sysml_semantics::{derived, identity, lowered, Database, Global, Project, SourceFile};
use sysml_syntax::Language;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

fn walk(g: &Graph, x: ElementIx, out: &mut Vec<ElementIx>) {
    out.push(x);
    for r in g.owned_relationships(x) {
        walk(g, *r, out);
    }
    for e in g.owned_related_elements(x) {
        walk(g, *e, out);
    }
}

fn walk_xmi<'a>(n: &'a xmi::Node, out: &mut Vec<&'a xmi::Node>) {
    out.push(n);
    for c in n.children("ownedRelationship") {
        walk_xmi(c, out);
    }
    for c in n.children("ownedRelatedElement") {
        walk_xmi(c, out);
    }
}

#[test]
fn every_kerml_library_element_has_the_pilots_element_id() {
    let corpus = sysml_corpus::Corpus::load().unwrap();
    let db = Database::default();
    let sources: Vec<_> = corpus
        .sources()
        .filter(|s| s.language == sysml_corpus::Language::KerML)
        .collect();
    let files: Vec<SourceFile> = sources
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let id = FileId(i as u32);
            SourceFile::new(&db, id, s.id.clone(), Language::KerML, s.text.clone())
        })
        .collect();
    let project = Project::new(&db, files.clone());

    let (mut same, mut different) = (0, Vec::new());
    for (fi, f) in files.iter().enumerate() {
        let bytes = sysml_corpus::read_spec(&sysml_corpus::kerml_library_xmi(&sources[fi].id));
        let document = xmi::parse(std::str::from_utf8(&bytes.unwrap()).unwrap());
        let mut theirs = Vec::new();
        walk_xmi(&document, &mut theirs);
        let l = lowered(&db, *f);
        let g = &l.graph;
        let mut ours = Vec::new();
        walk(g, l.root, &mut ours);
        assert_eq!(ours.len(), theirs.len(), "{}", sources[fi].id);
        let at = |e: ElementIx| Global {
            file: *f,
            index: e.index() as u32,
        };
        // The top-level library packages, by their position in the walk, with their ids.
        for (i, x) in ours.iter().enumerate() {
            let element = at(*x);
            // Elements without a KerML 9.1 id are skipped: the root namespaces and their
            // memberships of the packages. The exact count below catches any other element
            // that loses its id, as one of a package no longer lowered as standard would.
            let Some(expected) = identity::element_id(&db, project, element) else {
                continue;
            };
            let id = theirs[i].attr("elementId").unwrap_or_default();
            if id == expected.to_string() {
                same += 1;
            } else {
                different.push(format!(
                    "{}: {} `{}`: pilot {id}, ours {expected}",
                    sources[fi].id,
                    g.metaclass(*x).name(),
                    derived::path(&db, project, element)
                ));
            }
        }
    }
    assert!(
        // 24,931 elements, without the 36 root namespaces and their memberships of the packages.
        different.is_empty() && same == 24_859,
        "{same} ids as the pilot's; {} different:\n{}",
        different.len(),
        different
            .iter()
            .take(40)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
