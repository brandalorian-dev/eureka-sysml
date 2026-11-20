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

use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};

/// The local spec folder: `EUREKA_SPECS_DIR` if set, otherwise `docs/specs/` in this repository.
pub fn specs_dir() -> PathBuf {
    std::env::var_os("EUREKA_SPECS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/specs")))
}

const LOCK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../specs.lock.json");

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

/// What a corpus case is expected to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expectation {
    /// A valid model: it parses, resolves and validates without errors. Every standard library
    /// file and example model is valid.
    Valid,
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
    /// Checksums, keyed by file path.
    #[serde(default)]
    pub checksum: BTreeMap<String, Checksum>,
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
    /// The model files, sorted by path.
    pub sources: Vec<Source>,
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
    /// Load the corpus from [`specs_dir`].
    pub fn load() -> Result<Corpus, CorpusError> {
        Corpus::load_from(&specs_dir())
    }

    /// Load the corpus files that `specs.lock.json` lists from `specs`.
    pub fn load_from(specs: &Path) -> Result<Corpus, CorpusError> {
        let lock: Value = serde_json::from_str(&read_text(Path::new(LOCK))?)
            .map_err(|e| CorpusError(format!("specs.lock.json: {e}")))?;
        let entries = lock["files"]
            .as_array()
            .ok_or_else(|| CorpusError("specs.lock.json has no files".into()))?;
        let mut corpus = Corpus {
            libraries: Vec::new(),
            models: Vec::new(),
        };
        for e in entries {
            let path = e["path"].as_str().unwrap_or_default();
            if path.ends_with(".kpar") {
                let url = e["url"].as_str().unwrap_or_default().to_owned();
                corpus.libraries.push(read_library(specs, path, url)?);
            } else if let Some(language) = Language::of_path(path) {
                corpus.models.push(Source {
                    id: path.to_owned(),
                    path: path.to_owned(),
                    language,
                    text: read_text(&specs.join(path))?,
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

    /// The library that a usage `resource` IRI refers to, after [`ERRATA`] corrections.
    pub fn resolve_usage(&self, resource: &str) -> Option<&Library> {
        let iri = ERRATA
            .iter()
            .find_map(|e| match *e {
                Erratum::UsageResource { listed, actual, .. } if listed == resource => Some(actual),
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

fn read_text(path: &Path) -> Result<String, CorpusError> {
    std::fs::read_to_string(path).map_err(|e| CorpusError(format!("{}: {e}", path.display())))
}

/// Operating-system metadata that archiving tools add (ADR 0006).
fn is_os_metadata(path: &str) -> bool {
    let file = path.rsplit('/').next().unwrap_or(path);
    path.starts_with("__MACOSX/") || matches!(file, ".DS_Store" | "Thumbs.db" | "desktop.ini")
}

fn read_library(specs: &Path, file: &str, url: String) -> Result<Library, CorpusError> {
    let err = |msg: String| CorpusError(format!("{file}: {msg}"));
    let archive = std::fs::File::open(specs.join(file)).map_err(|e| err(e.to_string()))?;
    let mut zip = zip::ZipArchive::new(archive).map_err(|e| err(e.to_string()))?;

    let mut files: Vec<(String, String)> = Vec::new();
    let mut ignored = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| err(e.to_string()))?;
        // A name that is not valid UTF-8 is an error rather than being replaced lossily.
        let name = entry.name().map_err(|e| err(e.to_string()))?.into_owned();
        if entry.is_dir() {
            continue;
        }
        if is_os_metadata(&name) {
            ignored.push(name);
            continue;
        }
        let mut text = String::new();
        entry
            .read_to_string(&mut text)
            .map_err(|e| err(format!("{name}: {e}")))?;
        files.push((name, text));
    }

    // ADR 0006: KerML 10.3 puts .project.json and .meta.json at the archive's top level. The OMG
    // archives wrap the whole project in one folder; that folder is then the project root.
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
    for (name, text) in files {
        let path = name[root.len()..].to_owned();
        match path.as_str() {
            ".project.json" => project_json = Some(text),
            ".meta.json" => meta_json = Some(text),
            _ => match Language::of_path(&path) {
                Some(language) => models.push((path, language, text)),
                None => return Err(err(format!("unexpected archive entry {name}"))),
            },
        }
    }
    let parse = |what: &str, text: Option<String>| -> Result<Value, CorpusError> {
        let text = text.ok_or_else(|| err(format!("no {what} at the project root")))?;
        serde_json::from_str(&text).map_err(|e| err(format!("{what}: {e}")))
    };
    let project_json = parse(".project.json", project_json)?;
    let meta_json = parse(".meta.json", meta_json)?;
    let project: Project = serde_json::from_value(project_json.clone())
        .map_err(|e| err(format!(".project.json: {e}")))?;
    let meta: Meta =
        serde_json::from_value(meta_json.clone()).map_err(|e| err(format!(".meta.json: {e}")))?;

    let language = models
        .first()
        .map(|m| m.1)
        .ok_or_else(|| err("no model files".into()))?;
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

    Ok(Library {
        file: file.to_owned(),
        url,
        project,
        meta,
        project_json,
        meta_json,
        language,
        sources,
        ignored,
    })
}
