# Porting notes

`subgraphsearch` reimplements `sub_search`,
[a set of C programs written in 2002](https://github.com/vglazer/USRA/tree/master/subgraph_finding)
together with their graph generators; the original reports are there too. These notes cover how the
programs map onto `subgraphsearch`, how to reproduce their runs exactly, and how the port is tested
against them.

## The original programs

| Program | What it does | In `subgraphsearch` |
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
  density is per mille (0 to 1000); `subgraphsearch` takes it as a fraction (0 to 1).
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

## Performance

By default experiments run in parallel, which makes a search roughly 10 times faster than the C
programs on a multi-core machine.

Legacy mode is single-threaded. There, `sub_search`, `asub_search` and `wsub_search` run about 1.5
to 2 times slower than the C programs compiled with `-O2`, while `reg_search` and `ereg_search` run
faster. Most of the time goes to the loop that scans candidate swaps; the C programs read a byte
adjacency matrix with 32-bit indices, where `subgraphsearch` reads a bitset with 64-bit indices.

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

The original programs used Canadian spelling (`neighbourhood`, `minimised`). `subgraphsearch`
uses American spelling in its own code and documentation, but legacy mode prints the programs'
messages exactly as they did.
