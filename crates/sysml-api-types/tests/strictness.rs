// SPDX-License-Identifier: Apache-2.0
//! What the API types accept and reject. Request bodies are checked strictly against
//! `Schema.json`; the leniency in ADR 0005 applies to response records only.

use serde_json::{json, Value};
use sysml_api_types::*;
use sysml_model::Metaclass;

const ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";

fn err<T: serde::de::DeserializeOwned + std::fmt::Debug>(v: Value) -> String {
    serde_json::from_value::<T>(v)
        .expect_err("expected a deserialization error")
        .to_string()
}

fn prim(value: Value) -> Value {
    json!({"@type": "PrimitiveConstraint", "inverse": false, "operator": "in", "property": "x", "value": value})
}

// ---- required, nullable request properties must be present ----

#[test]
fn data_version_request_requires_payload() {
    // API 7.1.2: payload null means the data is deleted, so a missing payload must not read as null.
    let missing = json!({"@type": "DataVersion", "identity": {"@id": ID}});
    assert!(err::<DataVersionRequest>(missing).contains("payload"));
    let delete: DataVersionRequest =
        serde_json::from_value(json!({"identity": {"@id": ID}, "payload": null})).unwrap();
    assert_eq!(delete.payload, None);
}

#[test]
fn branch_request_requires_head() {
    assert!(err::<BranchRequest>(json!({"name": "b"})).contains("head"));
    assert!(serde_json::from_value::<BranchRequest>(json!({"name": "b", "head": null})).is_ok());
}

#[test]
fn primitive_constraint_request_requires_value() {
    let missing = json!({"@type": "PrimitiveConstraint", "operator": "=", "property": "x"});
    assert!(err::<PrimitiveConstraintRequest>(missing).contains("value"));
}

// ---- required lists in requests must be present ----

#[test]
fn commit_request_requires_change() {
    assert!(err::<CommitRequest>(json!({"@type": "Commit"})).contains("change"));
}

#[test]
fn composite_constraint_requires_constraint() {
    let missing = json!({"@type": "CompositeConstraint", "operator": "and"});
    assert!(err::<CompositeConstraintRequest>(missing.clone()).contains("constraint"));
    assert!(err::<CompositeConstraint>(missing).contains("constraint"));
}

#[test]
fn response_records_stay_lenient() {
    // ADR 0005: the MergeCommit example omits `alias` and `name`.
    let c: Commit = serde_json::from_value(json!({
        "@id": ID, "@type": "Commit", "created": "1970-01-01T00:00:00.000Z", "description": null,
        "owningProject": {"@id": ID}, "previousCommit": []
    }))
    .unwrap();
    assert!(c.alias.is_empty() && c.name.is_none());
}

// ---- minItems ----

#[test]
fn composite_constraint_needs_two_constraints() {
    // Schema.json: CompositeConstraint.constraint has minItems 2 (API 7.1.4: "two or more Constraints").
    let one = json!({"@type": "CompositeConstraint", "operator": "and", "constraint": [prim(json!(["a"]))]});
    assert!(err::<CompositeConstraint>(one.clone()).contains("at least 2"));
    assert!(err::<CompositeConstraintRequest>(one).contains("at least 2"));
}

#[test]
fn primitive_constraint_value_needs_one_item() {
    // Schema.json: PrimitiveConstraint.value is null or a list with minItems 1.
    assert!(err::<PrimitiveConstraint>(prim(json!([]))).contains("at least 1"));
    assert!(err::<PrimitiveConstraintRequest>(prim(json!([]))).contains("at least 1"));
    assert!(serde_json::from_value::<PrimitiveConstraint>(prim(Value::Null)).is_ok());
}

// ---- identifiers ----

#[test]
fn ids_must_be_hyphenated_uuids() {
    // Schema.json `"format": "uuid"`: the RFC 9562 string form, 8-4-4-4-12 hex digits.
    for bad in [
        "urn:uuid:3fa85f64-5717-4562-b3fc-2c963f66afa6",
        "3fa85f6457174562b3fc2c963f66afa6",
        "{3fa85f64-5717-4562-b3fc-2c963f66afa6}",
        "not-a-uuid",
    ] {
        assert!(
            serde_json::from_value::<Identified>(json!({"@id": bad})).is_err(),
            "{bad}"
        );
    }
    let ok: Identified = serde_json::from_value(json!({"@id": ID})).unwrap();
    assert_eq!(serde_json::to_value(ok).unwrap(), json!({"@id": ID}));
}

#[test]
fn upper_case_ids_are_written_in_lower_case() {
    // RFC 9562 §4: hex digits are case-insensitive on input and written in lower case (ADR 0005).
    let upper = ID.to_ascii_uppercase();
    let r: Identified = serde_json::from_value(json!({"@id": upper})).unwrap();
    assert_eq!(r.id.to_string(), ID);
}

// ---- reserved keys in open property maps ----

#[test]
fn element_properties_cannot_shadow_id_or_type() {
    let mut properties = serde_json::Map::new();
    properties.insert("@type".into(), json!("PartUsage"));
    let e = Element {
        id: ID.parse().unwrap(),
        metaclass: Metaclass::PartUsage,
        properties,
    };
    assert!(serde_json::to_value(&e)
        .unwrap_err()
        .to_string()
        .contains("@type"));

    let mut properties = serde_json::Map::new();
    properties.insert("@id".into(), json!(ID));
    let r = ElementRequest {
        id: None,
        metaclass: Metaclass::PartUsage,
        properties,
    };
    assert!(serde_json::to_value(&r)
        .unwrap_err()
        .to_string()
        .contains("@id"));
}

#[test]
fn error_additional_cannot_shadow_named_properties() {
    let mut e: Error =
        serde_json::from_value(json!({"@type": "Error", "description": "x"})).unwrap();
    e.additional.insert("description".into(), json!("y"));
    assert!(serde_json::to_value(&e)
        .unwrap_err()
        .to_string()
        .contains("description"));
}

// ---- union errors name the problem ----

#[test]
fn union_errors_name_the_member_and_field() {
    let typo = json!({"@type": "ProjectUsage", "@id": ID, "usedComit": {"@id": ID}});
    let msg = err::<DataRequest>(typo);
    assert!(
        msg.contains("ProjectUsage") && msg.contains("usedComit"),
        "{msg}"
    );

    let msg = err::<Constraint>(
        json!({"@type": "PrimitiveConstraint", "inverse": false, "property": "x", "value": null}),
    );
    assert!(msg.contains("operator"), "{msg}");

    let msg = err::<Data>(json!({"@type": "NotAMetaclass", "@id": ID}));
    assert!(msg.contains("NotAMetaclass"), "{msg}");

    let msg = err::<Data>(json!({"@id": ID}));
    assert!(msg.contains("@type"), "{msg}");
}

#[test]
fn constraint_union_rejects_other_records() {
    let msg = err::<Constraint>(json!({"@type": "Project"}));
    assert!(msg.contains("Project"), "{msg}");
}
