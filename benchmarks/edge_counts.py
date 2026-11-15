"""BENCHMARKS.md, "Exact edge counts": k vertices spanning exactly m edges in random graphs.

The graphs are G(n, 0.6) with seed n, for n = 20, 23, ..., 59; k is 30% of n, and m the most edges
subgraphsearch finds among k vertices, the hardest kind of target. subgraphsearch runs 60
experiments of up to 2,000 moves (median of 9 timings). With --solvers, the general solvers try
each target with a time limit (--limit seconds, 10 by default).

    uv run python benchmarks/edge_counts.py
    uv run --with ortools --with pulp==2.9.0 --with highspy --with pyscipopt python benchmarks/edge_counts.py --solvers all
"""

import argparse

from common import timed
from solvers import SOLVERS, cpsat_exact, mip

import subgraphsearch as ss


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--solvers", default="", help=f"comma-separated, from {', '.join(SOLVERS)}, or 'all'")
    parser.add_argument("--limit", type=float, default=10.0, help="seconds per solver run")
    parser.add_argument("--sizes", default="20:60:3", help="graph sizes as start:stop:step")
    args = parser.parse_args()
    solvers = SOLVERS if args.solvers == "all" else [s for s in args.solvers.split(",") if s]

    start, stop, step = map(int, args.sizes.split(":"))
    for n in range(start, stop, step):
        g = ss.generate("exponential", n, 0.6, seed=n)
        k = round(0.3 * n)
        m = ss.densest(g, k, experiments=60, max_moves=2000, seed=1).value
        search = lambda g=g, k=k, m=m: ss.search(g, k, m, experiments=60, max_moves=2000)
        search()
        r, t = timed(search, repeats=9)
        row = [f"{n} vertices, target {k} vertices and {m} edges: subgraphsearch {r.successes}/60 in {t * 1000:.2f} ms"]
        for name in solvers:
            if name == "cpsat":
                status, seconds = cpsat_exact(n, g.edge_list(), k, m, args.limit)
                row.append(f"CP-SAT {status} in {seconds:.2f} s")
            else:
                found, seconds = mip(name, n, [(u, v, 1) for u, v in g.edge_list()], k, args.limit, exact_edges=m)
                outcome = found if isinstance(found, str) else "found" if found else "nothing"
                row.append(f"{name} {outcome} in {seconds:.2f} s")
        print("; ".join(row), flush=True)


if __name__ == "__main__":
    main()
