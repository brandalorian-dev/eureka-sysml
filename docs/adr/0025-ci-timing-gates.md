# ADR 0025: CI timing gates by instruction counts (decision D6)

- Status: Accepted
- Date: 2026-11-24
- Decision: D6 of the Phase 1 execution plan, evaluated in step 1a.11 under its §2.5 methodology
- Evidence: `docs/proj-mgmt/evidence/1a.11-library-snapshot-and-timing-gates.md`

## Context

ADR 0007 runs every benchmark once in CI and gates none, because timings on shared GitHub runners vary too much for a fixed threshold. It adds gates with the first budget that can be measured, using a dedicated runner or a measure that does not depend on the machine, such as instruction counts. `library-load` is that first budget (step 1a.11). The execution plan's default for D6 is instruction counts on GitHub-hosted runners, through a Cachegrind-based harness, with wall-clock budgets measured but not gated until a dedicated runner exists.

## Criteria

- **A gate must not fail without cause.** It fails a PR only when the code got slower, not because a different runner picked up the job.
- **It must catch a regression small enough to matter**, well before a budget is missed.
- **It must use what CI already has:** the `ubuntu-24.04` runners, no new service, and few new tools. Python scripts use only the standard library (CLAUDE.md).

## Options

1. **Instruction counts** with Valgrind's Cachegrind, on GitHub's runners.
2. **Wall-clock timings** with a threshold, on GitHub's runners.
3. **A dedicated runner**, whose timings are stable enough to gate.

## Evidence

The same workload ran on three GitHub `ubuntu-24.04` runners and on the development machine: the KerML libraries loaded from text in release.

- **Wall clock.**
  - Within a runner, 20 runs spread by 2–3%.
  - The means differ by 46% from runner to runner: 263, 334 and 383 ms. Even two runners with the same CPU model differ by 15%.
  - A threshold that never fails falsely would miss any regression smaller than about half.
- **Instruction counts.**
  - Nine runs on two CPU models lie within 0.005% of each other: 3,373,902,690 to 3,374,067,966.
  - The development machine counts 0.13% more, from its own toolchain and system libraries.
- **A dedicated runner** does not exist. Running one is a service to buy, secure and maintain, outside this step.

## Decision

Adopt option 1.

- **What is gated.** The instruction count of loading the libraries, from a small probe binary run once under Cachegrind (`valgrind --tool=cachegrind --cache-sim=no`). Later budgets add their own probes.
- **The baseline** is a count measured on CI, committed with the benchmark. A PR fails if its count exceeds the baseline by more than 2%. That is 400 times the runner-to-runner noise, and still a small regression. A PR that makes loading faster, or slower on purpose, updates the baseline in the same PR, where review sees it.
- **The harness** is a Python script using only the standard library. It runs the probe under Valgrind, reads `I refs`, compares the count with the baseline, and writes the result to the job summary. iai-callgrind would add a crate and a runner binary to pin, for one count.
- **Wall clock** is measured on the same runner with the `divan` benchmark and reported in the job summary against the budget, without gating, as ADR 0007 has it. It is checked against the budget in each phase's performance step.
- **Not on the development machine.** Its counts differ from CI's, so the baseline is taken on CI only. A developer can still compare two builds of their own.

## Consequences

- Step 1a.11's third PR adds the probe, the script, the baseline and a CI job that installs Valgrind.
- The gate guards against regressions, not the budget itself. The budget is still met by milliseconds measured on real machines, and a CI count can only say that nothing got slower.
- Valgrind does not run WebAssembly, so the browser figure stays a measurement, under Node, not a gate.
- If GitHub's runners change toolchain or libraries, the counts may shift together. The baseline is then retaken in its own PR, with the evidence.
