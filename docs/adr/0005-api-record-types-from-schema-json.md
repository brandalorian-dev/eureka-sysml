# ADR 0005: API record types generated from Schema.json

- Status: Accepted
- Date: 2026-11-20

## Context

`sysml-api-types` generates the Systems Modeling API and Services 1.0 record types (API 7.1, REST/HTTP PSM 8.1.2) from the normative `docs/specs/api/omg/normative/Schema.json`. The tests use the examples in the normative `OpenAPI.json`. Some parts of these files contradict the PIM text or are inconsistent with each other, and some choices are not dictated by the specification.

## Decision

**Scope of generation.**
- The 30 definitions whose `$id` contains `SystemsModelingAPI/` are generated: 26 records and 4 unions (`Data`, `DataRequest`, `Constraint`, `ConstraintRequest`).
- The 182 `<Metaclass>Request` element schemas are not generated. Element payloads use the hand-written `Element` and `ElementRequest`: `@id`, a `@type` that must name a metaclass in `SysML.json`, and every other property kept as a JSON value. Typed element access belongs to `sysml-model`, and validating payloads belongs to the semantic layer.
- `Data` refers to `SysML/20230201/Element`, which `Schema.json` does not define. It is read as the element serialization of KerML 10.4.5 with the current metamodel (`SysML.json`, 20250201).

**Corrections to Schema.json.**
- `Commit.owningProject` has `$comment` `…/Branch`. API 7.1.2 says "owningProject is the Project that owns the Commit", and the PIM diagram agrees. We follow the PIM. The JSON type is the same (`Identified`), so this only affects documentation and `reference_target`.
- `PrimitiveConstraintRequest` has the `$id` and `title` of `PrimitiveConstraint`. Its `$defs` key is used as the type name.

**Errors in the OpenAPI.json examples.**
- The `MergeCommit` example omits `alias` and `name`, which `Commit` requires. When deserializing, a missing required list (such as `alias`) reads as empty, and a missing required nullable property reads as `null`. When serializing, required properties are always written. This follows the robustness principle: be strict in what we send and accept the spec's own example.
- The `Commits` example links to `#/components/examples/Commit` instead of `…/Commit/value`. The test resolver reads such links as pointing at the value.

**Representation choices.**
- A request property that is optional and nullable is `Option<Option<T>>`, so that "absent" (leave unchanged on update) and `null` (clear) stay distinct.
- `@type` is a marker type per record that serializes as its constant and rejects any other value. Union members are therefore told apart by `@type`, and an invalid `ProjectUsage` cannot be mistaken for an element.
- `date-time` and `uri` values are kept as written (`DateTime`, `Iri` newtypes). Parsing and ordering of timestamps is left to `sysml-versioning`, which needs the monotonic-timestamp rules of API 7.1.2.
- Numbers in `PrimitiveConstraint.value` are kept as `serde_json::Number`, so integers are not rounded through `f64`.

## Consequences

- A new `Schema.json` revision that changes the layout fails `scripts/check_schema_assumptions.py` and the `build.rs` assertions, with a message that names the definition.
- The TypeScript types and client generated from `OpenAPI.json` (PROJECT_PLAN §4.2) are separate work.
- If OMG fixes the `Commit.owningProject` comment or the examples, the corrections here become no-ops and can be removed.
