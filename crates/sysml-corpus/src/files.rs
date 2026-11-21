// SPDX-License-Identifier: Apache-2.0
//! Which lock entries are corpus files, and their language.
//!
//! Shared by the loader (`Corpus::from_lock`) and by `build.rs`, which embeds the corpus files
//! into wasm32 builds (ADR 0010). One definition, so the two cannot disagree about which files the
//! corpus reads.

/// The language of a textual model file, from its extension (KerML 10.2, SysML 10.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Language {
    /// `.kerml`
    KerML,
    /// `.sysml`
    SysML,
}

impl Language {
    /// The language of a file path, or `None` if it is not a textual model file.
    pub fn of_path(path: &str) -> Option<Language> {
        if path.ends_with(".kerml") {
            Some(Language::KerML)
        } else if path.ends_with(".sysml") {
            Some(Language::SysML)
        } else {
            None
        }
    }
}

/// A standard library: a project interchange file (KerML 10.3).
pub fn is_library_archive(path: &str) -> bool {
    path.ends_with(".kpar")
}

/// A file the corpus loads: a library archive or a textual model.
pub fn is_corpus_file(path: &str) -> bool {
    is_library_archive(path) || Language::of_path(path).is_some()
}

/// The folder of the XMI serialization of the KerML libraries (the oracle of ADR 0009 and of the
/// lowering tests, step 1a.6).
pub const KERML_LIBRARY_XMI: &str = "SysML-v2-Release/2025-02/sysml.library.xmi/Kernel Libraries/";

/// Whether a path is the XMI serialization of a KerML library file (`.kermlx`).
pub fn is_kerml_library_xmi(path: &str) -> bool {
    path.starts_with(KERML_LIBRARY_XMI) && path.ends_with(".kermlx")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    #[test]
    fn corpus_files() {
        assert!(is_corpus_file("normative/Systems-Library.kpar"));
        assert!(is_corpus_file("informative/ptc-25-04-31.sysml"));
        assert!(is_corpus_file("x/Base.kerml"));
        // The UUID oracle's XMI serializations are reference data, not corpus files (ADR 0009).
        assert!(!is_corpus_file(
            "SysML-v2-Release/2025-02/sysml.library.xmi/Systems Library/Parts.sysmlx"
        ));
        assert!(!is_corpus_file(
            "SysML-v2-Release/2025-02/sysml.library.xmi/Kernel Libraries/Base.kermlx"
        ));
        assert!(!is_corpus_file("normative/SysML.json"));
        assert!(!is_corpus_file(
            "KerML/OMG KerML v1.0 (formal-26-03-01).pdf"
        ));
    }

    #[test]
    fn languages() {
        assert_eq!(Language::of_path("a.kerml"), Some(Language::KerML));
        assert_eq!(Language::of_path("a.sysml"), Some(Language::SysML));
        assert_eq!(Language::of_path("a.sysmlx"), None);
    }
}
