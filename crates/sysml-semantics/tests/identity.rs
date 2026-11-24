// SPDX-License-Identifier: Apache-2.0
//! Library element identity (KerML 9.1; step 1a.11): the `elementId` of every element of a
//! standard library package is a version 5 UUID. The pilot's ids are the oracle of every written
//! element (`tests/derived_oracle.rs`); these tests cover the rest.

mod common;

use common::{model, model_with_library};
use std::collections::HashMap;
use sysml_semantics::identity;
use sysml_semantics::Global;
use uuid::Uuid;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[test]
fn a_top_level_library_package_has_the_uuid_of_its_url() {
    // uuid5(NameSpace_URL, "https://www.omg.org/spec/KerML/" + name), computed apart from the
    // engine (Python's `uuid.uuid5`) and so from the pilot's ids
    let expected = [
        ("Base", "cdd5d1e3-fe4b-52bd-8a01-51a53f22ba47"),
        ("ScalarValues", "40bb440c-5036-58e1-8675-5afccb8b8f1d"),
        ("Links", "d7ddb15a-29e3-558b-bd3d-ac8b2fa107f2"),
        ("Occurrences", "4f007a44-242e-5ef2-8d16-1f11cff07447"),
    ];
    let m = model_with_library("");
    for (name, id) in expected {
        assert_eq!(
            identity::library_package_id(identity::KERML, name).to_string(),
            id
        );
        let package = sysml_semantics::derived::library(&m.db, m.project, name).unwrap();
        assert_eq!(
            identity::element_id(&m.db, m.project, package).map(|u| u.to_string()),
            Some(id.to_owned()),
            "{name}"
        );
    }
}

#[test]
fn a_sysml_library_package_has_the_sysml_url() {
    // SysML 9.1: the prefix of the SysML libraries, here for ISQ, computed apart from the engine
    assert_eq!(
        identity::library_package_id(identity::SYSML, "ISQ").to_string(),
        "3fcc2ef4-a31c-522b-b69d-717d74bccfa6"
    );
    // a package written in SysML notation is one of the SysML libraries (ADR 0009)
    let db = sysml_semantics::Database::default();
    let file = sysml_semantics::SourceFile::new(
        &db,
        sysml_model::FileId(0),
        "ISQ.sysml".into(),
        sysml_syntax::Language::SysML,
        "standard library package ISQ { }".into(),
    );
    let project = sysml_semantics::Project::new(&db, vec![file]);
    let root = sysml_semantics::lowered(&db, file).root;
    let g = &sysml_semantics::lowered(&db, file).graph;
    let package = g.owned_elements(root).next().unwrap();
    let x = Global {
        file,
        index: package.index() as u32,
    };
    assert_eq!(
        identity::element_id(&db, project, x).map(|u| u.to_string()),
        Some("3fcc2ef4-a31c-522b-b69d-717d74bccfa6".to_owned())
    );
}

#[test]
fn an_element_of_a_library_package_has_the_uuid_of_its_path() {
    // uuid5(the package's id, path())
    let m = model_with_library("");
    let (db, p) = (&m.db, m.project);
    let x = sysml_semantics::derived::library(db, p, "Base::things").unwrap();
    let base = identity::library_package_id(identity::KERML, "Base");
    let path = sysml_semantics::derived::path(db, p, x);
    assert_eq!(
        identity::element_id(db, p, x),
        Some(Uuid::new_v5(&base, path.as_bytes()))
    );
}

#[test]
fn implied_elements_of_the_libraries_have_distinct_uuids() {
    // their path() counts their position after the owned relationships (ADR 0022), so every
    // element of a library file, implied or not, has its own id
    let m = model_with_library("");
    let (db, p) = (&m.db, m.project);
    let mut ids: HashMap<Uuid, Global> = HashMap::new();
    let (mut implied, mut implied_with_id) = (0, 0);
    for f in p.files(db) {
        let v = sysml_semantics::view(db, p, *f);
        for index in 0..v.len() as u32 {
            let x = Global { file: *f, index };
            let id = identity::element_id(db, p, x);
            if v.is_implied(x.ix()) {
                implied += 1;
                implied_with_id += usize::from(id.is_some());
            }
            if let Some(id) = id {
                if let Some(other) = ids.insert(id, x) {
                    panic!("{id}: {x:?} and {other:?}");
                }
            }
        }
    }
    // every implied element of a library file is in its package, and so has an id; the count
    // is that of the libraries' overlays since step 1a.10
    assert_eq!(implied_with_id, implied);
    assert_eq!(implied, 52_327);
}

#[test]
fn elements_outside_standard_library_packages_have_no_library_uuid() {
    // their ids are DataIdentity ids, assigned outside the queries (ADR 0019)
    let m = model("package P { classifier C; } library package L { classifier D; }");
    let (db, p) = (&m.db, m.project);
    for path in ["P", "P::C", "L", "L::D"] {
        assert_eq!(identity::element_id(db, p, m.get(path)), None, "{path}");
    }
    assert_eq!(identity::element_id(db, p, m.root()), None);
    // a standard library package that is not top-level is not one 9.1 names
    let m = model("package P { standard library package L { classifier D; } }");
    assert_eq!(
        identity::element_id(&m.db, m.project, m.get("P::L::D")),
        None
    );
}
