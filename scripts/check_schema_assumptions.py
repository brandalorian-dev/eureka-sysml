#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Check the structural assumptions that crates/sysml-model/build.rs makes about SysML.json.

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
for e in errors:
    print("ERROR:", e, file=sys.stderr)
sys.exit(1 if errors else 0)
