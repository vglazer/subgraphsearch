"""The Graph type and graph generation."""

from __future__ import annotations

import math
from collections.abc import Iterable, Sequence
from os import PathLike
from typing import TYPE_CHECKING

import numpy as np

from . import _core

if TYPE_CHECKING:
    from ._search import SearchResult

MODELS = ("exponential", "power", "geometric")


class Graph:
    """An immutable simple undirected graph on vertices ``0 .. n-1``.

    Build one from an edge list, read it from a file (graph6, sparse6, Matrix Market, edge list
    or ``sub_search`` adjacency lists), generate it with :func:`subgraphsearch.generate`, or convert it from
    networkx, igraph or SciPy with :mod:`subgraphsearch.convert`.

    >>> g = Graph(4, [(0, 1), (1, 2), (2, 3)])
    >>> g.num_vertices, g.num_edges
    (4, 3)
    """

    __slots__ = ("_g",)

    def __init__(self, n: int = 0, edges: Iterable[tuple[int, int]] | np.ndarray = ()):
        if isinstance(n, _core.Graph):
            self._g = n
            return
        if isinstance(edges, np.ndarray):
            arr = np.ascontiguousarray(edges, dtype="<u4").reshape(-1, 2)
            self._g = _core.Graph.from_edge_bytes(int(n), arr.tobytes())
        else:
            self._g = _core.Graph(int(n), [(int(u), int(v)) for u, v in edges])

    # ---- construction and I/O

    @staticmethod
    def read(path: str | PathLike, format: str | None = None, index: int | None = None) -> Graph:
        """Reads a graph file. The format is inferred from the extension unless given:
        ``.g6`` graph6, ``.s6`` sparse6, ``.mtx`` Matrix Market, ``.edges``/``.csv`` edge list,
        ``.txt`` ``sub_search`` '-1'-terminated adjacency lists.

        graph6 and sparse6 files can hold many graphs, one per line; pick one with ``index``
        (from 0), or read them all with :func:`read_all`.
        """
        if index is not None:
            return _pick(read_all(path, format), index, str(path))
        return Graph(_core.Graph.read(str(path), format))

    @staticmethod
    def parse(text: str, format: str, index: int | None = None) -> Graph:
        """Parses a graph from text, e.g. ``Graph.parse("DQc", "graph6")``."""
        if index is not None:
            return _pick(parse_all(text, format), index, "the text")
        return Graph(_core.Graph.parse(text, format))

    @staticmethod
    def point_graph(blocks: Iterable[Iterable[int]], k: int = 1, *, points: int | None = None) -> Graph:
        """The graph on the points of an incidence structure (for example a block design), in which
        two points are adjacent when exactly ``k`` of the ``blocks`` contain both.

        ``points`` is the number of points, by default one more than the largest listed.

        >>> fano = [(0, 1, 2), (0, 3, 4), (0, 5, 6), (1, 3, 5), (1, 4, 6), (2, 3, 6), (2, 4, 5)]
        >>> Graph.point_graph(fano).num_edges   # every two points share a line: K7
        21
        """
        return Graph(_core.incidence_graph(_blocks(blocks), int(k), False, points))

    @staticmethod
    def block_graph(blocks: Iterable[Iterable[int]], k: int = 1, *, points: int | None = None) -> Graph:
        """The graph on the ``blocks`` of an incidence structure, in which two blocks are adjacent
        when they share exactly ``k`` points.

        >>> pairs = [(i, j) for i in range(8) for j in range(i + 1, 8)]
        >>> t8 = Graph.block_graph(pairs)   # the triangular graph T(8)
        >>> t8.num_vertices, set(t8.degrees().tolist())
        (28, {12})
        """
        return Graph(_core.incidence_graph(_blocks(blocks), int(k), True, points))

    def dumps(self, format: str, highlight: Iterable[int] = ()) -> str:
        """Serializes the graph. ``highlight`` marks vertices in ``dot`` and ``graphml`` output."""
        return self._g.dumps(format, [int(v) for v in highlight])

    def write(self, path: str | PathLike, format: str | None = None, highlight: Iterable[int] = ()) -> None:
        """Writes the graph to a file (format inferred from the extension unless given)."""
        self._g.write(str(path), format, [int(v) for v in highlight])

    def to_graph6(self) -> str:
        """The graph6 string for this graph (no header or newline)."""
        return self.dumps("graph6").strip()

    # ---- basic properties

    @property
    def num_vertices(self) -> int:
        return self._g.num_vertices

    @property
    def num_edges(self) -> int:
        return self._g.num_edges

    n = num_vertices
    m = num_edges

    @property
    def density(self) -> float:
        """Edge density ``m / (n choose 2)``."""
        return self._g.density

    def __len__(self) -> int:
        return self.num_vertices

    def edges(self) -> np.ndarray:
        """Edges as an ``(m, 2)`` uint32 array with ``u < v``, sorted."""
        return np.frombuffer(self._g.edge_bytes(), dtype="<u4").reshape(-1, 2).copy()

    def edge_list(self) -> list[tuple[int, int]]:
        """Edges as a list of ``(u, v)`` tuples with ``u < v``."""
        return self._g.edges()

    def degrees(self) -> np.ndarray:
        return np.asarray(self._g.degrees(), dtype=np.int64)

    def neighbors(self, u: int) -> np.ndarray:
        return np.asarray(self._g.neighbors(int(u)), dtype=np.int64)

    def has_edge(self, u: int, v: int) -> bool:
        return self._g.has_edge(int(u), int(v))

    def components(self) -> tuple[np.ndarray, int]:
        """``(label per vertex, number of connected components)``, by breadth-first search."""
        labels, count = self._g.components()
        return np.asarray(labels, dtype=np.int64), count

    def induced_edge_count(self, vertices: Iterable[int]) -> int:
        """Number of edges among ``vertices``."""
        return self._g.induced_edge_count([int(v) for v in vertices])

    # ---- shapes: whether the graph is one; for a subgraph a search found, ask its induced graph,
    # e.g. g.induced(r.best.vertices).is_path()

    def is_connected(self) -> bool:
        """Whether the graph has one connected component (and at least one vertex)."""
        return self.n > 0 and self.components()[1] == 1

    def is_tree(self) -> bool:
        """Whether the graph is a tree: connected, with one edge fewer than vertices."""
        return self.is_connected() and self.m == self.n - 1

    def is_path(self) -> bool:
        """Whether the graph is a path: a tree with no vertex of degree above 2."""
        return self.is_tree() and int(self.degrees().max()) <= 2

    def is_cycle(self) -> bool:
        """Whether the graph is a cycle: connected, at least 3 vertices, every degree 2."""
        return self.n >= 3 and self.is_connected() and bool((self.degrees() == 2).all())

    def is_clique(self) -> bool:
        """Whether every pair of vertices is joined."""
        return self.m == self.n * (self.n - 1) // 2

    def is_independent(self) -> bool:
        """Whether no pair of vertices is joined: the vertices form an independent set."""
        return self.m == 0

    # ---- matrices

    def adjacency_matrix(self, dtype=np.int8) -> np.ndarray:
        """Dense ``n x n`` adjacency matrix."""
        a = np.zeros((self.n, self.n), dtype=dtype)
        e = self.edges()
        a[e[:, 0], e[:, 1]] = 1
        a[e[:, 1], e[:, 0]] = 1
        return a

    def laplacian(self, normalized: bool = False) -> np.ndarray:
        """Dense Laplacian ``L = D - A``, or ``I - D^-1/2 A D^-1/2`` if ``normalized``."""
        a = self.adjacency_matrix(dtype=np.float64)
        d = a.sum(axis=1)
        if not normalized:
            return np.diag(d) - a
        with np.errstate(divide="ignore"):
            s = np.where(d > 0, 1.0 / np.sqrt(d), 0.0)
        return np.diag((d > 0).astype(float)) - s[:, None] * a * s[None, :]

    # ---- operations (return new graphs)

    def complement(self) -> Graph:
        return Graph(self._g.complement())

    def induced(self, vertices: Sequence[int]) -> Graph:
        """The subgraph induced on ``vertices``; vertex ``i`` of the result is ``vertices[i]``."""
        return Graph(self._g.induced([int(v) for v in vertices]))

    def switch(self, vertices: Iterable[int]) -> Graph:
        """Seidel switching with respect to ``vertices``."""
        return Graph(self._g.switch([int(v) for v in vertices]))

    def common_neighborhood(self, vertices: Iterable[int]) -> list[int]:
        """Vertices outside ``vertices`` adjacent to all of them."""
        return self._g.common_neighborhood([int(v) for v in vertices])

    def common_non_neighborhood(self, vertices: Iterable[int]) -> list[int]:
        """Vertices outside ``vertices`` adjacent to none of them."""
        return self._g.common_non_neighborhood([int(v) for v in vertices])

    # ---- search, layout, display

    def search(self, size: int, edges: int, **kwargs) -> SearchResult:
        """Shorthand for :func:`subgraphsearch.search` on this graph."""
        from ._search import search

        return search(self, size, edges, **kwargs)

    def layout(self, method: str = "spring", *, iterations: int = 150, seed: int = 1) -> np.ndarray:
        """Vertex positions in the unit square, as an ``(n, 2)`` array.

        ``method`` is ``"spring"`` (force-directed, computed in Rust), ``"spectral"`` (the
        Laplacian eigenvectors for the two smallest non-zero eigenvalues) or ``"circle"``.
        """
        if method == "spring":
            return np.asarray(self._g.spring_layout(iterations, seed), dtype=float).reshape(-1, 2)
        if method == "circle":
            t = 2 * np.pi * np.arange(self.n) / max(self.n, 1)
            return np.column_stack([0.5 + 0.45 * np.cos(t), 0.5 + 0.45 * np.sin(t)])
        if method == "spectral":
            if self.n < 3:
                return self.layout("circle")
            _, vecs = np.linalg.eigh(self.laplacian(normalized=True))
            xy = vecs[:, 1:3]
            lo, hi = xy.min(axis=0), xy.max(axis=0)
            span = (hi - lo).max() or 1.0
            return 0.5 + (xy - (lo + hi) / 2) / span
        raise ValueError(f"unknown layout method {method!r} (expected spring, spectral or circle)")

    def __repr__(self) -> str:
        return f"Graph(vertices={self.n}, edges={self.m}, density={self.density:.4f})"

    def __eq__(self, other: object) -> bool:
        return isinstance(other, Graph) and self._g == other._g

    def __hash__(self) -> int:
        return hash(self._g)

    def _repr_html_(self) -> str:
        from ._html import graph_html

        return graph_html(self)


def _pick(graphs: list[Graph], index: int, source: str) -> Graph:
    if not -len(graphs) <= index < len(graphs):
        raise IndexError(f"index {index} is out of range: {source} holds {len(graphs)} graph(s)")
    return graphs[index]


def read(path: str | PathLike, format: str | None = None, index: int | None = None) -> Graph:
    """Reads a graph file; see :meth:`Graph.read`."""
    return Graph.read(path, format, index)


def read_all(path: str | PathLike, format: str | None = None) -> list[Graph]:
    """Reads every graph in a file. graph6 and sparse6 files hold one graph per line, as nauty's
    ``geng`` and collections such as House of Graphs produce them; other formats hold one."""
    return [Graph(g) for g in _core.Graph.read_all(str(path), format)]


def parse_all(text: str, format: str) -> list[Graph]:
    """Parses every graph in ``text``; see :func:`read_all`."""
    return [Graph(g) for g in _core.Graph.parse_all(text, format)]


def dumps_all(graphs: Iterable[Graph], format: str = "graph6") -> str:
    """Serializes several graphs, one per line. Only graph6 and sparse6 can hold more than one."""
    return _core.dumps_all([g._g for g in graphs], format)


def write_all(path: str | PathLike, graphs: Iterable[Graph], format: str | None = None) -> None:
    """Writes several graphs to one file (graph6 or sparse6; inferred from the extension unless
    given)."""
    if format is None:
        ext = str(path).rsplit(".", 1)[-1].lower()
        format = {"g6": "graph6", "graph6": "graph6", "s6": "sparse6", "sparse6": "sparse6"}.get(ext)
        if format is None:
            raise ValueError(f"cannot tell a multi-graph format from {path!r}; use a .g6 or .s6 file or pass format=")
    with open(path, "w", encoding="ascii") as f:
        f.write(dumps_all(graphs, format))


def generate(
    model: str, n: int, density: float | None = None, *, mean_degree: float | None = None, seed: int = 1, rng: str = "xoshiro"
) -> Graph:
    """A random graph.

    Parameters
    ----------
    model:
        ``"exponential"`` (Erdős–Rényi), ``"power"`` (ggen's model: each vertex joined to a fixed
        number of random later vertices) or ``"geometric"`` (random geometric graph in the unit
        square).
    n:
        Number of vertices.
    density:
        Target edge density between 0 and 1, in steps of 0.001.
    mean_degree:
        Instead of ``density``, for ``"exponential"``: the mean number of neighbors, so each pair is
        an edge with probability ``mean_degree / (n - 1)``. Any value from 0 to ``n - 1`` works, and
        the graph is generated in time proportional to its edges rather than to the pairs of
        vertices, which suits large sparse graphs. Needs ``rng="xoshiro"``.
    seed:
        Positive random seed.
    rng:
        ``"xoshiro"`` (default) or ``"legacy"``, which reproduces the original ``ggen``'s graphs exactly
        (``density`` is ``ggen``'s per-mille density divided by 1000).
    """
    if model not in MODELS:
        raise ValueError(f"unknown model {model!r} (expected one of {', '.join(MODELS)})")
    if seed <= 0:
        raise ValueError(f"seed must be a positive integer, got {seed}")
    if (density is None) == (mean_degree is None):
        raise ValueError("give exactly one of density and mean_degree")
    if mean_degree is not None:
        if model != "exponential" or rng != "xoshiro":
            raise ValueError('mean_degree needs model="exponential" and rng="xoshiro"')
        return Graph(_core.sparse_random_graph(int(n), float(mean_degree), int(seed)))
    permille = round(density * 1000)
    if not 0 <= permille <= 1000 or not math.isclose(permille, density * 1000, abs_tol=1e-6):
        raise ValueError(f"density must be between 0 and 1 in steps of 0.001, got {density}")
    return Graph(_core.generate_graph(model, int(n), permille, int(seed), rng))


def hypercube(dimension: int) -> Graph:
    """The ``dimension``-dimensional hypercube: vertices ``0`` to ``2**dimension - 1``, joined when
    they differ in one bit. Up to 16 dimensions."""
    return Graph(_core.hypercube_graph(int(dimension)))


def grid(rows: int, cols: int) -> Graph:
    """The ``rows`` by ``cols`` grid: vertex ``r * cols + c`` is joined to the vertices beside it in
    its row and column."""
    return Graph(_core.grid_graph(int(rows), int(cols)))


class WeightedGraph:
    """An immutable undirected graph whose edges carry integer weights from 1 to 255, as searched
    by the original ``wsub_search``: the search target is then a total weight rather than an edge count.

    >>> w = WeightedGraph(3, [(0, 1, 2), (1, 2, 5)])
    >>> w.total_weight, w.weight(2, 1)
    (7, 5)
    """

    __slots__ = ("_g",)

    def __init__(self, n: int = 0, edges: Iterable[tuple[int, int, int]] | np.ndarray = ()):
        if isinstance(n, _core.WeightedGraph):
            self._g = n
            return
        self._g = _core.WeightedGraph(int(n), [(int(u), int(v), int(w)) for u, v, w in edges])

    @staticmethod
    def from_graph(graph: Graph) -> WeightedGraph:
        """Every edge of ``graph`` with weight 1."""
        return WeightedGraph(_core.WeightedGraph.from_graph(graph._g))

    @staticmethod
    def read(path: str | PathLike, format: str | None = None) -> WeightedGraph:
        """Reads weights from an edge list's third column, a Matrix Market file's values, or
        weighted lists (``format="wlists"``); formats without weights give every edge weight 1."""
        return WeightedGraph(_core.WeightedGraph.read(str(path), format))

    @staticmethod
    def parse(text: str, format: str) -> WeightedGraph:
        return WeightedGraph(_core.WeightedGraph.parse(text, format))

    def dumps(self, format: str, highlight: Iterable[int] = ()) -> str:
        """Serializes with weights: ``edgelist``, ``mtx``, ``wlists``, ``dot`` or ``graphml``."""
        return self._g.dumps(format, [int(v) for v in highlight])

    def write(self, path: str | PathLike, format: str | None = None, highlight: Iterable[int] = ()) -> None:
        self._g.write(str(path), format, [int(v) for v in highlight])

    @property
    def num_vertices(self) -> int:
        return self._g.num_vertices

    @property
    def num_edges(self) -> int:
        return self._g.num_edges

    n = num_vertices
    m = num_edges

    @property
    def total_weight(self) -> int:
        return self._g.total_weight

    @property
    def structure(self) -> Graph:
        """The unweighted graph with the same edges."""
        return Graph(self._g.structure())

    @property
    def density(self) -> float:
        """Edge density of the underlying unweighted graph."""
        return self.structure.density

    def __len__(self) -> int:
        return self.num_vertices

    def edges(self) -> np.ndarray:
        """Edges as an ``(m, 3)`` array of ``(u, v, weight)`` with ``u < v``."""
        return np.asarray(self._g.edges(), dtype=np.int64).reshape(-1, 3)

    def edge_list(self) -> list[tuple[int, int, int]]:
        return self._g.edges()

    def weight(self, u: int, v: int) -> int:
        """The weight of ``{u, v}``, or 0 if it is not an edge."""
        return self._g.weight(int(u), int(v))

    def neighbors(self, u: int) -> np.ndarray:
        return np.asarray(self._g.neighbors(int(u)), dtype=np.int64)

    def degrees(self) -> np.ndarray:
        """Number of neighbors of each vertex."""
        return self.structure.degrees()

    def weighted_degrees(self) -> np.ndarray:
        """Total weight of the edges at each vertex."""
        e = self.edges()
        return np.bincount(e[:, 0], weights=e[:, 2], minlength=self.n).astype(np.int64) + np.bincount(
            e[:, 1], weights=e[:, 2], minlength=self.n
        ).astype(np.int64)

    def induced_weight(self, vertices: Iterable[int]) -> int:
        """Total weight of the edges among ``vertices``."""
        return self._g.induced_weight([int(v) for v in vertices])

    def weight_matrix(self, dtype=np.int16) -> np.ndarray:
        """Dense ``n x n`` matrix of weights (0 for non-edges)."""
        a = np.zeros((self.n, self.n), dtype=dtype)
        e = self.edges()
        a[e[:, 0], e[:, 1]] = e[:, 2]
        a[e[:, 1], e[:, 0]] = e[:, 2]
        return a

    def layout(self, method: str = "spring", **kwargs) -> np.ndarray:
        """Positions of the underlying unweighted graph; see :meth:`Graph.layout`."""
        return self.structure.layout(method, **kwargs)

    def search(self, size: int, weight: int, **kwargs):
        """Shorthand for :func:`subgraphsearch.search` on this graph, with a target total ``weight``."""
        from ._search import search

        return search(self, size, weight, **kwargs)

    def __repr__(self) -> str:
        return f"WeightedGraph(vertices={self.n}, edges={self.m}, total_weight={self.total_weight})"

    def __eq__(self, other: object) -> bool:
        return isinstance(other, WeightedGraph) and self._g == other._g

    __hash__ = None  # type: ignore[assignment]

    def _repr_html_(self) -> str:
        from ._html import weighted_graph_html

        return weighted_graph_html(self)


def generate_weighted(n: int, mean_weight: float, *, seed: int = 1, rng: str = "xoshiro") -> WeightedGraph:
    """A weighted random graph as the original ``wggen`` makes it: ``n (n - 1) / 2 * mean_weight`` times,
    a uniformly random pair of vertices has its weight increased by 1.

    ``mean_weight`` is the mean weight per vertex pair, in steps of 0.01 (``wggen``'s density
    divided by 100); it may exceed 1. ``rng="legacy"`` reproduces ``wggen``'s graphs exactly.
    """
    percent = round(mean_weight * 100)
    if percent < 0 or not math.isclose(percent, mean_weight * 100, abs_tol=1e-6):
        raise ValueError(f"mean_weight must be a non-negative multiple of 0.01, got {mean_weight}")
    if seed <= 0:
        raise ValueError(f"seed must be a positive integer, got {seed}")
    return WeightedGraph(_core.generate_weighted(int(n), percent, int(seed), rng))


def structure_of(graph) -> Graph:
    """The unweighted graph underlying ``graph`` (which may already be unweighted)."""
    return graph.structure if isinstance(graph, WeightedGraph) else graph


def _blocks(blocks: Iterable[Iterable[int]]) -> list[list[int]]:
    return [[int(p) for p in block] for block in blocks]
