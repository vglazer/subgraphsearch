"""subgraphsearch: find induced subgraphs with a given number of vertices and edges.

Cliques, independent sets and everything in between, regular subgraphs, and subgraphs of a given
total weight, by a fast randomized local search; with random graph generators, graph operations,
standard formats, conversions to networkx / igraph / SciPy, and visualization.

>>> import subgraphsearch as ss
>>> g = ss.generate("exponential", 100, 0.6, seed=1)
>>> r = ss.search(g, size=8, edges=28, experiments=5, max_moves=200)  # 8-cliques
>>> r.successes
5
"""

from ._core import __version__
from ._graph import (
    MODELS,
    Graph,
    WeightedGraph,
    dumps_all,
    generate,
    generate_weighted,
    parse_all,
    read,
    read_all,
    write_all,
)
from ._search import Experiment, SearchParams, SearchResult, search, sweep
from . import convert, legacy, viz  # noqa: E402
from . import _core


def formats() -> dict[str, bool]:
    """Supported file formats, mapped to whether they can be read (DOT and GraphML are
    write-only)."""
    return dict(_core.formats_list())


__all__ = [
    "MODELS",
    "Experiment",
    "Graph",
    "SearchParams",
    "SearchResult",
    "WeightedGraph",
    "__version__",
    "convert",
    "dumps_all",
    "formats",
    "generate",
    "generate_weighted",
    "legacy",
    "parse_all",
    "read",
    "read_all",
    "search",
    "sweep",
    "viz",
    "write_all",
]
