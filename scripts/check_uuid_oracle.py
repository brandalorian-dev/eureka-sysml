#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Check that the library UUID oracle follows KerML 9.1 (ADR 0009).

The oracle is the XMI serialization of the standard libraries in SysML-v2-Release 2025-02, pinned in
specs.lock.json under docs/specs/SysML-v2-Release/2025-02/sysml.library.xmi/. Its library text is
identical to the normative .kpar files. This script recomputes, independently of the pilot, the
elementId that KerML 9.1 prescribes for every element it can, and compares:

  - top-level library package: uuid5(NAMESPACE_URL, prefix + package name), where the prefix is
    https://www.omg.org/spec/KerML/ for the Kernel libraries (KerML 9.1) and
    https://www.omg.org/spec/SysML/ for the others (SysML 9.1)
  - every other element in the package: uuid5(package UUID, path()), with path() from
    KerML 8.3.2.1.2 (Element), 8.3.2.1.3 (Relationship) and 8.3.2.4.8 (OwningMembership)

The KerML rules used:
  - effectiveName() (8.3.3, Feature): the declaredName if the element has a declaredName or a
    declaredShortName; otherwise the effective name of its namingFeature(), the redefinedFeature of
    its first ownedRedefinition. Other elements use the declaredName (8.3.2.1).
  - escapedName() (8.3.2.1): the name, else the shortName, quoted as an unrestricted name (8.2.2.3)
    when it is not a basic name.
  - qualifiedName (8.3.2.1, deriveElementQualifiedName): null unless the element is the first
    ownedMember of its owningNamespace with its name.
  - The OwningMembership kinds come from the subtype closure in SysML.json (which includes
    FeatureValue).

Implied redefinitions (KerML 8.4) are not in this XMI, so elements named only through one (for
example unnamed `result` parameters) cannot be recomputed here; they are reported, not failed. Root
namespaces are outside KerML 9.1 and are skipped.

Exits non-zero unless every top-level package and every named element matches.
"""
from __future__ import annotations

import collections
import json
import re
import sys
import uuid
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ORACLE = ROOT / "docs" / "specs" / "SysML-v2-Release" / "2025-02" / "sysml.library.xmi"
SYSML_JSON = ROOT / "docs" / "specs" / "normative" / "SysML.json"
XSI = "{http://www.w3.org/2001/XMLSchema-instance}type"
OWNED = {"ownedRelationship", "ownedRelatedElement"}
BASIC_NAME = re.compile(r"[A-Za-z_][A-Za-z_0-9]*\Z")  # KerML 8.2.2.3
# KerML 8.2.2.3, Table 4: characters written as escape sequences inside an unrestricted name.
ESCAPES = {"\\": "\\\\", "'": "\\'", "\b": "\\b", "\f": "\\f", "\t": "\\t", "\n": "\\n"}


def owning_membership_kinds(sysml_json: Path = SYSML_JSON) -> set[str]:
    """OwningMembership and all its subtypes, from the `anyOf` subtype lists in SysML.json."""
    defs = json.loads(sysml_json.read_text(encoding="utf-8"))["$defs"]
    subtypes = {n: [a["$ref"].rsplit("/", 1)[-1] for a in d.get("anyOf", [])[1:]] for n, d in defs.items()}
    kinds, todo = set(), ["OwningMembership"]
    while todo:
        kind = todo.pop()
        if kind not in kinds:
            kinds.add(kind)
            todo.extend(subtypes.get(kind, []))
    return kinds


def escaped(name: str) -> str:
    """escapedName() formatting (KerML 8.3.2.1, 8.2.2.3)."""
    if BASIC_NAME.match(name):
        return name
    return "'" + "".join(ESCAPES.get(c, c) for c in name) + "'"


class Node:
    def __init__(self, model: "Model", x: ET.Element, parent: "Node | None"):
        self.model = model
        self.parent = parent
        self.kind = x.tag.split("}")[-1]
        self.type = (x.get(XSI) or x.tag).split(":")[-1].split("}")[-1]
        self.id = x.get("elementId")
        self.declared_name = x.get("declaredName")
        self.declared_short_name = x.get("declaredShortName")
        self.redefined = None  # for a Redefinition: the elementId of its redefinedFeature
        if self.type == "Redefinition":
            for c in x:
                href = c.get("href") or ""
                if c.tag.split("}")[-1] == "redefinedFeature" and "#" in href:
                    self.redefined = href.rsplit("#", 1)[1]
            if self.redefined is None and x.get("redefinedFeature"):
                self.redefined = x.get("redefinedFeature").split()[0]
        self._naming = False
        self._cache: dict[str, object] = {}  # name and qualified name never change once loaded
        self.children = [Node(model, c, self) for c in x if c.tag.split("}")[-1] in OWNED]
        model.by_id[self.id] = self

    def is_relationship(self) -> bool:
        return self.kind == "ownedRelationship"

    def is_owning_membership(self) -> bool:
        return self.is_relationship() and self.type in self.model.owning_kinds

    def owned_members(self) -> list["Node"]:
        """ownedMember: the elements this Namespace owns through OwningMemberships, in order."""
        return [r.children[0] for r in self.children if r.is_owning_membership() and r.children]

    def owning_namespace(self) -> "Node | None":
        membership = self.parent
        if membership is not None and membership.is_owning_membership():
            return membership.parent
        return None

    @property
    def name(self) -> str | None:
        """effectiveName() (KerML 8.3.3 for Features; 8.3.2.1 otherwise)."""
        if "name" not in self._cache:
            self._cache["name"] = self._effective_name()
        return self._cache["name"]

    def _effective_name(self) -> str | None:
        if self.is_relationship() or self.declared_name is not None or self.declared_short_name is not None:
            return self.declared_name
        # namingFeature(): the redefinedFeature of the first ownedRedefinition, if any.
        first = next((r for r in self.children if r.is_relationship() and r.type == "Redefinition"), None)
        if first is None or first.redefined not in self.model.by_id or self._naming:
            return None
        self._naming = True  # guards against a redefinition cycle in malformed input
        try:
            return self.model.by_id[first.redefined].name
        finally:
            self._naming = False

    def escaped_name(self) -> str | None:
        """escapedName() (KerML 8.3.2.1): the name, otherwise the shortName."""
        n = self.name if self.name is not None else self.declared_short_name
        return None if n is None else escaped(n)

    def qualified_name(self) -> str | None:
        """deriveElementQualifiedName (KerML 8.3.2.1)."""
        if "qualified_name" not in self._cache:
            self._cache["qualified_name"] = self._qualified_name()
        return self._cache["qualified_name"]

    def _qualified_name(self) -> str | None:
        if self.is_relationship():
            return None
        ns = self.owning_namespace()
        if ns is None:
            return None
        if self.name is not None:
            same = [m for m in ns.owned_members() if m.name == self.name]
            if same and same[0] is not self:
                return None
        if ns.parent is None:  # the owningNamespace is a root Namespace
            return self.escaped_name()
        q, e = ns.qualified_name(), self.escaped_name()
        return None if q is None or e is None else q + "::" + e

    def path(self) -> str:
        if self.is_relationship():
            if self.is_owning_membership() and self.children:  # KerML 8.3.2.4.8
                q = self.children[0].qualified_name()
                if q is not None:
                    return q + "/owningMembership"
            siblings = [c for c in self.parent.children if c.is_relationship()]  # KerML 8.3.2.1.3
            return self.parent.path() + "/" + str(siblings.index(self) + 1)
        q = self.qualified_name()  # KerML 8.3.2.1.2
        if q is not None:
            return q
        if self.parent is None:
            return ""
        siblings = [c for c in self.parent.children if not c.is_relationship()]
        return self.parent.path() + "/" + str(siblings.index(self) + 1)

    def walk(self):
        yield self
        for c in self.children:
            yield from c.walk()


def library_package(root: Node) -> Node:
    """The single top-level LibraryPackage of a library model (KerML 9.1, SysML 9.1)."""
    packages = [m for m in root.owned_members() if m.type == "LibraryPackage"]
    if len(packages) != 1:
        raise ValueError(f"expected exactly one top-level LibraryPackage, found {len(packages)}")
    return packages[0]


class Model:
    """All library files, parsed together so that redefinitions resolve across files."""

    def __init__(self, xmi_roots: list[ET.Element], owning_kinds: set[str]):
        self.owning_kinds = owning_kinds
        self.by_id: dict[str, Node] = {}
        self.roots = [Node(self, x, None) for x in xmi_roots]

    def expected_ids(self, root: Node, prefix: str):
        """Yield (element, category, expected UUID) for every element under the library package."""
        package = library_package(root)
        package_id = uuid.uuid5(uuid.NAMESPACE_URL, prefix + package.name)
        for e in root.walk():
            if e is root:
                continue  # root namespaces are outside KerML 9.1
            if e is package:
                yield e, "top-level package", package_id
            elif e.is_relationship():
                yield e, "relationship", uuid.uuid5(package_id, e.path())
            else:
                category = "named element" if e.qualified_name() else "unnamed element"
                yield e, category, uuid.uuid5(package_id, e.path())


STRICT = ("top-level package", "named element")


def main() -> int:
    files = sorted(ORACLE.rglob("*.kermlx")) + sorted(ORACLE.rglob("*.sysmlx"))
    if len(files) != 94:
        print(f"expected 94 oracle files under {ORACLE.relative_to(ROOT)}, found {len(files)}. "
              "Run `python scripts/fetch_specs.py fetch`.", file=sys.stderr)
        return 1
    model = Model([ET.parse(f).getroot() for f in files], owning_membership_kinds())
    stats: collections.Counter = collections.Counter()
    failures = []
    for f, root in zip(files, model.roots):
        kerml = f.relative_to(ORACLE).parts[0] == "Kernel Libraries"
        prefix = "https://www.omg.org/spec/KerML/" if kerml else "https://www.omg.org/spec/SysML/"
        try:
            for e, category, expected in model.expected_ids(root, prefix):
                ok = str(expected) == e.id
                stats[(category, ok)] += 1
                if not ok and category in STRICT:
                    failures.append(f"{f.name}: {e.type} {e.path()!r}: elementId {e.id}, KerML 9.1 gives {expected}")
        except ValueError as err:
            failures.append(f"{f.name}: {err}")
    total = sum(stats.values())
    good = sum(n for (_, ok), n in stats.items() if ok)
    print(f"{total} elements in {len(files)} library files; {good} ({100 * good / max(total, 1):.2f}%) match KerML 9.1")
    for category in ("top-level package", "named element", "unnamed element", "relationship"):
        # A mismatch is a failure only where this script can recompute every id (see the docstring).
        other = "MISMATCH" if category in STRICT else "not recomputable here"
        print(f"  {category:18} {stats[(category, True)]:6} match, {stats[(category, False)]:5} {other}")
    for line in failures[:20]:
        print("ERROR:", line, file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
