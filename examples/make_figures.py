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
    comparison(out)


# Recorded for the README's comparison (CP-SAT 9.15 on 16 cores, stopped after 10 s; subgraphsearch 3.0 with
# 60 experiments of up to 2,000 moves). Random graphs of density 0.6 with seed n; the target is the most edges
# subgraphsearch finds among 30% of the vertices: vertices, subgraphsearch ms, CP-SAT ms (None: nothing in 10 s).
EDGE_SCALING = [(20, 0.13, 25), (23, 0.14, 18), (26, 0.13, 23), (29, 0.13, 414), (32, 0.11, 650), (35, 0.17, 1125),
                (38, 0.15, 4490), (41, 0.12, 85), (44, 0.17, None), (47, 0.35, None), (50, 0.83, 8814),
                (53, 1.28, None), (56, 0.63, None), (59, 0.31, None)]
# MDPLIB SOM-b instances: items, chosen, best known value, CP-SAT's best after 10 s. subgraphsearch reaches every
# best known value (with kick_size=16, 60 experiments of up to 20,000 moves).
DIVERSITY = [(100, 10, 333, 329), (100, 20, 1195, 1171), (100, 30, 2457, 2457), (100, 40, 4142, 4127),
             (200, 40, 4450, 4263), (300, 60, 9689, 9297), (400, 40, 4658, 4374), (400, 80, 16956, 15595)]


def comparison(out: Path) -> None:
    """The README's comparison with CP-SAT: exact edge counts as graphs grow, and the maximum diversity benchmark."""
    fig, (cp, mine, mdp) = plt.subplots(1, 3, figsize=(15, 4.2), layout="constrained",
                                        gridspec_kw={"width_ratios": [1, 1, 1.15]})
    done = [(n, t) for n, _, t in EDGE_SCALING if t is not None]
    out_of_time = [n for n, _, t in EDGE_SCALING if t is None]
    cp.plot([n for n, _ in done], [t / 1000 for _, t in done], "o", color="#4c6ef5", label="found")
    cp.plot(out_of_time, [10] * len(out_of_time), "^", color="#4c6ef5", markerfacecolor="white",
            label="nothing in 10 s")
    cp.axhline(10, color="#868e96", lw=0.8, ls="--")
    cp.set_ylim(0, 11)
    cp.set_ylabel("seconds")
    cp.set_title("CP-SAT, the strongest general solver tried", fontsize=11, loc="left")
    cp.legend(frameon=False, fontsize=9, loc="upper left")
    mine.plot([n for n, _, _ in EDGE_SCALING], [ms for _, ms, _ in EDGE_SCALING], "o", color="#d6336c")
    mine.set_ylim(0, 11)
    mine.set_ylabel("milliseconds")
    mine.set_title("subgraphsearch: the same targets", fontsize=11, loc="left")
    for ax in (cp, mine):
        ax.set_xlabel("vertices in the graph")
    labels = [f"{n} items, {k} chosen" for n, k, _, _ in DIVERSITY]
    shortfall = [100 * (best - cp_best) / best for _, _, best, cp_best in DIVERSITY]
    ys = range(len(DIVERSITY))
    mdp.barh(list(ys), shortfall, color="#4c6ef5", label="CP-SAT after 10 s")
    mdp.plot([0] * len(DIVERSITY), list(ys), "o", color="#d6336c", label="subgraphsearch")
    for y, s in zip(ys, shortfall):
        mdp.annotate("matched" if s == 0 else f"{s:.1f}%", (s, y), xytext=(6, 0), textcoords="offset points",
                     va="center", fontsize=8)
    mdp.set_yticks(list(ys), labels, fontsize=9)
    mdp.invert_yaxis()
    mdp.set_xlim(-0.3, 9.5)
    mdp.set_xlabel("% below the best known value")
    mdp.set_title("maximum diversity (MDPLIB)", fontsize=11, loc="left")
    mdp.legend(frameon=False, fontsize=9, loc="upper right")
    fig.savefig(out / "comparison.png", dpi=100, facecolor="white")
    print(out / "comparison.png")
    benchmarks(out)


# Recorded for BENCHMARKS.md: the general solvers, each stopped after 10 s (None: nothing found; "not run").
SOLVER_COLORS = {"CP-SAT": "#4c6ef5", "SCIP": "#12b886", "HiGHS": "#f59f00", "CBC": "#868e96"}
# Exact edge counts on the six graphs of BENCHMARKS.md's table: vertices -> seconds per solver.
EDGE_SOLVERS = {20: {"CP-SAT": 0.025, "SCIP": 0.10, "HiGHS": 0.35, "CBC": 0.51},
                29: {"CP-SAT": 0.41, "SCIP": 0.78, "HiGHS": 3.0, "CBC": 2.6},
                38: {"CP-SAT": 4.5, "SCIP": None, "HiGHS": None, "CBC": None},
                44: {"CP-SAT": None, "SCIP": None, "HiGHS": None, "CBC": None},
                50: {"CP-SAT": 8.8, "SCIP": None, "HiGHS": None, "CBC": None},
                59: {"CP-SAT": None, "SCIP": None, "HiGHS": None, "CBC": None}}
# Maximum diversity: the best total each solver found in 10 s, by instance as in DIVERSITY.
DIVERSITY_SOLVERS = {"SCIP": [206, 847, 2101, 3665, 3830, 8350, 3635, "not run"],
                     "HiGHS": [209, 915, 1911, 3881, 3582, None, None, "not run"],
                     "CBC": [221, 853, 1940, 3534, 3426, None, None, "not run"]}


def benchmarks(out: Path) -> None:
    """BENCHMARKS.md's figures: the general solvers beside each other and beside subgraphsearch."""
    # exact edge counts: the general solvers in seconds, subgraphsearch in milliseconds
    sizes = list(EDGE_SOLVERS)
    fig, (gen, mine) = plt.subplots(1, 2, figsize=(13, 4.2), layout="constrained",
                                    gridspec_kw={"width_ratios": [1.5, 1]})
    offsets = dict(zip(SOLVER_COLORS, (-1.2, -0.4, 0.4, 1.2)))
    for name, color in SOLVER_COLORS.items():
        xs = [n + offsets[name] for n in sizes]
        times = [EDGE_SOLVERS[n][name] for n in sizes]
        gen.plot([x for x, t in zip(xs, times) if t is not None], [t for t in times if t is not None], "o",
                 color=color, label=name, markersize=7)
        gen.plot([x for x, t in zip(xs, times) if t is None], [10] * sum(t is None for t in times), "^",
                 color=color, markerfacecolor="white", markersize=7)
    gen.plot([], [], "^", color="#495057", markerfacecolor="white", label="nothing in 10 s")
    gen.axhline(10, color="#adb5bd", lw=0.8, ls="--")
    gen.set_ylim(0, 11)
    gen.set_xticks(sizes)
    gen.set_ylabel("seconds")
    gen.set_xlabel("vertices in the graph")
    gen.set_title("general solvers: CP-SAT, and the MIP solvers SCIP, HiGHS and CBC", fontsize=11, loc="left")
    gen.legend(frameon=False, fontsize=9, loc="center left", bbox_to_anchor=(0, 0.55))
    mine.plot(sizes, [ms for n, ms, _ in EDGE_SCALING if n in EDGE_SOLVERS], "o", color="#d6336c", markersize=7)
    mine.set_ylim(0, 11)
    mine.set_xticks(sizes)
    mine.set_ylabel("milliseconds")
    mine.set_xlabel("vertices in the graph")
    mine.set_title("subgraphsearch: every target found", fontsize=11, loc="left")
    fig.savefig(out / "benchmarks_edges.png", dpi=100, facecolor="white")
    print(out / "benchmarks_edges.png")

    # maximum diversity: % below the best known value, by solver
    labels = [f"{n} items, {k} chosen" for n, k, _, _ in DIVERSITY]
    found = {"CP-SAT": [cp for *_, cp in DIVERSITY], **DIVERSITY_SOLVERS}
    fig, ax = plt.subplots(figsize=(11, 6.2), layout="constrained")
    height = 0.18
    cap = 45   # the x-axis limit, in %
    for j, (name, color) in enumerate(SOLVER_COLORS.items()):
        for i, ((_, _, best, _), value) in enumerate(zip(DIVERSITY, found[name])):
            y = i + (j - 1.5) * height
            if isinstance(value, int):
                short = 100 * (best - value) / best
                ax.barh(y, short, height=height, color=color, label=name if i == 0 else None)
                text = "matched" if short == 0 else f"{short:.1f}%" if short < 1 else f"{short:.0f}%"
                ax.annotate(text, (short, y), xytext=(3, 0), textcoords="offset points", va="center", fontsize=7)
            else:
                ax.annotate(f"{name}: " + ("no solution" if value is None else "not run"), (0, y), xytext=(10, 0),
                            textcoords="offset points", va="center", fontsize=7, color=color, fontweight="bold")
    ax.plot([0] * len(DIVERSITY), range(len(DIVERSITY)), "D", color="#d6336c", markersize=7,
            label="subgraphsearch: matched every one", zorder=3)
    ax.set_yticks(range(len(DIVERSITY)), labels)
    ax.invert_yaxis()
    ax.set_xlim(-1, cap)
    ax.set_xlabel("% below the best known value after 10 s")
    ax.set_title("maximum diversity (MDPLIB): how far each method falls short", fontsize=11, loc="left")
    fig.legend(frameon=False, fontsize=9, loc="outside lower center", ncol=5)
    fig.savefig(out / "benchmarks_diversity.png", dpi=100, facecolor="white")
    print(out / "benchmarks_diversity.png")


if __name__ == "__main__":
    main()
