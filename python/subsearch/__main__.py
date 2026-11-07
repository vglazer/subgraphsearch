"""Command-line access to the Python-side visualisations.

    python -m subsearch explore GRAPH [--result RESULT.json] [-o PAGE.html]
    python -m subsearch report GRAPH [--result RESULT.json] [-o FIGURE.png]

GRAPH is any readable graph file; RESULT.json comes from ``subsearch search --json`` (the Rust
CLI) or ``SearchResult.save``.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from ._graph import Graph, WeightedGraph
from ._search import SearchResult


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python -m subsearch", description=__doc__.split("\n\n")[0])
    sub = parser.add_subparsers(dest="command", required=True)

    explore = sub.add_parser("explore", help="write an interactive HTML page for a graph and search result")
    report = sub.add_parser("report", help="write a figure summarising a graph and, if given, a search result")
    for p in (explore, report):
        p.add_argument("graph", type=Path, help="graph file")
        p.add_argument("--format", help="graph format, if not implied by the extension")
        p.add_argument("--result", type=Path, help="search result JSON")
        p.add_argument("-o", "--output", type=Path, help="output file")
        p.add_argument("--weighted", action="store_true", help="read edge weights (edge list, mtx or wlists)")
    explore.add_argument("--max-conflict", type=int, default=0, help="also offer near-misses within this many edges")
    explore.add_argument("--layout", default="spring", choices=["spring", "spectral", "circle"])

    args = parser.parse_args(argv)
    graph = WeightedGraph.read(args.graph, args.format) if args.weighted else Graph.read(args.graph, args.format)
    result = SearchResult.load(args.result, graph) if args.result else None

    if args.command == "explore":
        from .viz import save_html

        out = args.output or args.graph.with_suffix(".html")
        save_html(out, result if result is not None else graph, max_conflict=args.max_conflict, layout=args.layout)
    else:
        import matplotlib

        matplotlib.use("Agg")
        from .viz import plot_graph_summary, plot_search_summary

        out = args.output or args.graph.with_suffix(".png")
        if result is None:
            plot_graph_summary(graph).savefig(out, dpi=120)
        else:
            stem = out.with_suffix("")
            plot_graph_summary(graph, result.best.vertices if result.best else ()).savefig(f"{stem}-graph{out.suffix}", dpi=120)
            plot_search_summary(result).savefig(f"{stem}-search{out.suffix}", dpi=120)
            out = Path(f"{stem}-graph{out.suffix}"), Path(f"{stem}-search{out.suffix}")
    print(*(out if isinstance(out, tuple) else (out,)), sep="\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
