"""Static node-link drawings of a graph, with a vertex set (such as a search result) highlighted."""

from __future__ import annotations

from collections.abc import Iterable

import numpy as np

from .._graph import Graph, WeightedGraph, structure_of
from ._style import ACCENT, NEUTRAL, axes


def plot_graph(
    graph: Graph | WeightedGraph,
    highlight: Iterable[int] = (),
    ax=None,
    *,
    layout: str | np.ndarray = "spring",
    seed: int = 1,
    node_size: float | None = None,
    title: str | None = None,
):
    """Draws ``graph`` with the vertices in ``highlight`` and the edges among them in color.

    ``layout`` is a method name for :meth:`Graph.layout` (``"spring"``, ``"spectral"`` or
    ``"circle"``) or an ``(n, 2)`` array of positions. Edges outside the highlighted subgraph are
    drawn faintly, more so for larger graphs, so the subgraph stands out.
    """
    from matplotlib.collections import LineCollection

    g = structure_of(graph)
    ax = axes(ax)
    pos = g.layout(layout, seed=seed) if isinstance(layout, str) else np.asarray(layout, dtype=float)
    if pos.shape != (g.n, 2):
        raise ValueError(f"layout must have shape ({g.n}, 2), got {pos.shape}")
    marked = np.zeros(g.n, dtype=bool)
    marked[[int(v) for v in highlight]] = True

    e = g.edges().astype(np.int64)
    inside = marked[e[:, 0]] & marked[e[:, 1]] if len(e) else np.zeros(0, dtype=bool)
    segments = pos[e]  # (m, 2, 2)
    faint = min(0.6, 40.0 / max(g.m, 1) ** 0.5)
    ax.add_collection(LineCollection(segments[~inside], colors=NEUTRAL, linewidths=0.6, alpha=faint, zorder=1))
    if inside.any():
        ax.add_collection(LineCollection(segments[inside], colors=ACCENT, linewidths=1.8, alpha=0.95, zorder=2))

    size = node_size if node_size is not None else max(6.0, min(60.0, 3000.0 / max(g.n, 1)))
    ax.scatter(pos[~marked, 0], pos[~marked, 1], s=size, color=NEUTRAL, zorder=3, linewidths=0)
    ax.scatter(pos[marked, 0], pos[marked, 1], s=size * 2.2, color=ACCENT, zorder=4, edgecolors="white", linewidths=0.8)
    ax.set_aspect("equal")
    ax.margins(0.03)
    ax.set_axis_off()
    if title:
        ax.set_title(title, fontsize=10, loc="left")
    return ax
