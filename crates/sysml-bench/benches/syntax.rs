// SPDX-License-Identifier: Apache-2.0
//! Benchmarks of `sysml-syntax` (ADR 0011): lexing the corpus and the 1M-element synthetic model,
//! and parsing the KerML library. The synthetic model is SysML, whose productions arrive in
//! Phase 1b, so it is only lexed for now.
//!
//! `cargo bench -p sysml-bench --bench syntax` measures; `cargo test --workspace --bench '*'` runs
//! each once.

use divan::counter::BytesCount;
use divan::{black_box, Bencher};
use sysml_syntax::{lex, parse, Language};
use sysml_synth::{generate, Config};

fn main() {
    divan::main();
}

fn language(l: sysml_corpus::Language) -> Language {
    match l {
        sysml_corpus::Language::KerML => Language::KerML,
        sysml_corpus::Language::SysML => Language::SysML,
    }
}

/// Lexing every corpus source in its own language (KerML 8.2.2, SysML 8.2.2.1.2).
#[divan::bench(sample_count = 20)]
fn lex_corpus(bencher: Bencher) {
    let corpus = sysml_corpus::Corpus::load().unwrap();
    let sources: Vec<(Language, String)> = corpus
        .sources()
        .map(|s| (language(s.language), s.text.clone()))
        .collect();
    let bytes: usize = sources.iter().map(|(_, t)| t.len()).sum();
    bencher.counter(BytesCount::new(bytes)).bench(|| {
        sources
            .iter()
            .map(|(l, t)| lex(*l, black_box(t)).tokens.len())
            .sum::<usize>()
    });
}

/// Lexing the 1M-element synthetic model (PROJECT_PLAN §7), file by file.
#[divan::bench(sample_count = 10, sample_size = 1)]
fn lex_synthetic_1m(bencher: Bencher) {
    let model = generate(&Config::with_target_elements(Config::LARGE));
    bencher.counter(BytesCount::new(model.bytes())).bench(|| {
        model
            .files
            .iter()
            .map(|f| lex(Language::SysML, black_box(&f.text)).tokens.len())
            .sum::<usize>()
    });
}

/// Parsing the 36 KerML library files (KerML 8.2.3–8.2.5; the Kernel constructs that step 1a.3
/// does not parse yet are skipped).
#[divan::bench(sample_count = 20)]
fn parse_kerml_library(bencher: Bencher) {
    let corpus = sysml_corpus::Corpus::load().unwrap();
    let sources: Vec<String> = corpus
        .sources()
        .filter(|s| s.language == sysml_corpus::Language::KerML)
        .map(|s| s.text.clone())
        .collect();
    let bytes: usize = sources.iter().map(String::len).sum();
    bencher.counter(BytesCount::new(bytes)).bench(|| {
        sources
            .iter()
            .map(|t| parse(Language::KerML, black_box(t)).diagnostics().len())
            .sum::<usize>()
    });
}
