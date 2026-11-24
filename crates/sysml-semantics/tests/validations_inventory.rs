// SPDX-License-Identifier: Apache-2.0
//! The inventory of validation constraints (step 1a.10, KerML 8.3):
//! `crates/sysml-semantics/validations.json` lists every `validate…` constraint of `KerML.xmi`,
//! each `done` or `open`.
//!
//! - A `done` entry names its function in `sysml_semantics::validate`, which `VALIDATIONS` lists for
//!   the constraint's metaclass with the clause of that metaclass, and has a test in
//!   `tests/validate.rs` of the same name.
//! - That test has a model that violates the constraint, unless the entry's `negative` says why no
//!   text can and that its negative test comes with interchange input in step 1b.6.
//! - No entry is open: all 88 constraints are done.

use std::collections::BTreeSet;
use sysml_model::CONSTRAINTS;
use sysml_semantics::validate::VALIDATIONS;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const INVENTORY: &str = include_str!("../validations.json");
const FUNCTIONS: &str = include_str!("../src/validate.rs");
const TESTS: &str = include_str!("validate.rs");

/// The body of the test function `name` in tests/validate.rs.
fn test_body(name: &str) -> Option<&'static str> {
    let start = TESTS.find(&format!("fn {name}("))?;
    let rest = &TESTS[start..];
    Some(&rest[..rest.find("\n}\n").unwrap_or(rest.len())])
}

/// Whether a test has a model that violates its constraint.
fn has_negative(body: &str) -> bool {
    // `violates(`, `relationship_violates(`, or an assertion that an element violates it
    body.contains("violates(") || body.contains("assert!(violated(")
}

#[test]
fn the_inventory_lists_every_validation_with_its_status() {
    let inventory: serde_json::Value = serde_json::from_str(INVENTORY).unwrap();
    assert_eq!(inventory["version"], 1);
    let listed = inventory["validations"].as_object().unwrap();
    let constraints: Vec<_> = CONSTRAINTS
        .iter()
        .filter(|c| c.name.starts_with("validate"))
        .collect();
    assert_eq!(
        constraints.len(),
        88,
        "KerML 8.3 has 88 validation constraints"
    );
    let expected: BTreeSet<String> = constraints
        .iter()
        .map(|c| format!("{}::{}", c.metaclass, c.name))
        .collect();
    let keys: BTreeSet<String> = listed.keys().cloned().collect();
    let mut problems = Vec::new();
    for k in expected.difference(&keys) {
        problems.push(format!("{k}: in KerML.xmi, not in validations.json"));
    }
    for k in keys.difference(&expected) {
        problems.push(format!("{k}: in validations.json, not in KerML.xmi"));
    }
    for c in &constraints {
        let key = format!("{}::{}", c.metaclass, c.name);
        let Some(entry) = listed.get(&key) else {
            continue;
        };
        let text = |field: &str| entry[field].as_str().filter(|s| !s.is_empty());
        match entry["status"].as_str().unwrap_or_default() {
            "done" => {
                let f = text("function").unwrap_or_default();
                if f.is_empty() || !FUNCTIONS.contains(&format!("pub fn {f}(")) {
                    problems.push(format!("{key}: no function `{f}` in validate.rs"));
                }
                if !VALIDATIONS
                    .iter()
                    .any(|(m, n, _, _)| m.name() == c.metaclass && *n == c.name)
                {
                    problems.push(format!("{key}: not in VALIDATIONS"));
                }
                match test_body(f) {
                    None => problems.push(format!("{key}: no test `{f}` in tests/validate.rs")),
                    Some(body) => match (has_negative(body), text("negative")) {
                        (true, Some(_)) => {
                            problems.push(format!("{key}: a negative test, and a reason for none"))
                        }
                        (false, None) => problems.push(format!("{key}: no negative test")),
                        (false, Some(n)) if !n.starts_with("step 1b.6: ") => {
                            problems.push(format!("{key}: `negative` does not name step 1b.6"))
                        }
                        _ => {}
                    },
                }
            }
            s => problems.push(format!("{key}: status `{s}` without what it needs")),
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn validations_lists_each_constraint_once_with_its_clause() {
    let inventory: serde_json::Value = serde_json::from_str(INVENTORY).unwrap();
    let done = inventory["validations"]
        .as_object()
        .unwrap()
        .values()
        .filter(|e| e["status"] == "done")
        .count();
    let names: BTreeSet<(&str, &str)> = VALIDATIONS
        .iter()
        .map(|(m, n, _, _)| (m.name(), *n))
        .collect();
    assert_eq!(names.len(), VALIDATIONS.len(), "a constraint listed twice");
    assert_eq!(VALIDATIONS.len(), done);
    for (_, n, clause, _) in VALIDATIONS {
        assert!(clause.starts_with("KerML 8.3."), "{n}: clause {clause}");
    }
}
