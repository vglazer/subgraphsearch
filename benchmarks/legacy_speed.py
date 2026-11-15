"""PORTING.md, "Speed": the original C programs against subgraphsearch, one thread and many.

Each of the five search programs runs the same input three ways: compiled from the original C source
with `cc -O2`, through `subgraphsearch legacy` (the same output, on one thread), and through
`subgraphsearch search` with the same settings (kicks off, tabu tenure 1), on one thread and on 16.
Every target is unreachable, so every run makes exactly experiments x moves moves and the times
compare like for like. Times are medians of --repeats runs, each a whole process.

The C programs come from a clone of the repository holding the original
(https://github.com/vglazer/USRA), and the command from a release build of this one:

    cargo build --release -p subgraphsearch
    uv run python benchmarks/legacy_speed.py --original ~/repos/USRA
"""

import argparse
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

from common import ROOT, read_mdplib

import subgraphsearch as ss


def lists(g: ss.Graph) -> str:
    """The programs' adjacency lists: each vertex's neighbors, ended by -1."""
    return "".join(" ".join(map(str, g.neighbors(v))) + " -1\n" for v in range(g.n))


def weighted_lists(n: int, triples) -> str:
    """Weighted lists, as wsub_search reads them: neighbor x with weight w is written (w - 1) * n + x."""
    rows = [[] for _ in range(n)]
    for i, j, w in triples:
        if w:
            rows[i].append((w - 1) * n + j)
            rows[j].append((w - 1) * n + i)
    return "".join(" ".join(map(str, sorted(r))) + " -1\n" for r in rows)


def median_time(cmd: list[str], stdin: str | None, repeats: int) -> float:
    times = []
    for _ in range(repeats):
        start = time.perf_counter()
        p = subprocess.run(cmd, check=False, input=stdin, capture_output=True, text=True)
        times.append(time.perf_counter() - start)
        if p.returncode not in (0, 2) or not p.stdout.strip():
            sys.exit(f"{' '.join(cmd[:3])} failed: {p.stderr.strip()[:300]}")
    return statistics.median(times)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--original", type=Path, default=Path.home() / "repos/USRA",
                        help="a clone of the repository holding the original C programs")
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/subgraphsearch",
                        help="the subgraphsearch command (a release build)")
    parser.add_argument("--threads", type=int, default=16, help="threads for the parallel runs")
    parser.add_argument("--repeats", type=int, default=3)
    args = parser.parse_args()
    src = args.original / "subgraph_finding/src"
    if not src.is_dir() or not args.binary.exists():
        sys.exit("needs --original (a clone of github.com/vglazer/USRA) and a release build: "
                 "cargo build --release -p subgraphsearch")

    work = Path(tempfile.mkdtemp())
    dense = ss.generate("exponential", 1000, 0.5, seed=1)
    (work / "dense.txt").write_text(lists(dense), encoding="utf-8")
    n, k, triples = read_mdplib("SOM-b_14_n400_m80")
    (work / "somb.txt").write_text(weighted_lists(n, triples), encoding="utf-8")
    # program, what it searches for, (vertices, size, target, experiments, moves, div_freq, div_dur), graph, search options
    cases = [
        ("sub_search", "a 100-clique, G(1000, 0.5)", (1000, 100, 4950, 32, 2000, 300, 8), "dense.txt", ["-m", "4950"]),
        ("asub_search", "the same, with aspiration", (1000, 100, 4950, 32, 2000, 300, 8), "dense.txt", ["-m", "4950", "--aspiration"]),
        ("wsub_search", f"total weight 17,500, SOM-b n400 m{k}", (n, k, 17500, 32, 10000, 300, 10), "somb.txt",
         ["-m", "17500", "--weighted", "--from", "wlists"]),
        ("reg_search", "a 40-vertex independent set as 0-regular, G(1000, 0.5)", (1000, 40, 0, 32, 200, 300, 8), "dense.txt", ["--degree", "0"]),
        ("ereg_search", "the same in two phases", (1000, 40, 0, 32, 1000, 300, 8), "dense.txt", ["--degree", "0", "--two-phase"]),
    ]
    print(f"{'program':<12} {'moves':>8} {'C -O2':>7} {'legacy':>7} {'x C':>5} {f'{args.threads} thr':>7} {'x 1 thr':>7} {'x C':>5}   search")
    for prog, what, (v, size, target, exps, moves, freq, dur), graph, options in cases:
        binary = work / prog
        subprocess.run(["cc", "-O2", "-o", str(binary), str(src / f"{prog}.c")], check=True)
        stdin = f"{v} {size} {target} 0 {exps} {moves} {freq} {dur} 1 0\n" + (work / graph).read_text(encoding="utf-8")
        c = median_time([str(binary)], stdin, args.repeats)
        legacy = median_time([str(args.binary), "legacy", prog.replace("_", "-")], stdin, args.repeats)
        search = [str(args.binary), "search", str(work / graph), "-k", str(size), *options, "-n", str(exps),
                  "--moves", str(moves), "--div-freq", str(freq), "--div-dur", str(dur), "-s", "1"]
        if "--degree" not in options:   # the degree search has neither kicks nor a tenure to set
            search += ["--kick-size", "0", "--tabu-tenure", "1"]
        one = median_time([*search, "--threads", "1"], None, args.repeats)
        many = median_time([*search, "--threads", str(args.threads)], None, args.repeats)
        print(f"{prog:<12} {exps * moves:>8,} {c:>7.2f} {legacy:>7.2f} {c / legacy:>5.1f} {many:>7.3f} {one / many:>7.1f} "
              f"{c / many:>5.0f}   {what}", flush=True)


if __name__ == "__main__":
    main()
