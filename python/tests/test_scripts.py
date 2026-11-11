"""Smoke tests for the scripts in examples/."""

import importlib.util
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]


def load(path: Path):
    """Import a script as a module, without running its main()."""
    spec = importlib.util.spec_from_file_location(path.stem, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_quickstart_notebooks_are_up_to_date():
    """Both checked-in notebooks have the cells make_quickstart.py builds: rerun it after editing."""
    nbformat = pytest.importorskip("nbformat")
    pytest.importorskip("nbclient")
    cells = [c.source for c in load(ROOT / "examples" / "make_quickstart.py").cells]
    for notebook in (ROOT / "examples" / "quickstart.ipynb",
                     ROOT / "python" / "subgraphsearch" / "quickstart.ipynb"):
        assert [c.source for c in nbformat.read(notebook, as_version=4).cells] == cells, notebook


# CI turns EncodingWarning into an error; matplotlib's font cache raises one, not the script
@pytest.mark.filterwarnings("ignore::EncodingWarning:matplotlib")
def test_make_figures_draws_the_readme_figures(tmp_path):
    pytest.importorskip("matplotlib")
    load(ROOT / "examples" / "make_figures.py").main(tmp_path)
    assert (tmp_path / "overview.png").stat().st_size > 0
    assert (tmp_path / "comparison.png").stat().st_size > 0
