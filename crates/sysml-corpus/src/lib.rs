// SPDX-License-Identifier: Apache-2.0
//! Test corpus harness (PROJECT_PLAN §8: "Conformance corpus as the acceptance gate").
//!
//! [`Corpus::load`] reads every model in the local, never-committed `docs/specs/` that
//! `specs.lock.json` lists:
//! - the 10 standard libraries (3 KerML, 7 SysML), as project interchange files (`.kpar`,
//!   KerML 10.3), with their `.project.json` and `.meta.json`;
//! - the example models (`informative/*.sysml`, such as SimpleVehicleModel).
//!
//! Each [`Source`] has a stable id, its [`Language`], its text and an [`Expectation`]. Parser,
//! semantics and interchange tests iterate [`Corpus::sources`] and report failures by id.
//!
//! The OMG archives deviate from KerML 10.3 in a few places. Those are corrected by the
//! [`ERRATA`] table and recorded in ADR 0006.
//!
//! The corpus loads on every target. WebAssembly has no file system, so on wasm32 [`read_spec`]
//! serves copies of the spec files that `build.rs` embeds at build time.

use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::io::Read;
use std::path::PathBuf;

mod files;
use files::is_library_archive;
pub use files::{is_corpus_file, is_kerml_library_xmi, Language, KERML_LIBRARY_XMI};

/// The path of the XMI serialization of a KerML library source, by its id
/// (`Kernel Semantic Library/Base.kerml` gives `…/Kernel Libraries/Kernel Semantic Library/Base.kermlx`):
/// the abstract syntax the OMG pilot implementation built from it, used as a test oracle.
pub fn kerml_library_xmi(source_id: &str) -> String {
    format!("{KERML_LIBRARY_XMI}{source_id}x")
}

/// The local spec folder: `EUREKA_SPECS_DIR` if set, otherwise `docs/specs/` in this repository.
pub fn specs_dir() -> PathBuf {
    std::env::var_os("EUREKA_SPECS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/specs")))
}

/// The contents of the repository's `specs.lock.json`, compiled in so that it is available on
/// every target. [`Corpus::load`] checks every corpus file against it, so `EUREKA_SPECS_DIR` must
/// point at a folder holding the files this lock pins. Editing the file takes effect at the next
/// build.
pub const LOCK: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../specs.lock.json"
));

#[cfg(target_arch = "wasm32")]
mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded_specs.rs"));
}

/// Read a spec file by its path in `specs.lock.json`, for example
/// `KerML/normative/KerML-Model-Interchange.json`.
///
/// Natively this reads from [`specs_dir`]. On wasm32, which has no file system, it serves the
/// copy embedded at build time; `build.rs` embeds the corpus files and the few others the tests
/// read.
pub fn read_spec(path: &str) -> Result<Vec<u8>, CorpusError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let full = specs_dir().join(path);
        std::fs::read(&full).map_err(|e| CorpusError(format!("{}: {e}", full.display())))
    }
    #[cfg(target_arch = "wasm32")]
    {
        embedded::FILES
            .iter()
            .find(|(p, _)| *p == path)
            .map(|(_, bytes)| bytes.to_vec())
            .ok_or_else(|| {
                CorpusError(format!(
                    "{path} is not embedded for wasm32; add it to EXTRA in crates/sysml-corpus/build.rs"
                ))
            })
    }
}

/// The largest archive entry the loader reads, in bytes. The largest standard library file is
/// about 150 KB; the limit stops a malformed or hostile archive from exhausting memory.
pub const MAX_ENTRY_BYTES: u64 = 16 * 1024 * 1024;

/// What a corpus case is expected to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expectation {
    /// A valid model: it parses, resolves and validates without errors. Every standard library
    /// file and example model is valid, and so is every spec snippet not marked otherwise.
    Valid,
    /// Part of a model, or a schematic with placeholders such as `...`: not a complete root
    /// namespace, so not required to parse (spec snippets only, ADR 0016).
    Fragment,
    /// Not a valid model: the parser must report an error. For spec snippets that are mistakes in
    /// the specification, or extraction artifacts (ADR 0016).
    Invalid,
}

/// A textual example from a specification (step 1a.5, ADR 0016), extracted from the PDF by
/// `scripts/extract_spec_snippets.py` into `docs/specs/snippets/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snippet {
    /// The source. Its id is `<spec>/<clause>/<n>`, for example `kerml/7.2.5.4/1`, and its path
    /// is under `docs/specs/snippets/`.
    pub source: Source,
    /// `kerml` or `sysml` (SysML v2.0 Part 1).
    pub spec: String,
    /// The clause the example is in, for example `7.2.5.4`.
    pub clause: String,
    /// The page of the PDF it starts on.
    pub page: u32,
    /// Why the snippet is not valid, if it is not (`crates/sysml-corpus/snippets.json`).
    pub reason: Option<String>,
}

/// The expectations of the spec snippets that are not valid models, by id, with the reason:
/// `crates/sysml-corpus/snippets.json`, compiled in.
pub fn snippet_expectations() -> Result<BTreeMap<String, (Expectation, String)>, CorpusError> {
    let table: Value = serde_json::from_str(include_str!("../snippets.json"))
        .map_err(|e| CorpusError(format!("snippets.json: {e}")))?;
    let mut out = BTreeMap::new();
    for (id, entry) in table.as_object().into_iter().flatten() {
        if id.starts_with('$') {
            continue;
        }
        let expect = match entry[0].as_str() {
            Some("fragment") => Expectation::Fragment,
            Some("invalid") => Expectation::Invalid,
            other => {
                return Err(CorpusError(format!(
                    "snippets.json: {id}: unknown {other:?}"
                )))
            }
        };
        let reason = entry[1].as_str().unwrap_or_default().to_owned();
        out.insert(id.clone(), (expect, reason));
    }
    Ok(out)
}

/// The spec snippets, in the order of the specifications (ADR 0016). They are not in
/// `specs.lock.json`: `python scripts/extract_spec_snippets.py` extracts them from the pinned PDFs
/// into `docs/specs/snippets/`, and on wasm32 `build.rs` embeds them.
pub fn snippets() -> Result<Vec<Snippet>, CorpusError> {
    let index = read_spec("snippets/index.json").map_err(|e| {
        CorpusError(format!(
            "{e}. Run `python scripts/extract_spec_snippets.py` to extract the spec snippets."
        ))
    })?;
    snippets_from(&index, read_spec)
}

/// The snippets of an `index.json` (version 1), reading each file with `read`. Anything the
/// index does not state exactly (its version, a field, the language) is an error rather than a
/// guess, so that a newer extraction cannot be misread.
fn snippets_from(
    index: &[u8],
    read: impl Fn(&str) -> Result<Vec<u8>, CorpusError>,
) -> Result<Vec<Snippet>, CorpusError> {
    let bad = |what: String| CorpusError(format!("snippets/index.json: {what}"));
    let index: Value = serde_json::from_slice(index).map_err(|e| bad(e.to_string()))?;
    if index["version"].as_u64() != Some(1) {
        return Err(bad(format!(
            "version {} is not 1: re-extract with `python scripts/extract_spec_snippets.py`",
            index["version"]
        )));
    }
    let entries = index["snippets"]
        .as_array()
        .ok_or_else(|| bad("no `snippets` list".to_owned()))?;
    let expectations = snippet_expectations()?;
    let mut out = Vec::new();
    for (n, entry) in entries.iter().enumerate() {
        let field = |name: &str| {
            entry[name]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| bad(format!("entry {n} has no string `{name}`")))
        };
        let id = field("id")?;
        let path = format!("snippets/{}", field("path")?);
        let language = match field("language")?.as_str() {
            "kerml" => Language::KerML,
            "sysml" => Language::SysML,
            other => return Err(bad(format!("{id}: unknown language `{other}`"))),
        };
        let page = entry["page"]
            .as_u64()
            .and_then(|p| u32::try_from(p).ok())
            .ok_or_else(|| bad(format!("{id}: no numeric `page`")))?;
        let text =
            String::from_utf8(read(&path)?).map_err(|e| CorpusError(format!("{path}: {e}")))?;
        let (expect, reason) = match expectations.get(&id) {
            Some((expect, reason)) => (*expect, Some(reason.clone())),
            None => (Expectation::Valid, None),
        };
        out.push(Snippet {
            source: Source {
                id,
                path,
                language,
                text,
                expect,
            },
            spec: field("spec")?,
            clause: field("clause")?,
            page,
            reason,
        });
    }
    Ok(out)
}

/// One textual model in the corpus.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// A stable, unique id. For a library file: `<project name>/<path in project>`, for example
    /// `Kernel Semantic Library/Base.kerml`. For a model: its path under `docs/specs/`.
    pub id: String,
    /// The path within its project (library files) or under `docs/specs/` (models).
    pub path: String,
    /// The language, from the file extension.
    pub language: Language,
    /// The file contents (UTF-8).
    pub text: String,
    /// What the case is expected to do.
    pub expect: Expectation,
}

/// `.project.json` (KerML 10.3, Table 12; the `Project` definition in KerML-Model-Interchange.json).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Project {
    /// The project name.
    pub name: String,
    /// An optional description.
    #[serde(default)]
    pub description: Option<String>,
    /// The project version.
    pub version: String,
    /// An optional license.
    #[serde(default)]
    pub license: Option<String>,
    /// Maintainers.
    #[serde(default)]
    pub maintainer: Vec<String>,
    /// An optional website IRI.
    #[serde(default)]
    pub website: Option<String>,
    /// Topics.
    #[serde(default)]
    pub topic: Vec<String>,
    /// The projects this project uses.
    #[serde(default)]
    pub usage: Vec<Usage>,
}

/// One entry of `.project.json` `usage` (KerML 10.3, InterchangeProjectUsage).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// The IRI of the used project.
    pub resource: String,
    /// The versions of the used project that may be used.
    #[serde(default)]
    pub version_constraint: Option<String>,
}

/// `.meta.json` (KerML 10.3, Table 13; the `Meta` definition in KerML-Model-Interchange.json).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    /// Top-level element names and short names, mapped to the file that contains them.
    pub index: BTreeMap<String, String>,
    /// When the archive was created (ISO 8601).
    pub created: String,
    /// The language metamodel IRI. KerML if absent.
    #[serde(default)]
    pub metamodel: Option<String>,
    /// Whether derived property values are included.
    #[serde(default)]
    pub includes_derived: Option<bool>,
    /// Whether implied relationships are included.
    #[serde(default)]
    pub includes_implied: Option<bool>,
    /// Checksums, keyed by file path relative to the project root. Verified on loading.
    #[serde(default)]
    pub checksum: BTreeMap<String, Checksums>,
}

/// The checksums for one file. KerML 10.3 Table 13 describes "a list of one or more objects",
/// while the normative KerML-Model-Interchange.json defines a single object; both are read
/// (ADR 0006).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Checksums {
    /// A single checksum, as in the normative schema.
    One(Checksum),
    /// A list of checksums, as in Table 13.
    Many(Vec<Checksum>),
}

impl Checksums {
    /// Every checksum given for the file.
    pub fn all(&self) -> &[Checksum] {
        match self {
            Checksums::One(c) => std::slice::from_ref(c),
            Checksums::Many(cs) => cs,
        }
    }
}

/// One `.meta.json` checksum.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Checksum {
    /// The checksum value.
    pub value: String,
    /// The algorithm, for example `SHA256`.
    pub algorithm: String,
}

/// A standard library: one project interchange file.
#[derive(Clone, Debug)]
pub struct Library {
    /// The archive path under `docs/specs/`, as in `specs.lock.json`.
    pub file: String,
    /// The pinned download URL, which is also the IRI other projects use it by.
    pub url: String,
    /// The parsed `.project.json`.
    pub project: Project,
    /// The parsed `.meta.json`.
    pub meta: Meta,
    /// `.project.json` as written.
    pub project_json: Value,
    /// `.meta.json` as written.
    pub meta_json: Value,
    /// The language of every model file in the archive.
    pub language: Language,
    /// The textual model files, sorted by path.
    pub sources: Vec<Source>,
    /// JSON (`.json`) and XMI (`.xmi`) model interchange files (KerML 10.2), sorted by path. They
    /// are recorded but not loaded as sources.
    pub interchange_files: Vec<String>,
    /// Archive entries skipped as operating-system metadata (ADR 0006), as full archive paths.
    pub ignored: Vec<String>,
}

impl Library {
    /// The model file at `path`, relative to the project root.
    pub fn source_at(&self, path: &str) -> Option<&Source> {
        self.sources.iter().find(|s| s.path == path)
    }

    /// The model file that `.meta.json` indexes `name` to, after [`ERRATA`] corrections. `name`
    /// may be the entry as listed or, for a misspelled entry, the element's actual name.
    pub fn resolve_index(&self, name: &str) -> Option<&Source> {
        if let Some(s) = self
            .meta
            .index
            .get(name)
            .and_then(|listed| self.source_at(listed))
        {
            return Some(s);
        }
        ERRATA.iter().find_map(|e| match *e {
            Erratum::IndexPath {
                library,
                name: listed_name,
                actual_name,
                actual,
                ..
            } if library == self.project.name && (name == listed_name || name == actual_name) => {
                self.source_at(actual)
            }
            _ => None,
        })
    }
}

/// A known defect in the OMG library archives, and its correction (ADR 0006).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Erratum {
    /// A `.meta.json` index entry names a file that is not in the archive.
    IndexPath {
        /// The project name.
        library: &'static str,
        /// The index entry as listed.
        name: &'static str,
        /// The name the element actually has (the same as `name` unless that is also wrong).
        actual_name: &'static str,
        /// The file the index names.
        listed: &'static str,
        /// The file that contains the element.
        actual: &'static str,
    },
    /// A `.project.json` usage names a resource IRI that no library has.
    UsageResource {
        /// The using project's name.
        library: &'static str,
        /// The IRI as listed.
        listed: &'static str,
        /// The IRI of the intended library.
        actual: &'static str,
    },
}

/// Known defects in the OMG 20250201 library archives. The corpus tests check that each one still
/// occurs, so a corrected upstream release shows up as a test failure.
pub const ERRATA: &[Erratum] = &[
    Erratum::IndexPath {
        library: "SysML Systems Library",
        name: "AnalysisCases",
        actual_name: "AnalysisCases",
        listed: "AnalysisCase.sysml",
        actual: "AnalysisCases.sysml",
    },
    Erratum::IndexPath {
        library: "SysML Requirement Derivation Library",
        // The file declares `package RequirementDerivation`; the entry misspells the name too.
        name: "RequirementsDerivation",
        actual_name: "RequirementDerivation",
        listed: "RequirementsDerivation.sysml",
        actual: "RequirementDerivation.sysml",
    },
    Erratum::UsageResource {
        library: "SysML Analysis Library",
        listed: "https://www.omg.org/spec/KerML/20250201/Quantities-and-Units-Library.kpar",
        actual: "https://www.omg.org/spec/SysML/20250201/Quantities-and-Units-Domain-Library.kpar",
    },
    Erratum::UsageResource {
        library: "SysML Geometry Library",
        listed: "https://www.omg.org/spec/SysML/20250201/Quantities-and-Units-Library.kpar",
        actual: "https://www.omg.org/spec/SysML/20250201/Quantities-and-Units-Domain-Library.kpar",
    },
];

/// The whole corpus.
#[derive(Clone, Debug)]
pub struct Corpus {
    /// The standard libraries, in `specs.lock.json` order.
    pub libraries: Vec<Library>,
    /// The example models, in `specs.lock.json` order.
    pub models: Vec<Source>,
}

impl Corpus {
    /// Load the corpus, as pinned by the repository's `specs.lock.json`, through [`read_spec`].
    pub fn load() -> Result<Corpus, CorpusError> {
        Corpus::from_lock(LOCK, read_spec)
    }

    /// Load the files that the lock file `lock` lists, reading each one with `read` (given its
    /// path in the lock). Every file must match its SHA-256 in the lock.
    pub fn from_lock(
        lock: &str,
        read: impl Fn(&str) -> Result<Vec<u8>, CorpusError>,
    ) -> Result<Corpus, CorpusError> {
        let lock: Value =
            serde_json::from_str(lock).map_err(|e| CorpusError(format!("specs.lock.json: {e}")))?;
        let entries = lock["files"]
            .as_array()
            .ok_or_else(|| CorpusError("specs.lock.json has no files".into()))?;
        let mut corpus = Corpus {
            libraries: Vec::new(),
            models: Vec::new(),
        };
        for (i, e) in entries.iter().enumerate() {
            let field = |name: &str| e.get(name).and_then(Value::as_str);
            let path = field("path")
                .ok_or_else(|| CorpusError(format!("specs.lock.json: entry {i} has no path")))?;
            let lock_err = |msg: &str| CorpusError(format!("specs.lock.json: {path}: {msg}"));
            if is_library_archive(path) {
                let url = field("url")
                    .ok_or_else(|| lock_err("a library needs a url (its usage IRI)"))?;
                let bytes = read_locked(&read, path, field("sha256"))?;
                corpus
                    .libraries
                    .push(Library::from_archive(&bytes, path, url)?);
            } else if let Some(language) = Language::of_path(path) {
                let bytes = read_locked(&read, path, field("sha256"))?;
                let text = String::from_utf8(bytes).map_err(|_| lock_err("not valid UTF-8"))?;
                corpus.models.push(Source {
                    id: path.to_owned(),
                    path: path.to_owned(),
                    language,
                    text,
                    expect: Expectation::Valid,
                });
            }
        }
        Ok(corpus)
    }

    /// Every source: the library files, then the example models.
    pub fn sources(&self) -> impl Iterator<Item = &Source> {
        self.libraries
            .iter()
            .flat_map(|l| &l.sources)
            .chain(&self.models)
    }

    /// The library with this project name.
    pub fn library(&self, name: &str) -> Option<&Library> {
        self.libraries.iter().find(|l| l.project.name == name)
    }

    /// The library that the project named `using` refers to by the usage IRI `resource`. An
    /// [`ERRATA`] correction applies only to the library it was recorded for.
    pub fn resolve_usage(&self, using: &str, resource: &str) -> Option<&Library> {
        let iri = ERRATA
            .iter()
            .find_map(|e| match *e {
                Erratum::UsageResource {
                    library,
                    listed,
                    actual,
                } if library == using && listed == resource => Some(actual),
                _ => None,
            })
            .unwrap_or(resource);
        self.libraries.iter().find(|l| l.url == iri)
    }
}

/// A corpus file is missing or malformed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CorpusError(pub String);

impl fmt::Display for CorpusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}. Run `python scripts/fetch_specs.py verify`.", self.0)
    }
}

impl std::error::Error for CorpusError {}

fn hex_sha256(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Read a corpus file and check it against its SHA-256 in `specs.lock.json`.
fn read_locked(
    read: &impl Fn(&str) -> Result<Vec<u8>, CorpusError>,
    path: &str,
    sha256: Option<&str>,
) -> Result<Vec<u8>, CorpusError> {
    let expected =
        sha256.ok_or_else(|| CorpusError(format!("specs.lock.json: {path}: no sha256")))?;
    let bytes = read(path)?;
    if !hex_sha256(&bytes).eq_ignore_ascii_case(expected) {
        return Err(CorpusError(format!(
            "{path} does not match its sha256 in specs.lock.json (modified, or EUREKA_SPECS_DIR holds another release)"
        )));
    }
    Ok(bytes)
}

/// Operating-system metadata that archiving tools add (ADR 0006).
fn is_os_metadata(path: &str) -> bool {
    let file = path.rsplit('/').next().unwrap_or(path);
    path.starts_with("__MACOSX/") || matches!(file, ".DS_Store" | "Thumbs.db" | "desktop.ini")
}

/// What an archive entry is (KerML 10.2, 10.3).
enum Entry {
    ProjectJson,
    MetaJson,
    /// A textual model file.
    Text(Language),
    /// A JSON or XMI model interchange file.
    Interchange,
}

fn classify(path: &str) -> Option<Entry> {
    match path {
        ".project.json" => Some(Entry::ProjectJson),
        ".meta.json" => Some(Entry::MetaJson),
        _ if path.ends_with(".json") || path.ends_with(".xmi") => Some(Entry::Interchange),
        _ => Language::of_path(path).map(Entry::Text),
    }
}

impl Library {
    /// Read a project interchange file (KerML 10.3) from its bytes. `file` names it in errors and
    /// `url` is the IRI other projects use it by.
    pub fn from_archive(bytes: &[u8], file: &str, url: &str) -> Result<Library, CorpusError> {
        let err = |msg: String| CorpusError(format!("{file}: {msg}"));
        let mut zip =
            zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| err(e.to_string()))?;

        let mut files: Vec<(String, Vec<u8>)> = Vec::new();
        let mut ignored = Vec::new();
        for i in 0..zip.len() {
            let entry = zip.by_index(i).map_err(|e| err(e.to_string()))?;
            // A name that is not valid UTF-8 is an error rather than being replaced lossily.
            let name = entry.name().map_err(|e| err(e.to_string()))?.into_owned();
            if entry.is_dir() {
                continue;
            }
            if is_os_metadata(&name) {
                ignored.push(name);
                continue;
            }
            let too_big = || err(format!("{name} is larger than {MAX_ENTRY_BYTES} bytes"));
            if entry.size() > MAX_ENTRY_BYTES {
                return Err(too_big());
            }
            // The declared size may be wrong, so the read itself is limited too.
            let mut data = Vec::new();
            entry
                .take(MAX_ENTRY_BYTES + 1)
                .read_to_end(&mut data)
                .map_err(|e| err(format!("{name}: {e}")))?;
            if data.len() as u64 > MAX_ENTRY_BYTES {
                return Err(too_big());
            }
            files.push((name, data));
        }

        // ADR 0006: KerML 10.3 puts .project.json and .meta.json at the archive's top level. The
        // OMG archives wrap the whole project in one folder; that folder is then the project root.
        let first = |n: &str| n.split('/').next().unwrap_or_default().to_owned();
        let root = match files.first().map(|(n, _)| first(n)) {
            Some(top)
                if files
                    .iter()
                    .all(|(n, _)| n.contains('/') && first(n) == top) =>
            {
                format!("{top}/")
            }
            _ => String::new(),
        };

        let mut project_json = None;
        let mut meta_json = None;
        let mut models = Vec::new();
        let mut interchange_files = Vec::new();
        let mut contents: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        for (name, data) in files {
            let path = name[root.len()..].to_owned();
            let text = |data: &[u8]| {
                String::from_utf8(data.to_vec())
                    .map_err(|_| err(format!("{name} is not valid UTF-8")))
            };
            match classify(&path) {
                Some(Entry::ProjectJson) => project_json = Some(text(&data)?),
                Some(Entry::MetaJson) => meta_json = Some(text(&data)?),
                Some(Entry::Text(language)) => models.push((path.clone(), language, text(&data)?)),
                Some(Entry::Interchange) => interchange_files.push(path.clone()),
                None => return Err(err(format!("unexpected archive entry {name} (KerML 10.2 allows .kerml/.sysml, .json and .xmi)"))),
            }
            contents.insert(path, data);
        }
        let parse = |what: &str, text: Option<String>| -> Result<Value, CorpusError> {
            let text = text.ok_or_else(|| err(format!("no {what} at the project root")))?;
            serde_json::from_str(&text).map_err(|e| err(format!("{what}: {e}")))
        };
        let project_json = parse(".project.json", project_json)?;
        let meta_json = parse(".meta.json", meta_json)?;
        let project: Project = serde_json::from_value(project_json.clone())
            .map_err(|e| err(format!(".project.json: {e}")))?;
        let meta: Meta = serde_json::from_value(meta_json.clone())
            .map_err(|e| err(format!(".meta.json: {e}")))?;

        // KerML 10.3 Table 13: checksums of some or all model interchange files.
        for (path, checksums) in &meta.checksum {
            let data = contents.get(path).ok_or_else(|| {
                err(format!(
                    ".meta.json has a checksum for {path}, which is not in the archive"
                ))
            })?;
            for c in checksums.all() {
                let actual = match c.algorithm.as_str() {
                    "SHA256" => hex_sha256(data),
                    other => {
                        return Err(err(format!(
                            "{path}: checksum algorithm {other} is not supported (only SHA256)"
                        )))
                    }
                };
                if !actual.eq_ignore_ascii_case(&c.value) {
                    return Err(err(format!(
                        "{path} does not match its .meta.json checksum"
                    )));
                }
            }
        }

        let language = models
            .first()
            .map(|m| m.1)
            .ok_or_else(|| err("no textual model files".into()))?;
        if models.iter().any(|m| m.1 != language) {
            return Err(err(
                "model files in more than one language (KerML 10.3)".into()
            ));
        }
        let mut sources: Vec<Source> = models
            .into_iter()
            .map(|(path, language, text)| Source {
                id: format!("{}/{path}", project.name),
                path,
                language,
                text,
                expect: Expectation::Valid,
            })
            .collect();
        sources.sort_by(|a, b| a.path.cmp(&b.path));
        interchange_files.sort();

        Ok(Library {
            file: file.to_owned(),
            url: url.to_owned(),
            project,
            meta,
            project_json,
            meta_json,
            language,
            sources,
            interchange_files,
            ignored,
        })
    }
}

#[cfg(test)]
mod snippet_index_tests {
    use super::*;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    fn read(_: &str) -> Result<Vec<u8>, CorpusError> {
        Ok(b"class C;\n".to_vec())
    }

    fn index(entry: &str) -> String {
        format!(r#"{{"version": 1, "snippets": [{entry}]}}"#)
    }

    const GOOD: &str = r#"{"id": "kerml/7.2/1", "spec": "kerml", "clause": "7.2", "page": 3,
        "language": "kerml", "path": "kerml/7.2-1.kerml"}"#;

    #[test]
    fn a_well_formed_index() {
        let s = snippets_from(index(GOOD).as_bytes(), read).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].source.id, "kerml/7.2/1");
        assert_eq!(s[0].source.language, Language::KerML);
        assert_eq!(s[0].page, 3);
    }

    #[test]
    fn a_malformed_index_is_an_error() {
        let bad = [
            // An index this loader does not know.
            r#"{"version": 2, "snippets": []}"#.to_owned(),
            // A language that is neither `kerml` nor `sysml`.
            index(&GOOD.replace(r#""language": "kerml""#, r#""language": "kerml-expr""#)),
            // A missing or non-numeric page, and a missing id.
            index(&GOOD.replace(r#""page": 3"#, r#""page": "3""#)),
            index(&GOOD.replace(r#""id": "kerml/7.2/1", "#, "")),
        ];
        for text in bad {
            assert!(snippets_from(text.as_bytes(), read).is_err(), "{text}");
        }
    }
}
