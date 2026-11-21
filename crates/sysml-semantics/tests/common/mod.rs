// SPDX-License-Identifier: Apache-2.0
//! Helpers of the derived-property tests: a model from KerML text, and its elements found and
//! shown by their declared names through the graph, independently of the functions under test.

#![allow(dead_code)]

use sysml_model::{Attr, ElementIx, FileId, Metaclass};
use sysml_semantics::{lowered, Database, Global, Project, SourceFile};
use sysml_syntax::Language;

pub struct Model {
    pub db: Database,
    pub project: Project,
    pub file: SourceFile,
}

pub fn model(text: &str) -> Model {
    let db = Database::default();
    let file = SourceFile::new(
        &db,
        FileId(0),
        "test.kerml".into(),
        Language::KerML,
        text.into(),
    );
    let project = Project::new(&db, vec![file]);
    Model { db, project, file }
}

impl Model {
    pub fn graph(&self) -> std::sync::Arc<sysml_semantics::Lowered> {
        lowered(&self.db, self.file).clone()
    }

    pub fn at(&self, e: ElementIx) -> Global {
        Global {
            file: self.file,
            index: e.index() as u32,
        }
    }

    pub fn root(&self) -> Global {
        self.at(self.graph().root)
    }

    /// The element at a path of declared names (or short names); `#n` is the n-th (from 1) owned
    /// element with the metaclass name before it, for example `Comment#1`.
    pub fn get(&self, path: &str) -> Global {
        let l = self.graph();
        let g = &l.graph;
        let mut x = l.root;
        for segment in path.split("::") {
            let owned: Vec<ElementIx> = g.owned_elements(x).collect();
            x = match segment.split_once('#') {
                Some((m, n)) => *owned
                    .iter()
                    .filter(|e| g.metaclass(**e).name() == m)
                    .nth(n.parse::<usize>().unwrap() - 1)
                    .unwrap_or_else(|| panic!("no {segment} in {path}")),
                None => *owned
                    .iter()
                    .find(|e| {
                        g.string(**e, Attr::DeclaredName) == Some(segment)
                            || g.string(**e, Attr::DeclaredShortName) == Some(segment)
                    })
                    .unwrap_or_else(|| panic!("no {segment} in {path}")),
            };
        }
        self.at(x)
    }

    /// The owning relationship of the element at a path.
    pub fn membership(&self, path: &str) -> Global {
        let l = self.graph();
        self.at(l.graph.owning_relationship(self.get(path).ix()).unwrap())
    }

    /// The n-th (from 1) owned relationship of a metaclass of the element at a path.
    pub fn relationship(&self, path: &str, m: Metaclass, n: usize) -> Global {
        let l = self.graph();
        let owner = if path.is_empty() {
            l.root
        } else {
            self.get(path).ix()
        };
        let r = l
            .graph
            .owned_relationships(owner)
            .iter()
            .filter(|r| l.graph.metaclass(**r) == m)
            .nth(n - 1)
            .copied()
            .unwrap_or_else(|| panic!("no {m:?} {n} in {path}"));
        self.at(r)
    }

    /// An element as its declared names, through its owners; a relationship as its metaclass and
    /// what it owns or names.
    pub fn label(&self, x: Global) -> String {
        let l = lowered(&self.db, x.file).clone();
        let at = |e: ElementIx| Global {
            file: x.file,
            index: e.index() as u32,
        };
        let g = &l.graph;
        let m = g.metaclass(x.ix());
        if m.specializes(Metaclass::Relationship) && g.string(x.ix(), Attr::DeclaredName).is_none()
        {
            let owned = g.owned_related_elements(x.ix()).first();
            return match (owned, g.string(x.ix(), Attr::MemberName)) {
                (Some(e), _) => format!("{}({})", m.name(), self.label(at(*e))),
                (None, Some(n)) => format!("{}({n})", m.name()),
                (None, None) => m.name().to_owned(),
            };
        }
        let mut names = Vec::new();
        let mut e = Some(x.ix());
        while let Some(i) = e {
            match g
                .string(i, Attr::DeclaredName)
                .or(g.string(i, Attr::DeclaredShortName))
            {
                Some(n) => names.push(n.to_owned()),
                None if i == x.ix() => names.push(format!("({})", g.metaclass(i).name())),
                None => {}
            }
            e = g.owner(i);
        }
        names.reverse();
        names.join("::")
    }

    pub fn labels<T: std::borrow::Borrow<Global>>(
        &self,
        xs: impl IntoIterator<Item = T>,
    ) -> Vec<String> {
        xs.into_iter().map(|x| self.label(*x.borrow())).collect()
    }

    pub fn label_of(&self, x: Option<Global>) -> Option<String> {
        x.map(|x| self.label(x))
    }
}

pub fn strings(xs: Vec<std::sync::Arc<str>>) -> Vec<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

/// A model of KerML text with the KerML libraries, for what needs them (`Base::Anything` …).
pub fn model_with_library(text: &str) -> Model {
    let db = Database::default();
    let corpus = sysml_corpus::Corpus::load().unwrap();
    let mut files = Vec::new();
    for s in corpus.sources() {
        if s.language == sysml_corpus::Language::KerML {
            let id = FileId(files.len() as u32 + 1);
            files.push(SourceFile::new(
                &db,
                id,
                s.id.clone(),
                Language::KerML,
                s.text.clone(),
            ));
        }
    }
    let file = SourceFile::new(
        &db,
        FileId(0),
        "test.kerml".into(),
        Language::KerML,
        text.into(),
    );
    files.push(file);
    let project = Project::new(&db, files);
    Model { db, project, file }
}
