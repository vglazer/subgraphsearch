# subsearch

[![CI](https://github.com/vglazer/subsearch/actions/workflows/ci.yml/badge.svg)](https://github.com/vglazer/subsearch/actions/workflows/ci.yml)

Find **induced subgraphs with a prescribed number of vertices and edges**: cliques,
independent sets, and everything in between. `subsearch` is a Rust port, with a Python package,
of `sub_search` from the [USRA stochastic graph search project](https://github.com/vglazer/USRA/tree/master/subgraph_finding),
together with its graph generator `ggen`.

- **Faithful.** A `legacy` mode reproduces the original C programs byte for byte (same seed,
  same output), checked against fixtures recorded from them.
- **Fast.** Experiments run in parallel; a typical search is about 9× faster than the C program
  on a multi-core machine.
- **Standard formats.** graph6, sparse6, Matrix Market, edge lists, DOT and GraphML, plus the
  original adjacency lists. See [docs/FORMATS.md](docs/FORMATS.md).
- **Visual.** Search diagnostics, spectral and structural plots, Jupyter displays, and a
  self-contained interactive HTML viewer for graphs and the subgraphs found in them.

No other Python or Rust library searches for induced subgraphs with an exact (or near-exact) edge
count; they cover only the extreme cases (cliques, independent sets) or densest-subgraph variants.

## Install

Requires Rust 1.87+ and, for Python, [uv](https://docs.astral.sh/uv/) (or pip with maturin).

```sh
cargo install --path crates/subsearch-cli       # the `subsearch` command
uv sync --all-extras                            # the Python package, in .venv
```

## Command line

```sh
# a random graph: 100 vertices, density 0.6 (graph6 output)
subsearch generate exponential -n 100 -d 0.6 -s 1 -o g.g6

# search for 11-cliques: 11 vertices, 55 edges
subsearch search g.g6 -k 11 -m 55 -n 20 --moves 1000

# also report near misses within 2 edges, and save everything (with per-move traces) as JSON
subsearch search g.g6 -k 30 -m 340 -n 60 --max-conflict 2 --trace --json result.json

# look at the results
uv run python -m subsearch explore g.g6 --result result.json -o result.html   # interactive page
uv run python -m subsearch report g.g6 --result result.json -o result.png     # figures

# files holding many graphs (graph6/sparse6, one per line, e.g. from nauty's geng)
subsearch info many.g6                                  # one line per graph
subsearch search many.g6 --all -k 10 -m 30 -q           # search every graph
subsearch search many.g6 --index 3 -k 10 -m 30          # just graph 3 (from 0)

# graphs: statistics, conversion, operations
subsearch info g.g6 --degrees
subsearch convert g.g6 g.dot --highlight 3,17,42
subsearch op complement g.g6 gc.s6
subsearch op common-neighbours g.g6 nbhd.g6 --vertices 3,17
```

`subsearch search` exits with status 2 when no experiment succeeds, so scripts can tell "nothing
found" from success.

### Search parameters

| Option | Meaning | Default |
| --- | --- | --- |
| `-k`, `--vertices` | subgraph vertices, including `--fixed` ones | required |
| `-m`, `--edges` | subgraph edges | required |
| `--fixed` | vertices the subgraph must contain | none |
| `-n`, `--experiments` | independent restarts | 10 |
| `--moves` | maximum moves (vertex swaps) per experiment | 1000 |
| `--div-freq`, `--div-dur` | make `div-dur` random moves at the start of every `div-freq` moves | 250, 4 |
| `-s`, `--seed`, `--rng` | random seed; `xoshiro` (default) or `legacy` | 1, xoshiro |
| `--aspiration` | limited aspiration, as in USRA's `asub_search` (see below) | off |

Each experiment starts from random vertices and repeatedly swaps one subgraph vertex for one
outside vertex, choosing at random among the swaps that bring the edge count closest to the
target. When the edge count did not change on the last move, only part of the neighbourhood is
examined (the "restricted" local search), and the two vertices just swapped are never swapped
straight back. Bursts of random moves help it escape local minima. The
[original report](https://github.com/vglazer/USRA/blob/master/subgraph_finding/doc/sub_search.md)
describes the algorithm in detail.

With `--aspiration` (`aspiration=True` in Python), the swaps involving the two vertices just
swapped, which the restricted search otherwise excludes, are also examined after each move, and
one is taken whenever it would beat the best value found so far. This is the
[`asub_search`](https://github.com/vglazer/USRA/blob/master/subgraph_finding/doc/asub_search.md)
variant. On random test graphs it made no consistent difference, so it is off by default; try it
on searches that stall.

## Python

```python
import subsearch as ss
from subsearch import viz

g = ss.generate("exponential", 100, 0.6, seed=1)
r = ss.search(g, size=30, edges=340, experiments=60, max_moves=2000, trace=True)
print(r.summary())
r.solutions()                                         # distinct vertex sets found
viz.plot_search_summary(r)                            # run-time distribution, conflicts, traces
viz.plot_graph_summary(g, highlight=r.best.vertices)  # degrees, spectrum, adjacency
viz.save_html("result.html", r)                       # interactive viewer; viz.show(r) in Jupyter
```

In Jupyter, graphs and results display as summaries with small charts. See the
[quickstart notebook](examples/quickstart.ipynb) for a full tour.

- **Graphs:** `ss.Graph(n, edges)`, `ss.read(path)`, `g.write(path)`, `g.complement()`,
  `g.induced(vs)`, `g.switch(vs)`, `g.common_neighbourhood(vs)`, `g.layout()`,
  `g.adjacency_matrix()`, `g.laplacian()`.
- **Multi-graph files:** `ss.read_all(path)`, `ss.read(path, index=3)`, `ss.write_all(path, graphs)`.
- **Other libraries:** `subsearch.convert.{to,from}_{networkx,igraph,scipy,numpy}`.
- **Tuning:** `ss.sweep(g, size, edges, "div_dur", [0, 4, 16])` with `viz.plot_sweep`.
- **Spectra:** `viz.laplacian_spectrum` (dense, or the `k` smallest via SciPy for large graphs)
  and `viz.spectral_summary`.
- **Results from the CLI:** `ss.SearchResult.load("result.json", g)`.

## Compatibility with the original programs

`subsearch legacy sub-search`, `subsearch legacy asub-search` and `subsearch legacy ggen` (and
`ss.legacy` in Python) read exactly what `sub_search`, `asub_search` and `ggen` read on stdin and
print exactly what they printed. Only
the CPU time and host name on `sub_search`'s last line can differ. Elsewhere, `--rng legacy`
gives the same graphs and search runs through the modern interfaces:

```sh
echo "2 100 0 600 1 0 0 0" | subsearch legacy ggen                         # as ggen
subsearch generate exponential -n 100 -d 0.6 -s 1 --rng legacy -o g.g6   # the same graph, as graph6
```

The fixtures in `tests/fixtures` were recorded from the C programs (see `scripts/make_fixtures.sh`)
and cover all of `ggen`'s graph types and operations, every `sub_search` and `asub_search`
output mode, fixed vertices, edge cases and error handling.

The only differences are on invalid input: the port reports problems the C programs silently
mishandled (out-of-range or duplicate vertices, truncated input), and `sub_search` input may not
contain self-loops.

### Performance

On a single search, the port takes about 1.35× the C program's CPU time. It keeps the adjacency
matrix as a bitset, an eighth of the C program's memory. In normal use experiments run in
parallel: 400 experiments of up to 3,000 moves on a 100-vertex graph take 0.11 s, against the C
program's 0.95 s, on a 16-core laptop. `--rng legacy` is always sequential, to reproduce the C
program's single random stream.

## Repository layout

| Path | Contents |
| --- | --- |
| `crates/subsearch-core` | the library: graph, search, generators, operations, formats, layout, legacy emulation |
| `crates/subsearch-cli` | the `subsearch` command and the golden-fixture tests |
| `crates/subsearch-py` | PyO3 bindings (`subsearch._core`) |
| `python/subsearch` | the Python package: API, conversions, visualisation |
| `tests/fixtures` | output recorded from the original C programs |
| `examples` | the quickstart notebook and the script that builds it |
| `docs` | format guide |

## Development

```sh
cargo test --release                                  # Rust unit, doc and golden-fixture tests
uv run pytest                                         # Python tests (build the CLI first for the round-trip test)
uv run python examples/make_quickstart.py             # rebuild and re-execute the notebook
USRA_DIR=~/repos/USRA scripts/make_fixtures.sh        # re-record fixtures from the C code
```

## Roadmap

`sub_search`, `asub_search` (`--aspiration`) and `ggen` are done. The other USRA programs share
the same engine and come next:

- `wsub_search`: weighted edges, with `wggen` and a weighted format;
- `reg_search` and `ereg_search`: regular induced subgraphs of a given degree.

## License

MIT. The algorithms are those of the original USRA programs (2002).
