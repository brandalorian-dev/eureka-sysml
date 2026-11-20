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
- The `MergeCommit` example omits `alias` and `name`, which `Commit` requires. In **response records only**, a missing `alias` reads as empty and a missing `name` reads as `null`. These are the Record attributes of API 7.1.1. Every other required property is enforced, and request bodies get no leniency. A missing `DataVersionRequest.payload`, for example, is an error and is never read as `null`, which would mean "delete" (API 7.1.2). When serializing, required properties are always written.
- The `Commits` example links to `#/components/examples/Commit` instead of `…/Commit/value`. The test resolver reads such links as pointing at the value.

**Representation choices.**
- A request property that is optional and nullable is `Option<Option<T>>`, so that "absent" (leave unchanged on update) and `null` (clear) stay distinct.
- A required, nullable property must be present; only an explicit `null` reads as `None`.
- `minItems` is enforced when deserializing: `CompositeConstraint.constraint` needs at least 2 items, and a non-null `PrimitiveConstraint.value` at least 1. `build.rs` and `check_schema_assumptions.py` stop on any schema keyword they do not handle, so no constraint in `Schema.json` is silently ignored.
- `@type` is a marker type per record that serializes as its constant and rejects any other value.
- A union reads `@type` first and hands the value to that member. Errors therefore name the member and the bad property, and an invalid `ProjectUsage` cannot be mistaken for an element. An `@type` that names no other member is read as an element metaclass.
- Identifiers (`"format": "uuid"`) are an `Id` type that accepts only the hyphenated RFC 9562 §4 form. The URN, braced and unhyphenated forms are rejected. Hex digits are case-insensitive on input and written in lower case, as RFC 9562 recommends, so an upper-case ID is echoed in lower case.
- The open property maps (`Element.properties`, `ElementRequest.properties`, `Error.additional`) must not repeat a named property such as `@id` or `@type`. Serialization fails if they do, rather than writing a key twice.
- `date-time` and `uri` values are kept as written (`DateTime`, `Iri` newtypes). Parsing and ordering of timestamps is left to `sysml-versioning`, which needs the monotonic-timestamp rules of API 7.1.2.
- Numbers in `PrimitiveConstraint.value` are kept as `serde_json::Number`, so integers are not rounded through `f64`.

## Consequences

- A new `Schema.json` revision that changes the layout fails `scripts/check_schema_assumptions.py` and the `build.rs` assertions, with a message that names the definition.
- The TypeScript types and client generated from `OpenAPI.json` (PROJECT_PLAN §4.2) are separate work.
- If OMG fixes the `Commit.owningProject` comment or the examples, the corrections here become no-ops and can be removed.
