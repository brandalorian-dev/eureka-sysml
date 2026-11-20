// SPDX-License-Identifier: Apache-2.0
//! Schema rules for the API records (API 7.1, REST/HTTP PSM `Schema.json`) that the OpenAPI
//! examples do not exercise.

use serde_json::json;
use sysml_api_types::*;
use sysml_model::Metaclass;

const ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";

fn project() -> serde_json::Value {
    json!({
        "@id": ID, "@type": "Project", "alias": [], "created": "2026-11-20T00:00:00Z",
        "defaultBranch": {"@id": ID}, "description": null, "name": "p"
    })
}

#[test]
fn schema_definition_counts() {
    // Schema.json (Systems Modeling API 1.0): 26 record objects and 4 unions.
    assert_eq!(API_DEFINITIONS.len(), 30);
    assert!(API_DEFINITIONS.contains(&"PrimitiveConstraintRequest"));
    assert!(!API_DEFINITIONS.contains(&"ElementRequest"));
}

#[test]
fn type_tag_must_match() {
    assert!(serde_json::from_value::<Project>(project()).is_ok());
    let mut wrong = project();
    wrong["@type"] = json!("Branch");
    let err = serde_json::from_value::<Project>(wrong)
        .unwrap_err()
        .to_string();
    assert!(err.contains("Project"), "{err}");
}

#[test]
fn unknown_properties_are_rejected() {
    // Schema.json: "additionalProperties": false on every record except Error.
    let mut extra = project();
    extra["owner"] = json!("someone");
    assert!(serde_json::from_value::<Project>(extra).is_err());
}

#[test]
fn required_scalar_is_enforced() {
    let mut missing = project();
    missing.as_object_mut().unwrap().remove("created");
    assert!(serde_json::from_value::<Project>(missing).is_err());
}

#[test]
fn request_type_tag_is_optional_but_always_written() {
    // Schema.json: ProjectRequest requires only `name`.
    let req: ProjectRequest = serde_json::from_value(json!({"name": "p"})).unwrap();
    assert_eq!(
        serde_json::to_value(&req).unwrap(),
        json!({"@type": "Project", "name": "p"})
    );
}

#[test]
fn absent_and_null_are_distinct_in_requests() {
    // An optional, nullable request property has three states: absent, null and a value.
    let absent: ProjectRequest = serde_json::from_value(json!({"name": "p"})).unwrap();
    let null: ProjectRequest =
        serde_json::from_value(json!({"name": "p", "description": null})).unwrap();
    let value: ProjectRequest =
        serde_json::from_value(json!({"name": "p", "description": "d"})).unwrap();
    assert_eq!(absent.description, None);
    assert_eq!(null.description, Some(None));
    assert_eq!(value.description, Some(Some("d".to_owned())));
    assert_eq!(
        serde_json::to_value(&null).unwrap()["description"],
        serde_json::Value::Null
    );
    assert!(serde_json::to_value(&absent)
        .unwrap()
        .get("description")
        .is_none());
}

#[test]
fn required_nullable_is_written_as_null() {
    let p: Project = serde_json::from_value(project()).unwrap();
    assert_eq!(p.description, None);
    assert_eq!(serde_json::to_value(&p).unwrap(), project());
}

#[test]
fn commit_owning_project_is_a_project_reference() {
    // API 7.1.2: "owningProject is the Project that owns the Commit". Schema.json's `$comment`
    // names Branch; ADR 0005 follows the PIM. The type is the same `Identified`; this checks
    // that the generated documentation was corrected.
    assert_eq!(reference_target("Commit", "owningProject"), Some("Project"));
    assert_eq!(reference_target("Branch", "head"), Some("Commit"));
    assert_eq!(
        reference_target("ExternalRelationship", "elementEnd"),
        Some("Element")
    );
    assert_eq!(reference_target("Commit", "name"), None);
}

#[test]
fn error_keeps_additional_properties() {
    // Schema.json: Error is the only record with "additionalProperties": true.
    let json = json!({"@type": "Error", "description": "not found", "status": 404});
    let e: Error = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(e.description.as_deref(), Some("not found"));
    assert_eq!(e.additional["status"], json!(404));
    assert_eq!(serde_json::to_value(&e).unwrap(), json);
}

#[test]
fn operators_use_spec_literals() {
    // API 7.1.4: Operator and JoinOperator.
    let ops: Vec<&str> = Operator::ALL.iter().map(|o| o.as_str()).collect();
    assert_eq!(ops, ["<", "<=", "=", ">", ">=", "in", "instanceOf"]);
    assert_eq!(serde_json::to_value(Operator::Le).unwrap(), json!("<="));
    assert_eq!(
        serde_json::from_value::<Operator>(json!("instanceOf")).unwrap(),
        Operator::InstanceOf
    );
    assert!(serde_json::from_value::<Operator>(json!("!=")).is_err());
    let joins: Vec<&str> = JoinOperator::ALL.iter().map(|o| o.as_str()).collect();
    assert_eq!(joins, ["and", "or"]);
}

#[test]
fn constraint_values() {
    // API 7.1.4: a PrimitiveConstraint value is a list of booleans, numbers, strings,
    // element references or nulls.
    let c: PrimitiveConstraint = serde_json::from_value(json!({
        "@type": "PrimitiveConstraint", "inverse": true, "operator": "in", "property": "x",
        "value": [true, 4.5, 7, "s", {"@id": ID}, null]
    }))
    .unwrap();
    let v = c.value.as_ref().unwrap();
    assert_eq!(v[0], ConstraintValue::Boolean(true));
    assert!(matches!(&v[1], ConstraintValue::Number(n) if n.as_f64() == Some(4.5)));
    assert!(matches!(&v[2], ConstraintValue::Number(n) if n.as_u64() == Some(7)));
    assert_eq!(v[3], ConstraintValue::String("s".to_owned()));
    assert!(matches!(&v[4], ConstraintValue::Reference(r) if r.id.to_string() == ID));
    assert_eq!(v[5], ConstraintValue::Null);
}

#[test]
fn composite_constraints_nest() {
    let prim = json!({"@type": "PrimitiveConstraint", "inverse": false, "operator": "=",
                      "property": "name", "value": ["a"]});
    let json = json!({"@type": "CompositeConstraint", "operator": "or", "constraint": [
        prim.clone(),
        {"@type": "CompositeConstraint", "operator": "and", "constraint": [prim.clone(), prim]}
    ]});
    let c: Constraint = serde_json::from_value(json.clone()).unwrap();
    match &c {
        Constraint::CompositeConstraint(cc) => {
            assert_eq!(cc.operator, JoinOperator::Or);
            assert!(matches!(
                cc.constraint[1],
                Constraint::CompositeConstraint(_)
            ));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(serde_json::to_value(&c).unwrap(), json);
}

#[test]
fn element_payload_must_name_a_metaclass() {
    let ok: Data =
        serde_json::from_value(json!({"@id": ID, "@type": "PartUsage", "declaredName": "wheel"}))
            .unwrap();
    assert!(matches!(ok, Data::Element(ref e) if e.metaclass == Metaclass::PartUsage));
    assert!(serde_json::from_value::<Data>(json!({"@id": ID, "@type": "NotAMetaclass"})).is_err());
    // An invalid ProjectUsage must not be accepted as an element.
    assert!(
        serde_json::from_value::<Data>(json!({"@id": ID, "@type": "ProjectUsage", "junk": 1}))
            .is_err()
    );
}

#[test]
fn element_request_id_is_optional() {
    let req: ElementRequest = serde_json::from_value(json!({"@type": "PartUsage"})).unwrap();
    assert_eq!(req.id, None);
    assert_eq!(
        serde_json::to_value(&req).unwrap(),
        json!({"@type": "PartUsage"})
    );
}
