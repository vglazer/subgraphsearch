"""Finds the largest binary error-correcting codes of lengths 9 to 12 as cliques.

A code of length n and minimum distance d is a set of n-bit words, any two of which differ in at least
d places; A(n, d) is the most words such a code can have. In the graph on all 2^n words, joined when they
differ in at least d places, a code is a clique. Written as +-1 vectors, two words that differ in t places
have inner product n - 2t, so the graph is `ss.Graph.from_vectors(words, inner_products)` with the inner
products n - 2t for t >= d.

`ss.largest` grows the code size until the search fails. For every length and distance here it reaches
A(n, d), which is known exactly for these: 20, 40, 72 and 144 words at distance 4, and 6, 12 and 24 at
distance 6. Each code found is checked pair by pair. The whole script takes about half a minute.

    uv run python examples/codes.py
"""

import itertools
import time

import numpy as np

import subgraphsearch as ss

# A(n, d), known exactly
KNOWN = {(9, 4): 20, (10, 4): 40, (11, 4): 72, (12, 4): 144, (10, 6): 6, (11, 6): 12, (12, 6): 24}

for (n, d), best in KNOWN.items():
    t0 = time.perf_counter()
    words = np.array(list(itertools.product((1, -1), repeat=n)))
    far_apart = ss.Graph.from_vectors(words, range(-n, n - 2 * d + 1, 2))
    r = ss.largest(far_apart, "clique", start=best - 4, experiments=32, max_moves=5000)
    code = words[list(r.best.best.vertices)]
    distances = (n - code @ code.T) // 2
    smallest = distances[np.triu_indices(len(code), 1)].min()
    assert smallest >= d
    print(f"length {n:2}, distance {d}: a code of {r.size:3} words (A(n, d) = {best:3}), smallest distance "
          f"{smallest}, {time.perf_counter() - t0:.1f} s")
