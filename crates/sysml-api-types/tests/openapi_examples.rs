// SPDX-License-Identifier: Apache-2.0
//! Conformance tests built from the examples in the normative REST/HTTP PSM (API 8.1, `OpenAPI.json`).
//!
//! Every example must deserialize into the record type that the PSM uses it for, and serialize back
//! to the same JSON. The examples are read from the local, never-committed `docs/specs/`.

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::sync::OnceLock;
use sysml_api_types::*;
use sysml_model::Metaclass;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

fn openapi() -> &'static Value {
    static DOC: OnceLock<Value> = OnceLock::new();
    DOC.get_or_init(|| {
        // Through sysml-corpus, so that it also works on wasm32, which has no file system.
        let bytes = sysml_corpus::read_spec("api/omg/normative/OpenAPI.json")
            .unwrap_or_else(|e| panic!("{e}"));
        serde_json::from_slice(&bytes).expect("OpenAPI.json is not valid JSON")
    })
}

/// Replace `{"$ref": "#/components/examples/…"}` links with the example they point to.
fn resolve(v: &Value) -> Value {
    match v {
        Value::Object(map) if map.len() == 1 && map.contains_key("$ref") => {
            let pointer = map["$ref"].as_str().unwrap().trim_start_matches('#');
            let target = openapi()
                .pointer(pointer)
                .unwrap_or_else(|| panic!("dangling example link {pointer}"));
            // ADR 0005: the `Commits` example links to the `Commit` example object rather than its
            // `value`. Read such a link as pointing at the value.
            let target = if !pointer.ends_with("/value") && target.get("value").is_some() {
                &target["value"]
            } else {
                target
            };
            resolve(target)
        }
        Value::Object(map) => {
            Value::Object(map.iter().map(|(k, v)| (k.clone(), resolve(v))).collect())
        }
        Value::Array(items) => Value::Array(items.iter().map(resolve).collect()),
        other => other.clone(),
    }
}

fn example(name: &str) -> Value {
    let raw = openapi()
        .pointer(&format!("/components/examples/{name}/value"))
        .unwrap_or_else(|| panic!("OpenAPI.json has no example {name}"));
    resolve(raw)
}

/// Deserialize the example as `T` and check that it serializes back to the same JSON.
fn round_trip<T: Serialize + DeserializeOwned>(name: &str) -> T {
    let json = example(name);
    let parsed: T = serde_json::from_value(json.clone()).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(
        serde_json::to_value(&parsed).unwrap(),
        json,
        "example {name} does not round-trip"
    );
    parsed
}

/// Examples that are not API records: JSON schemas served by the `/meta/datatypes` endpoints.
const NOT_RECORDS: &[&str] = &["Datatypes", "ExternalDataDatatype"];

const COVERED: &[&str] = &[
    "Project",
    "Projects",
    "ProjectRequest",
    "Branch",
    "Branches",
    "BranchRequest",
    "Tag",
    "Tags",
    "TagRequest",
    "Commit",
    "Commits",
    "ElementCommitRequest",
    "RelationshipCommitRequest",
    "ProjectUsageCommitRequest",
    "DeleteCommitRequest",
    "MergeCommit",
    "MergeConflicting",
    "Element",
    "Elements",
    "Relationship",
    "Relationships",
    "ProjectUsage",
    "ProjectUsageRequest",
    "DataVersion",
    "DataVersions",
    "DataIdentity",
    "DataDifference",
    "DataDifferences",
    "Query",
    "Queries",
    "QueryRequest",
];

#[test]
fn every_example_is_covered() {
    let all: BTreeSet<&str> = openapi()["components"]["examples"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let known: BTreeSet<&str> = COVERED.iter().chain(NOT_RECORDS).copied().collect();
    assert_eq!(
        all, known,
        "OpenAPI.json examples changed; add the new ones to this test"
    );
}

#[test]
fn project_branch_tag() {
    // API 7.1.2
    let p: Project = round_trip("Project");
    assert_eq!(p.name, "string");
    round_trip::<Vec<Project>>("Projects");
    let req: ProjectRequest = round_trip("ProjectRequest");
    assert_eq!(req.description, Some(Some("string".to_owned())));
    assert_eq!(req.alias, None);

    let b: Branch = round_trip("Branch");
    assert_eq!(b.deleted, None);
    round_trip::<Vec<Branch>>("Branches");
    let breq: BranchRequest = round_trip("BranchRequest");
    assert!(breq.head.is_some());

    round_trip::<Tag>("Tag");
    round_trip::<Vec<Tag>>("Tags");
    round_trip::<TagRequest>("TagRequest");
}

#[test]
fn commits() {
    // API 7.1.2
    let c: Commit = round_trip("Commit");
    assert_eq!(c.previous_commit.len(), 1);
    round_trip::<Vec<Commit>>("Commits");

    let element: CommitRequest = round_trip("ElementCommitRequest");
    match &element.change[0].payload {
        Some(DataRequest::Element(e)) => assert_eq!(e.metaclass, Metaclass::Element),
        other => panic!("expected an element payload, got {other:?}"),
    }
    let relationship: CommitRequest = round_trip("RelationshipCommitRequest");
    match &relationship.change[0].payload {
        Some(DataRequest::Element(e)) => assert_eq!(e.metaclass, Metaclass::Dependency),
        other => panic!("expected a relationship payload, got {other:?}"),
    }
    let usage: CommitRequest = round_trip("ProjectUsageCommitRequest");
    assert!(matches!(
        usage.change[0].payload,
        Some(DataRequest::ProjectUsage(_))
    ));

    // API 7.1.2: a DataVersion whose payload is null deletes the data with that identity.
    let delete: CommitRequest = round_trip("DeleteCommitRequest");
    assert_eq!(delete.change[0].payload, None);
    assert!(delete.change[0].identity.is_some());
}

#[test]
fn merge_results() {
    // API 8.1.2: MergeResult.mergeCommit maps to Commit, MergeResult.conflict to DataIdentity [0..*].
    // ADR 0005: the MergeCommit example omits the required `alias` and `name`. They are accepted as
    // missing (empty and null) and are written out on serialization.
    let json = example("MergeCommit");
    let c: Commit = serde_json::from_value(json.clone()).expect("MergeCommit");
    assert!(c.alias.is_empty());
    assert_eq!(c.name, None);
    let mut expected = json;
    expected["alias"] = serde_json::json!([]);
    expected["name"] = Value::Null;
    assert_eq!(serde_json::to_value(&c).unwrap(), expected);

    round_trip::<Vec<DataIdentity>>("MergeConflicting");
}

#[test]
fn elements_and_relationships() {
    // API 7.1.2 Data; KerML 10.4.5 element serialization.
    let e: Element = round_trip("Element");
    assert_eq!(e.metaclass, Metaclass::Element);
    assert!(e.properties.contains_key("ownedRelationship"));
    round_trip::<Vec<Element>>("Elements");
    let r: Data = round_trip("Relationship");
    assert!(matches!(r, Data::Element(ref e) if e.metaclass == Metaclass::Dependency));
    round_trip::<Vec<Data>>("Relationships");
}

#[test]
fn project_usage() {
    // API 7.1.2
    let u: Data = round_trip("ProjectUsage");
    assert!(matches!(u, Data::ProjectUsage(_)));
    let req: ProjectUsageRequest = round_trip("ProjectUsageRequest");
    assert!(req.used_commit.is_some() && req.used_project.is_none());
}

#[test]
fn data_versions_and_differences() {
    // API 7.1.2, 7.2.3
    let v: DataVersion = round_trip("DataVersion");
    assert!(matches!(v.payload, Some(Data::Element(_))));
    round_trip::<Vec<DataVersion>>("DataVersions");
    round_trip::<DataIdentity>("DataIdentity");
    let d: DataDifference = round_trip("DataDifference");
    assert!(d.base_data.is_some() && d.compare_data.is_some());
    round_trip::<Vec<DataDifference>>("DataDifferences");
}

#[test]
fn queries() {
    // API 7.1.4
    let q: Query = round_trip("Query");
    match &q.r#where {
        Some(Constraint::PrimitiveConstraint(p)) => {
            assert!(!p.inverse);
            assert_eq!(p.operator, Operator::Eq);
        }
        other => panic!("expected a primitive constraint, got {other:?}"),
    }
    round_trip::<Vec<Query>>("Queries");
    let req: QueryRequest = round_trip("QueryRequest");
    match &req.r#where {
        Some(Some(ConstraintRequest::PrimitiveConstraint(p))) => {
            assert_eq!(p.inverse, None);
            assert_eq!(
                p.value,
                Some(vec![ConstraintValue::String("Element".to_owned())])
            );
        }
        other => panic!("expected a primitive constraint, got {other:?}"),
    }
}
