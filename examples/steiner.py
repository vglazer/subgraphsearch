"""Finds Steiner triple systems on up to 81 points as independent sets.

A Steiner triple system on v points is a set of triples of points that contains every pair of points
exactly once; there is one exactly when v leaves remainder 1 or 3 on division by 6, and it has
v(v - 1)/6 triples. Equivalently, it is that many triples no two of which share a pair, since they
then cover all v(v - 1)/2 pairs. In the graph on all triples, joined when they share two points
(`ss.Graph.block_graph(triples, 2)`), a Steiner triple system is an independent set of that size.

The search finds one for every v here, up to 81 points: 1,080 triples among 85,320. A short tabu
tenure suits this problem, so the script sets `tabu_tenure=2` rather than the longer default. Each
system found is checked pair by pair. The whole script takes about 40 seconds on 16 cores.
Methods made for designs, such as hill-climbing (D. R. Stinson, 1985), are faster still.

    uv run python examples/steiner.py
"""

import time
from itertools import combinations

import subgraphsearch as ss

for v in (7, 9, 13, 15, 19, 21, 25, 27, 31, 37, 43, 49, 55, 61, 69, 81):
    t0 = time.perf_counter()
    triples = list(combinations(range(v), 3))
    share_a_pair = ss.Graph.block_graph(triples, 2)
    size = v * (v - 1) // 6
    r = ss.search(share_a_pair, size, 0, experiments=8, max_moves=20000, div_freq=300, div_dur=8, tabu_tenure=2)
    system = [triples[i] for i in r.best.vertices]
    pairs = [pair for triple in system for pair in combinations(triple, 2)]
    assert r.successes and len(pairs) == len(set(pairs)) == v * (v - 1) // 2
    print(f"{v:2} points: {size:4} triples among {len(triples):6,}, every pair once; "
          f"{r.successes} of 8 experiments, {time.perf_counter() - t0:.1f} s")
