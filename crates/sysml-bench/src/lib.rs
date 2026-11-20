// SPDX-License-Identifier: Apache-2.0
//! Benchmark harness for the PROJECT_PLAN §7 performance budgets.
//!
//! [`BUDGETS`] is §7 as data: each budget with the phase from which it can be measured and, once
//! it exists, the benchmark that measures it. A test keeps it identical to the plan.
//!
//! The benchmarks live in `benches/` and use `divan`:
//! - `cargo bench -p sysml-bench` measures them.
//! - `cargo test --workspace --bench '*'` runs each once, as CI does, so they keep working.
//!
//! In Phase 0 no budget can be measured yet: each needs the parser, the store or the web client.
//! `benches/phase0.rs` measures the building blocks the budgets will rest on: synthetic model
//! generation at the §7 sizes, corpus loading, and API payload handling at scale.

/// A phase of PROJECT_PLAN §6.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Phase {
    /// Phase 0, foundations.
    P0,
    /// Phase 1a, KerML core.
    P1a,
    /// Phase 1b, SysML language, CLI and language server.
    P1b,
    /// Phase 2, server and persistence.
    P2,
    /// Phase 3, web client.
    P3,
    /// Phase 4, collaboration.
    P4,
}

/// One PROJECT_PLAN §7 budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    /// A short, stable identifier.
    pub id: &'static str,
    /// The operation, exactly as in the §7 table.
    pub operation: &'static str,
    /// The budget, exactly as in the §7 table.
    pub budget: &'static str,
    /// The first phase that delivers what the budget needs.
    pub measurable_from: Phase,
    /// What has to exist before it can be measured.
    pub needs: &'static str,
    /// The benchmark that measures it (`file::function`), once there is one.
    pub benchmark: Option<&'static str>,
}

/// PROJECT_PLAN §7, row for row.
pub const BUDGETS: &[Budget] = &[
    Budget {
        id: "library-load",
        operation: "Load the standard libraries (from the pre-resolved snapshot)",
        budget: "< 300 ms server, < 1 s browser",
        measurable_from: Phase::P1a,
        needs: "the pre-resolved library snapshot (Phase 1a)",
        benchmark: None,
    },
    Budget {
        id: "branch-cold-load",
        operation: "Cold-load a 1M-element branch on the server (from a snapshot)",
        budget: "< 30 s",
        measurable_from: Phase::P2,
        needs: "sysml-store snapshots of a 1M-element synthetic branch",
        benchmark: None,
    },
    Budget {
        id: "re-resolve",
        operation: "Re-resolve after a typical commit (< 100 changed elements)",
        budget: "< 1 s server",
        measurable_from: Phase::P1b,
        needs: "incremental SysML resolution over a synthetic model",
        benchmark: None,
    },
    Budget {
        id: "keystroke-diagnostics",
        operation: "Keystroke → diagnostics in the browser (working set ≤ 50k elements)",
        budget: "< 100 ms",
        measurable_from: Phase::P3,
        needs: "the WASM worker and the browser working set",
        benchmark: None,
    },
    Budget {
        id: "language-features",
        operation: "Completion, hover, go-to-definition",
        budget: "< 50 ms local, < 300 ms if the server is needed",
        measurable_from: Phase::P1b,
        needs: "the language server",
        benchmark: None,
    },
    Budget {
        id: "find-references",
        operation: "Find references / impact query over 1M elements",
        budget: "< 2 s",
        measurable_from: Phase::P1b,
        needs: "resolved 1M-element synthetic model and its reference index",
        benchmark: None,
    },
    Budget {
        id: "commit-delivery",
        operation: "Delivering a commit to other clients on the branch",
        budget: "< 1 s",
        measurable_from: Phase::P4,
        needs: "WebSocket live updates",
        benchmark: None,
    },
    Budget {
        id: "browser-memory",
        operation: "Browser memory for a typical working set",
        budget: "< 1 GB",
        measurable_from: Phase::P3,
        needs: "the web client and its working set",
        benchmark: None,
    },
];
