"""The general solvers BENCHMARKS.md compares with: CP-SAT, and SCIP, HiGHS and CBC through PuLP.

Install them only to run these comparisons, for example
``uv run --with ortools --with pulp==2.9.0 --with highspy --with pyscipopt python benchmarks/edge_counts.py
--solvers all``; the numbers in BENCHMARKS.md came from OR-Tools 9.15, PuLP 2.9.0, HiGHS 1.16 and
PySCIPOpt 6.3.
Each returns ``(value, seconds)``: for an exact target, whether a solution was found (or proved
impossible); for a maximization, the best total found.
"""

from __future__ import annotations

import time

SOLVERS = ("cpsat", "scip", "highs", "cbc")


def cpsat_exact(n: int, edges, k: int, m: int, limit: float):
    """CP-SAT on k vertices with exactly m edges: a yes-or-no variable per vertex, and the edge count
    as half the sum of the chosen vertices' degrees within the set. Returns ``(status, seconds)``."""
    from ortools.sat.python import cp_model

    nbrs = [[] for _ in range(n)]
    for u, v in edges:
        nbrs[u].append(v)
        nbrs[v].append(u)
    model = cp_model.CpModel()
    x = [model.NewBoolVar(f"x{v}") for v in range(n)]
    model.Add(sum(x) == k)
    degrees = []
    for v in range(n):
        d = model.NewIntVar(0, min(len(nbrs[v]), k - 1), f"d{v}")
        model.Add(d == sum(x[u] for u in nbrs[v])).OnlyEnforceIf(x[v])
        model.Add(d == 0).OnlyEnforceIf(x[v].Not())
        degrees.append(d)
    model.Add(sum(degrees) == 2 * m)
    return _cpsat_solve(model, limit, lambda solver, status: solver.StatusName(status))


def cpsat_max_weight(n: int, triples, k: int, limit: float, score=None):
    """CP-SAT maximizing the total weight of k items: the same encoding with weighted degrees, which
    must be integers. Returns ``(best total, seconds)``, the total ``None`` if nothing was found; the
    total is of the chosen items' pairs in ``score`` (default ``triples``), so that a search on
    rounded distances can be scored on the original ones."""
    from ortools.sat.python import cp_model

    nbrs = [[] for _ in range(n)]
    for u, v, w in triples:
        if w:
            nbrs[int(u)].append((int(v), int(w)))
            nbrs[int(v)].append((int(u), int(w)))
    model = cp_model.CpModel()
    x = [model.NewBoolVar(f"x{v}") for v in range(n)]
    model.Add(sum(x) == k)
    degrees = []
    for v in range(n):
        d = model.NewIntVar(0, sum(w for _, w in nbrs[v]), f"d{v}")
        model.Add(d == sum(w * x[u] for u, w in nbrs[v])).OnlyEnforceIf(x[v])
        model.Add(d == 0).OnlyEnforceIf(x[v].Not())
        degrees.append(d)
    model.Maximize(sum(degrees))

    def total(solver, status):
        if status not in (cp_model.OPTIMAL, cp_model.FEASIBLE):
            return None
        chosen = {v for v in range(n) if solver.Value(x[v])}
        return sum(float(w) for u, v, w in (triples if score is None else score) if int(u) in chosen and int(v) in chosen)

    return _cpsat_solve(model, limit, total)


def _cpsat_solve(model, limit, value):
    from ortools.sat.python import cp_model

    solver = cp_model.CpSolver()
    solver.parameters.max_time_in_seconds = limit
    solver.parameters.num_workers = 16
    start = time.perf_counter()
    status = solver.Solve(model)
    return value(solver, status), time.perf_counter() - start


def mip(name: str, n: int, triples, k: int, limit: float, exact_edges: int | None = None):
    """:func:`mip_in_process` in a separate process each time: SCIP's and HiGHS's native libraries
    can crash when loaded into one process. A crash comes back as ``("crashed", seconds)``."""
    import multiprocessing
    from concurrent.futures import ProcessPoolExecutor
    from concurrent.futures.process import BrokenProcessPool

    start = time.perf_counter()
    with ProcessPoolExecutor(max_workers=1, mp_context=multiprocessing.get_context("spawn")) as pool:
        try:
            return pool.submit(mip_in_process, name, n, [tuple(t) for t in triples], k, limit, exact_edges).result()
        except BrokenProcessPool:
            return "crashed", time.perf_counter() - start


def mip_in_process(name: str, n: int, triples, k: int, limit: float, exact_edges: int | None = None):
    """A mixed-integer solver through PuLP, with the standard formulation: a yes-or-no variable per
    vertex and per edge, the edge's 1 exactly when both its ends are chosen. With ``exact_edges``
    it looks for that many edges (``triples`` all of weight 1) and returns whether it found them;
    otherwise it maximizes the total weight and returns the best total (``None`` if none).
    Returns ``(value, seconds)``."""
    import pulp

    prob = pulp.LpProblem("subgraph", pulp.LpMaximize)
    x = [pulp.LpVariable(f"x{v}", cat="Binary") for v in range(n)]
    z, total = [], []
    for i, (u, v, w) in enumerate(triples):
        e = pulp.LpVariable(f"z{i}", cat="Binary")
        prob += e <= x[int(u)]
        prob += e <= x[int(v)]
        prob += e >= x[int(u)] + x[int(v)] - 1
        z.append(e)
        total.append(float(w) * e)
    prob += pulp.lpSum(x) == k
    if exact_edges is not None:
        prob += pulp.lpSum(z) == exact_edges
        prob += 0
    else:
        prob += pulp.lpSum(total)
    solver = {
        "cbc": lambda: pulp.PULP_CBC_CMD(msg=False, timeLimit=limit, threads=16),
        "highs": lambda: pulp.HiGHS(msg=False, timeLimit=limit),
        "scip": lambda: pulp.SCIP_PY(msg=False, timeLimit=limit),
    }[name]()
    start = time.perf_counter()
    prob.solve(solver)
    seconds = time.perf_counter() - start
    # judge the solver by the vertices it chose, checked here, not by its own report
    chosen = {v for v in range(n) if (x[v].varValue or 0) > 0.5}
    weight = sum(float(w) for u, v, w in triples if int(u) in chosen and int(v) in chosen)
    found = len(chosen) == k
    if exact_edges is not None:
        return found and weight == exact_edges, seconds
    return (weight if found else None), seconds
