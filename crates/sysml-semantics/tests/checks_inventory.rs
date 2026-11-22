// SPDX-License-Identifier: Apache-2.0
//! The inventory of semantic constraints (step 1a.9, KerML 8.4.2):
//! `crates/sysml-semantics/checks.json` lists every `check…` constraint of `KerML.xmi`, each
//! `done`, `open` or `deferred`, with the table of its implied relationship.
//!
//! - A `done` entry names its function in `sysml_semantics::checks`, which `CHECKS` lists for the
//!   constraint's metaclass, and has a golden test in `tests/checks.rs` of the same name.
//! - An `open` entry says what it waits for; a `deferred` one is a constraint without OCL.
//! - The table is the one the constraint's name gives (KerML 8.4.2: a specialization constraint
//!   has "Specialization" in its name, and so on), and the tables have the sizes of KerML 8.4.2.

use std::collections::{BTreeMap, BTreeSet};
use sysml_model::CONSTRAINTS;
use sysml_semantics::checks::CHECKS;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const INVENTORY: &str = include_str!("../checks.json");
const FUNCTIONS: &str = include_str!("../src/checks.rs");
const TESTS: &str = include_str!("checks.rs");

/// The table of a constraint, by the words of its name (KerML 8.4.2).
fn table(name: &str) -> &'static str {
    if name.ends_with("TypeFeaturing") {
        "TypeFeaturing"
    } else if name.ends_with("BindingConnector") {
        "BindingConnector"
    } else if name.ends_with("Redefinition") {
        "Redefinition"
    } else {
        assert!(name.contains("Specialization"), "{name}");
        "Specialization"
    }
}

#[test]
fn the_inventory_lists_every_check_with_its_status() {
    let inventory: serde_json::Value = serde_json::from_str(INVENTORY).unwrap();
    assert_eq!(inventory["version"], 1);
    let listed = inventory["checks"].as_object().unwrap();
    let constraints: Vec<_> = CONSTRAINTS
        .iter()
        .filter(|c| c.name.starts_with("check"))
        .collect();
    let expected: BTreeSet<String> = constraints
        .iter()
        .map(|c| format!("{}::{}", c.metaclass, c.name))
        .collect();
    let keys: BTreeSet<String> = listed.keys().cloned().collect();
    let mut problems = Vec::new();
    for k in expected.difference(&keys) {
        problems.push(format!("{k}: in KerML.xmi, not in checks.json"));
    }
    for k in keys.difference(&expected) {
        problems.push(format!("{k}: in checks.json, not in KerML.xmi"));
    }
    let mut sizes: BTreeMap<&str, usize> = BTreeMap::new();
    for c in &constraints {
        let key = format!("{}::{}", c.metaclass, c.name);
        let Some(entry) = listed.get(&key) else {
            continue;
        };
        let t = table(c.name);
        *sizes.entry(t).or_default() += 1;
        if entry["table"] != t {
            problems.push(format!("{key}: table {} is not {t}", entry["table"]));
        }
        let text = |field: &str| entry[field].as_str().filter(|s| !s.is_empty());
        match entry["status"].as_str().unwrap_or_default() {
            "done" => {
                let f = text("function").unwrap_or_default();
                if f.is_empty() || !FUNCTIONS.contains(&format!("pub fn {f}(")) {
                    problems.push(format!("{key}: no function `{f}` in checks.rs"));
                }
                if !TESTS.contains(&format!("fn {f}(")) {
                    problems.push(format!("{key}: no test `{f}` in tests/checks.rs"));
                }
                if !CHECKS
                    .iter()
                    .any(|(m, n, _)| m.name() == c.metaclass && *n == c.name)
                {
                    problems.push(format!("{key}: not in CHECKS"));
                }
            }
            "open" if text("pending").is_some() => {}
            "deferred" if text("reason").is_some() && c.body.trim() == "TBD" => {}
            s => problems.push(format!("{key}: status `{s}` without what it needs")),
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    // KerML 8.4.2: 59 specialization, 8 redefinition, 6 type-featuring and 7 binding-connector
    // constraints
    assert_eq!(
        sizes,
        BTreeMap::from([
            ("BindingConnector", 7),
            ("Redefinition", 8),
            ("Specialization", 59),
            ("TypeFeaturing", 6),
        ])
    );
}

#[test]
fn checks_lists_each_done_constraint_once() {
    let inventory: serde_json::Value = serde_json::from_str(INVENTORY).unwrap();
    let done = inventory["checks"]
        .as_object()
        .unwrap()
        .values()
        .filter(|e| e["status"] == "done")
        .count();
    let names: BTreeSet<(&str, &str)> = CHECKS.iter().map(|(m, n, _)| (m.name(), *n)).collect();
    assert_eq!(names.len(), CHECKS.len(), "a constraint listed twice");
    assert_eq!(CHECKS.len(), done);
}
