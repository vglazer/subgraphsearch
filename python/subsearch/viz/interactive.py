"""A self-contained interactive HTML view of a graph and the subgraphs a search found.

The page has no external dependencies: open it in a browser, send it to someone, or display it
in a notebook with :func:`show`. It draws on a canvas, so it stays responsive with thousands of
vertices and tens of thousands of edges. Drag to pan, scroll to zoom, hover for details, click a
vertex to pin it, and pick a solution from the menu to highlight it.
"""

from __future__ import annotations

import html
import json
from collections.abc import Iterable
from os import PathLike

import numpy as np

from .._graph import Graph, WeightedGraph, structure_of
from .._search import SearchResult

_TEMPLATE = r"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>__TITLE__</title>
<style>
:root {
  --bg: #ffffff; --fg: #1f2328; --muted: #656d76; --panel: #f6f8fa; --border: #d0d7de;
  --node: #57606a; --edge: rgba(87, 96, 106, 0.22); --accent: #d6336c; --accent-edge: rgba(214, 51, 108, 0.85);
  --hover: #0969da;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --bg: #0d1117; --fg: #e6edf3; --muted: #8d96a0; --panel: #161b22; --border: #30363d;
    --node: #8d96a0; --edge: rgba(141, 150, 160, 0.22); --accent: #f06595; --accent-edge: rgba(240, 101, 149, 0.9);
    --hover: #58a6ff;
  }
}
:root[data-theme="dark"] {
  --bg: #0d1117; --fg: #e6edf3; --muted: #8d96a0; --panel: #161b22; --border: #30363d;
  --node: #8d96a0; --edge: rgba(141, 150, 160, 0.22); --accent: #f06595; --accent-edge: rgba(240, 101, 149, 0.9);
  --hover: #58a6ff;
}
* { box-sizing: border-box; }
html, body { margin: 0; height: 100%; background: var(--bg); color: var(--fg);
  font: 14px/1.45 system-ui, -apple-system, "Segoe UI", sans-serif; }
body { display: flex; flex-direction: column; }
header { padding: 10px 16px; border-bottom: 1px solid var(--border); display: flex; flex-wrap: wrap;
  gap: 8px 20px; align-items: center; }
h1 { font-size: 15px; margin: 0; font-weight: 600; }
.stats { color: var(--muted); font-variant-numeric: tabular-nums; }
.controls { display: flex; flex-wrap: wrap; gap: 8px 16px; align-items: center; margin-left: auto; }
select, button, input[type=number] { font: inherit; color: var(--fg); background: var(--panel);
  border: 1px solid var(--border); border-radius: 6px; padding: 3px 8px; }
input[type=number] { width: 7em; }
label { display: inline-flex; gap: 6px; align-items: center; cursor: pointer; }
main { position: relative; flex: 1; min-height: 360px; }
canvas { position: absolute; inset: 0; width: 100%; height: 100%; touch-action: none; cursor: grab; }
canvas.dragging { cursor: grabbing; }
#tip { position: absolute; pointer-events: none; background: var(--panel); border: 1px solid var(--border);
  border-radius: 6px; padding: 6px 9px; font-size: 12px; display: none; max-width: 260px;
  box-shadow: 0 4px 14px rgba(0,0,0,.12); font-variant-numeric: tabular-nums; }
#pinned { position: absolute; left: 12px; bottom: 12px; background: var(--panel); border: 1px solid var(--border);
  border-radius: 8px; padding: 8px 11px; font-size: 12px; max-width: min(420px, calc(100% - 24px));
  max-height: 40%; overflow: auto; display: none; }
.legend { position: absolute; right: 12px; bottom: 12px; font-size: 12px; color: var(--muted);
  background: var(--panel); border: 1px solid var(--border); border-radius: 8px; padding: 6px 10px; }
.dot { display: inline-block; width: 9px; height: 9px; border-radius: 50%; margin-right: 5px; vertical-align: -1px; }
@media (max-width: 640px) { .controls { margin-left: 0; } header { padding: 8px 12px; } }
</style>
</head>
<body>
<header>
  <h1>__TITLE__</h1>
  <span class="stats" id="stats"></span>
  <div class="controls">
    <label id="solwrap">highlight <select id="solution"></select></label>
    <label><input type="checkbox" id="dim" checked> fade other edges</label>
    <label>find vertex <input type="number" id="find" min="0" placeholder="#"></label>
    <button id="reset" type="button">reset view</button>
  </div>
</header>
<main>
  <canvas id="c" aria-label="graph drawing"></canvas>
  <div id="tip"></div>
  <div id="pinned"></div>
  <div class="legend" id="legend"></div>
</main>
<script>
const DATA = __DATA__;
(() => {
  const n = DATA.n, pos = DATA.pos, E = DATA.edges, deg = DATA.degrees, sols = DATA.solutions;
  const W = DATA.weights;  // per-edge weights, or null for unweighted graphs
  const wOf = i => (W ? W[i / 2] : 1);
  const wdeg = new Array(n).fill(0);
  for (let i = 0; i < E.length; i += 2) { wdeg[E[i]] += wOf(i); wdeg[E[i + 1]] += wOf(i); }
  const canvas = document.getElementById("c"), ctx = canvas.getContext("2d");
  const tip = document.getElementById("tip"), pinned = document.getElementById("pinned");
  const sel = document.getElementById("solution"), dim = document.getElementById("dim");
  document.getElementById("stats").textContent =
    `${n.toLocaleString()} vertices · ${(E.length / 2).toLocaleString()} edges · density ${DATA.density.toFixed(4)}` +
    (W ? ` · total weight ${W.reduce((a, b) => a + b, 0).toLocaleString()}` : "");

  const adj = Array.from({length: n}, () => []), adjw = Array.from({length: n}, () => []);
  for (let i = 0; i < E.length; i += 2) {
    adj[E[i]].push(E[i + 1]); adj[E[i + 1]].push(E[i]);
    adjw[E[i]].push(wOf(i)); adjw[E[i + 1]].push(wOf(i));
  }
  // edges grouped by weight, so each group is one canvas path with its own line width
  const groups = new Map();
  for (let i = 0; i < E.length; i += 2) { const w = wOf(i); if (!groups.has(w)) groups.set(w, []); groups.get(w).push(i); }
  const widthOf = w => (W ? 0.7 + 0.9 * Math.log2(w) : 1);

  if (sols.length) {
    sel.add(new Option("none", "-1"));
    sols.forEach((s, i) => sel.add(new Option(s.label, String(i))));
    sel.value = "0";
  } else {
    document.getElementById("solwrap").style.display = "none";
  }
  let member = new Uint8Array(n);
  function setSolution() {
    member = new Uint8Array(n);
    const i = Number(sel.value);
    if (i >= 0 && sols[i]) for (const v of sols[i].vertices) member[v] = 1;
    legend(); draw();
  }

  const css = () => getComputedStyle(document.documentElement);
  let view = {x: 0, y: 0, k: 1}, size = {w: 1, h: 1}, hover = -1, pin = -1;
  function fit() {
    const r = canvas.getBoundingClientRect(), dpr = window.devicePixelRatio || 1;
    size = {w: r.width, h: r.height};
    canvas.width = Math.max(1, Math.round(r.width * dpr)); canvas.height = Math.max(1, Math.round(r.height * dpr));
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }
  function home() {
    const s = Math.min(size.w, size.h) * 0.9;
    view = {k: s, x: (size.w - s) / 2, y: (size.h - s) / 2};
  }
  const sx = i => view.x + pos[2 * i] * view.k, sy = i => view.y + pos[2 * i + 1] * view.k;

  function radius() { return Math.max(1.5, Math.min(7, 0.9 * Math.sqrt(view.k * view.k / Math.max(n, 1)) * 0.35)); }

  function draw() {
    const c = css(), r = radius();
    ctx.clearRect(0, 0, size.w, size.h);
    const any = member.some(x => x);
    // edges outside the subgraph
    ctx.strokeStyle = c.getPropertyValue("--edge");
    ctx.globalAlpha = any && dim.checked ? 0.35 : 1;
    for (const [w, idx] of groups) {
      ctx.lineWidth = widthOf(w); ctx.beginPath();
      for (const i of idx) {
        const u = E[i], v = E[i + 1];
        if (member[u] && member[v]) continue;
        ctx.moveTo(sx(u), sy(u)); ctx.lineTo(sx(v), sy(v));
      }
      ctx.stroke();
    }
    ctx.globalAlpha = 1;
    // subgraph edges
    if (any) {
      ctx.strokeStyle = c.getPropertyValue("--accent-edge");
      for (const [w, idx] of groups) {
        ctx.lineWidth = 1 + widthOf(w); ctx.beginPath();
        for (const i of idx) {
          const u = E[i], v = E[i + 1];
          if (member[u] && member[v]) { ctx.moveTo(sx(u), sy(u)); ctx.lineTo(sx(v), sy(v)); }
        }
        ctx.stroke();
      }
    }
    // edges of the hovered or pinned vertex
    for (const f of [pin, hover]) {
      if (f < 0) continue;
      ctx.lineWidth = 1.6; ctx.strokeStyle = c.getPropertyValue("--hover"); ctx.beginPath();
      for (const v of adj[f]) { ctx.moveTo(sx(f), sy(f)); ctx.lineTo(sx(v), sy(v)); }
      ctx.stroke();
    }
    // vertices
    const node = c.getPropertyValue("--node"), accent = c.getPropertyValue("--accent");
    ctx.fillStyle = node; ctx.beginPath();
    for (let i = 0; i < n; i++) if (!member[i]) { ctx.moveTo(sx(i) + r, sy(i)); ctx.arc(sx(i), sy(i), r, 0, 2 * Math.PI); }
    ctx.fill();
    ctx.fillStyle = accent; ctx.beginPath();
    const R = r * 1.6;
    for (let i = 0; i < n; i++) if (member[i]) { ctx.moveTo(sx(i) + R, sy(i)); ctx.arc(sx(i), sy(i), R, 0, 2 * Math.PI); }
    ctx.fill();
    for (const f of [pin, hover]) {
      if (f < 0) continue;
      ctx.strokeStyle = c.getPropertyValue("--hover"); ctx.lineWidth = 2;
      ctx.beginPath(); ctx.arc(sx(f), sy(f), (member[f] ? R : r) + 3, 0, 2 * Math.PI); ctx.stroke();
    }
    if (n <= 60) {  // label small graphs
      ctx.fillStyle = c.getPropertyValue("--muted"); ctx.font = "11px system-ui, sans-serif";
      for (let i = 0; i < n; i++) ctx.fillText(String(i), sx(i) + r + 3, sy(i) - r - 1);
    }
  }

  function nearest(mx, my) {
    let best = -1, bd = Infinity;
    const lim = Math.max(8, radius() * 2.5) ** 2;
    for (let i = 0; i < n; i++) {
      const dx = sx(i) - mx, dy = sy(i) - my, d = dx * dx + dy * dy;
      if (d < bd) { bd = d; best = i; }
    }
    return bd <= lim ? best : -1;
  }
  function describe(v) {
    const inSub = adj[v].filter(u => member[u]).length;
    const wSub = adj[v].reduce((t, u, k) => t + (member[u] ? adjw[v][k] : 0), 0);
    const anySub = member.some(x => x);
    return `<b>vertex ${v}</b>${member[v] ? " · in subgraph" : ""}<br>degree ${deg[v]}` +
      (W ? `, total weight ${wdeg[v]}` : "") +
      (anySub ? `<br>${inSub} neighbour${inSub === 1 ? "" : "s"} in the subgraph` + (W ? ` (weight ${wSub})` : "") : "");
  }
  function showPin() {
    if (pin < 0) { pinned.style.display = "none"; return; }
    const nb = adj[pin].slice().sort((a, b) => a - b);
    pinned.innerHTML = describe(pin) + `<br><span style="color:var(--muted)">neighbours:</span> ${nb.join(" ") || "none"}`;
    pinned.style.display = "block";
  }
  function legend() {
    const any = member.some(x => x), k = member.reduce((a, b) => a + b, 0);
    let sub = 0, subw = 0;
    if (any) for (let i = 0; i < E.length; i += 2) if (member[E[i]] && member[E[i + 1]]) { sub++; subw += wOf(i); }
    document.getElementById("legend").innerHTML =
      (any ? `<div><span class="dot" style="background:var(--accent)"></span>subgraph: ${k} vertices, ${sub} edges${W ? `, total weight ${subw}` : ""}</div>` : "") +
      `<div><span class="dot" style="background:var(--node)"></span>other vertices</div>`;
  }

  let drag = null;
  canvas.addEventListener("pointerdown", e => {
    drag = {x: e.clientX, y: e.clientY, vx: view.x, vy: view.y, moved: false};
    canvas.setPointerCapture(e.pointerId); canvas.classList.add("dragging");
  });
  canvas.addEventListener("pointermove", e => {
    const r = canvas.getBoundingClientRect(), mx = e.clientX - r.left, my = e.clientY - r.top;
    if (drag) {
      const dx = e.clientX - drag.x, dy = e.clientY - drag.y;
      if (Math.abs(dx) + Math.abs(dy) > 3) drag.moved = true;
      view.x = drag.vx + dx; view.y = drag.vy + dy; tip.style.display = "none"; draw(); return;
    }
    const h = nearest(mx, my);
    if (h !== hover) { hover = h; draw(); }
    if (h >= 0) {
      tip.innerHTML = describe(h); tip.style.display = "block";
      tip.style.left = Math.min(mx + 14, r.width - tip.offsetWidth - 4) + "px";
      tip.style.top = Math.min(my + 14, r.height - tip.offsetHeight - 4) + "px";
    } else tip.style.display = "none";
  });
  canvas.addEventListener("pointerup", e => {
    canvas.classList.remove("dragging");
    if (drag && !drag.moved) {
      const r = canvas.getBoundingClientRect();
      pin = nearest(e.clientX - r.left, e.clientY - r.top); showPin(); draw();
    }
    drag = null;
  });
  canvas.addEventListener("pointerleave", () => { hover = -1; tip.style.display = "none"; draw(); });
  canvas.addEventListener("wheel", e => {
    e.preventDefault();
    const r = canvas.getBoundingClientRect(), mx = e.clientX - r.left, my = e.clientY - r.top;
    const f = Math.exp(-e.deltaY * 0.0015);
    view.x = mx - (mx - view.x) * f; view.y = my - (my - view.y) * f; view.k *= f; draw();
  }, {passive: false});
  document.getElementById("reset").addEventListener("click", () => { home(); draw(); });
  document.getElementById("find").addEventListener("change", e => {
    const v = Number(e.target.value);
    if (Number.isInteger(v) && v >= 0 && v < n) {
      pin = v; showPin();
      view.x = size.w / 2 - pos[2 * v] * view.k; view.y = size.h / 2 - pos[2 * v + 1] * view.k; draw();
    }
  });
  sel.addEventListener("change", setSolution);
  dim.addEventListener("change", draw);
  window.addEventListener("resize", () => { fit(); draw(); });
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", draw);
  fit(); home(); setSolution();
})();
</script>
</body>
</html>
"""


def _solutions(result: SearchResult | None, highlight: Iterable[int] | None, max_conflict: int) -> list[dict]:
    sols = []
    if highlight is not None:
        vs = sorted({int(v) for v in highlight})
        if vs:
            sols.append({"label": f"given set ({len(vs)} vertices)", "vertices": vs})
    if result is not None:
        seen = set()
        for e in sorted(result.experiments, key=lambda e: (e.best_conflict, e.index)):
            if e.best_conflict > max_conflict or e.vertices in seen:
                continue
            seen.add(e.vertices)
            tag = "exact" if e.success else f"off by {e.best_conflict}"
            sols.append({"label": f"experiment {e.index}: {tag}", "vertices": list(e.vertices)})
    return sols


def to_html(
    graph: Graph | WeightedGraph | SearchResult,
    highlight: Iterable[int] | None = None,
    *,
    result: SearchResult | None = None,
    layout: str | np.ndarray = "spring",
    max_conflict: int = 0,
    title: str | None = None,
    seed: int = 1,
) -> str:
    """The interactive page as an HTML string.

    Pass a :class:`~subsearch.SearchResult` (directly or as ``result=``) to choose among its
    solutions within ``max_conflict`` edges of the target, and/or ``highlight`` to mark a vertex
    set. ``layout`` is a method name for :meth:`Graph.layout` or an ``(n, 2)`` array.
    """
    if isinstance(graph, SearchResult):
        result, graph = graph, graph.graph
    weights = graph.edges()[:, 2].tolist() if isinstance(graph, WeightedGraph) else None
    graph = structure_of(graph)
    pos = graph.layout(layout, seed=seed) if isinstance(layout, str) else np.asarray(layout, dtype=float)
    if pos.shape != (graph.n, 2):
        raise ValueError(f"layout must have shape ({graph.n}, 2), got {pos.shape}")
    if result is not None and title is None:
        p = result.params
        title = f"{p.size}-vertex subgraphs {result.target}: {result.successes}/{len(result.experiments)} experiments succeeded"
    data = {
        "n": graph.n,
        "density": graph.density,
        "pos": np.round(pos.ravel(), 4).tolist(),
        "edges": graph.edges().ravel().tolist(),
        "degrees": graph.degrees().tolist(),
        "solutions": _solutions(result, highlight, max_conflict),
        "weights": weights,
    }
    title = title or f"Graph on {graph.n} vertices"
    # "</" cannot appear inside the script element
    payload = json.dumps(data, separators=(",", ":")).replace("</", "<\\/")
    return _TEMPLATE.replace("__TITLE__", html.escape(title)).replace("__DATA__", payload)


def save_html(path: str | PathLike, graph: Graph | SearchResult, highlight: Iterable[int] | None = None, **kwargs) -> str:
    """Writes :func:`to_html`'s page to ``path`` and returns the path."""
    page = to_html(graph, highlight, **kwargs)
    with open(path, "w", encoding="utf-8") as f:
        f.write(page)
    return str(path)


def show(graph: Graph | SearchResult, highlight: Iterable[int] | None = None, *, height: int = 560, **kwargs):
    """Displays the interactive page inline in a Jupyter notebook."""
    import warnings

    from IPython.display import HTML

    page = to_html(graph, highlight, **kwargs)
    frame = (
        f'<iframe srcdoc="{html.escape(page, quote=True)}" style="width:100%;height:{height}px;border:1px solid #d0d7de;border-radius:8px" '
        'sandbox="allow-scripts"></iframe>'
    )
    with warnings.catch_warnings():
        # IPython suggests IFrame for iframes, but IFrame cannot carry inline (srcdoc) content
        warnings.filterwarnings("ignore", message="Consider using IPython.display.IFrame")
        return HTML(frame)
