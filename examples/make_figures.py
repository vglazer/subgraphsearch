"""Draws the figures shown in the README (run: uv run python examples/make_figures.py)."""

from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import networkx as nx
import numpy as np

import subgraphsearch as ss
from subgraphsearch import viz

OUT = Path(__file__).resolve().parents[1] / "assets"


def found(result: ss.SearchResult) -> tuple[int, ...]:
    """The best subgraph found, which must be an exact solution: captions state what is shown."""
    if result.best is None or not result.best.success:
        raise SystemExit(f"no solution for a figure: {result.summary()}")
    return result.best.vertices


def within(g: ss.Graph, vs) -> dict[int, list[int]]:
    """Each vertex's neighbors among ``vs``."""
    s = set(vs)
    return {v: [w for w in g.neighbors(v).tolist() if w in s] for v in vs}


def in_order(g: ss.Graph, vs) -> list[int]:
    """The vertices of an induced path or cycle, in order along it."""
    nbrs = within(g, vs)
    order = [next((v for v in vs if len(nbrs[v]) == 1), vs[0])]
    while len(order) < len(vs):
        order.append(next(w for w in nbrs[order[-1]] if w not in order))
    return order


def ring(order, radius: float) -> dict[int, tuple[float, float]]:
    """Positions around a circle, in the given order."""
    t = 2 * np.pi * np.arange(len(order)) / len(order) + np.pi / 2
    return {v: (0.5 + radius * np.cos(a), 0.5 + radius * np.sin(a)) for v, a in zip(order, t)}


def wave(order) -> dict[int, tuple[float, float]]:
    """Positions along one period of a sine wave, in the given order."""
    x = np.linspace(0.1, 0.9, len(order))
    return {v: (xi, 0.5 + 0.18 * np.sin(2 * np.pi * (xi - 0.1) / 0.8)) for v, xi in zip(order, x)}


def layered(g: ss.Graph, vs) -> dict[int, tuple[float, float]]:
    """A tree drawn top down from a central vertex: leaves evenly spaced, parents above their children."""
    tree = nx.Graph([(u, w) for u, ws in within(g, vs).items() for w in ws])
    root = nx.center(tree)[0]
    depth = nx.single_source_shortest_path_length(tree, root)
    children = {v: [w for w in tree[v] if depth[w] == depth[v] + 1] for v in tree}
    x: dict[int, float] = {}

    def place(v):
        for w in children[v]:
            place(w)
        x[v] = sum(x[w] for w in children[v]) / len(children[v]) if children[v] else len(x)

    place(root)
    leaves, height = max(x.values()) or 1, max(depth.values()) or 1
    return {v: (0.08 + 0.84 * x[v] / leaves, 0.88 - 0.76 * depth[v] / height) for v in tree}


def around(g: ss.Graph, shape: dict[int, tuple[float, float]]) -> np.ndarray:
    """A layout with the subgraph pinned in its shape and the other vertices placed around it."""
    graph = nx.Graph([(u, v) for u in range(g.n) for v in g.neighbors(u).tolist() if u < v])
    rng = np.random.default_rng(1)
    start = {v: shape.get(v, tuple(rng.random(2))) for v in range(g.n)}
    pos = nx.spring_layout(graph, pos=start, fixed=list(shape), seed=1, k=0.22, iterations=200)
    return np.array([pos[v] for v in range(g.n)])


def main(out: Path = OUT) -> None:
    out.mkdir(exist_ok=True)
    # one random graph, searched for each of the four kinds of subgraph
    g = ss.generate("exponential", 36, 0.25, seed=4)
    settings = {"experiments": 100, "max_moves": 2000, "seed": 1}
    cycle = found(ss.search(g, 12, degree=2, connected=True, **settings))
    path = found(ss.search(g, 15, path=True, **settings))
    # the tree with the most leaves, so that it looks least like a path
    trees = ss.search(g, 16, 15, connected=True, **settings).solutions()
    if not trees:
        raise SystemExit("no solution for a figure: no 16-vertex tree")
    tree = max(trees, key=lambda vs: int((g.induced(list(vs)).degrees() == 1).sum()))
    clique = found(ss.search(g, 5, 10, **settings))

    panels = [
        (cycle, ring(in_order(g, cycle), 0.36), "an induced 12-cycle", "search(g, 12, degree=2, connected=True)"),
        (path, wave(in_order(g, path)), "an induced path on 15 vertices", "search(g, 15, path=True)"),
        (tree, layered(g, tree), "an induced tree on 16 vertices", "search(g, 16, 15, connected=True)"),
        (clique, ring(list(clique), 0.2), "a 5-clique", "search(g, 5, 10)"),
    ]
    fig, axs = plt.subplots(2, 2, figsize=(11, 10.5), layout="constrained")
    for ax, (vs, shape, what, call) in zip(axs.flat, panels):
        viz.plot_graph(g, vs, ax, layout=around(g, shape), title=f"{what}\n{call}")
    fig.suptitle("one random graph (36 vertices, 156 edges), four searches", fontsize=12)

    fig.savefig(out / "overview.png", dpi=100, facecolor="white")
    print(out / "overview.png")


if __name__ == "__main__":
    main()
