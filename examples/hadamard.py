"""Finds Hadamard matrices of orders 8, 12, 16 and 20 as cliques.

A Hadamard matrix of order n is an n x n matrix of +1s and -1s whose rows are pairwise orthogonal:
H H^T = n I. Multiplying columns by -1 makes the first row all +1s, and multiplying rows by -1 then
makes the first column all +1s. The other n - 1 rows are then +-1 vectors that start with +1, sum to
0 (being orthogonal to the first row) and are pairwise orthogonal. So in the graph on all such
vectors, joined when they are orthogonal (`ss.Graph.from_vectors(rows, [0])`), a Hadamard matrix is
a clique of size n - 1, and any such clique, with the all-ones row added, is one.

There are C(n - 1, n/2 - 1) such vectors: 35 for order 8, 462 for 12 and 6,435 for 16. For order 20
there would be 92,378, so the second row is fixed too, as +1 on the first half and -1 on the second
(any Hadamard matrix of order 20 can be brought to that form by permuting columns). The other rows
then have five +1s in each half, which leaves 126 * 252 = 31,752 vectors, 169 million orthogonal
pairs, and a clique of size 18 to find. Building that graph takes about a second and 1.6 GB, and the
search a fraction of a second.

    uv run python examples/hadamard.py
"""

import itertools
import time

import numpy as np

import subgraphsearch as ss


def balanced(length: int, plus: int, first: int | None = None) -> np.ndarray:
    """The +-1 vectors of the given length with `plus` entries +1, one per row; with `first`, only
    those whose first entry is `first`."""
    rows = []
    for where in itertools.combinations(range(length), plus):
        v = -np.ones(length, dtype=np.int64)
        v[list(where)] = 1
        if first is None or v[0] == first:
            rows.append(v)
    return np.array(rows)


def find(n: int, fixed: np.ndarray, candidates: np.ndarray) -> np.ndarray:
    """A Hadamard matrix of order n whose first rows are `fixed`, the rest chosen from `candidates`."""
    t0 = time.perf_counter()
    g = ss.Graph.from_vectors(candidates, [0])
    built = time.perf_counter() - t0
    t0 = time.perf_counter()
    k = n - len(fixed)
    r = ss.search(g, k, k * (k - 1) // 2, experiments=16, max_moves=2000)  # a clique on k vertices
    found = time.perf_counter() - t0
    assert r.successes and g.induced(r.best.vertices).is_clique()
    h = np.vstack([fixed, candidates[list(r.best.vertices)]])
    assert (h @ h.T == n * np.eye(n, dtype=np.int64)).all()
    print(f"order {n}: {len(candidates):,} vectors, {g.m:,} orthogonal pairs, graph built in {built:.2f} s; "
          f"{r.successes} of 16 experiments found a clique of {k} in {found:.3f} s; H H^T = {n} I")
    return h


ones = {n: np.ones((1, n), dtype=np.int64) for n in (8, 12, 16, 20)}
for n in (8, 12, 16):
    h = find(n, ones[n], balanced(n, n // 2, first=1))
    if n == 12:
        print("\n".join("".join("+" if x > 0 else "-" for x in row) for row in h))

# order 20, with the second row fixed as well: five +1s in each half, the first entry +1
halves = [np.concatenate([a, b]) for a in balanced(10, 5, first=1) for b in balanced(10, 5)]
second = np.r_[np.ones(10), -np.ones(10)].astype(np.int64)
find(20, np.vstack([ones[20], second]), np.array(halves))
