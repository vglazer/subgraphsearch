"""Draws the figures shown in the README (run: uv run python examples/make_figures.py)."""

from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

import subgraphsearch as ss  # noqa: E402
from subgraphsearch import viz  # noqa: E402

OUT = Path(__file__).resolve().parents[1] / "assets"


def crossings(pos, edges) -> int:
    """How many pairs of the given edges cross in a drawing: fewer makes a subgraph easier to follow."""
    def ccw(a, b, c):
        return (c[1] - a[1]) * (b[0] - a[0]) > (b[1] - a[1]) * (c[0] - a[0])
    count = 0
    for i, (u, v) in enumerate(edges):
        for x, y in edges[i + 1:]:
            if len({u, v, x, y}) == 4:
                a, b, c, d = pos[u], pos[v], pos[x], pos[y]
                count += ccw(a, c, d) != ccw(b, c, d) and ccw(a, b, c) != ccw(a, b, d)
    return count


def found(result: ss.SearchResult) -> tuple[int, ...]:
    """The best subgraph found, which must be an exact solution: captions state what is shown."""
    if result.best is None or not result.best.success:
        raise SystemExit(f"no solution for a figure: {result.summary()}")
    return result.best.vertices


def main() -> None:
    OUT.mkdir(exist_ok=True)
    fig = plt.figure(figsize=(12, 9.6), layout="constrained")
    grid = fig.add_gridspec(2, 6, height_ratios=[1.3, 1])

    # a regular subgraph: every chosen vertex has exactly two chosen neighbors, here forming one cycle
    rnd = ss.generate("exponential", 60, 0.08, seed=3)
    r = ss.search(rnd, 12, degree=2, two_phase=True, seed=1, experiments=400)
    found(r)
    cycle = next(vs for vs in r.solutions() if rnd.induced(list(vs)).components()[1] == 1)
    cycle_edges = [(u, v) for i, u in enumerate(cycle) for v in cycle[i + 1:] if rnd.has_edge(u, v)]
    layout = min((rnd.layout("spring", seed=seed) for seed in range(1, 41)), key=lambda pos: crossings(pos, cycle_edges))
    viz.plot_graph(rnd, cycle, fig.add_subplot(grid[0, :3]), layout=layout,
                   title="an induced 12-cycle (a 2-regular subgraph) in a random graph")

    # a search confined to a fixed vertex's neighborhood, in a random geometric graph
    geo = ss.generate("geometric", 200, 0.05, seed=6)
    v = int(geo.degrees().argmax())
    nbrs = geo.common_neighborhood([v])
    rn = ss.search(geo.induced(nbrs), 9, 32 - 9, seed=1)  # v is adjacent to all 9 others: 9 + 23 = 32 edges
    sub = {v} | {nbrs[i] for i in found(rn)}
    # draw the part of the graph within two steps of v, zoomed in
    ball = sorted({v} | set(nbrs) | {w for u in nbrs for w in geo.neighbors(u).tolist()})
    index = {u: i for i, u in enumerate(ball)}
    local = geo.induced(ball)
    pos = local.layout("spring", seed=1)
    ax = fig.add_subplot(grid[0, 3:])
    viz.plot_graph(local, [index[u] for u in sub], ax, layout=pos,
                   title="a dense subgraph in the neighborhood of a fixed vertex (circled)\nof a random geometric graph: 10 vertices, 32 edges")
    rest = [index[u] for u in nbrs if u not in sub]
    ax.scatter(pos[rest, 0], pos[rest, 1], s=70, color="#f4a3bf", zorder=4, edgecolors="white", linewidths=0.8,
               label="the fixed vertex's other neighbors")
    ax.scatter(*pos[index[v]], s=420, facecolors="none", edgecolors="#343a40", linewidths=1.8, zorder=5)
    ax.legend(loc="upper left", frameon=False, fontsize=8)

    # one graph, 6 vertices at a time: an independent set, something in between, a clique
    g = ss.generate("exponential", 32, 0.45, seed=2)
    for col, (m, label) in enumerate([(0, "an independent set"), (7, "anything in between"), (15, "a clique")]):
        rm = ss.search(g, 6, m, seed=1, experiments=200)
        viz.plot_graph(g, found(rm), fig.add_subplot(grid[1, 2 * col:2 * col + 2]), layout="circle",
                       title=f"6 vertices, {m} edges: {label}")

    fig.savefig(OUT / "overview.png", dpi=110, facecolor="white")
    print(OUT / "overview.png")


if __name__ == "__main__":
    main()
