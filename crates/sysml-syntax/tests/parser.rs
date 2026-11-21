// SPDX-License-Identifier: Apache-2.0
//! The event-based parser framework (ADR 0011) on the root namespace. The productions are tested
//! in `grammar.rs`.

use sysml_syntax::{parse, Language, Severity, SyntaxKind as K};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[test]
fn an_empty_root_namespace() {
    let p = parse(Language::KerML, "");
    assert_eq!(p.syntax().kind(), K::RootNamespace);
    assert_eq!(p.syntax().text().to_string(), "");
    assert!(p.diagnostics().is_empty());
}

#[test]
fn a_root_namespace_of_trivia_only() {
    let text = "  // a note\n//* a\nmultiline note */\n\t\r\n";
    for lang in [Language::KerML, Language::SysML] {
        let p = parse(lang, text);
        assert_eq!(p.syntax().kind(), K::RootNamespace);
        assert_eq!(p.syntax().text().to_string(), text);
        assert!(p.diagnostics().is_empty(), "{:?}", p.diagnostics());
        assert!(
            p.syntax().children().next().is_none(),
            "trivia is tokens, not nodes"
        );
    }
}

#[test]
fn unexpected_tokens_are_wrapped_in_an_error_node_and_reported_once() {
    // Tokens that cannot start a member (KerML 8.2.3.4.1).
    let text = "// note\n+ - 42\n";
    let p = parse(Language::KerML, text);
    assert_eq!(p.syntax().text().to_string(), text, "lossless");
    let errors: Vec<_> = p
        .syntax()
        .children()
        .filter(|n| n.kind() == K::Error)
        .collect();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].text().to_string().trim_end(), "+ - 42");
    assert_eq!(p.diagnostics().len(), 1);
    let d = &p.diagnostics()[0];
    assert_eq!(d.code, "syntax::expected-member");
    assert_eq!(d.severity, Severity::Error);
    assert_eq!(
        u32::from(d.range.start()),
        8,
        "at the first unexpected token, after the note"
    );
}

#[test]
fn a_leading_byte_order_mark_parses_cleanly() {
    let p = parse(Language::SysML, "\u{feff}// note\n");
    assert!(p.diagnostics().is_empty(), "{:?}", p.diagnostics());
    assert_eq!(p.syntax().text().to_string(), "\u{feff}// note\n");
}

#[test]
fn lexical_diagnostics_are_reported_by_the_parser() {
    let p = parse(Language::KerML, "/* open");
    assert_eq!(p.syntax().text().to_string(), "/* open");
    assert!(p
        .diagnostics()
        .iter()
        .any(|d| d.code == "syntax::unterminated-comment"));
}

#[test]
fn every_corpus_source_parses_losslessly() {
    let corpus = sysml_corpus::Corpus::load().unwrap();
    for s in corpus.sources() {
        let lang = match s.language {
            sysml_corpus::Language::KerML => Language::KerML,
            sysml_corpus::Language::SysML => Language::SysML,
        };
        let p = parse(lang, &s.text);
        assert_eq!(p.syntax().text().to_string(), s.text, "{}", s.id);
    }
}
