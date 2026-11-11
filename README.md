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
| [Quickstart notebook](https://github.com/vglazer/subgraphsearch/blob/main/examples/quickstart.ipynb) | a hands-on tour of every kind of search and its applications |
| [ALGORITHM.md](ALGORITHM.md) | how the search works: moves, perturbations, variants, data structures |
| [TUNING.md](TUNING.md) | which search to use for a goal, and how to set its parameters |
| [GRAPHS.md](GRAPHS.md) | random graphs, operations, graphs from vectors and designs, file formats |
| [PORTING.md](PORTING.md) | the original C programs, and how to reproduce their runs exactly |
| [DEVELOPMENT.md](DEVELOPMENT.md) | building, testing and releasing |

## Python

```python
import subgraphsearch as ss
from subgraphsearch import viz

g = ss.generate("exponential", 100, 0.6, seed=1)   # a random graph: 100 vertices, density 0.6

r = ss.search(g, size=11, edges=55)    # 11-cliques
r = ss.search(g, size=30, edges=340)   # 30 vertices spanning exactly 340 edges
r = ss.search(g, size=10, degree=4)    # 4-regular induced subgraphs
big = ss.largest(g, "clique")          # the largest clique the search can find

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

subgraphsearch is one general search: it finds any number of vertices and edges, total weights,
and the shapes above with the same method. Below it is set beside the methods one would otherwise
use, on problems with public benchmarks or known records. Times are wall-clock on an Apple M3 Max
laptop with 16 cores; subgraphsearch's are for all its experiments together.

![Left: CP-SAT's time to find k vertices spanning exactly m edges grows to seconds and then past its 10-second limit as random graphs grow from 20 to 59 vertices. Middle: subgraphsearch finds the same targets in about a millisecond or less. Right: on eight maximum diversity benchmark instances, subgraphsearch reaches every best known value, while CP-SAT after 10 seconds falls up to 8% short](https://raw.githubusercontent.com/vglazer/subgraphsearch/main/assets/comparison.png)

### Exact edge counts

No specialized solver finds $`k`$ vertices spanning exactly $`m`$ edges, so the alternative is a
general constraint solver. Here it is OR-Tools'
[CP-SAT](https://developers.google.com/optimization/cp/cp_solver), with one yes-or-no variable per
vertex and the edge count as half the sum of the chosen vertices' degrees inside the set, on 16
cores and stopped after 10 seconds. The graphs are random, with density 0.6; the target is the most
edges subgraphsearch finds among 30% of the vertices, the hardest kind of target.

| Graph | Target | subgraphsearch | CP-SAT |
| --- | --- | --- | --- |
| 20 vertices | 6 vertices, 15 edges | 0.13&nbsp;ms | 25&nbsp;ms |
| 29 vertices | 9 vertices, 34 edges | 0.13&nbsp;ms | 0.41&nbsp;s |
| 38 vertices | 11 vertices, 52 edges | 0.15&nbsp;ms | 4.5&nbsp;s |
| 44 vertices | 13 vertices, 70 edges | 0.17&nbsp;ms | nothing in 10&nbsp;s |
| 50 vertices | 15 vertices, 91 edges | 0.83&nbsp;ms | 8.8&nbsp;s |
| 59 vertices | 18 vertices, 129 edges | 0.31&nbsp;ms | nothing in 10&nbsp;s |

subgraphsearch ran 60 experiments of up to 2,000 moves, and every one succeeded. What CP-SAT can do
and subgraphsearch cannot is prove a target impossible: one edge more on the 30-vertex graph of the
same kind, CP-SAT proves in 0.9 s that none exists.

### Maximum diversity

The [maximum diversity problem](https://grafo.etsii.urjc.es/optsicom/mdp.html) picks $`k`$ of
$`n`$ items so that the total distance between the chosen ones is as large as possible: a weighted
search, with the items as vertices and the distances as edge weights. Its standard benchmark,
MDPLIB, lists best known values found by two-hour runs of a scatter search made for the problem;
the specialist heuristics that lead on it, such as iterated tabu search, reach them too. Here
subgraphsearch aims at each best known value with 60 experiments of up to 20,000 moves and
`kick_size=16`, and CP-SAT maximizes the total distance for 10 seconds.

| Items | Chosen | Best known | subgraphsearch | CP-SAT after 10&nbsp;s |
| --- | --- | --- | --- | --- |
| 100 | 10 | 333 | 60 of 60 in 2&nbsp;ms | 329 |
| 100 | 20 | 1,195 | 60 of 60 in 16&nbsp;ms | 1,171 |
| 100 | 30 | 2,457 | 60 of 60 in 10&nbsp;ms | 2,457 |
| 100 | 40 | 4,142 | 60 of 60 in 3&nbsp;ms | 4,127 |
| 200 | 40 | 4,450 | 56 of 60 in 183&nbsp;ms | 4,263 |
| 300 | 60 | 9,689 | 59 of 60 in 0.42&nbsp;s | 9,297 |
| 400 | 40 | 4,658 | 39 of 60 in 0.70&nbsp;s | 4,374 |
| 400 | 80 | 16,956 | 41 of 60 in 1.1&nbsp;s | 15,595 |

CP-SAT's proved upper bounds lie 17% to 60% above the best known values, so it cannot show that any
of them is optimal either.

### Maximum cliques

Maximum cliques have specialists. On eight graphs of the
[DIMACS benchmark](https://networkrepository.com/dimacs.php), subgraphsearch aims at each known
clique number with 32 experiments of up to 20,000 moves. Beside it are DLS-MC, a local search made
for maximum cliques, from its published results (W. Pullan and H. H. Hoos, *Journal of Artificial
Intelligence Research* 25, 2006: mean time per run on one core of a 2.2 GHz Pentium IV, several
times slower than today's processors), and igraph's exact clique number, stopped after 90 seconds.

| Graph | Vertices | Largest clique | subgraphsearch | DLS-MC | igraph (exact) |
| --- | --- | --- | --- | --- | --- |
| C125.9 | 125 | 34 | 32 of 32 in 3.5&nbsp;ms | 0.1&nbsp;ms | over 90&nbsp;s |
| C250.9 | 250 | 44 | 32 of 32 in 64&nbsp;ms | 0.9&nbsp;ms | over 90&nbsp;s |
| p_hat300-3 | 300 | 36 | 32 of 32 in 37&nbsp;ms | 0.7&nbsp;ms | over 90&nbsp;s |
| MANN_a27 | 378 | 126 | 5 of 32 in 185&nbsp;ms | 48&nbsp;ms | over 90&nbsp;s |
| hamming8-4 | 256 | 16 | 32 of 32 in 2.4&nbsp;ms | under 0.1&nbsp;ms | 18&nbsp;s |
| keller4 | 171 | 11 | 32 of 32 in 1.0&nbsp;ms | under 0.1&nbsp;ms | 1.8&nbsp;s |
| brock200_2 | 200 | 12 | 11 at most | 24&nbsp;ms | 0.3&nbsp;s |
| brock400_2 | 400 | 29 | 25 at most | 0.48&nbsp;s | over 90&nbsp;s |

subgraphsearch is far faster than a general exact search, but a solver made for cliques is faster
still, and it solves the *brock* graphs, built to hide their largest clique from local search,
where subgraphsearch ends on a smaller one.

### Snake-in-the-box

The longest induced paths in the $`n`$-dimensional hypercube, *snakes*, are a long-studied search
problem, with records set by specialized searches.

| Hypercube | Longest known snake | subgraphsearch |
| --- | --- | --- |
| 7-cube | 51 vertices, the longest possible | 51, in 313 of 400 experiments, 0.5&nbsp;s |
| 8-cube | 99 vertices, the longest possible | 99, in 3 of 400 experiments, 21&nbsp;s |
| 9-cube | 192 vertices (2026) | 185 in under 3 minutes; 189 through a fixed 95-vertex snake in one half, in 36&nbsp;s |

It reaches the records up to the 8-cube with its general path search, and comes within 3 vertices
of the 9-cube's.

### In short

subgraphsearch is the tool for subgraphs with a prescribed number of vertices and edges or a
prescribed total weight: there it is fast where general solvers stall, and on maximum diversity it
matches the best published values. For maximum cliques, a dedicated clique solver is the better
choice. The [quickstart notebook](https://github.com/vglazer/subgraphsearch/blob/main/examples/quickstart.ipynb)
runs several of these searches.

## Applications

Many combinatorial objects are induced subgraphs of the right graph, so one search finds them all.

- **Choosing diverse items.** Picking $`k`$ of $`n`$ items so that the total distance between the
  chosen ones is as large as possible, the
  [maximum diversity problem](https://grafo.etsii.urjc.es/optsicom/mdp.html), is a weighted search:
  the items are vertices and the distances edge weights. See
  [How it compares](#maximum-diversity) for how it fares on the standard benchmark.
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
  the 8-cube (see [How it compares](#snake-in-the-box) and
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
results depend only on the seed. [ALGORITHM.md](ALGORITHM.md) describes the method in full, and
[TUNING.md](TUNING.md) how to choose a search and its settings.

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
