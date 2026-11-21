// SPDX-License-Identifier: Apache-2.0
//! Benchmarks of name resolution (step 1a.7, ADR 0019, ADR 0020): the KerML libraries resolved
//! from nothing, and again after an edit to one of their files.
//!
//! `cargo bench -p sysml-bench --bench semantics` measures; `cargo test --workspace --bench '*'`
//! runs each once.

use divan::counter::ItemsCount;
use divan::Bencher;
use salsa::Setter;
use sysml_model::FileId;
use sysml_semantics::{resolved_names, Database, Project, SourceFile};
use sysml_syntax::Language;

fn main() {
    divan::main();
}

fn library(db: &Database) -> (Project, Vec<SourceFile>) {
    let corpus = sysml_corpus::Corpus::load().unwrap();
    let files: Vec<SourceFile> = corpus
        .sources()
        .filter(|s| s.language == sysml_corpus::Language::KerML)
        .enumerate()
        .map(|(i, s)| {
            SourceFile::new(
                db,
                FileId(i as u32),
                s.id.clone(),
                Language::KerML,
                s.text.clone(),
            )
        })
        .collect();
    (Project::new(db, files.clone()), files)
}

fn resolve_all(db: &Database, project: Project, files: &[SourceFile]) -> usize {
    files
        .iter()
        .map(|f| resolved_names(db, project, *f).len())
        .sum()
}

/// Parsing, lowering and resolving every name of the 36 KerML library files in a new database.
#[divan::bench(sample_count = 10)]
fn resolve_kerml_library(bencher: Bencher) {
    let names = {
        let db = Database::default();
        let (project, files) = library(&db);
        resolve_all(&db, project, &files)
    };
    bencher.counter(ItemsCount::new(names)).bench(|| {
        let db = Database::default();
        let (project, files) = library(&db);
        resolve_all(&db, project, &files)
    });
}

/// Re-resolving the KerML libraries after a change of layout in `Occurrences.kerml`, which most
/// of the library inherits from: what the editor pays per keystroke there.
#[divan::bench(sample_count = 20)]
fn re_resolve_after_an_edit(bencher: Bencher) {
    let mut db = Database::default();
    let (project, files) = library(&db);
    resolve_all(&db, project, &files);
    let edited = *files
        .iter()
        .find(|f| f.path(&db).ends_with("Occurrences.kerml"))
        .unwrap();
    let text = edited.text(&db).clone();
    let mut spaces = 0;
    bencher.bench_local(move || {
        spaces = (spaces + 1) % 2;
        let changed = format!("{text}{}", " ".repeat(spaces));
        edited.set_text(&mut db).to(changed);
        resolve_all(&db, project, &files)
    });
}
