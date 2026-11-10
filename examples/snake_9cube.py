"""Finds a 189-vertex snake in the 9-cube by fixing an 8-cube snake in one half.

A snake is an induced path in a hypercube (see snake-in-the-box). The 9-cube is two 8-cubes, the
vertices below 256 and those above, with each vertex joined to its copy in the other half. A plain
path search for 189 vertices in the 9-cube does not succeed, but fixing a 95-vertex 8-cube snake in
the lower half with fixed= leaves only the other 94 vertices to place, most of them in the upper
half, and that search succeeds in most experiments. The record is 192 vertices (191 edges).

Not every 8-cube snake works: of the 95- to 97-vertex ones tried, about one in fifteen gave 189,
and the one found here, with seed 26, is one of them. Snakes of 98 or 99 vertices block too much of
the upper half, and shorter ones leave the search further from a path. Results depend only on the
seeds, so this script finds the same snake every time; it takes under a minute on 16 cores.

    uv run python examples/snake_9cube.py
"""

import time

import subgraphsearch as ss


def is_snake(n: int, vertices: list[int]) -> bool:
    """Checks, without the library, that the vertices induce a path in the n-cube."""
    vs = set(vertices)
    nbrs = {v: [v ^ (1 << b) for b in range(n) if v ^ (1 << b) in vs] for v in vs}
    degrees = sorted(len(a) for a in nbrs.values())
    if degrees[:2] != [1, 1] or degrees[2:] != [2] * (len(vs) - 2):
        return False
    seen, stack = {vertices[0]}, [vertices[0]]
    while stack:
        for w in nbrs[stack.pop()]:
            if w not in seen:
                seen.add(w)
                stack.append(w)
    return len(seen) == len(vs)


settings = {"path": True, "max_moves": 200_000, "div_freq": 300, "div_dur": 8}

t0 = time.perf_counter()
half = ss.search(ss.hypercube(8), 95, experiments=16, seed=26, **settings)
lower = sorted(int(v) for v in half.best.vertices)
print(f"8-cube: a {len(lower)}-vertex snake in {time.perf_counter() - t0:.1f} s")

t0 = time.perf_counter()
r = ss.search(ss.hypercube(9), 189, fixed=lower, experiments=64, seed=284, **settings)
moves = sum(e.moves for e in r.experiments)
print(f"9-cube: {r.successes} of 64 experiments found a 189-vertex snake, {moves:,} moves in "
      f"{time.perf_counter() - t0:.0f} s")

snake = sorted(int(v) for v in r.best.vertices)
assert len(snake) == 189 and is_snake(9, snake)
print(f"checked: {len(snake)} vertices, {len(snake) - 1} edges, "
      f"{sum(v < 256 for v in snake)} in the lower half and {sum(v >= 256 for v in snake)} in the upper")
print(snake)
