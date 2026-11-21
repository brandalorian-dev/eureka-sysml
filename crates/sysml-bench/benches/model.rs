// SPDX-License-Identifier: Apache-2.0
//! Benchmarks of the element graph (ADR 0017): lowering the KerML library from its syntax trees.
//!
//! `cargo bench -p sysml-bench --bench model` measures; `cargo test --workspace --bench '*'` runs
//! each once.

use divan::counter::ItemsCount;
use divan::{black_box, Bencher};
use sysml_model::FileId;
use sysml_syntax::{parse, Language};
use uuid::Uuid;

fn main() {
    divan::main();
}

/// Lowering the 36 KerML library files, already parsed, into element graphs (step 1a.6). The
/// rowan cursor over each green tree is made inside the measurement; it is cheap.
#[divan::bench(sample_count = 20)]
fn lower_kerml_library(bencher: Bencher) {
    let corpus = sysml_corpus::Corpus::load().unwrap();
    let trees: Vec<_> = corpus
        .sources()
        .filter(|s| s.language == sysml_corpus::Language::KerML)
        .map(|s| parse(Language::KerML, &s.text))
        .collect();
    let mut n = 0u128;
    let elements: usize = trees
        .iter()
        .map(|t| {
            sysml_semantics::lower(FileId(0), &t.syntax(), &mut || {
                n += 1;
                Uuid::from_u128(n)
            })
            .graph
            .len()
        })
        .sum();
    bencher.counter(ItemsCount::new(elements)).bench(|| {
        let mut n = 0u128;
        trees
            .iter()
            .map(|t| {
                sysml_semantics::lower(FileId(0), &black_box(t).syntax(), &mut || {
                    n += 1;
                    Uuid::from_u128(n)
                })
                .graph
                .len()
            })
            .sum::<usize>()
    });
}
