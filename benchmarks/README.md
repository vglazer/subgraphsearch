# Benchmark scripts

These scripts reproduce the measurements in [BENCHMARKS.md](../BENCHMARKS.md), one per section,
and the speed comparison in [PORTING.md](../PORTING.md#speed). Run them from the repository root with the package installed (`uv sync`); each prints one line per
case, and `--help` lists its options.

| Script | Section | Default run | Optional extras |
| --- | --- | --- | --- |
| `edge_counts.py` | Exact edge counts | under 1 s | `--solvers all`: CP-SAT, SCIP, HiGHS and CBC (up to 10 minutes) |
| `diversity.py` | Maximum diversity | about 10 s, after fetching data once | `--specialist`: MDG-a_21 to 24 at OBMA's budget, with and without `aspiration="full"` (about 7 minutes); `--solvers all` (about 20 minutes) |
| `cliques.py` | Maximum cliques | about 4 s | `--tenures`; `--igraph`: igraph's exact clique number, 90 s cap per graph |
| `hypercubes.py` | Snake-in-the-box | about 25 s | `--nine`: the 9-cube (about 2 minutes) |
| `steiner.py` | Steiner triple systems | about 75 s | `--points`, `--large` |
| `legacy_speed.py` | [PORTING.md](../PORTING.md#speed), Speed: the original C programs against legacy mode and threads | about 1 minute | `--threads`, `--repeats` |

Times are for a 16-core Apple M3 Max laptop, where BENCHMARKS.md's numbers were measured.

**Solvers.** The general solvers are needed only for `--solvers`; install them for the run:

```sh
uv run --with ortools --with pulp==2.9.0 --with highspy --with pyscipopt python benchmarks/edge_counts.py --solvers all
```

BENCHMARKS.md's numbers came from OR-Tools 9.15, PuLP 2.9.0, HiGHS 1.16 and PySCIPOpt 6.3; later
PuLP versions changed its API. Each mixed-integer solve runs in a process of its own, since SCIP's
and HiGHS's native libraries can crash when loaded into one. igraph's exact searches likewise run in
a separate process, so its times include about 0.3 s to start one.

**Data.** The DIMACS graphs and eight MDPLIB instances are in [examples/data](../examples/data/README.md).
The other MDPLIB instances (the 500-item SOM-b ones, GKD-c 1, MDG-a 1 and MDG-a_21) are fetched on
first use into `benchmarks/data`, which is not committed: only the needed files are read from
MDPLIB's 679 MB archive on Zenodo, by HTTP range requests. They are under the Creative Commons
Attribution 4.0 license, like the rest of MDPLIB.

**Specialists.** DLS-MC's and OBMA's times in BENCHMARKS.md come from their papers. Stinson's
hill-climbing for Steiner triple systems is implemented in `steiner.py`, as a reference for the
comparison only; it is not part of the library.
