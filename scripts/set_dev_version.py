"""Gives a TestPyPI rehearsal build its own version, VERSION.devN (run: python scripts/set_dev_version.py N).

TestPyPI, like PyPI, accepts each version only once, so every rehearsal of the release workflow
publishes a development release numbered after the workflow run (for example 0.1.0.dev7) instead
of the version in Cargo.toml. It sets the version statically in pyproject.toml, which maturin uses
in place of the crate version. Real releases do not run this.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main() -> None:
    if len(sys.argv) != 2 or not sys.argv[1].isdigit():
        raise SystemExit("usage: set_dev_version.py RUN_NUMBER")
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r'^version = "(\d+\.\d+\.\d+)"', cargo, re.MULTILINE)
    if match is None:
        raise SystemExit("no version = \"X.Y.Z\" in Cargo.toml")
    version = f"{match.group(1)}.dev{sys.argv[1]}"
    pyproject = ROOT / "pyproject.toml"
    text = pyproject.read_text(encoding="utf-8")
    new = text.replace('dynamic = ["version"]', f'version = "{version}"')
    if new == text:
        raise SystemExit('pyproject.toml has no dynamic = ["version"] to replace')
    pyproject.write_text(new, encoding="utf-8")
    print(version)


if __name__ == "__main__":
    main()
