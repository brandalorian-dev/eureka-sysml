#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Extract the textual examples (snippets) from the KerML and SysML specification PDFs.

Output goes to docs/specs/snippets/ (git-ignored, like all of docs/specs/): one file per snippet
and an index.json that `sysml-corpus` loads. Requires `pdftohtml` from poppler-utils on PATH.
The rules are ADR 0016:

  1. `pdftohtml -xml` gives every text run with its position and font. The specifications set
     code in Courier New, prose in Times New Roman and headings in Arial.
  2. A line whose runs are all Courier is a code line. Prose lines that contain a keyword in
     Courier are not, because they also have Times runs.
  3. Consecutive code lines form a block. A prose line ends the block; the page header and footer
     do not, so a block that continues on the next page stays one block.
  4. The clause of a block is the last numbered Arial heading before it.
  5. Only the clauses in SCOPE are kept (the language description and, for KerML, the semantics
     examples), and only blocks that look like models: with `;`, `{`, `}` or a comment, and not
     an EBNF production.
  6. A line at the start or end of a block that is one bare identifier is prose: a sentence that
     wrapped so that only its last word, set in Courier, is on the line. It is dropped.
  7. The text keeps the indentation (from the runs' positions) and the blank lines (from the gaps
     between lines).

Each snippet has the id `<spec>/<clause>/<n>`, n counting the snippets of the clause from 1.

Usage: python scripts/extract_spec_snippets.py [--check]
  --check  extract to a temporary directory and compare with docs/specs/snippets/
"""
from __future__ import annotations

import html
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPECS = Path(os.environ.get("EUREKA_SPECS_DIR", ROOT / "docs" / "specs"))
OUT = SPECS / "snippets"

# spec name -> (PDF path under docs/specs/, language, clause prefixes in scope)
DOCS = {
    "kerml": ("KerML/OMG KerML v1.0 (formal-26-03-01).pdf", "kerml", ("7.", "8.4.")),
    "sysml": ("OMG SysML v2.0 Part 1 - Language Specification (formal-26-03-02).pdf", "sysml", ("7.",)),
}

FONTSPEC_RE = re.compile(r'<fontspec id="(\d+)" size="(\d+)" family="([^"]+)"')
PAGE_RE = re.compile(r'<page number="(\d+)"[^>]*height="(\d+)"[^>]*>(.*?)</page>', re.S)
TEXT_RE = re.compile(r'<text top="(-?\d+)" left="(-?\d+)" width="(\d+)" height="(\d+)" font="(\d+)">(.*?)</text>', re.S)
HEADING_RE = re.compile(r"^(\d+(?:\.\d+)+)\s+\S")
PRODUCTION_RE = re.compile(r"^[A-Z][A-Za-z]*\s*(?::\s*[A-Za-z]+\s*)?=")
IDENTIFIER_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
MODEL_RE = re.compile(r"[;{}]|\*/")

# Lines in the top 7% or bottom 8% of a page are its header and footer.
HEADER, FOOTER = 0.07, 0.92


@dataclass
class Run:
    left: int
    width: int
    family: str
    size: int
    text: str


@dataclass
class Line:
    page: int
    top: int
    page_height: int
    runs: list[Run] = field(default_factory=list)

    def visible(self) -> list[Run]:
        return [r for r in self.runs if r.text.strip()]

    def text(self) -> str:
        return "".join(r.text for r in sorted(self.runs, key=lambda r: r.left))


@dataclass
class Block:
    clause: str
    page: int
    lines: list[Line] = field(default_factory=list)


def lines_of(xml: str) -> list[Line]:
    """The text lines of a `pdftohtml -xml` document, in reading order."""
    fonts = {m.group(1): (m.group(3), int(m.group(2))) for m in FONTSPEC_RE.finditer(xml)}
    lines: list[Line] = []
    for page in PAGE_RE.finditer(xml):
        number, height = int(page.group(1)), int(page.group(2))
        runs = []
        for m in TEXT_RE.finditer(page.group(3)):
            family, size = fonts[m.group(5)]
            text = html.unescape(re.sub(r"<[^>]+>", "", m.group(6)))
            runs.append((int(m.group(1)), Run(int(m.group(2)), int(m.group(3)), family, size, text)))
        runs.sort(key=lambda r: (r[0], r[1].left))
        for top, run in runs:
            if lines and lines[-1].page == number and abs(lines[-1].top - top) <= 3:
                lines[-1].runs.append(run)
            else:
                lines.append(Line(number, top, height, [run]))
    return lines


def is_code(line: Line) -> bool:
    runs = line.visible()
    return bool(runs) and all("Courier" in r.family for r in runs)


def is_margin(line: Line) -> bool:
    return line.top < HEADER * line.page_height or line.top > FOOTER * line.page_height


def heading(line: Line) -> str | None:
    runs = line.visible()
    if not runs or "Arial" not in runs[0].family or runs[0].size < 14:
        return None
    m = HEADING_RE.match(line.text().strip())
    return m.group(1) if m else None


def blocks_of(lines: list[Line]) -> list[Block]:
    """The code blocks, each with the clause it is in (rules 2–4)."""
    clause, blocks, current = "", [], None
    for line in lines:
        if is_margin(line):
            continue  # the header and footer do not end a block
        number = heading(line)
        if number:
            clause, current = number, None
            continue
        if is_code(line):
            if current is None:
                current = Block(clause, line.page)
                blocks.append(current)
            current.lines.append(line)
        elif line.visible():
            current = None
    return blocks


def trim(block: Block) -> Block:
    """Drop the bare-identifier lines at the start and end of a block (rule 6)."""
    def bare(line: Line) -> bool:
        runs = line.visible()
        return len(runs) == 1 and bool(IDENTIFIER_RE.match(runs[0].text.strip()))

    lines = block.lines
    while lines and bare(lines[0]):
        lines = lines[1:]
    while lines and bare(lines[-1]):
        lines = lines[:-1]
    return Block(block.clause, lines[0].page if lines else block.page, lines)


def render(block: Block) -> str:
    """The block's text, with its indentation and blank lines (rule 7)."""
    runs = [r for line in block.lines for r in line.visible()]
    widths = [r.width / len(r.text) for r in runs if len(r.text.strip()) >= 3]
    char = statistics.median(widths) if widths else 8.0
    left = min(r.left for r in runs)
    gaps = [b.top - a.top for a, b in zip(block.lines, block.lines[1:]) if a.page == b.page and b.top > a.top]
    # The line spacing is the smallest gap: a larger one is a blank line.
    spacing = min(gaps) if gaps else 15
    out: list[str] = []
    previous = None
    for line in block.lines:
        if previous and previous.page == line.page and line.top - previous.top > 1.6 * spacing:
            out.append("")
        text = ""
        for run in sorted(line.runs, key=lambda r: r.left):
            column = max(0, round((run.left - left) / char))
            if len(text) < column:
                text += " " * (column - len(text))
            text += run.text
        out.append(text.rstrip())
        previous = line
    return "\n".join(out).strip("\n") + "\n"


def is_model(text: str) -> bool:
    """Whether a block looks like a model rather than a production or a table (rule 5)."""
    first = next((ln.strip() for ln in text.splitlines() if ln.strip()), "")
    return bool(MODEL_RE.search(text)) and not PRODUCTION_RE.match(first)


def snippets(spec: str, xml: str) -> list[dict]:
    """The snippets of one specification: id, clause, page, language and text."""
    _, language, scope = DOCS[spec]
    counts: dict[str, int] = {}
    result = []
    for block in blocks_of(lines_of(xml)):
        if not block.clause.startswith(scope):
            continue
        block = trim(block)
        if not block.lines:
            continue
        text = render(block)
        if not is_model(text):
            continue
        counts[block.clause] = counts.get(block.clause, 0) + 1
        n = counts[block.clause]
        result.append(
            {
                "id": f"{spec}/{block.clause}/{n}",
                "spec": spec,
                "clause": block.clause,
                "page": block.page,
                "language": language,
                "path": f"{spec}/{block.clause}-{n}.{language}",
                "text": text,
            }
        )
    return result


def write(out: Path) -> int:
    if shutil.which("pdftohtml") is None:
        sys.exit("pdftohtml not found: install poppler-utils")
    if out.exists():
        shutil.rmtree(out)
    index = []
    for spec, (pdf, _, _) in DOCS.items():
        path = SPECS / pdf
        if not path.exists():
            sys.exit(f"{path}: not found. Run `python scripts/fetch_specs.py fetch` first.")
        xml = subprocess.run(
            ["pdftohtml", "-xml", "-i", "-q", "-stdout", str(path)], capture_output=True, check=True
        ).stdout.decode("utf-8", errors="replace")
        for s in snippets(spec, xml):
            target = out / s["path"]
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(s.pop("text"), encoding="utf-8", newline="\n")
            index.append(s)
    (out / "index.json").write_text(json.dumps({"version": 1, "snippets": index}, indent=1) + "\n", encoding="utf-8")
    return len(index)


def main(argv: list[str]) -> int:
    if argv[1:] == ["--check"]:
        with tempfile.TemporaryDirectory() as tmp:
            write(Path(tmp))
            different = subprocess.run(["diff", "-rq", tmp, str(OUT)], capture_output=True, text=True)
            if different.returncode:
                print(different.stdout or different.stderr)
                return 1
        print("snippets are up to date")
        return 0
    n = write(OUT)
    print(f"{n} snippets in {OUT.relative_to(ROOT) if OUT.is_relative_to(ROOT) else OUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
