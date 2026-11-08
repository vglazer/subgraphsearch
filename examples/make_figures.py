"""Draws the figures shown in the README (run: uv run python examples/make_figures.py)."""

from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

import subgraphsearch as ss  # noqa: E402
from subgraphsearch import viz  # noqa: E402

OUT = Path(__file__).resolve().parents[1] / "assets"


def paley(q: int) -> ss.Graph:
    squares = {x * x % q for x in range(1, q)}
    return ss.Graph(q, [(i, j) for i in range(q) for j in range(i + 1, q) if (j - i) % q in squares])


def found(result: ss.SearchResult) -> tuple[int, ...]:
    """The best subgraph found, which must be an exact solution: captions state what is shown."""
    if result.best is None or not result.best.success:
        raise SystemExit(f"no solution for a figure: {result.summary()}")
    return result.best.vertices


def main() -> None:
    OUT.mkdir(exist_ok=True)
    fig, axs = plt.subplots(2, 2, figsize=(12, 10), layout="constrained")

    # a regular subgraph of a strongly regular graph
    p = paley(37)
    r = ss.search(p, 12, degree=5, two_phase=True, seed=3, experiments=200)
    viz.plot_graph(p, found(r), axs[0, 0], layout="circle",
                   title="a 5-regular subgraph on 12 of the 37 vertices of a Paley graph")

    # a dense cluster in a random geometric graph
    geo = ss.generate("geometric", 200, 0.05, seed=6)
    rg = ss.search(geo, 14, 80, seed=2, experiments=200)
    viz.plot_graph(geo, found(rg), axs[0, 1], seed=2,
                   title="a 14-vertex cluster with 80 edges in a random geometric graph")

    # the found subgraph as a block of the adjacency matrix
    g = ss.generate("exponential", 100, 0.6, seed=1)
    rd = ss.search(g, 30, 345, seed=1, trace=True, experiments=60)
    viz.plot_adjacency(g, found(rd), axs[1, 0])
    axs[1, 0].set_title("30 vertices with 345 edges, gathered in the adjacency matrix", fontsize=10, loc="left")

    # how the search got there
    viz.plot_traces(rd, axs[1, 1], max_experiments=10)
    axs[1, 1].set_title("conflict per move, 10 experiments; shading marks random moves", fontsize=10, loc="left")

    fig.savefig(OUT / "overview.png", dpi=110, facecolor="white")
    print(OUT / "overview.png")


if __name__ == "__main__":
    main()
