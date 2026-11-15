"""BENCHMARKS.md, "Snake-in-the-box": the longest induced paths (snakes) and cycles (coils) in hypercubes.

By default: the 7-cube's 51-vertex snake and 48-vertex coil (400 experiments of up to 2,000 moves),
and the 8-cube's 99-vertex snake (400 of up to 20,000) and 96-vertex coil (200 of up to 20,000).
--nine adds 185-vertex snakes in the 9-cube (64 experiments of up to 300,000 moves, about 2.5
minutes). The 189-vertex snake through a fixed half is examples/snake_9cube.py.

    uv run python benchmarks/hypercubes.py
    uv run python benchmarks/hypercubes.py --nine
"""

import argparse

from common import timed

import subgraphsearch as ss


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--nine", action="store_true", help="also 185-vertex snakes in the 9-cube")
    args = parser.parse_args()
    cases = [
        ("7-cube snake", 7, 51, {"path": True}, 400, 2000),
        ("7-cube coil", 7, 48, {"degree": 2, "connected": True}, 400, 2000),
        ("8-cube snake", 8, 99, {"path": True}, 400, 20000),
        ("8-cube coil", 8, 96, {"degree": 2, "connected": True}, 200, 20000),
    ]
    if args.nine:
        cases.append(("9-cube snake", 9, 185, {"path": True}, 64, 300000))
    for name, d, k, shape, experiments, moves in cases:
        cube = ss.hypercube(d)
        r, t = timed(lambda cube=cube, k=k, shape=shape, experiments=experiments, moves=moves: ss.search(
            cube, k, experiments=experiments, max_moves=moves, div_freq=300, div_dur=8, **shape))
        print(f"{name} of {k} vertices: {r.successes} of {experiments} experiments in {t:.2f} s", flush=True)


if __name__ == "__main__":
    main()
