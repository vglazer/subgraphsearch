"""Conversions to and from networkx, igraph, SciPy sparse matrices and numpy arrays.

Each ``from_*`` function maps the foreign graph's vertices to ``0 .. n-1`` in their natural order
(``list(G.nodes)`` for networkx) and ignores self-loops, edge directions, weights and attributes.
"""

from __future__ import annotations

import numpy as np

from ._graph import Graph


def _clean(n: int, edges: np.ndarray) -> Graph:
    edges = np.asarray(edges, dtype=np.int64).reshape(-1, 2)
    edges = edges[edges[:, 0] != edges[:, 1]]
    return Graph(n, edges)


def to_networkx(graph: Graph):
    """A ``networkx.Graph`` with nodes ``0 .. n-1``."""
    import networkx as nx

    g = nx.Graph()
    g.add_nodes_from(range(graph.n))
    g.add_edges_from(graph.edge_list())
    return g


def from_networkx(g, *, return_nodes: bool = False):
    """A :class:`Graph` from a networkx graph. With ``return_nodes=True`` also returns the
    networkx nodes in vertex order, to map results back."""
    nodes = list(g.nodes)
    index = {v: i for i, v in enumerate(nodes)}
    edges = np.array([(index[u], index[v]) for u, v in g.edges()], dtype=np.int64)
    out = _clean(len(nodes), edges)
    return (out, nodes) if return_nodes else out


def to_igraph(graph: Graph):
    """An undirected ``igraph.Graph``."""
    import igraph as ig

    return ig.Graph(n=graph.n, edges=graph.edge_list(), directed=False)


def from_igraph(g) -> Graph:
    """A :class:`Graph` from an ``igraph.Graph`` (multi-edges collapse)."""
    return _clean(g.vcount(), np.array(g.get_edgelist(), dtype=np.int64))


def to_scipy(graph: Graph, format: str = "csr"):
    """The adjacency matrix as a symmetric SciPy sparse array of int8."""
    import scipy.sparse as sp

    e = graph.edges().astype(np.int64)
    rows = np.concatenate([e[:, 0], e[:, 1]])
    cols = np.concatenate([e[:, 1], e[:, 0]])
    data = np.ones(len(rows), dtype=np.int8)
    return sp.coo_array((data, (rows, cols)), shape=(graph.n, graph.n)).asformat(format)


def from_scipy(a) -> Graph:
    """A :class:`Graph` from a square sparse matrix: any non-zero off-diagonal entry, in either
    triangle, is an edge."""
    import scipy.sparse as sp

    coo = sp.coo_array(a)
    if coo.shape[0] != coo.shape[1]:
        raise ValueError(f"an adjacency matrix must be square, got {coo.shape}")
    mask = (coo.data != 0) & (coo.row != coo.col)
    r, c = coo.row[mask], coo.col[mask]
    pairs = np.unique(np.column_stack([np.minimum(r, c), np.maximum(r, c)]), axis=0)
    return Graph(coo.shape[0], pairs)


def from_numpy(a: np.ndarray) -> Graph:
    """A :class:`Graph` from a dense square adjacency matrix (non-zero off-diagonal = edge)."""
    a = np.asarray(a)
    if a.ndim != 2 or a.shape[0] != a.shape[1]:
        raise ValueError(f"an adjacency matrix must be square, got shape {a.shape}")
    sym = (a != 0) | (a.T != 0)
    u, v = np.nonzero(np.triu(sym, k=1))
    return Graph(a.shape[0], np.column_stack([u, v]))


def to_numpy(graph: Graph, dtype=np.int8) -> np.ndarray:
    """The dense adjacency matrix."""
    return graph.adjacency_matrix(dtype)
