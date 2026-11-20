// SPDX-License-Identifier: Apache-2.0
//! Loader rules (KerML 10.2, 10.3), checked against small archives built in memory and against
//! edited copies of `specs.lock.json`.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Write};
use sysml_corpus::{read_spec, Corpus, Language, Library, MAX_ENTRY_BYTES};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;
use zip::write::SimpleFileOptions;

const PROJECT: &[u8] = br#"{"name": "P", "version": "1.0.0"}"#;
const A: &[u8] = b"package A;";

fn kpar(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in entries {
        w.start_file(*name, options).unwrap();
        w.write_all(data).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn meta(checksum: Option<Value>) -> Vec<u8> {
    let mut m = json!({"index": {"A": "A.kerml"}, "created": "2026-11-20T00:00:00Z"});
    if let Some(c) = checksum {
        m["checksum"] = c;
    }
    serde_json::to_vec(&m).unwrap()
}

fn load(entries: &[(&str, &[u8])]) -> Result<Library, String> {
    Library::from_archive(&kpar(entries), "test.kpar", "https://example.org/test.kpar")
        .map_err(|e| e.to_string())
}

fn hex_sha256(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn archive_with_project_files_at_the_top_level() {
    // KerML 10.3: .project.json and .meta.json at the archive's top level.
    let m = meta(None);
    let l = load(&[
        (".project.json", PROJECT),
        (".meta.json", &m),
        ("A.kerml", A),
    ])
    .unwrap();
    assert_eq!(l.language, Language::KerML);
    assert_eq!(l.sources.len(), 1);
    assert_eq!(l.sources[0].id, "P/A.kerml");
}

#[test]
fn json_and_xmi_model_files_are_recorded() {
    // KerML 10.2: a model interchange file may be textual, JSON (.json) or XMI (.xmi).
    let m = meta(None);
    let l = load(&[
        (".project.json", PROJECT),
        (".meta.json", &m),
        ("A.kerml", A),
        ("B.json", b"[]"),
        ("sub/C.xmi", b"<xmi/>"),
    ])
    .unwrap();
    assert_eq!(l.sources.len(), 1);
    assert_eq!(l.interchange_files, ["B.json", "sub/C.xmi"]);
}

#[test]
fn other_entries_are_rejected() {
    let m = meta(None);
    let err = load(&[
        (".project.json", PROJECT),
        (".meta.json", &m),
        ("A.kerml", A),
        ("notes.txt", b"x"),
    ])
    .unwrap_err();
    assert!(err.contains("notes.txt"), "{err}");
}

#[test]
fn checksums_are_verified() {
    // KerML 10.3 Table 13: `checksum` maps file paths to checksums.
    let good = hex_sha256(A);
    let ok = |c: Value| {
        let m = meta(Some(c));
        load(&[
            (".project.json", PROJECT),
            (".meta.json", &m),
            ("A.kerml", A),
        ])
    };
    assert!(ok(json!({"A.kerml": {"value": good, "algorithm": "SHA256"}})).is_ok());
    // Table 13 describes a list per path; the normative schema has a single object. Both are read.
    assert!(ok(json!({"A.kerml": [{"value": good, "algorithm": "SHA256"}]})).is_ok());
    // Hex digits are compared without regard to case.
    assert!(ok(json!({"A.kerml": {"value": good.to_uppercase(), "algorithm": "SHA256"}})).is_ok());

    let err = ok(json!({"A.kerml": {"value": "00", "algorithm": "SHA256"}})).unwrap_err();
    assert!(err.contains("A.kerml") && err.contains("checksum"), "{err}");
    let err = ok(json!({"A.kerml": {"value": good, "algorithm": "MD5"}})).unwrap_err();
    assert!(err.contains("MD5"), "{err}");
    let err = ok(json!({"Missing.kerml": {"value": good, "algorithm": "SHA256"}})).unwrap_err();
    assert!(err.contains("Missing.kerml"), "{err}");
}

#[test]
fn oversized_entries_are_rejected() {
    let big = vec![b' '; MAX_ENTRY_BYTES as usize + 1];
    let m = meta(None);
    let err = load(&[
        (".project.json", PROJECT),
        (".meta.json", &m),
        ("A.kerml", &big),
    ])
    .unwrap_err();
    assert!(
        err.contains("A.kerml") && err.contains("larger than"),
        "{err}"
    );
}

// ---- specs.lock.json ----

fn lock() -> Value {
    serde_json::from_str(sysml_corpus::LOCK).unwrap()
}

fn lock_entry(path: &str) -> Value {
    lock()["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["path"] == path)
        .unwrap()
        .clone()
}

fn load_with(files: Vec<Value>) -> Result<Corpus, String> {
    Corpus::from_lock(
        &json!({"version": 1, "files": files}).to_string(),
        read_spec,
    )
    .map_err(|e| e.to_string())
}

#[test]
fn edited_lock_loads() {
    let corpus = load_with(vec![lock_entry("informative/ptc-25-04-31.sysml")]).unwrap();
    assert_eq!(corpus.models.len(), 1);
}

#[test]
fn lock_entry_without_a_path_is_an_error() {
    let err = load_with(vec![json!({"url": "https://example.org/x.kpar"})]).unwrap_err();
    assert!(err.contains("path"), "{err}");
}

#[test]
fn library_without_a_url_is_an_error() {
    let mut e = lock_entry("normative/Systems-Library.kpar");
    e.as_object_mut().unwrap().remove("url");
    let err = load_with(vec![e]).unwrap_err();
    assert!(
        err.contains("Systems-Library.kpar") && err.contains("url"),
        "{err}"
    );
}

#[test]
fn files_must_match_the_lock_checksum() {
    // A modified file, or EUREKA_SPECS_DIR pointing at a different release, fails clearly.
    let mut e = lock_entry("informative/ptc-25-04-31.sysml");
    e["sha256"] = json!("0".repeat(64));
    let err = load_with(vec![e]).unwrap_err();
    assert!(
        err.contains("ptc-25-04-31.sysml") && err.contains("specs.lock.json"),
        "{err}"
    );
}
