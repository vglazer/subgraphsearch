"""BENCHMARKS.md, "Steiner triple systems": subgraphsearch against Stinson's hill-climbing.

subgraphsearch finds a Steiner triple system on v points as an independent set of v(v - 1)/6 triples
in the graph of all triples joined when they share a pair: one experiment with tabu_tenure=2,
median of 3, timing the graph and the search separately. Stinson's hill-climbing (D. R. Stinson,
Annals of Discrete Mathematics 26, 1985), written here in plain Python on one core, builds one
directly: median of 5. Every system from either method is checked pair by pair. Stinson's method
alone also runs on --large point counts, beyond what subgraphsearch's graph can hold.

    uv run python benchmarks/steiner.py
"""

import argparse
import itertools
import random
import statistics
import time

from common import timed

import subgraphsearch as ss


def check(v: int, triples) -> None:
    pairs = [pair for t in triples for pair in itertools.combinations(t, 2)]
    assert len(triples) == v * (v - 1) // 6 and len(pairs) == len(set(pairs)) == v * (v - 1) // 2


def stinson(v: int, rng: random.Random) -> list[tuple[int, int, int]]:
    """A Steiner triple system on v points by Stinson's hill-climbing: pick a point x with an
    uncovered pair and two of its uncovered partners y and z; add {x, y, z}, first removing the
    triple that covers {y, z} if there is one. Every step keeps the triples pairwise compatible and
    either adds a triple or keeps the count, and with random choices it soon covers every pair."""
    other = [[-1] * v for _ in range(v)]                          # the third point of {x, y}'s triple
    live = [[y for y in range(v) if y != x] for x in range(v)]    # the uncovered partners of x
    pos = [[0] * v for _ in range(v)]
    for x in range(v):
        for i, y in enumerate(live[x]):
            pos[x][y] = i
    live_points, lp_pos = list(range(v)), list(range(v))

    def cover(x, y):
        lst = live[x]
        i, last = pos[x][y], lst[-1]
        lst[i], pos[x][last] = last, i
        lst.pop()
        if not lst:
            j, last_point = lp_pos[x], live_points[-1]
            live_points[j], lp_pos[last_point] = last_point, j
            live_points.pop()

    def uncover(x, y):
        if not live[x]:
            lp_pos[x] = len(live_points)
            live_points.append(x)
        pos[x][y] = len(live[x])
        live[x].append(y)

    def place(a, b, c, add):
        for p, q, r in ((a, b, c), (a, c, b), (b, c, a)):
            other[p][q] = other[q][p] = r if add else -1
            for s, t in ((p, q), (q, p)):
                (cover if add else uncover)(s, t)

    count = 0
    while count < v * (v - 1) // 6:
        x = live_points[rng.randrange(len(live_points))]
        y, z = rng.sample(live[x], 2)
        if other[y][z] == -1:
            count += 1
        else:
            place(y, z, other[y][z], add=False)
        place(x, y, z, add=True)
    return sorted({tuple(sorted((x, y, other[x][y]))) for x in range(v) for y in range(x + 1, v)})


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--points", default="31,49,61,81", help="point counts for both methods")
    parser.add_argument("--large", default="153,301,601,999", help="point counts for Stinson's method alone")
    args = parser.parse_args()
    for v in map(int, args.points.split(",")):
        triples = list(itertools.combinations(range(v), 3))
        g, build = timed(lambda triples=triples: ss.Graph.block_graph(triples, 2))
        k = v * (v - 1) // 6
        times = []
        for seed in (1, 2, 3):
            start = time.perf_counter()
            r = ss.search(g, k, 0, experiments=1, max_moves=40000, div_freq=300, div_dur=8, tabu_tenure=2, seed=seed)
            times.append(time.perf_counter() - start)
            assert r.successes, f"no system on {v} points with seed {seed}"
            check(v, [triples[i] for i in r.best.vertices])
        _, hill = timed(lambda v=v: check(v, stinson(v, random.Random(0))), repeats=5)
        print(f"{v} points, {k:,} triples: subgraphsearch {build:.2f} s for the graph, then "
              f"{statistics.median(times):.2f} s; Stinson's hill-climbing {hill * 1000:.1f} ms", flush=True)
    for v in map(int, args.large.split(",")):
        _, hill = timed(lambda v=v: check(v, stinson(v, random.Random(0))), repeats=5)
        print(f"{v} points, {v * (v - 1) // 6:,} triples: Stinson's hill-climbing {hill:.2f} s", flush=True)


if __name__ == "__main__":
    main()
