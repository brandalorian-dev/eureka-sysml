# SPDX-License-Identifier: Apache-2.0
"""Tests of extract_spec_snippets.py: the extraction rules of ADR 0016, on synthetic
`pdftohtml -xml` output."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import extract_spec_snippets as e  # noqa: E402

FONTS = (
    '<fontspec id="1" size="15" family="AAAA+TimesNewRomanPSMT" color="#000000"/>'
    '<fontspec id="2" size="14" family="AAAB+CourierNewPSMT" color="#000000"/>'
    '<fontspec id="3" size="15" family="AAAC+Arial" color="#000000"/>'
    '<fontspec id="4" size="11" family="AAAC+Arial" color="#000000"/>'
)


def text(top: int, left: int, font: int, s: str) -> str:
    width = 9 * len(s)
    return f'<text top="{top}" left="{left}" width="{width}" height="16" font="{font}">{s}</text>'


def doc(*pages: list[str]) -> str:
    body = "".join(
        f'<page number="{n}" position="absolute" top="0" left="0" height="1000" width="800">'
        + "".join(page)
        + "</page>"
        for n, page in enumerate(pages, 1)
    )
    return f"<pdf2xml>{FONTS}{body}</pdf2xml>"


HEADING = text(100, 100, 3, "7.2.5 Namespaces")
FOOTER = text(950, 100, 4, "24 Kernel Modeling Language v1.0")


class Extraction(unittest.TestCase):
    def ids(self, xml: str) -> list[str]:
        return [s["id"] for s in e.snippets("kerml", xml)]

    def test_a_courier_block_is_a_snippet_of_its_clause(self):
        xml = doc([HEADING, text(200, 100, 1, "For example:"),
                   text(220, 145, 2, "namespace N {"), text(236, 172, 2, "class C;"),
                   text(252, 145, 2, "}"), FOOTER])
        [s] = e.snippets("kerml", xml)
        self.assertEqual(s["id"], "kerml/7.2.5/1")
        self.assertEqual(s["path"], "kerml/7.2.5-1.kerml")
        self.assertEqual(s["language"], "kerml")
        self.assertEqual(s["page"], 1)
        # The indentation comes from the positions: 27 px at 9 px a character is 3 columns.
        self.assertEqual(s["text"], "namespace N {\n   class C;\n}\n")

    def test_a_keyword_in_a_prose_line_is_not_code(self):
        xml = doc([HEADING, text(200, 100, 1, "The keyword"), text(200, 200, 2, "namespace;"),
                   text(200, 300, 1, "starts a namespace.")])
        self.assertEqual(self.ids(xml), [])

    def test_prose_ends_a_block_and_snippets_are_numbered_per_clause(self):
        xml = doc([HEADING, text(220, 145, 2, "class A;"), text(240, 100, 1, "and"),
                   text(260, 145, 2, "class B;"),
                   text(300, 100, 3, "7.2.6 Next"), text(320, 145, 2, "class C;")])
        self.assertEqual(self.ids(xml), ["kerml/7.2.5/1", "kerml/7.2.5/2", "kerml/7.2.6/1"])

    def test_a_block_continues_across_a_page_break(self):
        xml = doc([HEADING, text(900, 145, 2, "namespace N {"), FOOTER],
                  [text(30, 100, 4, "Kernel Modeling Language v1.0"),
                   text(100, 172, 2, "class C;"), text(116, 145, 2, "}")])
        [s] = e.snippets("kerml", xml)
        self.assertEqual(s["text"], "namespace N {\n   class C;\n}\n")

    def test_a_gap_is_a_blank_line(self):
        xml = doc([HEADING, text(220, 145, 2, "class A;"), text(236, 145, 2, "class B;"),
                   text(282, 145, 2, "class C;")])
        [s] = e.snippets("kerml", xml)
        self.assertEqual(s["text"], "class A;\nclass B;\n\nclass C;\n")

    def test_productions_tables_and_other_clauses_are_left_out(self):
        xml = doc([HEADING, text(220, 145, 2, "Namespace = 'namespace' Name ;"),
                   text(240, 100, 1, "Keywords:"), text(260, 145, 2, "abstract"),
                   text(300, 100, 3, "8.2.3 Grammar"), text(320, 145, 2, "class C;")])
        self.assertEqual(self.ids(xml), [])

    def test_a_wrapped_word_at_the_edge_of_a_block_is_prose(self):
        xml = doc([HEADING, text(200, 100, 2, "Association"),
                   text(220, 145, 2, "assoc"), text(220, 200, 2, "A {"),
                   text(236, 145, 2, "}")])
        [s] = e.snippets("kerml", xml)
        self.assertEqual(s["text"], "assoc A {\n}\n")

    def test_the_heading_needs_the_heading_size(self):
        # A 11-point Arial line, such as a footer, is not a heading.
        xml = doc([HEADING, text(500, 100, 4, "7.9 Something"), text(520, 145, 2, "class C;")])
        self.assertEqual(self.ids(xml), ["kerml/7.2.5/1"])


if __name__ == "__main__":
    unittest.main()
