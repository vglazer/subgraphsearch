"""Sets the version of every crate to X.Y.Z (run: python scripts/bump_version.py X.Y.Z).

The crates share the workspace version in Cargo.toml, and the crates that depend on each other name
that version too, since a crate published to crates.io must say which version of its dependency it
needs. This updates both, then Cargo.lock. The Python package takes its version from the crate, so
pyproject.toml needs no change. Afterwards, commit and push, then create a release on GitHub with
the tag vX.Y.Z (see .github/workflows/release.yml).
"""

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFESTS = ["crates/subgraphsearch-cli/Cargo.toml", "crates/subgraphsearch-py/Cargo.toml"]


def replace(path: Path, pattern: str, repl: str, expected: int) -> None:
    text = path.read_text(encoding="utf-8")
    new, n = re.subn(pattern, repl, text, flags=re.MULTILINE)
    if n != expected:
        raise SystemExit(f"{path.relative_to(ROOT)}: expected {expected} version(s) to update, found {n}")
    path.write_text(new, encoding="utf-8")


def main() -> None:
    if len(sys.argv) != 2 or not re.fullmatch(r"\d+\.\d+\.\d+", sys.argv[1]):
        raise SystemExit("usage: bump_version.py X.Y.Z")
    version = sys.argv[1]
    replace(ROOT / "Cargo.toml", r'^version = "\d+\.\d+\.\d+"', f'version = "{version}"', 1)
    # the dependencies on this workspace's own crates, e.g. subgraphsearch-core = { version = "0.2.2", path = ... }
    pattern = r'^(subgraphsearch(?:-core)? = \{ version = ")\d+\.\d+\.\d+(", path = )'
    replace(ROOT / MANIFESTS[0], pattern, rf"\g<1>{version}\g<2>", 1)
    replace(ROOT / MANIFESTS[1], pattern, rf"\g<1>{version}\g<2>", 2)
    subprocess.run(["cargo", "update", "--workspace", "--offline", "--quiet"], cwd=ROOT, check=True)
    print(f"version {version}; now commit, push and create the release v{version}")


if __name__ == "__main__":
    main()
