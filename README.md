# subgraphsearch

[![CI](https://github.com/vglazer/subgraphsearch/actions/workflows/ci.yml/badge.svg)](https://github.com/vglazer/subgraphsearch/actions/workflows/ci.yml)

Find **induced subgraphs with a prescribed number of vertices and edges**: cliques,
independent sets, and everything in between. `subgraphsearch` is a Rust port, with a Python package,
of `sub_search` from the [USRA stochastic graph search project](https://github.com/vglazer/USRA/tree/master/subgraph_finding),
together with its graph generator `ggen`.

- **Faithful.** A `legacy` mode reproduces the original C programs byte for byte (same seed,
  same output), checked against fixtures recorded from them.
- **Fast.** Experiments run in parallel; a typical search is about 9× faster than the C program
  on a multi-core machine.
- **Standard formats.** graph6, sparse6, Matrix Market, edge lists, DOT and GraphML, plus the
  original adjacency lists; see [Graph formats](#graph-formats).
- **Visual.** Search diagnostics, spectral and structural plots, Jupyter displays, and a
  self-contained interactive HTML viewer for graphs and the subgraphs found in them.

No other Python or Rust library searches for induced subgraphs with an exact (or near-exact) edge
count; they cover only the extreme cases (cliques, independent sets) or densest-subgraph variants.

## Install

```sh
pip install subgraphsearch              # the Python package; add [all] for plotting and interop
cargo install subgraphsearch            # the `subgraphsearch` command (needs Rust 1.87+)
```

Prebuilt wheels cover Linux, macOS and Windows for Python 3.10 and later. From a checkout,
`cargo install --path crates/subgraphsearch-cli` and `uv sync --all-extras` build both from source.

## Command line

```sh
# a random graph: 100 vertices, density 0.6 (graph6 output)
subgraphsearch generate exponential -n 100 -d 0.6 -s 1 -o g.g6

# search for 11-cliques: 11 vertices, 55 edges
subgraphsearch search g.g6 -k 11 -m 55 -n 20 --moves 1000

# also report near misses within 2 edges, and save everything (with per-move traces) as JSON
subgraphsearch search g.g6 -k 30 -m 340 -n 60 --max-conflict 2 --trace --json result.json

# look at the results
uv run python -m subgraphsearch explore g.g6 --result result.json -o result.html   # interactive page
uv run python -m subgraphsearch report g.g6 --result result.json -o result.png     # figures

# regular subgraphs (USRA's reg_search / ereg_search): every vertex with degree 4 inside
subgraphsearch search g.g6 -k 10 --degree 4 -n 40
subgraphsearch search g.g6 -k 10 --degree 4 --two-phase -n 40   # right edge count first

# weighted graphs (USRA's wsub_search / wggen): -m is a total weight
subgraphsearch generate weighted -n 200 -d 0.65 -s 1 -o w.mtx   # mean weight 0.65 per vertex pair
subgraphsearch search w.mtx --weighted -k 15 -m 180 -n 40
subgraphsearch info w.mtx --weighted

# files holding many graphs (graph6/sparse6, one per line, e.g. from nauty's geng)
subgraphsearch info many.g6                                  # one line per graph
subgraphsearch search many.g6 --all -k 10 -m 30 -q           # search every graph
subgraphsearch search many.g6 --index 3 -k 10 -m 30          # just graph 3 (from 0)

# graphs: statistics, conversion, operations
subgraphsearch info g.g6 --degrees
subgraphsearch convert g.g6 g.dot --highlight 3,17,42
subgraphsearch op complement g.g6 gc.s6
subgraphsearch op common-neighbours g.g6 nbhd.g6 --vertices 3,17
```

`subgraphsearch search` exits with status 2 when no experiment succeeds, so scripts can tell "nothing
found" from success.

### Search parameters

| Option | Meaning | Default |
| --- | --- | --- |
| `-k`, `--vertices` | subgraph vertices, including `--fixed` ones | required |
| `-m`, `--edges` | subgraph edges | required, unless `--degree` |
| `-d`, `--degree` | instead: subgraphs regular of this degree, as in USRA's `reg_search` | |
| `--two-phase` | with `--degree`: reach the right edge count first, as in `ereg_search` | off |
| `--fixed` | vertices the subgraph must contain | none |
| `-n`, `--experiments` | independent restarts | 10 |
| `--moves` | maximum moves (vertex swaps) per experiment | 1000 |
| `--div-freq`, `--div-dur` | make `div-dur` random moves at the start of every `div-freq` moves | 250, 4 |
| `-s`, `--seed`, `--rng` | random seed; `xoshiro` (default) or `legacy` | 1, xoshiro |
| `--aspiration` | limited aspiration, as in USRA's `asub_search` (see below) | off |
| `--weighted` | edges carry weights and `-m` is a total weight, as in USRA's `wsub_search` | off |

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
import subgraphsearch as ss
from subgraphsearch import viz

g = ss.generate("exponential", 100, 0.6, seed=1)
r = ss.search(g, size=30, edges=340, experiments=60, max_moves=2000, trace=True)
print(r.summary())
r.solutions()                                         # distinct vertex sets found
viz.plot_search_summary(r)                            # run-time distribution, conflicts, traces
viz.plot_graph_summary(g, highlight=r.best.vertices)  # degrees, spectrum, adjacency
viz.save_html("result.html", r)                       # interactive viewer; viz.show(r) in Jupyter
```

In Jupyter, graphs and results display as summaries with small charts. See the
[quickstart notebook](https://github.com/vglazer/subgraphsearch/blob/main/examples/quickstart.ipynb) for a full tour.

- **Graphs:** `ss.Graph(n, edges)`, `ss.read(path)`, `g.write(path)`, `g.complement()`,
  `g.induced(vs)`, `g.switch(vs)`, `g.common_neighbourhood(vs)`, `g.layout()`,
  `g.adjacency_matrix()`, `g.laplacian()`.
- **Regular subgraphs:** `ss.search(g, size, degree=4)`, and `two_phase=True` for `ereg_search`.
- **Weighted graphs:** `ss.WeightedGraph(n, [(u, v, w), ...])`, `ss.generate_weighted(n, mean_weight)`,
  `ss.WeightedGraph.read(path)`; `ss.search` on one targets a total weight.
- **Multi-graph files:** `ss.read_all(path)`, `ss.read(path, index=3)`, `ss.write_all(path, graphs)`.
- **Other libraries:** `subgraphsearch.convert.{to,from}_{networkx,igraph,scipy,numpy}`.
- **Tuning:** `ss.sweep(g, size, edges, "div_dur", [0, 4, 16])` with `viz.plot_sweep`.
- **Spectra:** `viz.laplacian_spectrum` (dense, or the `k` smallest via SciPy for large graphs)
  and `viz.spectral_summary`.
- **Results from the CLI:** `ss.SearchResult.load("result.json", g)`.

## Graph formats

USRA's programs exchanged graphs as '-1'-terminated adjacency lists: line *i* lists the neighbours
*j > i* of vertex *i*. The format is compact and simple, but nothing else reads it. `subgraphsearch`
still reads and writes it (`lists`, `.txt`) for compatibility, and adds the standard formats below.

| Format | Name | Extensions | Read | Write | Best for |
| --- | --- | --- | --- | --- | --- |
| graph6 | `graph6` | `.g6` | ✓ | ✓ | dense or medium graphs (one or many per file); the default output |
| sparse6 | `sparse6` | `.s6` | ✓ | ✓ | sparse graphs (one or many per file) |
| Matrix Market | `mtx` | `.mtx` | ✓ | ✓ | SciPy, MATLAB, Julia, numerical code |
| edge list | `edgelist` | `.edges`, `.csv`, `.tsv` | ✓ | ✓ | spreadsheets, shell tools, anything |
| USRA lists | `lists` | `.txt` | ✓ | ✓ | the original C programs |
| Graphviz | `dot` | `.dot`, `.gv` | | ✓ | drawing with Graphviz, highlighting a subgraph |
| GraphML | `graphml` | `.graphml` | | ✓ | Gephi, Cytoscape, yEd |
| USRA weighted lists | `wlists` | (name it: `--from wlists`) | ✓ | ✓ | the original `wggen` and `wsub_search` |

### Sizes

The same graphs in each format (generated with `subgraphsearch generate ... -s 1`):

| Graph | Edges | graph6 | sparse6 | USRA lists | edge list | Matrix Market | GraphML |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| exponential, 100 vertices, density 0.6 | 2,969 | **0.8 KB** | 3.9 KB | 9.0 KB | 16.8 KB | 17.0 KB | 112 KB |
| geometric, 790 vertices, 0.15 | 38,440 | **51 KB** | 69 KB | 152 KB | 290 KB | 290 KB | 1.5 MB |
| power, 2,500 vertices, 0.2 | 615,771 | **508 KB** | 1.3 MB | 2.9 MB | 5.5 MB | 5.5 MB | 24 MB |
| exponential, 5,000 vertices, 0.002 | 25,028 | 2.0 MB | **58 KB** | 136 KB | 234 KB | 234 KB | 1.1 MB |

graph6 stores one bit per vertex pair, so its size depends only on the number of vertices
(`n(n-1)/12` bytes). sparse6 stores roughly `log2(n)` bits per edge. As a rule of thumb, graph6 is
smaller once the density exceeds about `log2(n) / n`: around 7% at 100 vertices, 0.25% at 5,000.

### Recommendations

- **Use graph6** for the dense, medium-sized graphs `sub_search` is typically run on. It is the
  standard in combinatorics: nauty's `geng` produces it; SageMath, networkx and igraph read it;
  and collections such as [House of Graphs](https://houseofgraphs.org/) and Brendan McKay's and
  Ted Spence's lists of strongly regular graphs are distributed in it.
- **Use sparse6** for large sparse graphs.
- **Keep families of graphs in one graph6 or sparse6 file**, one graph per line, which is how
  `geng` and graph catalogues deliver them. `subgraphsearch info many.g6` lists them,
  `subgraphsearch search many.g6 --all ...` searches each in turn, `--index N` picks one, and
  `subgraphsearch convert many.g6 many.s6` converts them all. In Python: `ss.read_all`,
  `ss.read(path, index=N)` and `ss.write_all`. A command that needs one graph refuses a
  multi-graph file unless given `--index`, rather than silently using the first graph.
- **Use an edge list** for exchange with tools that know nothing about graphs. `subgraphsearch` writes
  a `# vertices: N` header, so isolated vertices survive the round trip.
- **Use Matrix Market** to move adjacency matrices into numerical code
  (`scipy.io.mmread`). The reader accepts `pattern`, `integer` and `real` matrices, `general` or
  `symmetric`; any non-zero off-diagonal entry is an edge.
- **Use DOT or GraphML** to draw. Both take a vertex set to highlight
  (`subgraphsearch convert g.g6 g.dot --highlight 3,17,42`), which colours a found subgraph.

### Weighted graphs

Edge weights (integers from 1 to 255) are read and written with `--weighted` on the command line,
or as a `WeightedGraph` in Python. They are stored as:

- the **third column of an edge list** (`u v w`; a missing third column means weight 1);
- the **values of a Matrix Market file** (written as `integer symmetric`; `pattern` files read as
  weight 1, and non-integer values are refused);
- **USRA weighted lists** (`wlists`), the encoding `wggen` writes and `wsub_search` reads: entry
  `x` in row `i` is the edge `{i, x % n}` with weight `x / n + 1`. USRA used `.txt` for these as
  well as for unweighted lists, so name the format explicitly.

DOT output labels edges with their weights and scales their widths; GraphML output adds a
`weight` edge attribute. graph6, sparse6 and unweighted lists cannot hold weights: they read as
weight 1, and writing a weighted graph to them is an error rather than silently losing the weights.

### Not supported, and why

- **JSON node-link** (d3, networkx `node_link_data`): use `subgraphsearch.convert.to_networkx` and
  networkx's own JSON support. networkx is changing its default key from `links` to `edges`, so a
  fixed writer here would soon mismatch.
- **Reading DOT and GraphML** would need full parsers for formats that are mostly used for
  output; convert with networkx (`read_graphml`, `nx_pydot.read_dot`) and
  `subgraphsearch.convert.from_networkx` instead.

## Compatibility with the original programs

`subgraphsearch legacy sub-search`, `asub-search`, `wsub-search`, `reg-search`, `ereg-search`, `ggen`
and `wggen` (and `ss.legacy` in Python) read exactly what the C programs of the same names read, on stdin or, for `wggen`, the
command line, and print exactly what they printed. Only
the CPU time and host name on `sub_search`'s last line can differ. Elsewhere, `--rng legacy`
gives the same graphs and search runs through the modern interfaces:

```sh
echo "2 100 0 600 1 0 0 0" | subgraphsearch legacy ggen                         # as ggen
subgraphsearch generate exponential -n 100 -d 0.6 -s 1 --rng legacy -o g.g6   # the same graph, as graph6
```

The fixtures in `tests/fixtures` were recorded from the C programs (see `scripts/make_fixtures.sh`)
and cover all of `ggen`'s graph types and operations, `wggen`, every output mode of
`sub_search`, `asub_search`, `wsub_search`, `reg_search` and `ereg_search` (including a strongly
regular Paley graph), fixed vertices, edge cases and error handling.

The only differences are on invalid input: the port reports problems the C programs silently
mishandled (out-of-range or duplicate vertices, truncated input), and search input may not
contain self-loops. Two quirks of `ereg_search` are kept, because they change its results: its
first-phase edge target is computed from the number of *non-fixed* subgraph vertices, so with
`--fixed` it aims at the wrong edge count and rarely succeeds (plain `--degree` does not have this
problem); and its partitioned neighbour lists carry over between experiments, which changes only
the order in which solutions' adjacency lists are printed.

### Performance

On a single search, the port takes about 1.5× the C program's CPU time (1.43 s against 0.96 s
on the benchmark below, built with Rust 1.99; it was 1.35× with Rust 1.89). It keeps the adjacency
matrix as a bitset, an eighth of the C program's memory. In normal use experiments run in
parallel: 400 experiments of up to 3,000 moves on a 100-vertex graph take 0.11 s, against the C
program's 0.95 s, on a 16-core laptop. `--rng legacy` is always sequential, to reproduce the C
program's single random stream.

## Repository layout

| Path | Contents |
| --- | --- |
| `crates/subgraphsearch-core` | the library: graph, search, generators, operations, formats, layout, legacy emulation |
| `crates/subgraphsearch-cli` | the `subgraphsearch` command and the golden-fixture tests |
| `crates/subgraphsearch-py` | PyO3 bindings (`subgraphsearch._core`) |
| `python/subgraphsearch` | the Python package: API, conversions, visualisation |
| `tests/fixtures` | output recorded from the original C programs |
| `examples` | the quickstart notebook and the script that builds it |

## Development

```sh
cargo test --release                                  # Rust unit, doc and golden-fixture tests
uv run pytest                                         # Python tests (build the CLI first for the round-trip test)
uv run python examples/make_quickstart.py             # rebuild and re-execute the notebook
USRA_DIR=~/repos/USRA scripts/make_fixtures.sh        # re-record fixtures from the C code
```

## Roadmap

All seven USRA programs are ported: `sub_search`, `asub_search` (`--aspiration`), `wsub_search`
(`--weighted`), `reg_search` (`--degree`), `ereg_search` (`--degree --two-phase`), `ggen` and
`wggen`. Possible next steps: publishing to PyPI and crates.io, a time limit and progress output
for long searches, and closing the single-threaded speed gap with the C program.

## License

MIT. The algorithms are those of the original USRA programs (2002).
