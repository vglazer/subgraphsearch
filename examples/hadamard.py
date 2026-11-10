"""Finds Hadamard matrices of orders 8, 12 and 16 as cliques.

A Hadamard matrix of order n is an n x n matrix of +1s and -1s whose rows are pairwise orthogonal:
H H^T = n I. Multiplying columns by -1 makes the first row all +1s, and multiplying rows by -1 then
makes the first column all +1s. The other n - 1 rows are then +-1 vectors that start with +1, sum to
0 (being orthogonal to the first row) and are pairwise orthogonal. So in the graph on all such
vectors, joined when they are orthogonal, a Hadamard matrix is a clique of size n - 1, and any such
clique, with the all-ones row added, is one.

The graph has C(n - 1, n/2 - 1) vertices: 35 for order 8, 462 for 12 and 6,435 for 16. Searching it
takes a fraction of a second; building it takes most of the time. Order 20 would need 92,378 vectors,
more than this approach handles comfortably.

    uv run python examples/hadamard.py
"""

import itertools
import time

import numpy as np

import subgraphsearch as ss


def candidate_rows(n: int) -> np.ndarray:
    """The +-1 vectors of length n that start with +1 and sum to 0, one per row."""
    rows = []
    for plus in itertools.combinations(range(1, n), n // 2 - 1):  # where the other +1s go
        v = -np.ones(n, dtype=np.int64)
        v[0] = 1
        v[list(plus)] = 1
        rows.append(v)
    return np.array(rows)


for n in (8, 12, 16):
    t0 = time.perf_counter()
    rows = candidate_rows(n)
    orthogonal = np.argwhere(np.triu(rows @ rows.T == 0, 1))
    g = ss.Graph(len(rows), orthogonal)
    built = time.perf_counter() - t0

    t0 = time.perf_counter()
    k = n - 1
    r = ss.search(g, k, k * (k - 1) // 2, experiments=16, max_moves=1000)  # a clique on n - 1 vertices
    found = time.perf_counter() - t0
    clique = r.best.vertices
    assert r.successes and g.induced(clique).is_clique()

    h = np.vstack([np.ones(n, dtype=np.int64), rows[list(clique)]])
    assert (h @ h.T == n * np.eye(n, dtype=np.int64)).all()
    print(f"order {n}: {len(rows)} vectors, {g.m:,} orthogonal pairs, graph built in {built:.2f} s; "
          f"{r.successes} of 16 experiments found a clique of {k} in {found:.3f} s; H H^T = {n} I")
    if n == 12:
        print("\n".join("".join("+" if x > 0 else "-" for x in row) for row in h))
