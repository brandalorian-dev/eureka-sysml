// SPDX-License-Identifier: Apache-2.0
//! The inventory of derived properties and operations (step 1a.8, ADR 0021):
//! `crates/sysml-semantics/derived.json` lists every `derive…` rule and operation of `KerML.xmi`,
//! each `done`, `open` or `deferred`.
//!
//! - A `done` entry names its function in `sysml_semantics::derived`, and has a test in
//!   `tests/derived.rs` (Root) or `tests/derived_core.rs` (Core) named after it: the rule's name, or the metaclass and the operation, in
//!   snake case (`derive_element_owner`, `element_path`).
//! - The rules and operations the semantic and validation constraints use are "needed": those
//!   named in a `check…` or `validate…` constraint, and `path()`, and what their OCL uses in turn.
//!   A needed entry may not be deferred.
//! - In a layer listed in [`COMPLETE_LAYERS`], no entry is open.

use std::collections::{BTreeMap, BTreeSet};
use sysml_model::{Rule, CONSTRAINTS, DERIVE_RULES, OPERATIONS};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// The layers done so far, Root (KerML 8.3.2) first.
const COMPLETE_LAYERS: &[&str] = &["Root", "Core"];

const INVENTORY: &str = include_str!("../derived.json");
const FUNCTIONS: &[&str] = &[
    include_str!("../src/derived/root.rs"),
    include_str!("../src/derived/types.rs"),
];
const TESTS: &[&str] = &[include_str!("derived.rs"), include_str!("derived_core.rs")];

fn key(r: &Rule) -> String {
    format!("{}::{}", r.metaclass, r.name)
}

fn snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn identifiers(ocl: &str) -> BTreeSet<&str> {
    ocl.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|w| w.starts_with(|c: char| c.is_ascii_alphabetic()))
        .collect()
}

/// The property a derive rule defines: the name before its first `=`.
fn defined(r: &Rule) -> &str {
    r.body.split('=').next().unwrap_or_default().trim()
}

/// The names of the derived properties and operations that the constraints and `path()` need,
/// directly or through the OCL of what they use. A name stands for every rule or operation with
/// it, in any metaclass; matching the OCL's identifiers is an over-approximation.
fn needed() -> BTreeSet<String> {
    let properties: BTreeSet<&str> = DERIVE_RULES.iter().map(defined).collect();
    let operations: BTreeSet<&str> = OPERATIONS.iter().map(|r| r.name).collect();
    let uses = |ocl: &str| -> Vec<String> {
        identifiers(ocl)
            .into_iter()
            .filter(|w| properties.contains(w) || operations.contains(w))
            .map(str::to_owned)
            .collect()
    };
    let mut body: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for r in DERIVE_RULES {
        let rhs = r.body.split_once('=').map_or(r.body, |(_, b)| b);
        body.entry(defined(r)).or_default().extend(uses(rhs));
    }
    for r in OPERATIONS {
        body.entry(r.name).or_default().extend(uses(r.body));
    }
    let mut stack: Vec<String> = CONSTRAINTS.iter().flat_map(|c| uses(c.body)).collect();
    stack.push("path".into());
    let mut out = BTreeSet::new();
    while let Some(n) = stack.pop() {
        if out.insert(n.clone()) {
            stack.extend(body.get(n.as_str()).into_iter().flatten().cloned());
        }
    }
    out
}

#[test]
fn the_inventory_lists_every_rule_and_operation_with_its_status() {
    let inventory: serde_json::Value = serde_json::from_str(INVENTORY).unwrap();
    assert_eq!(inventory["version"], 1);
    let needed = needed();
    let mut problems = Vec::new();
    let mut counts: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for (section, rules, is_derive) in [
        ("derive", DERIVE_RULES, true),
        ("operations", OPERATIONS, false),
    ] {
        let listed = inventory[section].as_object().unwrap();
        let expected: BTreeSet<String> = rules.iter().map(key).collect();
        assert_eq!(expected.len(), rules.len(), "{section}: a duplicate key");
        let keys: BTreeSet<String> = listed.keys().cloned().collect();
        for k in expected.difference(&keys) {
            problems.push(format!("{k}: in KerML.xmi, not in derived.json"));
        }
        for k in keys.difference(&expected) {
            problems.push(format!("{k}: in derived.json, not in KerML.xmi"));
        }
        for r in rules {
            let Some(entry) = listed.get(&key(r)) else {
                continue;
            };
            let status = entry["status"].as_str().unwrap_or_default();
            *counts.entry((r.layer, status)).or_default() += 1;
            let is_needed = needed.contains(if is_derive { defined(r) } else { r.name });
            match status {
                "done" => {
                    let f = entry["function"].as_str().unwrap_or_default();
                    let defined = format!("pub fn {f}(");
                    if f.is_empty() || !FUNCTIONS.iter().any(|s| s.contains(&defined)) {
                        problems.push(format!("{}: no function `{f}` in derived.rs", key(r)));
                    }
                    let test = if is_derive {
                        snake(r.name)
                    } else {
                        format!("{}_{}", snake(r.metaclass), snake(r.name))
                    };
                    let named = format!("fn {test}(");
                    if !TESTS.iter().any(|s| s.contains(&named)) {
                        problems.push(format!("{}: no test `{test}` in tests/derived.rs", key(r)));
                    }
                }
                "deferred" if entry["reason"].as_str().is_some_and(|s| !s.is_empty()) => {
                    if is_needed {
                        problems.push(format!("{}: needed, but deferred", key(r)));
                    }
                }
                "open" if !COMPLETE_LAYERS.contains(&r.layer) => {}
                "open" => problems.push(format!("{}: open in a complete layer", key(r))),
                s => problems.push(format!("{}: status `{s}` without what it needs", key(r))),
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    println!("{counts:?}");
}

#[test]
fn the_constraints_need_most_rules_and_operations() {
    // A check of the analysis itself: path() and the names it uses are needed, and the counts
    // stay what they were when the inventory was made (step 1a.8): if KerML.xmi changes, the
    // needed set is reviewed again.
    let needed = needed();
    for n in [
        "path",
        "qualifiedName",
        "escapedName",
        "owningNamespace",
        "allSupertypes",
    ] {
        assert!(needed.contains(n), "{n} is needed");
    }
    let properties = DERIVE_RULES
        .iter()
        .map(defined)
        .filter(|p| needed.contains(*p))
        .collect::<BTreeSet<_>>()
        .len();
    let operations = OPERATIONS
        .iter()
        .map(|r| r.name)
        .filter(|o| needed.contains(*o))
        .collect::<BTreeSet<_>>()
        .len();
    assert_eq!((properties, operations), (56, 48));
}
