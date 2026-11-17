"""Sphinx configuration for subgraphsearch's API reference, built by Read the Docs
(.readthedocs.yaml) from the installed package's docstrings."""

import subgraphsearch

project = "subgraphsearch"
author = "Victor Glazer"
release = subgraphsearch.__version__
version = release

extensions = ["sphinx.ext.autodoc", "sphinx.ext.intersphinx", "sphinx.ext.viewcode"]
autodoc_default_options = {"members": True, "undoc-members": False, "show-inheritance": False}
autodoc_member_order = "bysource"
autodoc_typehints = "description"
intersphinx_mapping = {
    "python": ("https://docs.python.org/3", None),
    "numpy": ("https://numpy.org/doc/stable", None),
}

html_theme = "furo"
html_title = f"subgraphsearch {release}"
exclude_patterns = ["_build"]
