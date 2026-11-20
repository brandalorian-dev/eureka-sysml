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

Names are effective names: the declaredName, or for an unnamed Feature the effective name of the
Feature its first owned Redefinition redefines. Implied redefinitions (KerML 8.4) are not in this
XMI, so elements named only through one (for example unnamed `result` parameters) cannot be
recomputed here; they are reported, not failed. Root namespaces are outside KerML 9.1 and skipped.

Exits non-zero unless every top-level package and every named element matches.
"""
from __future__ import annotations

import collections
import re
import sys
import uuid
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ORACLE = ROOT / "docs" / "specs" / "SysML-v2-Release" / "2025-02" / "sysml.library.xmi"
XSI = "{http://www.w3.org/2001/XMLSchema-instance}type"
OWNED = {"ownedRelationship", "ownedRelatedElement"}
BASIC_NAME = re.compile(r"[A-Za-z_][A-Za-z_0-9]*\Z")  # KerML 8.2.2.3
BY_ID: dict[str, "Node"] = {}


def escaped(name: str) -> str:
    """KerML 8.2.2.3: a name that is not a basic name is written as an unrestricted name."""
    if BASIC_NAME.match(name):
        return name
    return "'" + name.replace("\\", "\\\\").replace("'", "\\'") + "'"


class Node:
    def __init__(self, x: ET.Element, parent: "Node | None"):
        self.parent = parent
        self.kind = x.tag.split("}")[-1]
        self.type = (x.get(XSI) or x.tag).split(":")[-1].split("}")[-1]
        self.id = x.get("elementId")
        self.declared = x.get("declaredName")
        self.redefines = None
        if self.type == "Redefinition":
            for c in x:
                href = c.get("href") or ""
                if c.tag.split("}")[-1] == "redefinedFeature" and "#" in href:
                    self.redefines = href.rsplit("#", 1)[1]
            if self.redefines is None and x.get("redefinedFeature"):
                self.redefines = x.get("redefinedFeature").split()[0]
        self._busy = False
        self.children = [Node(c, self) for c in x if c.tag.split("}")[-1] in OWNED]
        BY_ID[self.id] = self

    def is_relationship(self) -> bool:
        return self.kind == "ownedRelationship"

    def is_membership(self) -> bool:
        return self.is_relationship() and self.type.endswith("Membership")

    @property
    def name(self) -> str | None:
        if self.declared is not None or self.is_relationship():
            return self.declared
        if self._busy:
            return None
        self._busy = True
        try:
            for r in self.children:
                if r.is_relationship() and r.type == "Redefinition" and r.redefines in BY_ID:
                    return BY_ID[r.redefines].name
            return None
        finally:
            self._busy = False

    def qualified_name(self) -> str | None:
        if self.is_relationship() or self.name is None:
            return None
        membership = self.parent
        if membership is None or not membership.is_membership():
            return None
        namespace = membership.parent
        if namespace.parent is None:  # a member of a root namespace
            return escaped(self.name)
        q = namespace.qualified_name()
        return None if q is None else q + "::" + escaped(self.name)

    def path(self) -> str:
        if self.is_relationship():
            if self.is_membership() and self.children:  # KerML 8.3.2.4.8
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


def main() -> int:
    files = sorted(ORACLE.rglob("*.kermlx")) + sorted(ORACLE.rglob("*.sysmlx"))
    if len(files) != 94:
        print(f"expected 94 oracle files under {ORACLE.relative_to(ROOT)}, found {len(files)}. "
              "Run `python scripts/fetch_specs.py fetch`.", file=sys.stderr)
        return 1
    roots = [(f, Node(ET.parse(f).getroot(), None)) for f in files]
    stats: collections.Counter = collections.Counter()
    failures = []
    for f, root in roots:
        kerml = f.relative_to(ORACLE).parts[0] == "Kernel Libraries"
        prefix = "https://www.omg.org/spec/KerML/" if kerml else "https://www.omg.org/spec/SysML/"
        package = root.children[0].children[0]
        package_id = uuid.uuid5(uuid.NAMESPACE_URL, prefix + package.name)
        for e in root.walk():
            if e is root:
                continue
            if e is package:
                category, expected = "top-level package", package_id
            else:
                category = ("relationship" if e.is_relationship()
                            else "named element" if e.qualified_name() else "unnamed element")
                expected = uuid.uuid5(package_id, e.path())
            ok = str(expected) == e.id
            stats[(category, ok)] += 1
            if not ok and category in ("top-level package", "named element"):
                failures.append(f"{f.name}: {e.type} {e.path()!r}: elementId {e.id}, KerML 9.1 gives {expected}")
    total = sum(stats.values())
    good = sum(n for (_, ok), n in stats.items() if ok)
    print(f"{total} elements in {len(files)} library files; {good} ({100 * good / total:.2f}%) match KerML 9.1")
    for category in ("top-level package", "named element", "unnamed element", "relationship"):
        # A mismatch is a failure only where this script can recompute every id (see the docstring).
        other = "MISMATCH" if category in ("top-level package", "named element") else "not recomputable here"
        print(f"  {category:18} {stats[(category, True)]:6} match, {stats[(category, False)]:5} {other}")
    for line in failures[:20]:
        print("ERROR:", line, file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
