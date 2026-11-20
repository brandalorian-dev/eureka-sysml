// SPDX-License-Identifier: Apache-2.0
//! The budget table in this crate matches PROJECT_PLAN §7 row for row, so the plan and the
//! harness cannot drift apart.

use sysml_bench::{Phase, BUDGETS};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// The rows of the §7 table: (operation, budget).
fn plan_rows() -> Vec<(String, String)> {
    // A repository file, compiled in so that the test also runs on wasm32 (no file system).
    let plan = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/proj-mgmt/PROJECT_PLAN.md"
    ));
    let section = plan
        .split("\n## ")
        .find(|s| s.starts_with("7. Performance budgets"))
        .expect("PROJECT_PLAN.md has no §7 Performance budgets");
    section
        .lines()
        .filter(|l| l.starts_with('|'))
        .skip(2) // header and separator
        .map(|l| {
            let cells: Vec<&str> = l.trim_matches('|').split('|').map(str::trim).collect();
            assert_eq!(cells.len(), 2, "unexpected §7 row: {l}");
            (cells[0].to_owned(), cells[1].to_owned())
        })
        .collect()
}

#[test]
fn budgets_match_the_plan() {
    let ours: Vec<(String, String)> = BUDGETS
        .iter()
        .map(|b| (b.operation.to_owned(), b.budget.to_owned()))
        .collect();
    assert_eq!(
        ours,
        plan_rows(),
        "update BUDGETS in crates/sysml-bench/src/lib.rs to match PROJECT_PLAN §7"
    );
}

#[test]
fn budget_ids_are_unique() {
    let mut ids: Vec<&str> = BUDGETS.iter().map(|b| b.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), BUDGETS.len());
}

#[test]
fn no_budget_is_measurable_in_phase_0() {
    // Every §7 budget needs a later component (parser, store, web client). Phase 0 benchmarks
    // only the building blocks; this test changes when the first budget gets its benchmark.
    assert!(BUDGETS
        .iter()
        .all(|b| b.measurable_from > Phase::P0 && b.benchmark.is_none()));
}
