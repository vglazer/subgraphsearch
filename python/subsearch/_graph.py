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
    or USRA adjacency lists), generate it with :func:`subsearch.generate`, or convert it from
    networkx, igraph or SciPy with :mod:`subsearch.convert`.

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
    def read(path: str | PathLike, format: str | None = None) -> Graph:
        """Reads a graph file. The format is inferred from the extension unless given:
        ``.g6`` graph6, ``.s6`` sparse6, ``.mtx`` Matrix Market, ``.edges``/``.csv`` edge list,
        ``.txt`` USRA '-1'-terminated adjacency lists."""
        return Graph(_core.Graph.read(str(path), format))

    @staticmethod
    def parse(text: str, format: str) -> Graph:
        """Parses a graph from text, e.g. ``Graph.parse("DQc", "graph6")``."""
        return Graph(_core.Graph.parse(text, format))

    def dumps(self, format: str, highlight: Iterable[int] = ()) -> str:
        """Serialises the graph. ``highlight`` marks vertices in ``dot`` and ``graphml`` output."""
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

    def common_neighbourhood(self, vertices: Iterable[int]) -> list[int]:
        """Vertices outside ``vertices`` adjacent to all of them."""
        return self._g.common_neighbourhood([int(v) for v in vertices])

    def common_non_neighbourhood(self, vertices: Iterable[int]) -> list[int]:
        """Vertices outside ``vertices`` adjacent to none of them."""
        return self._g.common_non_neighbourhood([int(v) for v in vertices])

    # ---- search, layout, display

    def search(self, size: int, edges: int, **kwargs) -> SearchResult:
        """Shorthand for :func:`subsearch.search` on this graph."""
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


def read(path: str | PathLike, format: str | None = None) -> Graph:
    """Reads a graph file; see :meth:`Graph.read`."""
    return Graph.read(path, format)


def generate(model: str, n: int, density: float, *, seed: int = 1, rng: str = "xoshiro") -> Graph:
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
    seed:
        Positive random seed.
    rng:
        ``"xoshiro"`` (default) or ``"legacy"``, which reproduces USRA ``ggen``'s graphs exactly
        (``density`` is ``ggen``'s per-mille density divided by 1000).
    """
    if model not in MODELS:
        raise ValueError(f"unknown model {model!r} (expected one of {', '.join(MODELS)})")
    permille = round(density * 1000)
    if not 0 <= permille <= 1000 or not math.isclose(permille, density * 1000, abs_tol=1e-6):
        raise ValueError(f"density must be between 0 and 1 in steps of 0.001, got {density}")
    if seed <= 0:
        raise ValueError(f"seed must be a positive integer, got {seed}")
    return Graph(_core.generate_graph(model, int(n), permille, int(seed), rng))
