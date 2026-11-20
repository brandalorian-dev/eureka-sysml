#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Split the specification PDFs in docs/specs/ into one Markdown file per clause.

Output goes to docs/specs/text/<doc>/ (git-ignored, like all of docs/specs/). It lets
Claude Code and developers load an exact clause (for example KerML 8.4.4) instead of a
whole 600-page PDF. Requires `pdftotext` from poppler-utils on PATH.

Method:
  1. Run `pdftotext -layout` on each PDF.
  2. Read the table of contents, which consists of lines ending in dot leaders and a page number.
  3. Find each heading from the TOC in the body, in order. Each clause runs until the next heading.

Clauses are split down to SPLIT_DEPTH levels (for example 8.2.3). Deeper headings stay
inside their parent file.
"""
from __future__ import annotations

import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPECS = ROOT / "docs" / "specs"
OUT = SPECS / "text"
SPLIT_DEPTH = 3

DOCS = {
    "kerml": "KerML/OMG KerML v1.0 (formal-26-03-01).pdf",
    "sysml-part1": "OMG SysML v2.0 Part 1 - Language Specification (formal-26-03-02).pdf",
    "sysml-part2": "OMG SysML v2.0 Part 2 - v1 to v2 Transformation (formal-26-03-03).pdf",
    "api": "api/omg/OMG Systems Modeling API and Services v1.0 (formal-26-03-04).pdf",
}

TOC_RE = re.compile(r"^\s*((?:\d+|[A-Z])(?:\.\d+)*)\s+(.+?)\s*\.{4,}\s*\d+\s*$")
FOOTER_RE = re.compile(r"^\s*(?:\d+\s+)?(?:Kernel Modeling Language|Systems Modeling (?:Language|API)).*?\d*\s*$")


def slug(s: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", s.lower()).strip("-")[:60]


def depth(num: str) -> int:
    return num.count(".") + 1


def extract(name: str, pdf: Path) -> int:
    text = subprocess.run(["pdftotext", "-layout", str(pdf), "-"], capture_output=True, check=True).stdout
    lines = text.decode("utf-8", errors="replace").splitlines()

    toc, toc_end = [], 0
    for i, ln in enumerate(lines):
        m = TOC_RE.match(ln)
        if m and depth(m.group(1)) <= SPLIT_DEPTH:
            toc.append((m.group(1), re.sub(r"\s+", " ", m.group(2))))
        if m:
            toc_end = i
    if not toc:
        print(f"{name}: no table of contents found", file=sys.stderr)
        return 0

    # Locate each heading in the body (after the TOC), in order.
    starts, pos = [], toc_end + 1
    for num, title in toc:
        pat = re.compile(rf"^\s*(?:Annex\s+)?{re.escape(num)}\s+(?:Annex:\s*)?{re.escape(title[:40])}")
        for j in range(pos, len(lines)):
            if pat.match(lines[j]):
                starts.append((j, num, title))
                pos = j + 1
                break
    out_dir = OUT / name
    if out_dir.exists():
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True)

    index = [f"# {name}: clause index\n", f"Source: `docs/specs/{DOCS[name]}`\n"]
    for k, (j, num, title) in enumerate(starts):
        end = starts[k + 1][0] if k + 1 < len(starts) else len(lines)
        body = [ln.rstrip() for ln in lines[j:end] if not FOOTER_RE.match(ln) and "\f" not in ln]
        fname = f"{num}-{slug(title)}.md"
        (out_dir / fname).write_text(
            f"<!-- {name} clause {num}; extracted from docs/specs/{DOCS[name]} -->\n"
            f"# {num} {title}\n\n```text\n" + "\n".join(body).strip("\n") + "\n```\n",
            encoding="utf-8",
        )
        index.append(f"- [{num} {title}]({fname})")
    (out_dir / "INDEX.md").write_text("\n".join(index) + "\n", encoding="utf-8")
    missed = len(toc) - len(starts)
    print(f"{name}: {len(starts)} clauses written" + (f" ({missed} TOC entries not located)" if missed else ""))
    return len(starts)


def main() -> int:
    if not shutil.which("pdftotext"):
        print("pdftotext not found. Install poppler-utils.", file=sys.stderr)
        return 1
    total = 0
    for name, rel in DOCS.items():
        pdf = SPECS / rel
        if not pdf.exists():
            print(f"{name}: missing {rel}. Run `python scripts/fetch_specs.py status`.", file=sys.stderr)
            continue
        total += extract(name, pdf)
    return 0 if total else 1


if __name__ == "__main__":
    sys.exit(main())
