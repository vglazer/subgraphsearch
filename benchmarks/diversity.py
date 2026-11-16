"""BENCHMARKS.md, "Maximum diversity": MDPLIB instances, best known values, and the comparisons.

By default: the twelve SOM-b instances (60 experiments of up to 20,000 moves at each best known
value, kick_size=16), ss.densest reaching the same values without them (60 of up to 50,000 moves),
and the real-valued GKD-c 1 and MDG-a 1, scored on their original distances. Instances missing
from examples/data are fetched from MDPLIB on first use (see common.py).

--specialist adds MDG-a_21, 200 of 2000 items, with a budget per run equal to OBMA's 20 seconds on
one core, measured first, with the default settings and with aspiration="full" (which costs some
time per move, so its budget is measured separately); then MDG-a_22 to 24 with the same budgets,
against MDPLIB's values from 2-hour runs of G_SS. --solvers adds the general solvers, each maximizing for --limit seconds.

    uv run python benchmarks/diversity.py
    uv run python benchmarks/diversity.py --specialist
    uv run --with ortools --with pulp==2.9.0 --with highspy --with pyscipopt python benchmarks/diversity.py --solvers all
"""

import argparse

import numpy as np
from common import integer_instance, read_mdplib, timed
from solvers import SOLVERS, cpsat_max_weight, mip

import subgraphsearch as ss

SOM_B = [("SOM-b_1_n100_m10", 333), ("SOM-b_2_n100_m20", 1195), ("SOM-b_3_n100_m30", 2457),
         ("SOM-b_4_n100_m40", 4142), ("SOM-b_6_n200_m40", 4450), ("SOM-b_10_n300_m60", 9689),
         ("SOM-b_13_n400_m40", 4658), ("SOM-b_14_n400_m80", 16956), ("SOM-b_17_n500_m50", 7141),
         ("SOM-b_18_n500_m100", 26258), ("SOM-b_19_n500_m150", 56572), ("SOM-b_20_n500_m200", 97344)]
REAL = [("GKD-c_1_n500_m50", 19485.1875), ("MDG-a_1_n500_m50", 7833.83252)]
SETTINGS = {"div_freq": 300, "div_dur": 10, "kick_size": 16}
# HiGHS and CBC find nothing at 300 and 400 items, and on larger instances take minutes just to set up
# (CBC's time limit does not cover that), so like BENCHMARKS.md they are run only up to here
SMALL = {name for name, _ in SOM_B[:7]}


def solvers_for(name, solvers):
    """The solvers to run on an instance: all those asked for on the smaller ones, else CP-SAT and SCIP."""
    return [s for s in solvers if name in SMALL or s in ("cpsat", "scip")]


def som_b(solvers, limit):
    for name, best in SOM_B:
        w, k = integer_instance(name)
        r, t = timed(lambda w=w, k=k, best=best: ss.search(w, k, best, experiments=60, max_moves=20000, **SETTINGS))
        d = ss.densest(w, k, experiments=60, max_moves=50000, **SETTINGS)
        reached = sum(d._value(e.vertices) == best for e in d.result.experiments)
        row = [f"{name} (best known {best:,}): {r.successes} of 60 in {t * 1000:.0f} ms; densest {reached} of 60"]
        n, _, triples = read_mdplib(name)
        for solver in solvers_for(name, solvers):
            total, seconds = (cpsat_max_weight(n, triples, k, limit) if solver == "cpsat"
                              else mip(solver, n, triples, k, limit))
            row.append(f"{solver} {total} ({seconds:.0f} s)")
        print("; ".join(row), flush=True)


def real_valued(solvers, limit):
    for name, best in REAL:
        n, k, triples = read_mdplib(name, real=True)
        w, factor = ss.WeightedGraph.from_scores(triples, n)
        dist = np.zeros((n, n))
        dist[triples[:, 0].astype(int), triples[:, 1].astype(int)] = triples[:, 2]
        dist += dist.T
        d, t = timed(lambda w=w, k=k: ss.densest(w, k, experiments=60, max_moves=20000, **SETTINGS))
        totals = [dist[np.ix_(e.vertices, e.vertices)].sum() / 2 for e in d.result.experiments]
        matched = sum(total > best - 0.003 for total in totals)
        row = [(f"{name} (best known {best:,.2f}, weights x{factor:.3g}): {matched} of 60 match, the worst "
                f"{best - min(totals):.2f} short, in {t:.1f} s")]
        for solver in solvers_for(name, solvers):
            scaled = triples.copy()
            scaled[:, 2] = np.rint(scaled[:, 2] * (10 if name.startswith("GKD") else 25))
            total, seconds = (cpsat_max_weight(n, scaled, k, limit, score=triples) if solver == "cpsat"
                              else mip(solver, n, triples, k, limit))
            row.append(f"{solver} {total} ({seconds:.0f} s)")
        print("; ".join(row), flush=True)


# MDPLIB's best values for MDG-a_22 to 24, from 2-hour runs of G_SS (mdplib_2010_bestvalues.xls)
MORE_MDG_A = {"MDG-a_22_n2000_m200": 114327, "MDG-a_23_n2000_m200": 114123, "MDG-a_24_n2000_m200": 114040}


def specialist():
    w, k = integer_instance("MDG-a_21_n2000_m200")
    best = 114271
    budgets = {}
    for aspiration in (False, "full"):
        settings = {"div_freq": 300, "div_dur": 10, "aspiration": aspiration}
        _, t = timed(lambda settings=settings: ss.densest(w, k, experiments=1, max_moves=200_000, **settings), repeats=3)
        rate = 200_000 / t
        moves = budgets[aspiration] = int(round(rate * 20 / 100_000) * 100_000)
        label = "aspiration='full'" if aspiration else "default settings"
        print(f"MDG-a_21, {label}: {rate:,.0f} moves a second on one core, so OBMA's 20 s is {moves:,} moves",
              flush=True)
        for budget in (200_000, moves):
            d, t = timed(lambda budget=budget, settings=settings: ss.densest(w, k, experiments=30, max_moves=budget, **settings))
            totals = sorted(d._value(e.vertices) for e in d.result.experiments)
            print(f"  {budget:,} moves (tenure {d.result.params.tabu_tenure}): {sum(v >= best for v in totals)} of 30 "
                  f"reach {best:,}; the median ends {best - totals[15]} short, the worst {best - totals[0]} "
                  f"({t:.0f} s for all 30)", flush=True)
    for name, reference in MORE_MDG_A.items():
        w, k = integer_instance(name)
        for aspiration, moves in budgets.items():
            d = ss.densest(w, k, experiments=30, max_moves=moves, div_freq=300, div_dur=10, aspiration=aspiration)
            totals = sorted(d._value(e.vertices) for e in d.result.experiments)
            print(f"{name.split('_n')[0]}, {'aspiration=full' if aspiration else 'default settings'}, {moves:,} moves: "
                  f"{sum(v >= reference for v in totals)} of 30 reach MDPLIB's {reference:,}; best {totals[-1] - reference:+}, "
                  f"median {totals[15] - reference:+}, worst {totals[0] - reference:+}", flush=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--specialist", action="store_true", help="also MDG-a_21 at OBMA's budget")
    parser.add_argument("--solvers", default="", help=f"comma-separated, from {', '.join(SOLVERS)}, or 'all'")
    parser.add_argument("--limit", type=float, default=10.0, help="seconds per solver run")
    parser.add_argument("--only", default="", help="run only instances whose name contains this")
    args = parser.parse_args()
    solvers = SOLVERS if args.solvers == "all" else [s for s in args.solvers.split(",") if s]
    if args.only:
        SOM_B[:] = [x for x in SOM_B if args.only in x[0]]
        REAL[:] = [x for x in REAL if args.only in x[0]]
    som_b(solvers, args.limit)
    real_valued(solvers, args.limit)
    if args.specialist:
        specialist()


if __name__ == "__main__":
    main()
