// SPDX-License-Identifier: Apache-2.0
//! The corpus is complete and loadable: the 10 standard libraries (KerML 9, SysML 9) as project
//! interchange files (KerML 10.3), plus the SimpleVehicleModel example.

use std::collections::BTreeSet;
use std::sync::OnceLock;
use sysml_corpus::{Corpus, Erratum, Expectation, Language, ERRATA};

fn corpus() -> &'static Corpus {
    static CORPUS: OnceLock<Corpus> = OnceLock::new();
    CORPUS.get_or_init(|| Corpus::load().unwrap_or_else(|e| panic!("{e}")))
}

#[test]
fn ten_standard_libraries() {
    // (project name, version, language, number of model interchange files)
    let expected = [
        ("Kernel Data Type Library", "1.0.0", Language::KerML, 3),
        ("Kernel Function Library", "1.0.0", Language::KerML, 17),
        ("Kernel Semantic Library", "1.0.0", Language::KerML, 16),
        ("SysML Analysis Library", "2.0.0", Language::SysML, 4),
        (
            "SysML Cause and Effect Library",
            "2.0.0",
            Language::SysML,
            2,
        ),
        ("SysML Geometry Library", "2.0.0", Language::SysML, 2),
        ("SysML Metadata Library", "2.0.0", Language::SysML, 4),
        (
            "SysML Quantities and Units Library",
            "2.0.0",
            Language::SysML,
            23,
        ),
        (
            "SysML Requirement Derivation Library",
            "2.0.0",
            Language::SysML,
            2,
        ),
        ("SysML Systems Library", "2.0.0", Language::SysML, 21),
    ];
    let actual: Vec<_> = corpus()
        .libraries
        .iter()
        .map(|l| {
            (
                l.project.name.as_str(),
                l.project.version.as_str(),
                l.language,
                l.sources.len(),
            )
        })
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn one_language_per_library() {
    // KerML 10.3: "Each project interchange file shall only contain models in a single language",
    // and Table 13 note 3: `metamodel` identifies that language.
    for l in &corpus().libraries {
        let expected = match l.language {
            Language::KerML => "https://www.omg.org/spec/KerML/20250201",
            Language::SysML => "https://www.omg.org/spec/SysML/20250201",
        };
        assert_eq!(
            l.meta.metamodel.as_deref(),
            Some(expected),
            "{}",
            l.project.name
        );
        assert!(
            l.sources.iter().all(|s| s.language == l.language),
            "{}",
            l.project.name
        );
    }
}

#[test]
fn index_entries_resolve() {
    // KerML 10.3 Table 13 note 1: the index maps top-level names to model interchange files.
    for l in &corpus().libraries {
        let mut indexed = BTreeSet::new();
        for name in l.meta.index.keys() {
            let s = l.resolve_index(name).unwrap_or_else(|| {
                panic!("{}: index entry {name} does not resolve", l.project.name)
            });
            indexed.insert(s.id.as_str());
        }
        // Not required by KerML 10.3, but true of every standard library.
        let all: BTreeSet<&str> = l.sources.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            indexed, all,
            "{}: some files are not indexed",
            l.project.name
        );
    }
}

#[test]
fn index_names_occur_in_their_files() {
    // KerML 10.3 Table 13 note 1: the index lists the names of top-level elements. Until the
    // parser exists, check that each name occurs as a whole word in its file. This finds the
    // misspelled entries in ERRATA.
    let word = |text: &str, w: &str| {
        text.match_indices(w).any(|(i, _)| {
            let ident = |c: char| c.is_alphanumeric() || c == '_';
            !text[..i].ends_with(ident) && !text[i + w.len()..].starts_with(ident)
        })
    };
    let mut missing = BTreeSet::new();
    for l in &corpus().libraries {
        for name in l.meta.index.keys() {
            let s = l.resolve_index(name).unwrap();
            if !word(&s.text, name) {
                missing.insert((l.project.name.as_str(), name.as_str()));
            }
        }
    }
    let misspelled: BTreeSet<_> = ERRATA
        .iter()
        .filter_map(|e| match *e {
            Erratum::IndexPath {
                library,
                name,
                actual_name,
                ..
            } if name != actual_name => Some((library, name)),
            _ => None,
        })
        .collect();
    assert_eq!(missing, misspelled);
    // The actual name resolves too.
    let rd = corpus()
        .library("SysML Requirement Derivation Library")
        .unwrap();
    assert_eq!(
        rd.resolve_index("RequirementDerivation").unwrap().path,
        "RequirementDerivation.sysml"
    );
}

#[test]
fn index_errata_still_present() {
    // ADR 0006. When OMG fixes an archive, this fails and the erratum can be removed.
    let mut broken = BTreeSet::new();
    for l in &corpus().libraries {
        for (name, path) in &l.meta.index {
            if l.source_at(path).is_none() {
                broken.insert((l.project.name.as_str(), name.as_str(), path.as_str()));
            }
        }
    }
    let errata: BTreeSet<_> = ERRATA
        .iter()
        .filter_map(|e| match *e {
            Erratum::IndexPath {
                library,
                name,
                listed,
                ..
            } => Some((library, name, listed)),
            _ => None,
        })
        .collect();
    assert_eq!(broken, errata);
}

#[test]
fn usages_resolve_and_satisfy_version_constraints() {
    // KerML 10.3: every used project is identified by `resource`, optionally with a
    // `versionConstraint` that the used project's `version` must meet.
    let c = corpus();
    for l in &c.libraries {
        for u in &l.project.usage {
            let used = c
                .resolve_usage(&l.project.name, &u.resource)
                .unwrap_or_else(|| {
                    panic!("{}: usage {} does not resolve", l.project.name, u.resource)
                });
            if let Some(constraint) = &u.version_constraint {
                assert_eq!(
                    constraint, &used.project.version,
                    "{} uses {}",
                    l.project.name, used.project.name
                );
            }
        }
    }
}

#[test]
fn usage_errata_still_present() {
    let c = corpus();
    let listed: BTreeSet<(&str, &str)> = c
        .libraries
        .iter()
        .flat_map(|l| {
            l.project
                .usage
                .iter()
                .map(move |u| (l.project.name.as_str(), u.resource.as_str()))
        })
        .filter(|(_, r)| !c.libraries.iter().any(|l| l.url == *r))
        .collect();
    let errata: BTreeSet<(&str, &str)> = ERRATA
        .iter()
        .filter_map(|e| match *e {
            Erratum::UsageResource {
                library, listed, ..
            } => Some((library, listed)),
            _ => None,
        })
        .collect();
    assert_eq!(listed, errata);
}

#[test]
fn usage_errata_apply_only_to_their_library() {
    let c = corpus();
    let geometry_iri = "https://www.omg.org/spec/SysML/20250201/Quantities-and-Units-Library.kpar";
    let qu = c
        .resolve_usage("SysML Geometry Library", geometry_iri)
        .unwrap();
    assert_eq!(qu.project.name, "SysML Quantities and Units Library");
    // The same broken IRI listed by another library is reported, not silently corrected.
    assert!(c
        .resolve_usage("SysML Systems Library", geometry_iri)
        .is_none());
}

#[test]
fn os_metadata_is_ignored() {
    let systems = corpus().library("SysML Systems Library").unwrap();
    // Full archive paths. The __MACOSX folder sits beside the project folder, not inside it.
    assert_eq!(
        systems.ignored,
        [
            "Systems Library/.DS_Store",
            "__MACOSX/Systems Library/._.DS_Store"
        ]
    );
    for l in &corpus().libraries {
        assert!(l.sources.iter().all(|s| !s.id.contains("DS_Store")));
    }
}

#[test]
fn metadata_follows_the_interchange_schema() {
    // KerML 10.3: .project.json and .meta.json follow the Project and Meta definitions in the
    // normative KerML-Model-Interchange.json. Every property the archives use must be one the
    // schema defines, so the typed `Project` and `Meta` lose nothing.
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            sysml_corpus::specs_dir().join("KerML/normative/KerML-Model-Interchange.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let keys = |def: &str| -> BTreeSet<String> {
        schema["$defs"][def]["properties"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect()
    };
    let (project_keys, meta_keys) = (keys("Project"), keys("Meta"));
    for l in &corpus().libraries {
        for k in l.project_json.as_object().unwrap().keys() {
            assert!(
                project_keys.contains(k),
                "{}: .project.json property {k}",
                l.project.name
            );
        }
        for k in l.meta_json.as_object().unwrap().keys() {
            assert!(
                meta_keys.contains(k),
                "{}: .meta.json property {k}",
                l.project.name
            );
        }
    }
}

#[test]
fn simple_vehicle_model() {
    let svm = corpus()
        .models
        .iter()
        .find(|s| s.id == "informative/ptc-25-04-31.sysml")
        .expect("SimpleVehicleModel");
    assert_eq!(svm.language, Language::SysML);
    assert!(svm.text.starts_with("package SimpleVehicleModel"));
}

/// Lock entries that are not models: the specification documents, the metamodel and schema
/// files, the API pilot material, and the library UUID reference (the XMI serializations of
/// SysML-v2-Release 2025-02, ADR 0009). This list is independent of the loader's own filter, so a
/// model the loader skips (for example a new extension) fails the completeness test.
fn is_not_a_model(path: &str) -> bool {
    let ext = path.rsplit_once('.').map(|(_, e)| e).unwrap_or_default();
    ext == "pdf"
        || path.starts_with("api/")
        || (path.contains("normative/") && matches!(ext, "json" | "xmi"))
        || (path.starts_with("SysML-v2-Release/") && matches!(ext, "kermlx" | "sysmlx"))
}

#[test]
fn every_model_file_in_the_lock_is_in_the_corpus() {
    let lock: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../specs.lock.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let c = corpus();
    let in_corpus: BTreeSet<&str> = c
        .libraries
        .iter()
        .map(|l| l.file.as_str())
        .chain(c.models.iter().map(|m| m.id.as_str()))
        .collect();
    let models: BTreeSet<&str> = lock["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap())
        .filter(|p| !is_not_a_model(p))
        .collect();
    assert_eq!(
        models, in_corpus,
        "lock entries that are models must all be in the corpus"
    );
    assert_eq!((c.libraries.len(), c.models.len()), (10, 1));
}

#[test]
fn sources_are_unique_text() {
    let c = corpus();
    let ids: Vec<&str> = c.sources().map(|s| s.id.as_str()).collect();
    assert_eq!(ids.len(), 36 + 58 + 1);
    assert_eq!(
        ids.iter().collect::<BTreeSet<_>>().len(),
        ids.len(),
        "duplicate source ids"
    );
    assert!(ids.contains(&"Kernel Semantic Library/Base.kerml"));
    for s in c.sources() {
        assert!(!s.text.trim().is_empty(), "{} is empty", s.id);
        assert!(
            !s.text.starts_with('\u{feff}') && !s.text.contains('\0'),
            "{}",
            s.id
        );
        assert_eq!(s.expect, Expectation::Valid, "{}", s.id);
    }
}
