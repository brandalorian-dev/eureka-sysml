// SPDX-License-Identifier: Apache-2.0
//! The lowering of the 36 KerML library files against the abstract syntax the OMG pilot
//! implementation built from them: its XMI serialization (the oracle of ADR 0009), used as a
//! behavioral reference (CLAUDE.md).
//!
//! Both are rendered as canonical lines, one per element in ownership order: the depth, the role
//! (`rel` for an `ownedRelationship`, `el` for an `ownedRelatedElement`), the metaclass, the
//! attributes that differ from their default, and which relationship ends are present. The values
//! of the ends are left out: resolving names is step 1a.7. The differences that come from a
//! reading of the spec are listed, with their reason, in ADR 0018 and in `ACCEPTED` below.

#[path = "../../sysml-model/xmi.rs"]
#[allow(dead_code)]
mod xmi;

use sysml_model::{Attr, ElementIx, FileId, Graph, Metaclass, PropertyKind, Slot, Value};
use sysml_syntax::{parse, Language};
use uuid::Uuid;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// The canonical line of an element: its attributes and ends.
fn line(
    depth: usize,
    role: &str,
    metaclass: &str,
    attrs: Vec<(String, String)>,
    ends: Vec<String>,
) -> String {
    let mut attrs = attrs;
    attrs.sort();
    let mut ends = ends;
    ends.sort();
    ends.dedup();
    let attrs: Vec<String> = attrs
        .into_iter()
        .map(|(k, v)| format!("{k}={v:?}"))
        .collect();
    format!(
        "{}{role} {metaclass}{}{}",
        "  ".repeat(depth),
        attrs.iter().map(|a| format!(" {a}")).collect::<String>(),
        ends.iter().map(|e| format!(" ->{e}")).collect::<String>()
    )
}

/// Whether a value is the metamodel default, or the implicit default of the XMI serialization
/// (EMF leaves out `false`, `0` and unset values).
fn is_default(metaclass: Metaclass, name: &str, value: &str) -> bool {
    let default = metaclass
        .stored_attributes()
        .iter()
        .find(|a| a.name == name)
        .and_then(|a| a.default);
    match default {
        Some(d) => d == value,
        None => value == "false" || value == "0" || value == "0.0",
    }
}

fn kind(metaclass: Metaclass, name: &str) -> Option<PropertyKind> {
    metaclass
        .stored_attributes()
        .iter()
        .find(|a| a.name == name)
        .map(|a| a.kind)
}

/// A Real as the oracle and our values both print it.
fn real(text: &str) -> String {
    text.parse::<f64>()
        .map_or_else(|_| text.to_owned(), |r| format!("{r:?}"))
}

fn ours(g: &Graph, ix: ElementIx, depth: usize, role: &str, out: &mut Vec<(String, ElementIx)>) {
    let m = g.metaclass(ix);
    let mut attrs = Vec::new();
    for attr in Attr::ALL {
        for v in g.values(ix, *attr) {
            let text = match v {
                Value::Boolean(b) => b.to_string(),
                Value::String(s) => s.to_string(),
                Value::Integer(i) => i.to_string(),
                Value::Real(r) => format!("{r:?}"),
                Value::Enumeration(e) => (*e).to_owned(),
            };
            if !is_default(m, attr.name(), &text) {
                attrs.push((attr.name().to_owned(), text));
            }
        }
    }
    // The name of an end is the most specific stored attribute for its slot (`type` for the
    // target of a FeatureTyping).
    let end_name = |slot: Slot| {
        m.stored_attributes()
            .iter()
            .find(|a| a.slot == slot)
            .map(|a| a.name.to_owned())
    };
    let mut ends = Vec::new();
    if !g.source(ix).is_empty() {
        ends.extend(end_name(Slot::Source));
    }
    if !g.target(ix).is_empty() {
        ends.extend(end_name(Slot::Target));
    }
    out.push((line(depth, role, m.name(), attrs, ends), ix));
    for r in g.owned_relationships(ix) {
        ours(g, *r, depth + 1, "rel", out);
    }
    for e in g.owned_related_elements(ix) {
        ours(g, *e, depth + 1, "el", out);
    }
}

fn theirs(node: &xmi::Node, depth: usize, role: &str, out: &mut Vec<String>) {
    let name = node
        .attr("xsi:type")
        .unwrap_or(&node.tag)
        .trim_start_matches("sysml:");
    let m = Metaclass::parse(name).unwrap_or_else(|| panic!("unknown metaclass {name}"));
    let mut attrs = Vec::new();
    let mut ends = Vec::new();
    for (k, v) in &node.attrs {
        if k.contains(':') || k == "elementId" {
            continue;
        }
        match kind(m, k) {
            Some(PropertyKind::Reference { .. }) => ends.push(k.clone()),
            Some(PropertyKind::Real) => attrs.push((k.clone(), real(v))),
            _ if is_default(m, k, v) => {}
            _ => attrs.push((k.clone(), v.clone())),
        }
    }
    for c in &node.children {
        if c.attr("href").is_some() {
            ends.push(c.tag.clone());
        }
    }
    out.push(line(depth, role, m.name(), attrs, ends));
    for c in node.children("ownedRelationship") {
        theirs(c, depth + 1, "rel", out);
    }
    for c in node.children("ownedRelatedElement") {
        theirs(c, depth + 1, "el", out);
    }
}

/// Differences between the lowering and the oracle that come from a reading of the spec
/// (ADR 0018): what the oracle's line has, what the lowering has instead, and why. The oracle's
/// lines are rewritten by these before the comparison, so that any other difference still fails.
const ACCEPTED: &[(&str, &str, &str)] = &[(
    "rel ParameterMembership visibility=\"private\"",
    "rel ParameterMembership",
    "the pilot makes the membership of an argument private; KerML 8.2.5.8.1 (ArgumentMember) \
     sets no visibility, so it is the default, public",
), (
    "el Feature declaredName=\"\"",
    "el Feature",
    "the pilot names the sourceTargetFeature of a FeatureChainExpression with the empty string; \
     KerML 8.3.4.8.4 gives it no name",
)];

/// The same for our lines: what the lowering has, what the oracle has instead, and why.
const ACCEPTED_OURS: &[(&str, &str, &str)] = &[(
    "\\r\\n",
    "\\n",
    "a comment body keeps its line terminators as entered (KerML 8.2.3.3.2, Note 2), so a CR LF \
     stays one; the pilot stores an LF",
)];

#[test]
fn the_kerml_library_lowers_as_the_oracle() {
    let corpus = sysml_corpus::Corpus::load().unwrap();
    let mut failures = Vec::new();
    let mut files = 0;
    for s in corpus.sources() {
        if s.language != sysml_corpus::Language::KerML {
            continue;
        }
        files += 1;
        let p = parse(Language::KerML, &s.text);
        let mut n = 0u128;
        let lowered = sysml_semantics::lower(FileId(0), &p.syntax(), &mut || {
            n += 1;
            Uuid::from_u128(n)
        });
        assert!(
            lowered.diagnostics.is_empty(),
            "{}: {:?}",
            s.id,
            lowered.diagnostics
        );
        let mut lines = Vec::new();
        ours(&lowered.graph, lowered.root, 0, "root", &mut lines);
        let (mut got, elements): (Vec<String>, Vec<ElementIx>) = lines.into_iter().unzip();
        for line in &mut got {
            for (ours, theirs, _) in ACCEPTED_OURS {
                if line.contains(ours) {
                    *line = line.replace(ours, theirs);
                }
            }
        }

        let xml = sysml_corpus::read_spec(&sysml_corpus::kerml_library_xmi(&s.id)).unwrap();
        let root = xmi::parse(std::str::from_utf8(&xml).unwrap());
        let mut expected = Vec::new();
        theirs(&root, 0, "root", &mut expected);

        for line in &mut expected {
            for (theirs, ours, _) in ACCEPTED {
                if line.contains(theirs) {
                    *line = line.replace(theirs, ours);
                }
            }
        }
        let first = got
            .iter()
            .zip(&expected)
            .position(|(g, e)| g != e)
            .or_else(|| (got.len() != expected.len()).then(|| got.len().min(expected.len())));
        if let Some(i) = first {
            let from = i.saturating_sub(6);
            // The source text of the nearest element of ours before the difference.
            let text = elements[..i.min(elements.len())]
                .iter()
                .rev()
                .find_map(|e| lowered.graph.span(*e))
                .map(|sp| {
                    s.text[sp.start as usize..sp.end as usize]
                        .chars()
                        .take(160)
                        .collect::<String>()
                })
                .unwrap_or_default();
            failures.push(format!(
                "{} (element {i} of {}), near `{}`:\n  expected:\n{}\n  got:\n{}",
                s.id,
                expected.len(),
                text.replace('\n', " "),
                expected[from..(i + 2).min(expected.len())].join("\n"),
                got[from..(i + 2).min(got.len())].join("\n")
            ));
        }
    }
    assert_eq!(files, 36);
    assert!(
        failures.is_empty(),
        "{} of 36 files differ:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
