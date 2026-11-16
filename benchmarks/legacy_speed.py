"""PORTING.md, "Speed": the original C programs against subgraphsearch, one thread and many.

Each of the five search programs runs the same input three ways: compiled from the original C source
with `cc -O2`, through `subgraphsearch legacy` (the same output, on one thread), and through
`subgraphsearch search` with the same settings (kicks off, tabu tenure 1), on one thread and on 16.
Every target is unreachable, so every run makes exactly experiments x moves moves and the times
compare like for like. Times are medians of --repeats runs, each a whole process.

It also checks that legacy mode's output and exit status match the C program's, apart from the
timing at the end of the last line, on inputs far larger than the golden tests' fixtures, and
stops with the first difference if they do not.

The C programs come from a clone of the repository holding the original
(https://github.com/vglazer/USRA), and the command from a release build of this one:

    cargo build --release -p subgraphsearch
    uv run python benchmarks/legacy_speed.py --original ~/repos/USRA
"""

import argparse
import difflib
import re
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


def median_time(cmd: list[str], stdin: str | None, repeats: int) -> tuple[float, str, int]:
    """The median time of ``repeats`` runs, and the last run's output and exit status."""
    times = []
    for _ in range(repeats):
        start = time.perf_counter()
        p = subprocess.run(cmd, check=False, input=stdin, capture_output=True, text=True)
        times.append(time.perf_counter() - start)
        if p.returncode not in (0, 2) or not p.stdout.strip():
            sys.exit(f"{' '.join(cmd[:3])} failed: {p.stderr.strip()[:300]}")
    return statistics.median(times), p.stdout, p.returncode


def without_timing(output: str) -> list[str]:
    """The output without the CPU time and host name that end its last line ("T> 0.94 seconds H> host")."""
    return [re.sub(r"\s*T> .*$", "", line) for line in output.splitlines()]


def check_same(prog: str, c: tuple[float, str, int], legacy: tuple[float, str, int]) -> None:
    """Stops with the first difference if legacy mode's output or exit status differs from the C program's."""
    if c[2] != legacy[2]:
        sys.exit(f"{prog}: exit status {legacy[2]} in legacy mode, {c[2]} from the C program")
    a, b = without_timing(c[1]), without_timing(legacy[1])
    if a != b:
        diff = difflib.unified_diff(a, b, "C program", "legacy mode", n=1, lineterm="")
        sys.exit(f"{prog}: legacy mode's output differs from the C program's\n" + "\n".join(list(diff)[:12]))


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
    print(f"{'program':<12} {'moves':>8} {'C -O2':>7} {'legacy':>7} {'x C':>5} {'1 thr':>7} {f'{args.threads} thr':>7} "
          f"{'x 1 thr':>7} {'x C':>5}   search (output identical to C's)")
    for prog, what, (v, size, target, exps, moves, freq, dur), graph, options in cases:
        binary = work / prog
        subprocess.run(["cc", "-O2", "-o", str(binary), str(src / f"{prog}.c")], check=True)
        stdin = f"{v} {size} {target} 0 {exps} {moves} {freq} {dur} 1 0\n" + (work / graph).read_text(encoding="utf-8")
        c_run = median_time([str(binary)], stdin, args.repeats)
        legacy_run = median_time([str(args.binary), "legacy", prog.replace("_", "-")], stdin, args.repeats)
        check_same(prog, c_run, legacy_run)
        c, legacy = c_run[0], legacy_run[0]
        search = [str(args.binary), "search", str(work / graph), "-k", str(size), *options, "-n", str(exps),
                  "--moves", str(moves), "--div-freq", str(freq), "--div-dur", str(dur), "-s", "1"]
        if "--degree" not in options:   # the degree search has neither kicks nor a tenure to set
            search += ["--kick-size", "0", "--tabu-tenure", "1"]
        one = median_time([*search, "--threads", "1"], None, args.repeats)[0]
        many = median_time([*search, "--threads", str(args.threads)], None, args.repeats)[0]
        print(f"{prog:<12} {exps * moves:>8,} {c:>7.2f} {legacy:>7.2f} {c / legacy:>5.1f} {one:>7.2f} {many:>7.3f} "
              f"{one / many:>7.1f} {c / many:>5.0f}   {what}", flush=True)


if __name__ == "__main__":
    main()
