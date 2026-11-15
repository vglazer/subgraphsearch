"""BENCHMARKS.md, "Maximum cliques": the eight DIMACS graphs in examples/data.

By default, subgraphsearch aims at each known clique number with 32 experiments of up to 20,000
moves (median of 5 timings), and where it falls short reports the largest clique it finds.
--tenures adds a sweep of tabu tenures from 1 to 20; --igraph adds igraph's exact clique number,
stopped after --limit seconds (90 by default). DLS-MC's times in BENCHMARKS.md are from its paper.

    uv run python benchmarks/cliques.py
    uv run python benchmarks/cliques.py --tenures --igraph
"""

import argparse
import multiprocessing
import time

from common import dimacs, timed

import subgraphsearch as ss

SETTINGS = {"experiments": 32, "max_moves": 20000, "div_freq": 300, "div_dur": 8}


def _clique_number(edges, n, queue):
    import igraph

    queue.put(igraph.Graph(n=n, edges=edges).clique_number())


def igraph_clique_number(g: ss.Graph, limit: float):
    """igraph's exact clique number and the seconds it took, or ``None`` if it ran past ``limit``."""
    queue = multiprocessing.get_context("spawn").Queue()
    proc = multiprocessing.get_context("spawn").Process(target=_clique_number, args=(g.edge_list(), g.n, queue))
    start = time.perf_counter()
    proc.start()
    proc.join(limit)
    if proc.is_alive():
        proc.terminate()
        return None, limit
    return queue.get(), time.perf_counter() - start


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--tenures", action="store_true", help="also sweep tabu tenures from 1 to 20")
    parser.add_argument("--igraph", action="store_true", help="also igraph's exact clique number")
    parser.add_argument("--limit", type=float, default=90.0, help="seconds for igraph")
    args = parser.parse_args()
    for name, (g, k) in dimacs().items():
        r, t = timed(lambda g=g, k=k: ss.search(g, k, k * (k - 1) // 2, **SETTINGS), repeats=5)
        row = [f"{name} (largest clique {k}): {r.successes} of 32 in {t * 1000:.1f} ms"]
        if not r.successes:
            found = ss.largest(g, "clique", start=k - 8, lookahead=1, **SETTINGS).size
            row.append(f"largest found {found}")
        if args.tenures:
            hits = [ss.search(g, k, k * (k - 1) // 2, tabu_tenure=t, **SETTINGS).successes for t in (1, 2, 3, 5, 8, 12, 20)]
            row.append("by tenure 1, 2, 3, 5, 8, 12, 20: " + " ".join(map(str, hits)))
        if args.igraph:
            number, seconds = igraph_clique_number(g, args.limit)
            row.append(f"igraph {number} in {seconds:.1f} s" if number else f"igraph nothing in {args.limit:.0f} s")
        print("; ".join(row), flush=True)


if __name__ == "__main__":
    main()
