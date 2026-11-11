# subgraphsearch

[![CI](https://github.com/vglazer/subgraphsearch/actions/workflows/ci.yml/badge.svg)](https://github.com/vglazer/subgraphsearch/actions/workflows/ci.yml)
[![PyPI](https://img.shields.io/pypi/v/subgraphsearch)](https://pypi.org/project/subgraphsearch/)
[![crates.io: subgraphsearch](https://img.shields.io/crates/v/subgraphsearch?label=crates.io%3A%20subgraphsearch)](https://crates.io/crates/subgraphsearch)
[![crates.io: subgraphsearch-core](https://img.shields.io/crates/v/subgraphsearch-core?label=crates.io%3A%20subgraphsearch-core)](https://crates.io/crates/subgraphsearch-core)

Find **[induced subgraphs](https://en.wikipedia.org/wiki/Induced_subgraph) with a prescribed number of
vertices and edges** in a graph, or with a given total weight in a weighted graph. Searches are
[fast](#how-it-compares), reproducible from a seed and come with diagnostics for tuning them, and
near misses are reported alongside exact solutions, so the search also works as an approximation
algorithm. Many [graph formats](GRAPHS.md#formats) are supported.

On these targets subgraphsearch finds solutions in milliseconds where general solvers take seconds
or give out, and on the [maximum diversity problem](https://grafo.etsii.urjc.es/optsicom/mdp.html),
choosing items as different from each other as possible, it reaches the best published values in
about a second (see [How it compares](#how-it-compares)).

The same search finds induced [cycles](https://en.wikipedia.org/wiki/Cycle_(graph_theory)),
[paths](https://en.wikipedia.org/wiki/Induced_path), [trees](https://en.wikipedia.org/wiki/Tree_(graph_theory))
and [cliques](https://en.wikipedia.org/wiki/Clique_(graph_theory)), regular subgraphs of a given
degree, and subgraphs through given vertices.

![One random graph with 36 vertices and 156 edges, searched four times and drawn with viz.plot_graph: an induced 12-cycle, an induced path on 15 vertices, an induced tree on 16 vertices and a 5-clique, each drawn in its own shape with the rest of the graph around it](https://raw.githubusercontent.com/vglazer/subgraphsearch/main/assets/overview.png)

## Install

subgraphsearch is [on PyPI](https://pypi.org/project/subgraphsearch/) and requires Python 3.10 or
later and NumPy 1.24 or later. With [uv](https://docs.astral.sh/uv/):

```sh
uv add subgraphsearch                # add the package to your project
uv add "subgraphsearch[all]"         # also plotting and conversions to networkx, igraph and SciPy
uv tool install subgraphsearch       # just the `subgraphsearch` command, on your PATH
uvx subgraphsearch info g.g6         # or run the command without installing it
```

With pip:

```sh
pip install subgraphsearch           # the package and the `subgraphsearch` command
pip install "subgraphsearch[all]"    # also plotting and conversions to networkx, igraph and SciPy
pipx install subgraphsearch          # just the command, in its own environment
pipx run subgraphsearch info g.g6    # or run the command without installing it
```

Prebuilt wheels cover Linux (including Alpine), macOS and Windows (including ARM), so installing
needs no compiler.

To copy the quickstart notebook into the current directory and open it in JupyterLab:

```sh
uv run --with "subgraphsearch[all]" --with jupyterlab python -m subgraphsearch quickstart   # with uv
pip install "subgraphsearch[all]" jupyterlab && python -m subgraphsearch quickstart       # with pip
```

## Documentation

| Document | What it covers |
| --- | --- |
| [Quickstart notebook](https://github.com/vglazer/subgraphsearch/blob/main/examples/quickstart.ipynb) | A hands-on tour of every kind of search and its applications |
| [BENCHMARKS.md](BENCHMARKS.md) | How subgraphsearch compares with other methods on standard benchmarks |
| [ALGORITHM.md](ALGORITHM.md) | How the search works: moves, perturbations, variants, data structures |
| [TUNING.md](TUNING.md) | Which search to use for a goal, and how to set its parameters |
| [GRAPHS.md](GRAPHS.md) | Random graphs, operations, graphs from vectors and designs, file formats |
| [PORTING.md](PORTING.md) | The original C programs, and how to reproduce their runs exactly |
| [DEVELOPMENT.md](DEVELOPMENT.md) | Building, testing and releasing |

## Python

```python
import subgraphsearch as ss
from subgraphsearch import viz

g = ss.generate("exponential", 100, 0.6, seed=1)   # a random graph: 100 vertices, density 0.6

r = ss.search(g, size=11, edges=55)    # 11-cliques
r = ss.search(g, size=30, edges=340)   # 30 vertices spanning exactly 340 edges
r = ss.search(g, size=10, degree=4)    # 4-regular induced subgraphs
big = ss.largest(g, "clique")          # the largest clique the search can find
dense = ss.densest(g, 30)              # the 30 vertices spanning the most edges

print(r.summary())
r.solutions()                                  # the distinct vertex sets found
viz.plot_graph(g, highlight=r.best.vertices)   # draw one
viz.plot_search_summary(r)                     # how the search went
viz.save_html("result.html", r)                # an interactive viewer; viz.show(r) in Jupyter
```

In Jupyter, graphs and results display as summaries with small charts.

- **Searches:** a number of edges (`edges=`), a regular subgraph (`degree=`), an induced path
  (`path=True`, between two given vertices with `endpoints=`), a total weight in a weighted
  graph; any of them connected (`connected=True`) or through given vertices (`fixed=`), with
  near misses alongside exact solutions. `ss.largest` finds the largest induced path, cycle or
  tree, clique or independent set the search can reach.
- **Graphs:** from edges, files, other libraries (networkx, igraph, SciPy, NumPy), random models,
  hypercubes and grids, incidence structures such as block designs, and sets of vectors or binary
  words joined by their inner products or Hamming distances (`ss.Graph.from_vectors`,
  `ss.Graph.from_words`); complements, induced subgraphs, Seidel switching, and products of
  graphs (Cartesian, tensor, strong, lexicographic, replacement and zig-zag).
- **Seeing what happened:** drawings of graphs and the subgraphs found, diagnostics of a search,
  spectra, and parameter sweeps for tuning.

The [quickstart notebook](https://github.com/vglazer/subgraphsearch/blob/main/examples/quickstart.ipynb)
goes through all of these.

## How it compares

![Left: CP-SAT's time to find k vertices spanning exactly m edges grows to seconds and then past its 10-second limit as random graphs grow from 20 to 59 vertices. Middle: subgraphsearch finds the same targets in about a millisecond or less. Right: on eight maximum diversity benchmark instances, subgraphsearch reaches every best known value, while CP-SAT after 10 seconds falls up to 8% short](https://raw.githubusercontent.com/vglazer/subgraphsearch/main/assets/comparison.png)

- **Exact edge counts:** milliseconds, where general solvers (CP-SAT, and the MIP solvers SCIP,
  HiGHS and CBC) need seconds at a few dozen vertices and mostly find nothing within 10 seconds
  past 40.
- **Maximum diversity:** the best published values on the MDPLIB benchmark in about a second, where
  the best general solver, CP-SAT, falls up to 9% short and the MIP solvers up to 38%.
- **Snake-in-the-box:** the known records up to the 8-cube, and within 3 vertices of the 9-cube's.
- **Maximum cliques:** far faster than a general exact search; a dedicated clique solver is faster
  still.

[BENCHMARKS.md](BENCHMARKS.md) has the tables, settings and sources.

## Applications

Many combinatorial objects are induced subgraphs of the right graph, so one search finds them all.

- **Choosing diverse items.** Picking $`k`$ of $`n`$ items so that the total distance between the
  chosen ones is as large as possible, the
  [maximum diversity problem](https://grafo.etsii.urjc.es/optsicom/mdp.html), is a weighted search:
  the items are vertices and the distances edge weights. See
  [BENCHMARKS.md](BENCHMARKS.md#maximum-diversity) for how it fares on the standard benchmark.
- **Error-correcting codes.** A binary code of minimum distance $`d`$ is a clique in the graph of
  words joined when they differ in at least $`d`$ places.
  [examples/codes.py](examples/codes.py) finds codes of the largest possible size for lengths 9 to
  12, among them the 72-word code of length 11 and distance 4 in about 3 seconds.
- **Strongly regular graphs.** Switching a strongly regular graph with respect to a regular
  subgraph the search finds can give a new one with the same parameters: the Shrikhande graph from
  the 4×4 rook's graph, and the three Chang graphs from the triangular graph $`T(8)`$.
- **Spectral bounds.** Eigenvalues bound how large an independent set (Hoffman) or a clique
  (Delsarte) can be, and more generally how many edges any $`k`$ vertices can span: the expander
  mixing lemma, and Cheeger's inequality on expansion. The search finds sets that meet these
  bounds in the rook's graph, the triangular graphs and the hypercubes, so the bounds are sharp
  there; finds the sparse cuts a small spectral gap promises in a poor expander; and in good
  expanders, random regular and Paley graphs, shows how close to random every set must be.
- **Hadamard matrices.** The rows of a Hadamard matrix are pairwise orthogonal ±1 vectors, so it
  is a clique in the graph of such vectors joined when orthogonal.
  [examples/hadamard.py](examples/hadamard.py) finds matrices of orders 8 to 20 this way, the order-20
  graph having 31,752 vertices and 169 million edges.
- **Snakes and coils in hypercubes.** The longest induced paths and cycles in the $`n`$-cube are the
  snake-in-the-box and coil-in-the-box problems; the path search reaches the known records up to
  the 8-cube (see [BENCHMARKS.md](BENCHMARKS.md#snake-in-the-box) and
  [examples/snake_9cube.py](examples/snake_9cube.py)).
- **Equiangular lines.** Lines whose vectors have inner products ±c form a clique; the search
  finds the 16 equiangular lines in 6 dimensions, the most there can be.

The quickstart notebook works through each of these.

## How the search works

subgraphsearch is a randomized tabu search with perturbations. Each move swaps one subgraph vertex
for one outside vertex, chosen among the swaps that bring the edge count (or total weight) closest
to the target; recently swapped vertices are tabu, so the search does not cycle; and every few
hundred moves a perturbation, random moves or a *region kick* that rebuilds the part of the subgraph
nearest a random vertex, moves it out of local minima. Independent experiments run in parallel, and
results depend only on the seed. There is no fixed limit on the size of a graph; memory is the
limit, mostly one bit per pair of vertices, about 1.25 GB at 100,000 vertices
([details](ALGORITHM.md#data-structures-and-limits)). [ALGORITHM.md](ALGORITHM.md) describes the
method in full, and [TUNING.md](TUNING.md) how to choose a search and its settings.

## Command line

The `subgraphsearch` command comes with the Python package (see [Install](#install)); with a Rust
toolchain, `cargo install subgraphsearch` builds the same command from
[crates.io](https://crates.io/crates/subgraphsearch).

```sh
# make a graph
subgraphsearch generate exponential -n 100 -d 0.6 -s 1 -o g.g6    # random: 100 vertices, density 0.6
subgraphsearch generate hypercube --dimension 7 -o q7.g6           # or a standard one: the 7-cube

# search it
subgraphsearch search g.g6 -k 11 -m 55                            # 11-cliques
subgraphsearch search g.g6 -k 30 -m 340 --max-conflict 2          # 30 vertices, 340 edges; near misses too
subgraphsearch search g.g6 -k 10 --degree 4                       # 4-regular subgraphs
subgraphsearch search q7.g6 -k 36 --degree 2 --connected          # induced 36-cycles
subgraphsearch search q7.g6 -k 40 --path --endpoints 0,127        # induced 40-vertex paths from 0 to 127
subgraphsearch largest q7.g6 --shape path                         # the longest induced path: 51, the record

# look at the results
subgraphsearch search g.g6 -k 11 -m 55 --json result.json         # save them
python -m subgraphsearch explore g.g6 --result result.json        # explore them in an interactive page
subgraphsearch convert g.g6 g.dot --highlight 3,17,42             # draw a subgraph with Graphviz
```

Other subcommands print statistics (`info`), apply operations such as complements, induced
subgraphs, switching and products (`op`), and build graphs from block designs (`incidence`);
`search --weighted` searches for a total weight, and `search --all` every graph in a file.
`subgraphsearch --help` lists everything.

`subgraphsearch search` exits with status 2 when nothing is found.

| Option | Meaning | Default |
| --- | --- | --- |
| `-k`, `--vertices` | subgraph vertices | required |
| `-m`, `--edges` | subgraph edges (total weight, with `--weighted`) | required, unless `--degree` |
| `-d`, `--degree` | instead, a subgraph regular of this degree | |
| `--path` | instead, an induced path | |
| `--endpoints` | with `--path`, the two vertices the path runs between, e.g. `0,7` | none |
| `--connected` | require a connected subgraph | off |
| `--fixed` | vertices the subgraph must contain | none |
| `-n`, `--experiments` | independent restarts | 40 |
| `--moves` | maximum moves per experiment | 250 |
| `--div-freq`, `--div-dur` | make `div-dur` random moves at the start of every `div-freq` moves | 250, 4 |
| `--kick-size`, `--kick-ban` | region kicks in place of the random moves of every cycle after the first: vertices rebuilt per kick, and moves they stay out (0 turns kicks off) | 4, 20 (no kicks with `--degree`, `--path`, `--rng legacy`) |
| `-s`, `--seed` | random seed | 1 |
| `--weighted` | read edge weights; `-m` is then a total weight | off |

## Rust

The search, generators and formats are also a Rust library,
[`subgraphsearch-core`](https://crates.io/crates/subgraphsearch-core), which needs Rust 1.87 or
later. Add it with `cargo add subgraphsearch-core`; its API is documented on
[docs.rs](https://docs.rs/subgraphsearch-core).

```rust
use subgraphsearch_core::{Graph, SearchParams, search};

// find induced 4-vertex paths (3 edges) in a 12-cycle
let g = Graph::from_edges(12, (0..12).map(|i| (i, (i + 1) % 12))).unwrap();
let result = search(&g, &SearchParams::new(4, 3)).unwrap();
let found = result.experiments.iter().find(|e| e.success).unwrap();
assert_eq!(g.induced_edge_count(&found.vertices), 3);
```

## License

MIT
