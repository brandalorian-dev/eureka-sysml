# ADR 0007: Benchmark harness and synthetic models

- Status: Accepted
- Date: 2026-11-20

## Context

PROJECT_PLAN §7 sets performance budgets, says they are validated by the Phase 0 benchmark harness, and §8 calls for performance regression gates in CI using synthetic models. In Phase 0 nothing a budget measures exists yet: each needs the parser, the semantic layer, the store or the web client.

## Decision

**Budgets as data.** `sysml-bench` holds §7 as the `BUDGETS` table: each budget's operation and limit exactly as in the plan, the phase from which it can be measured, what it needs, and its benchmark once it has one. A test compares the table with the plan's §7 table row for row, so the two cannot drift apart. Another test records that no budget is measurable in Phase 0; it changes when the first budget gets its benchmark.

**Benchmarks with `divan`.** `divan` is lighter than `criterion` (no plotting or statistics dependencies), and `cargo test --benches` runs every benchmark once as a test. Phase 0 benchmarks the building blocks the budgets rest on: synthetic model generation at the §7 sizes, corpus loading, and a round trip of a large commit request through `sysml-api-types`.

**Synthetic models.** `sysml-synth` generates SysML text deterministically from a `Config`: one file per package, with imports, port and part definitions, specialization, typed attributes, ports and parts, multiplicities, connections and documentation. The syntax is taken from SysML 7.5.3 and 7.6.3 and from SimpleVehicleModel. The model is valid by construction: names are declared before use and visible through imports; feature names are unique to their definition, so nothing redeclares an inherited name; only root definitions declare ports, which all others inherit; and references point only at earlier definitions, so there are no cycles. The generator's own tests check name resolution textually. From Phase 1b, the parser checks the generated models.

**Model size.** Budgets are stated in elements. `Stats::elements` estimates the element count from documented per-construct rules: the explicit abstract syntax, relationships included, implied relationships excluded. `Config::with_target_elements` sizes a model to within 2% of a target (1k, the 50k working set, the 1M branch). The rules are an estimate until Phase 1b, when the parser's element counts replace or confirm them.

**No timing gates in CI yet.** Timings on shared GitHub runners vary too much for a fixed threshold. CI runs each benchmark once so that they keep compiling and working. Regression gates are added with the first budget that can be measured (Phase 1a or 1b), using either a dedicated runner or a measure that does not depend on the machine, such as instruction counts.

## Consequences

- Each later phase adds the benchmark for its budgets to `sysml-bench` and sets `benchmark` in `BUDGETS`.
- If PROJECT_PLAN §7 changes, `budgets_match_the_plan` fails until `BUDGETS` is updated.
- If the element-count rules turn out to be wrong, the size targets move, but the generated models stay the same for a given `Config`.
