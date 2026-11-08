"""Runs the quickstart notebook that `python -m subgraphsearch quickstart` hands to users, to catch
changes in subgraphsearch or its optional dependencies that break it.

Only checks that every cell runs without error: the outputs vary with platform and library
versions. Skipped unless nbformat, nbclient and ipykernel are installed.
"""

from pathlib import Path

import pytest
import subgraphsearch

nbformat = pytest.importorskip("nbformat")
nbclient = pytest.importorskip("nbclient")
pytest.importorskip("ipykernel")

NOTEBOOK = Path(subgraphsearch.__file__).with_name("quickstart.ipynb")


# CI turns EncodingWarning into an error; here it comes from jupyter_client starting the kernel,
# not from subgraphsearch, whose code runs in the kernel's own process
@pytest.mark.filterwarnings("ignore::EncodingWarning")
def test_quickstart_runs(monkeypatch):
    # Draw plots without a display
    monkeypatch.setenv("MPLBACKEND", "Agg")
    nb = nbformat.read(NOTEBOOK, as_version=4)
    nbclient.NotebookClient(nb, timeout=600, kernel_name="python3").execute()
