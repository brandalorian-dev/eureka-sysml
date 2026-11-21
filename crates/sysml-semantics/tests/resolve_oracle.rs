// SPDX-License-Identifier: Apache-2.0
//! Name resolution on the KerML libraries against the OMG pilot implementation (step 1a.7): every
//! name must resolve to the element the pilot resolved it to, in its XMI serialization (ADR 0009).
//!
//! The lowered trees equal the XMI's element by element (`tests/oracle.rs`), so an element is
//! identified by its position in the ownership walk of its file, on both sides. An XMI reference
//! is an `elementId` in the same file or an `href` to `<file>#<elementId>`.

#[path = "../../sysml-model/xmi.rs"]
#[allow(dead_code)]
mod xmi;

use std::collections::HashMap;
use sysml_model::{ElementIx, FileId, Graph};
use sysml_semantics::{lowered, resolved_names, Database, Global, Project, SourceFile};
use sysml_syntax::Language;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// Our elements in ownership order: owned relationships, then owned related elements.
fn walk(g: &Graph, x: ElementIx, out: &mut Vec<ElementIx>) {
    out.push(x);
    for r in g.owned_relationships(x) {
        walk(g, *r, out);
    }
    for e in g.owned_related_elements(x) {
        walk(g, *e, out);
    }
}

/// The XMI elements in the same order.
fn walk_xmi<'a>(n: &'a xmi::Node, out: &mut Vec<&'a xmi::Node>) {
    out.push(n);
    for c in n.children("ownedRelationship") {
        walk_xmi(c, out);
    }
    for c in n.children("ownedRelatedElement") {
        walk_xmi(c, out);
    }
}

fn id(n: &xmi::Node) -> &str {
    n.attr("elementId").or(n.attr("xmi:id")).unwrap_or_default()
}

#[test]
fn every_kerml_library_name_resolves_as_in_the_pilot() {
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
            SourceFile::new(
                &db,
                FileId(i as u32),
                s.id.clone(),
                Language::KerML,
                s.text.clone(),
            )
        })
        .collect();
    let project = Project::new(&db, files.clone());

    // Each file's XMI, its walk, and an index from elementId to (file, position).
    let documents: Vec<xmi::Node> = sources
        .iter()
        .map(|s| {
            let bytes = sysml_corpus::read_spec(&sysml_corpus::kerml_library_xmi(&s.id)).unwrap();
            xmi::parse(std::str::from_utf8(&bytes).unwrap())
        })
        .collect();
    let walks: Vec<Vec<&xmi::Node>> = documents
        .iter()
        .map(|d| {
            let mut w = Vec::new();
            walk_xmi(d, &mut w);
            w
        })
        .collect();
    let mut by_id: HashMap<&str, (usize, usize)> = HashMap::new();
    for (f, w) in walks.iter().enumerate() {
        for (i, n) in w.iter().enumerate() {
            by_id.insert(id(n), (f, i));
        }
    }
    // The XMI file name of each source, for hrefs.
    let file_of_name: HashMap<String, usize> = sources
        .iter()
        .enumerate()
        .map(|(i, s)| (format!("{}x", s.id.rsplit('/').next().unwrap()), i))
        .collect();

    let mut ours_position: Vec<HashMap<u32, usize>> = Vec::new();
    for f in &files {
        let l = lowered(&db, *f);
        let mut w = Vec::new();
        walk(&l.graph, l.root, &mut w);
        ours_position.push(
            w.iter()
                .enumerate()
                .map(|(i, x)| (x.index() as u32, i))
                .collect(),
        );
    }
    let position = |g: Global| -> (usize, usize) {
        let f = files.iter().position(|x| *x == g.file).unwrap();
        (f, ours_position[f][&g.index])
    };

    let (mut same, mut different, mut unresolved) = (0, Vec::new(), Vec::new());
    for (fi, f) in files.iter().enumerate() {
        let l = lowered(&db, *f);
        let g = &l.graph;
        for n in resolved_names(&db, project, *f) {
            let rel = ElementIx::from_index(n.relationship as usize);
            let m = g.metaclass(rel);
            let attr = m
                .stored_attributes()
                .iter()
                .find(|a| a.slot == n.slot)
                .map(|a| a.name)
                .unwrap();
            let node = walks[fi][ours_position[fi][&(n.relationship)]];
            // The pilot's reference: an attribute of ids, or a child with an href.
            let expected: Option<(usize, usize)> = node
                .attr(attr)
                .and_then(|ids| ids.split_whitespace().nth(n.position as usize))
                .and_then(|i| by_id.get(i).copied())
                .or_else(|| {
                    let href = node.children(attr).nth(n.position as usize)?.attr("href")?;
                    let (file, element) = href.split_once('#')?;
                    // The file must be one of the libraries; ids are unique across them.
                    file_of_name.get(file.rsplit('/').next()?)?;
                    by_id.get(element).copied()
                });
            let describe = |(f, i): (usize, usize)| {
                let x = walks[f][i];
                format!(
                    "{} {} in {}",
                    x.attr("xsi:type")
                        .unwrap_or("?")
                        .trim_start_matches("sysml:"),
                    x.attr("declaredName").unwrap_or("(unnamed)"),
                    sources[f].id
                )
            };
            let text = &sources[fi].text;
            let site = || {
                let span = g.span(rel).unwrap();
                let line = text[..span.start as usize].matches('\n').count() + 1;
                format!("{}:{line} {} `{attr}`", sources[fi].id, m.name())
            };
            match (n.target.map(position), expected) {
                (Some(ours), Some(theirs)) if ours == theirs => same += 1,
                (Some(ours), Some(theirs)) => different.push(format!(
                    "{}: ours {}, the pilot's {}",
                    site(),
                    describe(ours),
                    describe(theirs)
                )),
                (None, Some(theirs)) => {
                    unresolved.push(format!("{}: the pilot's {}", site(), describe(theirs)))
                }
                (Some(_), None) | (None, None) => {
                    different.push(format!("{}: no reference in the XMI", site()))
                }
            }
        }
    }
    assert!(
        different.is_empty() && unresolved.is_empty(),
        "{same} names as the pilot; {} different, {} unresolved:\n{}\n---\n{}",
        different.len(),
        unresolved.len(),
        different
            .iter()
            .take(40)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n"),
        unresolved
            .iter()
            .take(80)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n"),
    );
}
