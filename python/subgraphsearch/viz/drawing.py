"""Static node-link drawings of a graph, with a vertex set (such as a search result) highlighted."""

from __future__ import annotations

from collections.abc import Iterable, Mapping, Sequence

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
    labels: Mapping[int, str] | None = None,
    title: str | None = None,
):
    """Draws ``graph`` with the vertices in ``highlight`` and the edges among them in color.

    ``layout`` is a method name for :meth:`Graph.layout` (``"spring"``, ``"spectral"`` or
    ``"circle"``) or an ``(n, 2)`` array of positions. Edges outside the highlighted subgraph are
    drawn faintly, more so for larger graphs, so the subgraph stands out. ``labels`` maps vertices
    to text drawn beside them, for example names for the highlighted vertices.
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
    if labels:
        # place each label away from the center of the labeled vertices, so labels around a
        # group fan outward instead of piling up
        labeled = np.array([int(v) for v in labels])
        center = pos[labeled].mean(axis=0)
        for v, text in labels.items():
            x, y = pos[int(v)]
            dx, dy = x - center[0], y - center[1]
            norm = float(np.hypot(dx, dy)) or 1.0
            ux, uy = dx / norm, dy / norm
            ax.annotate(str(text), (x, y), xytext=(9 * ux, 9 * uy), textcoords="offset points", fontsize=8,
                        ha="left" if ux > 0.3 else "right" if ux < -0.3 else "center",
                        va="bottom" if uy > 0.3 else "top" if uy < -0.3 else "center", zorder=5, color="#343a40",
                        bbox={"boxstyle": "round,pad=0.15", "fc": "white", "ec": "none", "alpha": 0.75})
    ax.set_aspect("equal")
    ax.margins(0.03)
    ax.set_axis_off()
    if title:
        ax.set_title(title, fontsize=10, loc="left")
    return ax


def plot_fano_plane(ax=None, *, title: str | None = "the Fano plane: 7 points, 7 lines"):
    """Draws the Fano plane, :data:`subgraphsearch.designs.FANO_PLANE`, in its usual picture: the
    corners of a triangle, the midpoints of its sides and its center, with six straight lines and
    the circle through the midpoints. Each line has its own color."""
    import matplotlib.pyplot as plt

    from ..designs import FANO_PLANE

    if ax is None:
        _, ax = plt.subplots(figsize=(4.2, 4), layout="constrained")
    xy = {0: (0.0, 1.732), 1: (-1.0, 0.0), 2: (1.0, 0.0), 3: (-0.5, 0.866), 4: (0.0, 0.0), 5: (0.5, 0.866),
          6: (0.0, 0.577)}
    for line, color in zip(FANO_PLANE, plt.cm.tab10.colors):
        if set(line) == {3, 4, 5}:  # the circle through the midpoints, around the center
            radius = float(np.hypot(*np.subtract(xy[3], xy[6])))
            ax.add_patch(plt.Circle(xy[6], radius, fill=False, color=color, lw=2.2))
        else:  # a straight line, drawn between its two farthest points
            a, b = max(((a, b) for a in line for b in line if a < b),
                       key=lambda e: float(np.hypot(*np.subtract(xy[e[0]], xy[e[1]]))))
            ax.plot(*zip(xy[a], xy[b]), color=color, lw=2.2)
    for v, (x, y) in xy.items():
        ax.scatter(x, y, s=260, color="white", edgecolors="#343a40", linewidths=1.5, zorder=3)
        ax.text(x, y, str(v), ha="center", va="center", fontsize=10, zorder=4)
    ax.set_aspect("equal")
    ax.set_axis_off()
    if title:
        ax.set_title(title, fontsize=10, loc="left")
    return ax


def plot_switchings(
    graph: Graph,
    vertex_sets: Sequence[Iterable[int]],
    *,
    titles: Sequence[str] | None = None,
    graph_title: str | None = None,
    above: Sequence[Graph] | None = None,
    above_titles: Sequence[str] | None = None,
):
    """Draws ``graph`` and, beside it, ``graph`` Seidel-switched with respect to each of
    ``vertex_sets`` (:meth:`Graph.switch`), one column each.

    In each switched graph the set's vertices are highlighted and placed first around the circle, in
    path or cycle order, so the subgraph they induce (which switching leaves unchanged) is drawn as an
    arc. ``above`` optionally gives one small graph per column, the original's first, drawn in a row
    on top with each connected component on its own circle, for example the structure each set
    stands for. Returns the figure.
    """
    import matplotlib.pyplot as plt

    g = structure_of(graph)
    sets = [[int(v) for v in vs] for vs in vertex_sets]
    columns = len(sets) + 1
    if above is not None and len(above) != columns:
        raise ValueError(f"above needs one graph per column ({columns}), got {len(above)}")
    rows = 1 if above is None else 2
    fig, axs = plt.subplots(rows, columns, figsize=(3.75 * columns, 4.2 if rows == 1 else 7.4), layout="constrained",
                            height_ratios=None if rows == 1 else [0.75, 1.35], squeeze=False)
    bottom = axs[-1]
    plot_graph(g, (), bottom[0], layout="circle", title=graph_title)
    for col, vs in enumerate(sets, start=1):
        inside = [(u, w) for i, u in enumerate(vs) for w in vs[i + 1:] if g.has_edge(u, w)]
        first = [v for part in _in_order(inside) for v in part]
        first += [v for v in vs if v not in first]
        rest = [v for v in range(g.n) if v not in set(vs)]
        plot_graph(g.switch(vs), vs, bottom[col], layout=_ring(first + rest),
                   title=titles[col - 1] if titles else None)
    if above is not None:
        for col, (small, ax) in enumerate(zip(above, axs[0])):
            small = structure_of(small)
            parts = _in_order([tuple(e) for e in small.edges().tolist()])
            covered = {v for part in parts for v in part}
            parts += [[v] for v in range(small.n) if v not in covered]
            plot_graph(small, range(small.n), ax, layout=_side_by_side(parts, small.n), node_size=60,
                       title=above_titles[col] if above_titles else None)
            ax.set_aspect("equal", adjustable="datalim")
            ax.set_box_aspect(0.75)
            ax.margins(0.12)
    return fig


def _in_order(edges) -> list[list[int]]:
    """The connected components of the graph with these edges, each as its vertices in depth-first
    order from a vertex of least degree (path or cycle order, for paths and cycles), shortest first."""
    adjacency: dict[int, set[int]] = {}
    for u, v in edges:
        adjacency.setdefault(int(u), set()).add(int(v))
        adjacency.setdefault(int(v), set()).add(int(u))
    parts, seen = [], set()
    for start in sorted(adjacency, key=lambda v: (len(adjacency[v]), v)):
        if start in seen:
            continue
        order, stack = [], [start]
        while stack:
            v = stack.pop()
            if v in seen:
                continue
            seen.add(v)
            order.append(v)
            stack.extend(sorted(adjacency[v] - seen, reverse=True))
        parts.append(order)
    return sorted(parts, key=len)


def _side_by_side(parts: list[list[int]], n: int) -> np.ndarray:
    """Each part on its own small circle (a pair as a vertical segment), the circles in a row."""
    pos, x = np.zeros((n, 2)), 0.0
    for part in parts:
        if len(part) > 8:  # a large part on one circle, like the whole graph
            r = 1.0
        else:
            r = 0.18 * len(part) ** 0.5 if len(part) > 2 else 0.12
        for i, v in enumerate(part):
            angle = np.pi / 2 + 2 * np.pi * i / len(part)
            pos[v] = (x + r + r * np.cos(angle), r * np.sin(angle))
        x += 2 * r + 0.25
    return pos


def _ring(order: list[int]) -> np.ndarray:
    """Positions around a circle, in the given order of vertices."""
    angles = np.pi / 2 - 2 * np.pi * np.arange(len(order)) / len(order)
    pos = np.zeros((len(order), 2))
    pos[order] = np.column_stack([np.cos(angles), np.sin(angles)])
    return pos
