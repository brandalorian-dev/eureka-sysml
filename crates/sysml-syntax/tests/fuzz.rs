// SPDX-License-Identifier: Apache-2.0
//! Property tests of the parser (step 1a.4): whatever the input, it never panics, never stops
//! early (`syntax::parser-stuck`, the framework's safety net), and keeps every byte.
//!
//! The inputs are mutants of the KerML library: spans of tokens deleted, duplicated or replaced
//! by other tokens, which is how text looks while it is being edited. The regular run is short; a
//! longer run is `cargo test -p sysml-syntax --test fuzz -- --ignored`, with `FUZZ_SECONDS` (the
//! default is 300) for the agreed run time and `FUZZ_SEED` to replay a run.

use sysml_syntax::{lex, parse, Language};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// A xorshift generator: deterministic, so a failure can be replayed from its seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// The KerML library sources, each as its tokens' texts.
fn library() -> Vec<Vec<String>> {
    let corpus = sysml_corpus::Corpus::load().unwrap();
    corpus
        .sources()
        .filter(|s| s.language == sysml_corpus::Language::KerML)
        .map(|s| {
            let lexed = lex(Language::KerML, &s.text);
            let mut offset = 0;
            lexed
                .tokens
                .iter()
                .map(|t| {
                    let text = s.text[offset..offset + t.len as usize].to_owned();
                    offset += t.len as usize;
                    text
                })
                .collect()
        })
        .collect()
}

/// One mutant of `tokens`: up to three edits, each deleting, duplicating or replacing a span.
fn mutant(rng: &mut Rng, tokens: &[String], pool: &[String]) -> String {
    let mut t: Vec<String> = tokens.to_vec();
    for _ in 0..=rng.below(3) {
        let at = rng.below(t.len());
        let len = 1 + rng.below(12).min(t.len() - at - 1);
        match rng.below(3) {
            0 => {
                t.drain(at..at + len);
            }
            1 => {
                let span: Vec<String> = t[at..at + len].to_vec();
                let to = rng.below(t.len());
                t.splice(to..to, span);
            }
            _ => {
                t[at] = pool[rng.below(pool.len())].clone();
            }
        }
        if t.is_empty() {
            break;
        }
    }
    t.concat()
}

/// Check `text`, with `seed` to replay it.
fn check(text: &str, seed: u64) {
    let p = parse(Language::KerML, text);
    assert_eq!(p.syntax().text().to_string(), text, "seed {seed}: lossless");
    if let Some(d) = p
        .diagnostics()
        .iter()
        .find(|d| d.code == "syntax::parser-stuck")
    {
        let at = usize::from(d.range.start());
        let from = text[..at]
            .char_indices()
            .rev()
            .nth(300)
            .map_or(0, |(i, _)| i);
        let to = text[at..]
            .char_indices()
            .nth(300)
            .map_or(text.len(), |(i, _)| at + i);
        panic!(
            "seed {seed}: the parser stopped at {:?}:\n{}<<HERE>>{}",
            d.range,
            &text[from..at],
            &text[at..to]
        );
    }
}

fn run(iterations: impl Fn(usize) -> bool, seed: u64) -> usize {
    let lib = library();
    let pool: Vec<String> = lib
        .iter()
        .flatten()
        .filter(|t| !t.trim().is_empty())
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut rng = Rng(seed);
    let mut i = 0;
    while iterations(i) {
        let s = rng.next();
        let mut local = Rng(s | 1);
        let source = &lib[local.below(lib.len())];
        check(&mutant(&mut local, source, &pool), s | 1);
        i += 1;
    }
    i
}

#[test]
fn library_mutants_parse_without_panics() {
    run(|i| i < 200, 0x9e37_79b9_7f4a_7c15);
}

#[test]
#[ignore = "long run: `-- --ignored`, `FUZZ_SECONDS`"]
fn library_mutants_for_the_agreed_run_time() {
    let seconds: u64 = std::env::var("FUZZ_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    let start = std::time::Instant::now();
    let seed = std::env::var("FUZZ_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64)
        });
    let n = run(|_| start.elapsed().as_secs() < seconds, seed);
    println!("{n} mutants in {seconds} s, seed {seed}");
}

#[test]
fn found_by_fuzzing() {
    // An unclosed `[` in a flow declaration: the lookahead that decides the flow's form used to
    // run on past the `;` to the end of the file, and so spend the parser's fuel (more than
    // `FUEL` = 256 lookaheads) on the members after it.
    let members = "feature g : G; ".repeat(100);
    check(&format!("package P {{ flow f : T[0..; {members}}}"), 1);
    // A stray `.` in a function body: it was taken for the start of a result expression, which
    // then consumed nothing, again and again.
    check(
        "package P { inv { a implies (b.binding [0..1] c = [1] d; } feature e; }",
        2,
    );
}
