"""Writes PyPI's copy of the README to python/PYPI_README.md (not committed).

PyPI cannot render GitHub's math ($`...`$) and does not create heading anchors, so the copy turns
each math span into a code span with Unicode symbols and points in-page links and relative links
at GitHub. The release workflow runs this with --apply, which also points pyproject.toml's readme
at the copy, just before building; CI runs it without --apply on every push, so math it cannot
convert fails early. Run it locally to preview the PyPI page's text.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "README.md"
TARGET = ROOT / "python" / "PYPI_README.md"
REPO = "https://github.com/vglazer/subgraphsearch"

# LaTeX in the README's math spans, as plain text
LATEX = [
    (r"\sum_{v \in S}", "Σ_{v∈S}"),
    (r"\lambda", "λ"),
    (r"\mu", "μ"),
    (r"\sim", "~"),
    (r"\in", "∈"),
]


def plain_math(match: re.Match) -> str:
    text = match.group(1)
    for latex, plain in LATEX:
        text = text.replace(latex, plain)
    if "\\" in text:
        raise SystemExit(f"no plain-text form for the math {match.group(0)!r}: add it to LATEX in {__file__}")
    return f"`{text}`"


def pypi_readme(text: str) -> str:
    text = re.sub(r"\$`([^`]*)`\$", plain_math, text)
    text = re.sub(r"\]\(#", f"]({REPO}#", text)
    text = re.sub(r"\]\((?!https?://|#|mailto:)([^)]+)\)", rf"]({REPO}/blob/main/\1)", text)
    header = "<!-- Generated from README.md by scripts/make_pypi_readme.py; edit README.md instead. -->\n"
    return header + text


def main() -> None:
    args = sys.argv[1:]
    if args not in ([], ["--apply"]):
        raise SystemExit("usage: make_pypi_readme.py [--apply]")
    TARGET.write_text(pypi_readme(SOURCE.read_text(encoding="utf-8")), encoding="utf-8")
    print(TARGET)
    if args == ["--apply"]:
        pyproject = ROOT / "pyproject.toml"
        text = pyproject.read_text(encoding="utf-8")
        new = text.replace('readme = "README.md"', 'readme = "python/PYPI_README.md"')
        if new == text:
            raise SystemExit('pyproject.toml has no readme = "README.md" to replace')
        pyproject.write_text(new, encoding="utf-8")
        print("pyproject.toml now uses it as the readme")


if __name__ == "__main__":
    main()
