// SPDX-License-Identifier: Apache-2.0
//! Phase 0 benchmarks: the building blocks that the PROJECT_PLAN §7 budgets will rest on.
//!
//! `cargo bench -p sysml-bench` measures; `cargo test -p sysml-bench --benches` runs each once.

use divan::counter::{BytesCount, ItemsCount};
use divan::{black_box, Bencher};
use serde_json::json;
use sysml_api_types::CommitRequest;
use sysml_corpus::Corpus;
use sysml_synth::{generate, Config};

fn main() {
    divan::main();
}

/// Generating synthetic models at the §7 sizes: 1k, the 50k browser working set and the 1M
/// branch. The 1M model is the input for the `branch-cold-load`, `re-resolve` and
/// `find-references` budgets.
#[divan::bench(args = [Config::SMALL, Config::WORKING_SET, Config::LARGE], sample_count = 10, sample_size = 1)]
fn synth_generate(bencher: Bencher, elements: usize) {
    let config = Config::with_target_elements(elements);
    bencher
        .counter(ItemsCount::new(elements))
        .bench(|| generate(black_box(&config)));
}

/// Loading the whole corpus from `docs/specs/`: reading the 10 library archives and the example
/// model, checking their SHA-256 against `specs.lock.json`, and unzipping. A baseline for the
/// `library-load` budget, which starts from a pre-resolved snapshot instead.
#[divan::bench(sample_count = 10, sample_size = 1)]
fn corpus_load() -> Corpus {
    Corpus::load().unwrap()
}

/// A commit request body with `changes` element changes, as a client would POST it to
/// `/projects/{projectId}/commits` (API 8.1.3).
fn commit_request_json(changes: usize) -> String {
    let id = |n: usize| format!("00000000-0000-4000-8000-{n:012x}");
    let change: Vec<_> = (0..changes)
        .map(|n| {
            json!({
                "@type": "DataVersion",
                "identity": {"@id": id(n)},
                "payload": {
                    "@id": id(n),
                    "@type": "PartUsage",
                    "declaredName": format!("q{n}"),
                    "owningMembership": {"@id": id(changes + n)},
                    "isComposite": true
                }
            })
        })
        .collect();
    json!({"@type": "Commit", "change": change}).to_string()
}

/// Deserializing and re-serializing a commit request with 1k and 10k element changes, through
/// the strict `sysml-api-types` records. A building block of the `commit-delivery` budget.
#[divan::bench(args = [1_000, 10_000], sample_count = 20)]
fn api_commit_request_round_trip(bencher: Bencher, changes: usize) {
    let body = commit_request_json(changes);
    bencher.counter(BytesCount::new(body.len())).bench(|| {
        let request: CommitRequest = serde_json::from_str(black_box(&body)).unwrap();
        serde_json::to_string(&request).unwrap()
    });
}
