// SPDX-License-Identifier: Apache-2.0
//! Helpers shared by the grammar tests: the tree snapshot format.
#![allow(dead_code)] // each test binary uses some of the helpers

use expect_test::Expect;
use sysml_syntax::{parse, Language, SyntaxNode};

pub fn dump(node: &SyntaxNode, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    let tokens = |n: &SyntaxNode| -> Vec<String> {
        n.children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| !t.kind().is_trivia())
            .map(|t| format!("{:?}", t.text()))
            .collect()
    };
    if node.children().next().is_none() {
        let t = tokens(node);
        out.push_str(&format!("{pad}{:?}", node.kind()));
        if !t.is_empty() {
            out.push(' ');
            out.push_str(&t.join(" "));
        }
        out.push('\n');
        return;
    }
    out.push_str(&format!("{pad}{:?}\n", node.kind()));
    for e in node.children_with_tokens() {
        match e {
            rowan::NodeOrToken::Node(n) => dump(&n, indent + 1, out),
            rowan::NodeOrToken::Token(t) if !t.kind().is_trivia() => {
                out.push_str(&format!("{pad}  {:?}\n", t.text()));
            }
            rowan::NodeOrToken::Token(_) => {}
        }
    }
}

/// Parse `text` as KerML, check that the tree is lossless, and compare it with the snapshot.
pub fn check(text: &str, expect: Expect) {
    let p = parse(Language::KerML, text);
    assert_eq!(p.syntax().text().to_string(), text, "lossless");
    let mut out = String::new();
    for child in p.syntax().children() {
        dump(&child, 0, &mut out);
    }
    for d in p.diagnostics() {
        out.push_str(&format!(
            "{} at {:?} {:?}: {}\n",
            d.code, d.range, &text[d.range], d.message
        ));
    }
    expect.assert_eq(&out);
}

/// The diagnostic codes of `text`.
pub fn codes(text: &str) -> Vec<&'static str> {
    parse(Language::KerML, text)
        .diagnostics()
        .iter()
        .map(|d| d.code)
        .collect()
}
