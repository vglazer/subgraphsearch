"""Builds and executes examples/quickstart.ipynb (run: uv run python examples/make_quickstart.py)."""

from pathlib import Path

import nbformat
import nbformat.sign
from nbclient import NotebookClient

md, code = nbformat.v4.new_markdown_cell, nbformat.v4.new_code_cell
cells = [
    md("# subsearch quickstart\n\n"
       "`subsearch` finds **induced subgraphs with a prescribed number of vertices and edges** using the randomised "
       "restricted local search of USRA's `sub_search`, ported to Rust. This notebook walks through generating graphs, "
       "searching them, reading the results, and the visualisations."),
    code("import subsearch as ss\nfrom subsearch import viz\n\nss.__version__"),
    md("## Graphs\n\nGenerate one of `ggen`'s random models, or read graph6, sparse6, Matrix Market, edge lists or USRA adjacency lists. "
       "In a notebook a graph shows a summary, its degree distribution and, when small, a drawing."),
    code("g = ss.generate(\"exponential\", 100, 0.6, seed=1, rng=\"legacy\")  # exactly ggen's `2 100 0 600 1 0 0 0`\ng"),
    md("## Reproducing the original program\n\n`rng=\"legacy\"` replays the C program's random numbers, so this finds the same five 11-cliques "
       "as the example in the USRA README, in the same order."),
    code("cliques = ss.search(g, size=11, edges=55, experiments=5, max_moves=100, div_freq=25, div_dur=4, seed=72, rng=\"legacy\")\n"
         "cliques"),
    md("## A harder search\n\nThe default generator (xoshiro256++) runs experiments in parallel. Here we look for 30 vertices spanning 343 edges, "
       "far denser than the graph's 58% average, and record the objective after every move (`trace=True`)."),
    code("r = ss.search(g, size=30, edges=343, experiments=60, max_moves=2000, div_freq=300, div_dur=8, seed=5, trace=True)\nprint(r.summary())"),
    code("viz.plot_search_summary(r);"),
    md("The **run-time distribution** (left) is the probability of success within a given number of moves: still rising at the budget means more "
       "moves would help. The traces (right) show the bursts of random moves every `div_freq` moves.\n\n"
       "At this density the target is barely attainable: every success found the *same* vertex set."),
    code("len(r.solutions()), [len(ss.search(g, 30, m, experiments=60, max_moves=2000, div_freq=300, div_dur=8, seed=5).solutions()) for m in (330, 340)]"),
    md("## Graph structure\n\nDegree distribution against an Erdős–Rényi graph of the same density, the Laplacian spectrum (λ₁ > 0 means connected), "
       "and the adjacency matrix with the found subgraph moved to the top-left block."),
    code("viz.plot_graph_summary(g, highlight=r.best.vertices);"),
    md("`ggen`'s \"power\" model is not heavy-tailed, which the comparison makes plain:"),
    code("viz.plot_degree_distribution(ss.generate(\"power\", 1500, 0.2, seed=52));"),
    md("## Interactive view\n\nA self-contained page: drag to pan, scroll to zoom, hover or click vertices, and pick a solution to highlight. "
       "`viz.save_html` writes the same page to a file."),
    code("viz.show(ss.search(g, 30, 330, experiments=8, seed=2), max_conflict=0)"),
    md("## Tuning\n\n`ss.sweep` reruns a search across values of one parameter."),
    code("sweep = ss.sweep(g, 30, 340, \"div_dur\", [0, 4, 16, 64], experiments=40, max_moves=1500, div_freq=300)\n"
         "viz.plot_sweep(sweep, parameter=\"div_dur\");"),
    md("## Formats and other libraries"),
    code("import tempfile, pathlib\nfrom subsearch import convert\n\n"
         "path = pathlib.Path(tempfile.mkdtemp()) / \"g.g6\"\n"
         "g.write(path)\n"
         "assert ss.read(path) == g\n"
         "G = convert.to_networkx(g)\n"
         "A = convert.to_scipy(g)\n"
         "print(G, \"|\", repr(A))\n"
         "print(\"formats:\", ss.formats())"),
    md("## The original interfaces\n\n`ss.legacy` takes exactly what `sub_search` and `ggen` read on stdin and returns what they printed."),
    code("print(ss.legacy.ggen(\"2 10 0 500 42 0 0 0\"))"),
]
nb = nbformat.v4.new_notebook(cells=cells, metadata={"kernelspec": {"name": "python3", "display_name": "Python 3", "language": "python"}})
NotebookClient(nb, timeout=600, kernel_name="python3").execute()
# Sign the notebook, as `jupyter trust` would, so that its saved outputs (inline charts, the
# interactive viewer) display on this machine instead of being stripped as untrusted HTML.
# Elsewhere, re-running the cells or `jupyter trust quickstart.ipynb` does the same.
nbformat.sign.NotebookNotary().sign(nb)
out = Path(__file__).with_name("quickstart.ipynb")
nbformat.write(nb, out)
print(out, f"{out.stat().st_size // 1024} KB")
