# Porting

*Part of the [subgraphsearch documentation](README.md#documentation).*

subgraphsearch reimplements `sub_search`,
[a set of C programs written in 2002](https://github.com/vglazer/USRA/tree/master/subgraph_finding)
together with their graph generators; the original reports are there too. These notes cover how the
programs map onto subgraphsearch, how to reproduce their runs exactly, and how the port is tested
against them.

## The original programs

| Program | What it does | In subgraphsearch |
| --- | --- | --- |
| `sub_search` | induced subgraphs with a given number of vertices and edges | `search -k K -m M`, `ss.search(g, k, m)` |
| `asub_search` | `sub_search` with a limited aspiration criterion | `--aspiration`, `aspiration=True` |
| `wsub_search` | `sub_search` on weighted graphs, with a total-weight target | `--weighted`, a `WeightedGraph` |
| `reg_search` | regular induced subgraphs | `--degree D`, `degree=d` |
| `ereg_search` | `reg_search` that first finds the right number of edges | `--degree D --two-phase`, `two_phase=True` |
| `ggen` | random graphs, and operations on a given graph | `generate`, `op`, `ss.generate`, `Graph` methods |
| `wggen` | random weighted graphs | `generate weighted`, `ss.generate_weighted` |

`ggen`'s parameters correspond as follows:

- **Graph types** 2, 3 and 4 are the `exponential`, `power` and `geometric` models. `ggen`'s
  density is per mille (0 to 1000); subgraphsearch takes it as a fraction (0 to 1).
- **Fixed types** 1, -1, 2, -2 and 3 are `op induce`, `op remove`, `op common-neighbors`,
  `op common-non-neighbors` and `op switch` (`g.induced`, `g.common_neighborhood`, `g.switch`, ...).
  The complementation flag is `op complement` (`g.complement()`).
- **Incidence lists** (graph type 1) are `subgraphsearch incidence` and `ss.Graph.point_graph` /
  `ss.Graph.block_graph`: `ggen`'s primal graph is the point graph and its dual the block graph,
  with `ggen`'s density parameter as `k`. The new commands take one block per line instead of
  `-1`-terminated lists.

The `lists` and `wlists` formats are the programs' `-1`-terminated adjacency lists. In `wlists`,
entry `x` in row `i` is the edge `{i, x % n}` with weight `x / n + 1`, as `wggen` writes and
`wsub_search` reads.

## Reproducing original runs

There are two levels of reproduction:

- **Exact output.** `subgraphsearch legacy <program>` takes a program's stdin (or, for `wggen`,
  its arguments) and prints exactly what it printed, with the same exit status. Only the final
  `T> ... seconds H> host` line differs, since it reports CPU time and the host name. From Python,
  `ss.legacy.sub_search(stdin)` and the other search programs return `(stdout, status)`;
  `ss.legacy.ggen(stdin)` and `ss.legacy.wggen(args)` return the output.
- **Same results through the modern interfaces.** `--rng legacy` (`rng="legacy"`) uses the
  programs' random number generator, a variant of RANDU, so a given seed produces the same graphs
  and the same experiments, move for move. Experiments then run one after another, as in the C
  programs, instead of in parallel.

The port deliberately keeps the programs' quirks so that results match:

- `ereg_search` computes its phase-one edge target as `(size - fixed) * degree / 2` rather than
  `size * degree / 2`, so two-phase searches with fixed vertices rarely succeed. This applies in
  every mode, which is why the README advises `--degree` alone with `--fixed`.
- `reg_search` and `ereg_search` carry their partitioned neighbor lists over from one experiment
  to the next, which affects the order in which a solution's adjacency lists are printed.
- A search starts its best conflict at the graph's edge count (or total weight), an upper bound on
  the conflict only when the target is no larger. With a target above it, the programs never
  record a best subgraph and choose every move at random. The legacy rng keeps this; with xoshiro,
  such searches start from a bound above the target and find the closest subgraphs as usual.
- Input is parsed the way C's `scanf("%d")` parses it, so `5-1` reads as `5` followed by `-1`
  (`ggen`'s output relies on this), and numbers print as C's `%g` prints them.

One quirk is kept only in the exact output: for a failed experiment, `ereg_search` reports how far
the edge count was from the target if the experiment never reached its second phase, and
`size * size` if the second phase stalled at once. Neither is the conflict of the subgraph it
returns, so the modern interfaces recompute it from that subgraph: its deviation from regularity.
The experiments and their subgraphs are unchanged.

The intended differences are input checks the C programs lacked: unchecked vertex numbers,
self-loops and similar problems now produce an error instead of undefined behavior. On systems
other than Unix, the final timing line reports elapsed time and the `COMPUTERNAME` host name.

## Speed

subgraphsearch is faster than the C programs in two separate ways: each move is cheaper in the
counting search, and experiments run in parallel. The table separates the two. Each program runs the
same input three ways:
- compiled from the original source with `cc -O2` (the compiler's standard optimization level);
- through `subgraphsearch legacy`, which gives the same output on one thread;
- through `subgraphsearch search` with the programs' settings (no region kicks, tabu tenure 1), on 1
  and on 16 threads.

Every target is unreachable, so every run makes the same number of moves. Times are medians of
three runs on a 16-core Apple M3 Max, measured by
[benchmarks/legacy_speed.py](benchmarks/legacy_speed.py).

| Program | Search | Moves | C | Legacy, 1 thread | Faster than C | 16 threads | Faster than 1 thread | Faster than C |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `sub_search` | a 100-clique in G(1000, 0.5) | 64,000 | 2.13 s | 0.35 s | 6.0× | 0.041 s | 8.2× | 52× |
| `asub_search` | the same, with aspiration | 64,000 | 2.20 s | 0.41 s | 5.4× | 0.045 s | 8.9× | 49× |
| `wsub_search` | total weight 17,500 in SOM-b n400 m80 | 320,000 | 3.19 s | 0.65 s | 4.9× | 0.063 s | 10.3× | 51× |
| `reg_search` | a 40-vertex independent set, as 0-regular, in G(1000, 0.5) | 6,400 | 4.67 s | 5.13 s | 0.9× | 0.41 s | 12.4× | 11× |
| `ereg_search` | the same, in two phases | 32,000 | 0.57 s | 0.78 s | 0.7× | 0.089 s | 8.9× | 6× |

- **On one thread,** the counting search (`sub_search`, `asub_search`, `wsub_search`) is 5–6 times
  faster than the C programs, with the same output. It skips candidate swaps that cannot be best
  (*bounded rows* in [ALGORITHM.md](ALGORITHM.md#objective-and-moves)), which the C programs
  evaluate one by one. The degree search (`reg_search`, `ereg_search`) has no such shortcut, and
  the port is slower than the C programs there. The modern search on one thread is as fast as
  legacy mode.
- **On many threads,** experiments run in parallel. Legacy mode cannot do this: the programs'
  random number generator is one stream shared by all experiments, so they must run in order. The
  modern search gives each experiment its own stream, and 16 threads (12 performance and 4
  efficiency cores) run 8–12 times faster than one.
- **Together,** the counting search runs about 50 times faster than the C programs, and the
  degree search 6–11 times.

Compiling the C programs with the original Makefile's `-O3 -march=native` instead of `-O2` makes no
difference. The modern search's defaults, region kicks and a longer tabu tenure, change the search
itself rather than its speed; [TUNING.md](TUNING.md) covers them.

## Tests against the original programs

`tests/fixtures` holds the output of the original programs on a set of inputs, recorded by
`scripts/make_fixtures.sh` from a clone of the
[repository holding the original](https://github.com/vglazer/USRA/tree/master/subgraph_finding):

```sh
ORIGINAL_DIR=~/repos/USRA scripts/make_fixtures.sh
```

`tests/fixtures/SOURCE` records the commit the fixtures came from. The golden tests in
`crates/subgraphsearch-cli/tests/golden.rs` require the `legacy` commands' output, error output
and exit status to match byte for byte, apart from the timing line. The Python tests check
`ss.legacy` against the same fixtures, and check that `rng="legacy"` regenerates the recorded
graphs and finds the same subgraphs through `ss.search`.

## Naming and spelling

The original programs used Canadian spelling (`neighbourhood`, `minimised`). subgraphsearch
uses American spelling in its own code and documentation, but legacy mode prints the programs'
messages exactly as they did.
