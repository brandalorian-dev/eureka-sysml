# SPDX-License-Identifier: Apache-2.0
"""Tests for scripts/check_uuid_oracle.py: the KerML rules it uses to recompute library UUIDs.

Each test builds a small XMI model in the shape of the SysML-v2-Release library serializations.
Run with: python -m unittest discover -s scripts/tests
"""
import sys
import unittest
import uuid
import xml.etree.ElementTree as ET
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import check_uuid_oracle as oracle  # noqa: E402

PREFIX = "https://www.omg.org/spec/KerML/"
HEAD = ('<sysml:Namespace xmlns:xmi="http://www.omg.org/XMI" '
        'xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" '
        'xmlns:sysml="https://www.omg.org/spec/SysML/20250201" elementId="root">')


def rel(kind, eid, body="", extra=""):
    return f'<ownedRelationship xsi:type="sysml:{kind}" elementId="{eid}" {extra}>{body}</ownedRelationship>'


def elem(kind, eid, body="", **attrs):
    a = " ".join(f'{k}="{v}"' for k, v in attrs.items())
    return f'<ownedRelatedElement xsi:type="sysml:{kind}" elementId="{eid}" {a}>{body}</ownedRelatedElement>'


def redefinition(eid, target):
    return rel("Redefinition", eid, f'<redefinedFeature href="Other.kermlx#{target}"/>')


def library(*members, leading=""):
    """A root namespace owning `leading` relationships, then the library package P with `members`."""
    pkg = elem("LibraryPackage", "pkg", "".join(members), declaredName="P", isStandard="true")
    return HEAD + leading + rel("OwningMembership", "pkg-m", pkg) + "</sysml:Namespace>"


def model(*xmis):
    return oracle.Model([ET.fromstring(x) for x in xmis], oracle.owning_membership_kinds())


def expected_by_id(m, root_index=0):
    root = m.roots[root_index]
    return {node.id: (node.path(), str(expected)) for node, _, expected in m.expected_ids(root, PREFIX)}


# A library feature `y` that other models redefine.
OTHER = HEAD + rel("OwningMembership", "o-pm", elem("LibraryPackage", "o-pkg",
        rel("FeatureMembership", "o-ym", elem("Feature", "y", declaredName="y")) +
        rel("FeatureMembership", "o-zm", elem("Feature", "z", declaredName="z")),
        declaredName="Other", isStandard="true")) + "</sysml:Namespace>"


class EffectiveName(unittest.TestCase):
    def test_named_by_its_first_redefinition(self):
        # KerML 8.3.3: an unnamed Feature takes the effective name of its naming Feature.
        m = model(library(rel("FeatureMembership", "fm", elem("Feature", "f", redefinition("r", "y")))), OTHER)
        self.assertEqual(m.by_id["f"].name, "y")
        self.assertEqual(m.by_id["f"].path(), "P::y")

    def test_short_name_only_is_not_renamed(self):
        # KerML 8.3.3 effectiveName(): with a declaredShortName, the name is the declaredName (null).
        # escapedName() then falls back to the short name (KerML 8.3.2.1).
        m = model(library(rel("FeatureMembership", "fm",
                              elem("Feature", "f", redefinition("r", "y"), declaredShortName="s"))), OTHER)
        self.assertIsNone(m.by_id["f"].name)
        self.assertEqual(m.by_id["f"].path(), "P::s")

    def test_only_the_first_redefinition_names(self):
        # KerML 8.3.3 namingFeature(): ownedRedefinition->at(1), with no fallback to later ones.
        m = model(library(rel("FeatureMembership", "fm",
                              elem("Feature", "f", redefinition("r1", "missing") + redefinition("r2", "z")))), OTHER)
        self.assertIsNone(m.by_id["f"].name)

    def test_first_redefinition_wins_over_a_later_one(self):
        m = model(library(rel("FeatureMembership", "fm",
                              elem("Feature", "f", redefinition("r1", "y") + redefinition("r2", "z")))), OTHER)
        self.assertEqual(m.by_id["f"].name, "y")


class QualifiedName(unittest.TestCase):
    def test_repeated_names_after_the_first_have_no_qualified_name(self):
        # KerML 8.3.2.1 deriveElementQualifiedName: only the first member with a given name keeps it.
        m = model(library(rel("FeatureMembership", "m1", elem("Feature", "x1", declaredName="x")),
                          rel("FeatureMembership", "m2", elem("Feature", "x2", declaredName="x"))))
        self.assertEqual(m.by_id["x1"].qualified_name(), "P::x")
        self.assertIsNone(m.by_id["x2"].qualified_name())
        # The second falls back to positional paths (KerML 8.3.2.1.2, 8.3.2.1.3).
        self.assertEqual(m.by_id["m2"].path(), "P/2")
        self.assertEqual(m.by_id["x2"].path(), "P/2/1")

    def test_repeated_effective_names_count_too(self):
        m = model(library(rel("FeatureMembership", "m1", elem("Feature", "a", redefinition("r1", "y"))),
                          rel("FeatureMembership", "m2", elem("Feature", "b", redefinition("r2", "y")))), OTHER)
        self.assertEqual(m.by_id["a"].qualified_name(), "P::y")
        self.assertIsNone(m.by_id["b"].qualified_name())

    def test_unrestricted_names_are_escaped(self):
        # KerML 8.2.2.3 and escapedName(): a name that is not a basic name is quoted and escaped.
        m = model(library(rel("FeatureMembership", "m1", elem("Feature", "q", declaredName="it&apos;s a")),
                          rel("FeatureMembership", "m2", elem("Feature", "t", declaredName="a&#9;b"))))
        self.assertEqual(m.by_id["q"].qualified_name(), "P::'it\\'s a'")
        self.assertEqual(m.by_id["t"].qualified_name(), "P::'a\\tb'")


class FeatureValueIsAnOwningMembership(unittest.TestCase):
    def test_value_expression_is_a_namespace_member(self):
        # FeatureValue is an OwningMembership (KerML 8.3.4; the subtype closure in SysML.json).
        value = rel("FeatureValue", "fv", elem("Expression", "e", declaredName="e"))
        m = model(library(rel("FeatureMembership", "fm", elem("Feature", "f", value, declaredName="f"))))
        self.assertIn("FeatureValue", oracle.owning_membership_kinds())
        self.assertEqual(m.by_id["e"].qualified_name(), "P::f::e")
        self.assertEqual(m.by_id["fv"].path(), "P::f::e/owningMembership")


class LibraryPackage(unittest.TestCase):
    def test_found_after_a_leading_comment(self):
        comment = rel("OwningMembership", "c-m", elem("Comment", "c", body=""))
        m = model(library(leading=comment))
        self.assertEqual(oracle.library_package(m.roots[0]).id, "pkg")

    def test_two_packages_are_an_error(self):
        second = rel("OwningMembership", "p2-m", elem("LibraryPackage", "p2", declaredName="Q"))
        m = model(library(leading=second))
        with self.assertRaisesRegex(ValueError, "exactly one"):
            oracle.library_package(m.roots[0])

    def test_no_package_is_an_error(self):
        m = model(HEAD + "</sysml:Namespace>")
        with self.assertRaisesRegex(ValueError, "exactly one"):
            oracle.library_package(m.roots[0])


class ExpectedIds(unittest.TestCase):
    def test_package_and_member_uuids_follow_kerml_9_1(self):
        m = model(library(rel("FeatureMembership", "fm", elem("Feature", "f", declaredName="f"))))
        ids = expected_by_id(m)
        package = uuid.uuid5(uuid.NAMESPACE_URL, PREFIX + "P")
        self.assertEqual(ids["pkg"], ("P", str(package)))
        self.assertEqual(ids["f"], ("P::f", str(uuid.uuid5(package, "P::f"))))
        self.assertEqual(ids["fm"], ("P::f/owningMembership", str(uuid.uuid5(package, "P::f/owningMembership"))))
        self.assertNotIn("root", ids)  # root namespaces are outside KerML 9.1


if __name__ == "__main__":
    unittest.main()
