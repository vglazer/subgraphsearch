"""Rich HTML displays for Jupyter (``_repr_html_``).

Charts are SVG embedded as ``<img>`` data URIs. Outputs you produce by running a cell are trusted
and render normally. In an untrusted notebook (one executed elsewhere and not marked with
``jupyter trust``) JupyterLab's sanitiser drops data-URI images, and an image then degrades to its
alt text, which says how to fix it; inline ``<svg>`` would instead degrade to a jumble of its
labels. An image cannot inherit the notebook's text colour, so charts use a mid grey that reads
on both light and dark themes."""

from __future__ import annotations

import base64
import html
from typing import TYPE_CHECKING

import numpy as np

if TYPE_CHECKING:
    from ._graph import Graph, WeightedGraph
    from ._search import SearchResult

ACCENT = "#d6336c"
#: Small drawings are only worth showing (and storing in notebooks) for graphs up to this size.
DRAW_MAX_VERTICES, DRAW_MAX_EDGES = 200, 3000
MUTED = "#8c959f"

_STYLE = (
    "font-family: system-ui, sans-serif; font-size: 13px; line-height: 1.4;"
    "display: flex; gap: 24px; align-items: flex-start; flex-wrap: wrap"
)


def _table(title: str, rows: list[tuple[str, str]]) -> str:
    body = "".join(
        f"<tr><td style='padding:1px 12px 1px 0;opacity:.7'>{html.escape(k)}</td>"
        f"<td style='padding:1px 0;font-variant-numeric:tabular-nums;text-align:right'>{html.escape(v)}</td></tr>"
        for k, v in rows
    )
    return f"<div><div style='font-weight:600;margin-bottom:4px'>{html.escape(title)}</div><table style='border-collapse:collapse'>{body}</table></div>"


def _svg_img(body: str, width: int, height: int, label: str) -> str:
    """SVG content as an ``<img>`` with a base64 data URI."""
    svg = (
        f"<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='{height}' viewBox='0 0 {width} {height}' "
        f"font-family='system-ui, sans-serif'>{body}</svg>"
    )
    data = base64.b64encode(svg.encode()).decode()
    alt = f"[{label}: re-run this cell, or trust the notebook, to show the chart]"
    return f"<img src='data:image/svg+xml;base64,{data}' width='{width}' height='{height}' alt='{html.escape(alt)}'>"


def _bars(title: str, labels: list[str], values: list[int], width: int = 260, height: int = 110) -> str:
    """A small labelled bar chart."""
    n = max(len(values), 1)
    vmax = max(max(values, default=0), 1)
    bw = width / n
    plot_h = height - 28
    parts = []
    for i, (lab, v) in enumerate(zip(labels, values)):
        h = plot_h * v / vmax
        x = i * bw
        parts.append(
            f"<rect x='{x + 1:.1f}' y='{plot_h - h + 12:.1f}' width='{max(bw - 2, 1):.1f}' height='{h:.1f}' fill='{ACCENT}' rx='1'/>"
        )
        if v:
            parts.append(f"<text x='{x + bw / 2:.1f}' y='{plot_h - h + 9:.1f}' font-size='9' text-anchor='middle' fill='{MUTED}'>{v}</text>")
    parts.append(f"<line x1='0' x2='{width}' y1='{plot_h + 12}' y2='{plot_h + 12}' stroke='{MUTED}' stroke-opacity='.4'/>")
    first, last = (labels[0], labels[-1]) if labels else ("", "")
    parts.append(f"<text x='0' y='{height - 4}' font-size='9' fill='{MUTED}' opacity='.7'>{html.escape(first)}</text>")
    parts.append(f"<text x='{width}' y='{height - 4}' font-size='9' text-anchor='end' fill='{MUTED}' opacity='.7'>{html.escape(last)}</text>")
    img = _svg_img(''.join(parts), width, height, title)
    return f"<div><div style='font-weight:600;margin-bottom:4px'>{html.escape(title)}</div>{img}</div>"


def _drawable(graph: Graph) -> bool:
    return 0 < graph.n <= DRAW_MAX_VERTICES and graph.m <= DRAW_MAX_EDGES


def _drawing(graph: Graph, highlight: set[int] | None = None, size: int = 220) -> str:
    """A small node-link drawing, for graphs with at most a few hundred vertices."""
    pos = graph.layout("spring", iterations=100) * (size - 16) + 8
    highlight = highlight or set()
    parts = []
    for u, v in graph.edge_list():
        inside = u in highlight and v in highlight
        color, opacity, w = (ACCENT, 0.9, 1.6) if inside else (MUTED, 0.25, 0.7)
        parts.append(
            f"<line x1='{pos[u, 0]:.1f}' y1='{pos[u, 1]:.1f}' x2='{pos[v, 0]:.1f}' y2='{pos[v, 1]:.1f}' stroke='{color}' stroke-opacity='{opacity}' stroke-width='{w}'/>"
        )
    r = 3.2 if graph.n <= 60 else 2.2
    for i, (x, y) in enumerate(pos):
        fill = ACCENT if i in highlight else MUTED
        parts.append(f"<circle cx='{x:.1f}' cy='{y:.1f}' r='{r}' fill='{fill}' fill-opacity='{1 if i in highlight else 0.75}'/>")
    return _svg_img(''.join(parts), size, size, "graph drawing")


def graph_html(graph: Graph) -> str:
    d = graph.degrees()
    _, components = graph.components()
    rows = [
        ("vertices", f"{graph.n:,}"),
        ("edges", f"{graph.m:,}"),
        ("density", f"{graph.density:.4f}"),
        ("components", f"{components:,}"),
        ("degree min / mean / max", f"{d.min() if len(d) else 0} / {d.mean() if len(d) else 0:.1f} / {d.max() if len(d) else 0}"),
    ]
    blocks = [_table("Graph", rows)]
    if _drawable(graph):
        blocks.append(_drawing(graph))
    if graph.n > 0:
        counts = np.bincount(d)
        lo = int(d.min())
        blocks.append(_bars("degree distribution", [f"degree {i}" for i in range(lo, len(counts))], counts[lo:].tolist()))
    return f"<div style='{_STYLE}'>{''.join(blocks)}</div>"


def search_html(result: SearchResult) -> str:
    p = result.params
    fmt = lambda x: "–" if np.isnan(x) else f"{x:.1f}"  # noqa: E731
    from ._graph import structure_of

    rows = [
        ("target", f"{p.size} vertices, {result.target}"),
        ("experiments", f"{len(result.experiments)} × ≤{p.max_moves} moves"),
        ("successes", f"{result.successes} ({result.success_rate:.0%})"),
        ("relative cost", fmt(result.relative_cost) + " moves"),
        ("true cost", fmt(result.true_cost) + " moves"),
        ("average conflict", f"{result.average_conflict:.3f}"),
        ("rng / seed", f"{p.rng} / {p.seed}"),
    ]
    blocks = [_table("Search", rows)]
    box = p.max_moves // len(result.boxes()) + 1
    labels = [f"moves {i * box}–{min((i + 1) * box, p.max_moves) - 1}" for i in range(len(result.boxes()))]
    blocks.append(_bars("moves to success", labels, result.boxes().tolist()))
    conflicts = np.bincount([e.best_conflict for e in result.experiments]) if result.experiments else np.zeros(1, int)
    shown = conflicts[: min(len(conflicts), 25)]
    blocks.append(_bars("best conflict per experiment", [f"conflict {i}" for i in range(len(shown))], shown.tolist()))
    sols = result.solutions()
    if sols:
        items = "".join(f"<li><code>{' '.join(map(str, s))}</code></li>" for s in sols[:5])
        more = f"<div style='opacity:.7'>… and {len(sols) - 5} more</div>" if len(sols) > 5 else ""
        blocks.append(f"<div><div style='font-weight:600;margin-bottom:4px'>Solutions ({len(sols)} distinct)</div><ul style='margin:0;padding-left:18px'>{items}</ul>{more}</div>")
    if _drawable(structure_of(result.graph)) and result.best is not None:
        blocks.append(_drawing(structure_of(result.graph), set(result.best.vertices)))
    return f"<div style='{_STYLE}'>{''.join(blocks)}</div>"


def weighted_graph_html(graph: WeightedGraph) -> str:
    e = graph.edges()
    structure = graph.structure
    _, components = structure.components()
    w = e[:, 2] if len(e) else np.zeros(0, int)
    rows = [
        ("vertices", f"{graph.n:,}"),
        ("edges", f"{graph.m:,}"),
        ("total weight", f"{graph.total_weight:,}"),
        ("density", f"{structure.density:.4f}"),
        ("components", f"{components:,}"),
        ("weight min / mean / max", f"{w.min() if len(w) else 0} / {w.mean() if len(w) else 0:.2f} / {w.max() if len(w) else 0}"),
    ]
    blocks = [_table("Weighted graph", rows)]
    if _drawable(structure):
        blocks.append(_drawing(structure))
    if len(w):
        counts = np.bincount(w)
        blocks.append(_bars("edge weights", [f"weight {i}" for i in range(1, len(counts))], counts[1:].tolist()))
    return f"<div style='{_STYLE}'>{''.join(blocks)}</div>"
