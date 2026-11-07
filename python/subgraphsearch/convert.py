"""Conversions to and from networkx, igraph, SciPy sparse matrices and numpy arrays.

Each ``from_*`` function maps the foreign graph's vertices to ``0 .. n-1`` in their natural order
(``list(G.nodes)`` for networkx) and ignores self-loops, edge directions and attributes. Weights
are kept for a :class:`~subgraphsearch.WeightedGraph` (``to_networkx``, ``to_scipy``) and read when
asked for (``from_networkx(weight=...)``, ``from_scipy(weighted=True)``); otherwise ignored.
"""

from __future__ import annotations

import numpy as np

from ._graph import Graph, WeightedGraph


def _clean(n: int, edges: np.ndarray) -> Graph:
    edges = np.asarray(edges, dtype=np.int64).reshape(-1, 2)
    edges = edges[edges[:, 0] != edges[:, 1]]
    return Graph(n, edges)


def to_networkx(graph: Graph | WeightedGraph):
    """A ``networkx.Graph`` with nodes ``0 .. n-1`` (edge attribute ``weight`` for weighted graphs)."""
    import networkx as nx

    g = nx.Graph()
    g.add_nodes_from(range(graph.n))
    if isinstance(graph, WeightedGraph):
        g.add_weighted_edges_from(graph.edge_list())
    else:
        g.add_edges_from(graph.edge_list())
    return g


def from_networkx(g, *, weight: str | None = None, return_nodes: bool = False):
    """A :class:`Graph` from a networkx graph or, with ``weight`` naming an integer edge
    attribute (edges without it get 1), a :class:`WeightedGraph`. With ``return_nodes=True``
    also returns the networkx nodes in vertex order, to map results back."""
    nodes = list(g.nodes)
    index = {v: i for i, v in enumerate(nodes)}
    if weight is not None:
        triples = [(index[u], index[v], int(d.get(weight, 1))) for u, v, d in g.edges(data=True) if u != v]
        out = WeightedGraph(len(nodes), triples)
    else:
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


def to_scipy(graph: Graph | WeightedGraph, format: str = "csr"):
    """The adjacency matrix as a symmetric SciPy sparse array: int8 ones, or int16 weights for a
    weighted graph."""
    import scipy.sparse as sp

    e = graph.edges().astype(np.int64)
    rows = np.concatenate([e[:, 0], e[:, 1]])
    cols = np.concatenate([e[:, 1], e[:, 0]])
    if isinstance(graph, WeightedGraph):
        data = np.concatenate([e[:, 2], e[:, 2]]).astype(np.int16)
    else:
        data = np.ones(len(rows), dtype=np.int8)
    return sp.coo_array((data, (rows, cols)), shape=(graph.n, graph.n)).asformat(format)


def from_scipy(a, *, weighted: bool = False):
    """A :class:`Graph` from a square sparse matrix: any non-zero off-diagonal entry, in either
    triangle, is an edge. With ``weighted=True``, a :class:`WeightedGraph` whose weights are the
    entries (which must be integers from 1 to 255)."""
    import scipy.sparse as sp

    coo = sp.coo_array(a)
    if coo.shape[0] != coo.shape[1]:
        raise ValueError(f"an adjacency matrix must be square, got {coo.shape}")
    mask = (coo.data != 0) & (coo.row != coo.col)
    r, c = coo.row[mask], coo.col[mask]
    if weighted:
        vals = coo.data[mask]
        if np.any(vals != np.round(vals)) or np.any(vals < 1) or np.any(vals > 255):
            raise ValueError("weights must be integers from 1 to 255")
        return WeightedGraph(coo.shape[0], [(int(u), int(v), int(w)) for u, v, w in zip(r, c, vals)])
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
