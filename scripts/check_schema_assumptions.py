#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Check the structural assumptions that the code generators make about the normative schemas:
crates/sysml-model/build.rs (SysML.json) and crates/sysml-api-types/build.rs (API Schema.json).

Runs in CI right after the specs are fetched. If a future metamodel revision changes the
schema layout, this fails with a readable message before the Rust build fails in a confusing way.
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
schema = json.loads((ROOT / "docs/specs/normative/SysML.json").read_text(encoding="utf-8"))
defs = schema["$defs"]
errors = []


def classify(s):
    if "oneOf" in s:
        non_null = [a for a in s["oneOf"] if a.get("type") != "null"]
        return len(non_null) == 1 and len(non_null) < len(s["oneOf"]) and classify(non_null[0])
    if s.get("type") == "array":
        return classify(s["items"])
    if "$ref" in s:
        return True
    return (s.get("type"), s.get("format")) in {
        ("string", "uuid"), ("string", None), ("boolean", None), ("integer", None), ("number", None)}


metaclasses, enums = [], []
for name, d in defs.items():
    if "enum" in d:
        enums.append(name)
        continue
    if name == "Identified":
        continue
    metaclasses.append(name)
    if not name.isidentifier():
        errors.append(f"{name}: not a valid Rust identifier")
    obj = d["anyOf"][0] if "anyOf" in d else d
    if obj.get("type") != "object":
        errors.append(f"{name}: first anyOf entry is not an object schema")
    for sub in d.get("anyOf", [])[1:]:
        if "$ref" not in sub or sub["$ref"].rsplit("/", 1)[-1] not in defs:
            errors.append(f"{name}: anyOf subtype entry is not a $ref to a known definition")
    if obj.get("properties", {}).get("@type", {}).get("const") != name:
        errors.append(f"{name}: @type const does not match the definition name")
    for pname, ps in obj.get("properties", {}).items():
        if pname not in ("@id", "@type") and not classify(ps):
            errors.append(f"{name}.{pname}: unrecognized property schema {json.dumps(ps)[:120]}")
for e in enums:
    for lit in defs[e]["enum"]:
        if not (lit.isidentifier() and lit[0].isalpha()):
            errors.append(f"{e}: literal {lit!r} cannot become a Rust variant")

print(f"{len(metaclasses)} metaclasses, {len(enums)} enumerations")
if len(metaclasses) != 175 or len(enums) != 7:
    errors.append("counts differ from metamodel 20250201 (175/7); update crates/sysml-model tests")

# ---- crates/sysml-api-types/build.rs: the API record definitions in Schema.json ----
api_defs = json.loads((ROOT / "docs/specs/api/omg/normative/Schema.json").read_text(encoding="utf-8"))["$defs"]
API = [n for n, d in api_defs.items() if "SystemsModelingAPI/" in d.get("$id", "")]
KNOWN_ENUMS = {("PrimitiveConstraint", "operator"), ("CompositeConstraint", "operator")}
VALUE_ITEM = {"Identified", "boolean", "number", "string"}
KNOWN_KEYWORDS = {"$comment", "$ref", "const", "enum", "format", "items", "minItems", "oneOf", "type"}
UNION_TARGETS = set(API) | {"Element", "ElementRequest"}


class Shape(Exception):
    """A property schema that build.rs would reject."""


def api_shape(rec, prop, s):
    """Mirror Gen::ty in build.rs. Returns (nullable, many, min_items) or raises Shape."""
    unknown = set(s) - KNOWN_KEYWORDS
    if unknown:
        raise Shape(f"schema keyword(s) {sorted(unknown)} not supported by build.rs")
    if "minItems" in s and s.get("type") != "array":
        raise Shape("minItems outside an array schema")
    if "oneOf" in s:
        alts = s["oneOf"]
        non_null = [a for a in alts if a.get("type") != "null"]
        if len(non_null) == 1 and len(alts) == 2:
            nullable, many, min_items = api_shape(rec, prop, non_null[0])
            if nullable:
                raise Shape("nested nullable")
            return True, many, min_items
        kinds = {a["$ref"].rsplit("/", 1)[-1] if "$ref" in a else a.get("type") for a in non_null}
        if kinds != VALUE_ITEM or len(alts) != 5:  # PrimitiveConstraint.value items
            raise Shape("unrecognized oneOf")
        return False, False, None
    if s.get("type") == "array":
        nullable, many, _ = api_shape(rec, prop, s["items"])
        if nullable or many:
            raise Shape("array items are nullable or nested arrays")
        return False, True, s.get("minItems")
    if "$ref" in s:
        target = s["$ref"].rsplit("/", 1)[-1]
        if target == "Identified" and "$comment" not in s:
            raise Shape("reference without a $comment target")
        if target not in ("Identified", "Element", "ElementRequest") and target not in API:
            raise Shape(f"reference to unknown definition {target}")
        return False, False, None
    if "enum" in s:
        if (rec.removesuffix("Request"), prop) not in KNOWN_ENUMS:
            raise Shape("inline enum without a name in build.rs inline_enum()")
        return False, False, None
    if (s.get("type"), s.get("format")) not in {
            ("string", None), ("string", "uuid"), ("string", "date-time"), ("string", "uri"), ("boolean", None)}:
        raise Shape("unrecognized primitive type")
    return False, False, None


unions = 0
for name in API:
    d = api_defs[name]
    if "oneOf" in d:
        unions += 1
        for a in d["oneOf"]:
            target = a.get("$ref", "").rsplit("/", 1)[-1]
            if "$ref" not in a or target not in UNION_TARGETS:
                errors.append(f"API {name}: union member {json.dumps(a)[:80]} is not a $ref to a known definition")
        continue
    if d.get("type") != "object":
        errors.append(f"API {name}: not an object schema")
        continue
    if d["properties"].get("@type", {}).get("const") != name.removesuffix("Request"):
        errors.append(f"API {name}: @type const does not match the record name")
    required = set(d.get("required", []))
    for pname, ps in d["properties"].items():
        if pname == "@type":
            continue
        try:
            nullable, _, min_items = api_shape(name, pname, ps)
            if min_items is not None and nullable and pname not in required:
                raise Shape("minItems on an optional nullable list is not supported")
        except Shape as e:
            errors.append(f"API {name}.{pname}: {e}: {json.dumps(ps)[:120]}")
print(f"{len(API) - unions} API records, {unions} API unions")
if len(API) != 30 or unions != 4:
    errors.append("API definition counts differ from Systems Modeling API 1.0 (26 records, 4 unions); "
                  "update crates/sysml-api-types tests")
for e in errors:
    print("ERROR:", e, file=sys.stderr)
sys.exit(1 if errors else 0)
