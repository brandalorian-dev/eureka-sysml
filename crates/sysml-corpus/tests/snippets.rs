// SPDX-License-Identifier: Apache-2.0
//! The spec snippets (step 1a.5, ADR 0016): the textual examples of the KerML and SysML
//! specifications, extracted by `scripts/extract_spec_snippets.py`.

use std::collections::BTreeMap;
use sysml_corpus::{snippets, Expectation, Language, Snippet};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// The number of snippets of `spec` per clause, to two levels (for example `7.2`).
fn counts(all: &[Snippet], spec: &str) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<Vec<u32>, (String, usize)> = BTreeMap::new();
    for s in all.iter().filter(|s| s.spec == spec) {
        let clause: Vec<&str> = s.clause.split('.').take(2).collect();
        let key: Vec<u32> = clause.iter().map(|c| c.parse().unwrap()).collect();
        counts.entry(key).or_insert((clause.join("."), 0)).1 += 1;
    }
    counts.into_values().collect()
}

#[test]
fn the_snippets_per_clause() {
    // The extraction rules are ADR 0016; a change to them, or to the PDFs, changes these.
    let all = snippets().unwrap();
    let kerml: Vec<(&str, usize)> = vec![("7.2", 20), ("7.3", 53), ("7.4", 85), ("8.4", 63)];
    let sysml: Vec<(&str, usize)> = vec![
        ("7.2", 1),
        ("7.3", 3),
        ("7.4", 14),
        ("7.5", 39),
        ("7.6", 69),
        ("7.7", 8),
        ("7.8", 7),
        ("7.9", 33),
        ("7.10", 6),
        ("7.11", 22),
        ("7.12", 15),
        ("7.13", 42),
        ("7.14", 31),
        ("7.15", 13),
        ("7.16", 19),
        ("7.17", 156),
        ("7.18", 46),
        ("7.19", 11),
        ("7.20", 15),
        ("7.21", 40),
        ("7.22", 1),
        ("7.23", 15),
        ("7.24", 21),
        ("7.25", 22),
        ("7.26", 30),
        ("7.27", 23),
    ];
    let owned = |v: Vec<(&str, usize)>| -> Vec<(String, usize)> {
        v.into_iter().map(|(c, n)| (c.to_owned(), n)).collect()
    };
    assert_eq!(counts(&all, "kerml"), owned(kerml));
    assert_eq!(counts(&all, "sysml"), owned(sysml));
    assert_eq!(all.len(), 221 + 702);
}

#[test]
fn snippet_ids_are_stable_and_unique() {
    let all = snippets().unwrap();
    let mut seen = std::collections::HashSet::new();
    for s in &all {
        assert!(seen.insert(s.source.id.clone()), "{}", s.source.id);
        // `<spec>/<clause>/<n>`
        let parts: Vec<&str> = s.source.id.split('/').collect();
        assert_eq!(parts, [s.spec.as_str(), s.clause.as_str(), parts[2]]);
        assert!(parts[2].parse::<u32>().unwrap() >= 1);
        let language = if s.spec == "kerml" {
            Language::KerML
        } else {
            Language::SysML
        };
        assert_eq!(s.source.language, language, "{}", s.source.id);
        assert!(s.page > 0);
        assert!(!s.source.text.trim().is_empty(), "{}", s.source.id);
    }
}

#[test]
fn every_expectation_names_a_snippet_and_gives_a_reason() {
    let all = snippets().unwrap();
    let marked: Vec<&Snippet> = all
        .iter()
        .filter(|s| s.source.expect != Expectation::Valid)
        .collect();
    // The table in crates/sysml-corpus/snippets.json: every entry names a snippet.
    let table = sysml_corpus::snippet_expectations().unwrap();
    for id in table.keys() {
        assert!(
            all.iter().any(|s| &s.source.id == id),
            "{id} is not a snippet"
        );
    }
    assert_eq!(marked.len(), table.len());
    for s in marked {
        assert!(
            s.reason.as_deref().is_some_and(|r| !r.is_empty()),
            "{}",
            s.source.id
        );
    }
    let fragments = all
        .iter()
        .filter(|s| s.source.expect == Expectation::Fragment)
        .count();
    let invalid = all
        .iter()
        .filter(|s| s.source.expect == Expectation::Invalid)
        .count();
    assert_eq!((fragments, invalid), (20, 10));
}
