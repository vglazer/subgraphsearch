# subgraphsearch

[![CI](https://github.com/vglazer/subgraphsearch/actions/workflows/ci.yml/badge.svg)](https://github.com/vglazer/subgraphsearch/actions/workflows/ci.yml)
[![PyPI](https://img.shields.io/pypi/v/subgraphsearch)](https://pypi.org/project/subgraphsearch/)
[![crates.io: subgraphsearch](https://img.shields.io/crates/v/subgraphsearch?label=crates.io%3A%20subgraphsearch)](https://crates.io/crates/subgraphsearch)
[![crates.io: subgraphsearch-core](https://img.shields.io/crates/v/subgraphsearch-core?label=crates.io%3A%20subgraphsearch-core)](https://crates.io/crates/subgraphsearch-core)

Find **[induced subgraphs](https://en.wikipedia.org/wiki/Induced_subgraph) with a prescribed number of
vertices and edges** in a graph, or with a given total weight in a weighted graph. Searches are
[fast](#how-it-compares), reproducible from a seed and come with diagnostics for tuning them. Many
[graph formats](#graph-formats) are supported.

Near misses are reported in addition to exact solutions, enabling use as an approximation algorithm.
For exact edge counts, solutions take milliseconds where a general solver such as CP-SAT gives out
at a few dozen vertices. On the [maximum diversity problem](https://grafo.etsii.urjc.es/optsicom/mdp.html), choosing items as different from
each other as possible, the search reaches published best values in about a second (see
[How it compares](#how-it-compares)).

A search can also find induced [cycles](https://en.wikipedia.org/wiki/Cycle_(graph_theory)),
[paths](https://en.wikipedia.org/wiki/Induced_path), [trees](https://en.wikipedia.org/wiki/Tree_(graph_theory))
and [cliques](https://en.wikipedia.org/wiki/Clique_(graph_theory)), as well as regular subgraphs with a
given degree, and can be required to fix particular vertices. Graphs can be built from block designs
and other incidence structures, as well as switched with respect to the subgraphs a search finds.

See the
[quickstart notebook](https://github.com/vglazer/subgraphsearch/blob/main/examples/quickstart.ipynb)
for a complete tour.

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
| `--kick-size`, `--kick-ban` | instead, start every `div-freq` moves after the first by rebuilding this many subgraph vertices nearest a random one, keeping the vertices removed out for `kick-ban` moves; larger sizes can help much more on some problems, and 0 turns kicks off | 4 (0 with `--degree`, `--path` or `--rng legacy`), 20 |
| `-s`, `--seed` | random seed | 1 |
| `--weighted` | read edge weights; `-m` is then a total weight | off |

## Applications

Many combinatorial objects are induced subgraphs of the right graph, so one search finds them all.

- **Choosing diverse items.** Picking $`k`$ of $`n`$ items so that the total distance between the
  chosen ones is as large as possible, the [maximum diversity problem](https://grafo.etsii.urjc.es/optsicom/mdp.html), is a weighted search: the
  items are vertices and the distances edge weights. On the MDPLIB benchmark, subgraphsearch
  reaches the best known values, found by two-hour runs of a scatter search made for the problem,
  in about a second with up to 400 items, using region kicks. OR-Tools' CP-SAT, given 10 seconds,
  falls up to 8% short.
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
  snake-in-the-box and coil-in-the-box problems. The search reaches the known records up to the
  8-cube, and in the 9-cube finds snakes of 189 vertices, 3 short of the record
  ([examples/snake_9cube.py](examples/snake_9cube.py); see [ALGORITHM.md](ALGORITHM.md#variants)).
- **Equiangular lines.** Lines whose vectors have inner products ±c form a clique; the search
  finds the 16 equiangular lines in 6 dimensions, the most there can be.

The quickstart notebook works through each of these.

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

## How the search works

subgraphsearch is a randomized tabu search with perturbations. Each move swaps one subgraph vertex
for one outside vertex, chosen among the swaps that bring the edge count (or total weight) closest
to the target; recently swapped vertices are tabu, so the search does not cycle; and every few
hundred moves a perturbation, random moves or a *region kick* that rebuilds the part of the subgraph
nearest a random vertex, moves it out of local minima. Independent experiments run in parallel, and
results depend only on the seed. [ALGORITHM.md](ALGORITHM.md) describes the method in full: its
moves and perturbations, the variants for weights, regular subgraphs, connected subgraphs and paths,
and its data structures and limits.

## Graphs

Random graphs:


- **exponential**: [Erdős–Rényi](https://en.wikipedia.org/wiki/Erd%C5%91s%E2%80%93R%C3%A9nyi_model);
  each pair of vertices is an edge independently with probability equal to the density. With
  `--mean-degree d` (`mean_degree=d`) instead of a density, the probability is $`d / (n - 1)`$ for
  any $`d`$, and the graph is generated in time proportional to its edges rather than to the pairs
  of vertices: 100,000 vertices with 3 neighbors each take 0.2 s. Densities, in steps of 0.001,
  cannot make such sparse graphs.
- **geometric**: a [random geometric graph](https://en.wikipedia.org/wiki/Random_geometric_graph):
  random points in the unit square, joined when within a distance chosen so that the disc around a
  point covers a fraction of the square equal to the density. There is no wrap-around, so vertices
  near the edges have fewer neighbors and the actual density is lower.
- **power**: each vertex is joined to the same number of randomly chosen higher-numbered vertices.
  Degrees spread more widely than in exponential graphs but have no heavy tail, so despite the name
  these are not [scale-free](https://en.wikipedia.org/wiki/Scale-free_network) graphs. Power and
  geometric graphs only make sense for densities below about 0.5.
- **weighted**: random pairs of vertices have their weight increased by 1 until the mean weight
  per pair is reached, so weights above 1 are common once it approaches 1.
- **regular** (`-n n --degree d`, `ss.random_regular(n, d)`): a random $`d`$-regular graph, close
  to uniform among them; with high probability an expander nearly as good as possible.
- **hypercube** (`--dimension d`, `ss.hypercube(d)`): the
  [hypercube](https://en.wikipedia.org/wiki/Hypercube_graph) on the $`2^d`$ bit strings of length
  $`d`$, joined when they differ in one bit, up to 16 dimensions. Not random: the home of the
  snake-in-the-box and coil-in-the-box problems.
- **grid** (`--rows r --cols c`, `ss.grid(r, c)`): the $`r`$ by $`c`$ grid, vertex $`ic + j`$ in
  row $`i`$ and column $`j`$.

Graph operations help focus a search. Searching the
[complement](https://en.wikipedia.org/wiki/Complement_graph) is equivalent, since $`k`$ vertices
with $`m`$ edges become $`k`$ vertices with $`k(k-1)/2 - m`$, and for a dense graph the complement
is sparse and cheaper to work with. Inducing a subgraph on the common neighborhood of some vertices
restricts the search to vertices adjacent to all of them, and `switch` performs Seidel switching
(see [ALGORITHM.md](ALGORITHM.md#variants)) with respect to a vertex set, such as a regular subgraph found by the
search.

Graphs can also be built from an
[incidence structure](https://en.wikipedia.org/wiki/Incidence_structure), a set of points with a
family of subsets called blocks, such as a
[block design](https://en.wikipedia.org/wiki/Block_design). Its *point graph* joins two points when
exactly $`k`$ blocks contain both, and its *block graph* joins two blocks when they share exactly
$`k`$ points (`subgraphsearch incidence`, with one block per line; `ss.Graph.point_graph` and
`ss.Graph.block_graph` in Python). Many strongly regular graphs arise this way. The block graph of
all pairs from 8 points is the [triangular graph](https://en.wikipedia.org/wiki/Triangular_graph)
$`T(8)`$, with parameters $`(28, 12, 6, 4)`$, and switching it with respect to four disjoint pairs,
an independent set the search finds, gives one of the
[Chang graphs](https://en.wikipedia.org/wiki/Chang_graphs): strongly regular with the same
parameters, but not isomorphic to $`T(8)`$.

## Choosing a search and its settings

### Which search

| Goal | Search |
| --- | --- |
| $`k`$ vertices with exactly $`m`$ edges | `-k k -m m` (`size=k, edges=m`) |
| a $`k`$-clique, a $`k`$-vertex independent set | `-m` set to $`k(k-1)/2`$, or to 0 |
| the densest (or sparsest) $`k`$ vertices | raise (or lower) `-m` until successes stop; the threshold is sharp |
| subgraphs close to a target | add `--max-conflict n` (`r.solutions(max_conflict=n)`) |
| subgraphs containing given vertices | `--fixed`, or for cliques, search their common neighborhood (below) |
| regular subgraphs of degree $`d`$ | `--degree d --two-phase`, or `--degree d` alone with `--fixed` |
| induced cycles on $`k`$ vertices | `--degree 2 --connected` (`degree=2, connected=True`) |
| induced paths on $`k`$ vertices | `--path` (`path=True`) |
| induced trees on $`k`$ vertices | `-m` set to $`k - 1`$, with `--connected` (`edges=k-1, connected=True`) |
| a total weight in a weighted graph | `--weighted -m w` |

Some reformulations make a search easier:

- **Restrict to a neighborhood.** A subgraph whose vertices must all be adjacent to some vertices
  $`F`$ (a clique through $`F`$, for example) lies in their common neighborhood. Searching
  `g.induced(nbrs)`, with `nbrs = g.common_neighborhood(F)`, for $`k - |F|`$ vertices, instead of
  fixing $`F`$ in the whole graph, is typically about 3 times faster for cliques through a given
  vertex. Vertex `i` of the result is `nbrs[i]` of the original graph.
- **Search the complement of a dense graph.** $`k`$ vertices with $`m`$ edges become $`k`$ vertices
  with $`k(k-1)/2 - m`$ in the complement, which is sparse when the graph is dense.
- **Find the limit by bisection.** To find the most edges $`k`$ vertices can span, search a few
  values of $`m`$: the switch from always found to never found is abrupt (in a 100-vertex graph of
  density 0.6, for example, 30 vertices spanning 345 edges are easy to find and 350 are out of
  reach), and near-misses (`--max-conflict`) show how close a failed target came.
- **Let `largest` find the size.** For the largest induced path, cycle or tree, clique or
  independent set, `ss.largest` (`subgraphsearch largest --shape ...`) does the searching: it
  doubles the size while searches succeed, then bisects between the last success and the first
  failure. Some sizes can be impossible below the largest (between two hypercube vertices that
  differ in an odd number of bits, every path has an even number of vertices), so a size counts as
  reachable if a search succeeds at it or at one of the next `lookahead` sizes, 2 by default. With
  `endpoints`, it starts at the shortest path between them. The size it finds is a lower bound,
  like any heuristic's; more experiments or moves can raise it.

### Settings

Tune a search on your own graph by trying a few settings; `ss.sweep` runs one search per value of
a setting, and `viz.plot_sweep` compares them. As a rule:

- **Start with many short runs.** Easy targets are found early, so the defaults are 40
  experiments of up to 250 moves. To tune this for your target, run a pilot search with generous
  `--moves` (say 5000), look at the moves successful experiments needed (the success-time
  distribution, or `viz.plot_run_time_distribution`), and set `--moves` to two or three times the
  median, using the rest of the time for more `--experiments`. Do not go far below the size of the
  subgraph, since a random start may need about $`k`$ swaps to reach a solution.
- **Give hard targets longer runs and compare kick sizes.** Runs longer than `--div-freq` moves
  make region kicks, 4 vertices by default, which keep finding solutions where an unperturbed run
  would stall. Larger kicks can help much more on some problems and less on others, so compare a
  few sizes with `ss.sweep`; `--kick-size 0` turns them off.
- **Use more experiments to find more solutions.** The success rate estimates how hard a target is,
  and more experiments find more distinct solutions, although many repeat
  (`r.solutions()` lists each once).
- **Random moves:** the defaults, 4 every 250 moves, rarely need changing; `--div-dur 0` gives a
  pure local search.
- **Aspiration** rarely makes a difference; try it on searches that stall.
- **Seeds:** results depend only on the seed and the settings, not on the number of threads, so a
  search can be repeated exactly; change the seed for independent runs.

## Graph formats

| Format | Name | Extensions | Read | Write | Use |
| --- | --- | --- | --- | --- | --- |
| graph6 | `graph6` | `.g6` | ✓ | ✓ | dense or medium graphs; the default |
| sparse6 | `sparse6` | `.s6` | ✓ | ✓ | sparse graphs |
| Matrix Market | `mtx` | `.mtx` | ✓ | ✓ | SciPy, MATLAB, Julia; edge weights |
| edge list | `edgelist` | `.edges`, `.csv`, `.tsv` | ✓ | ✓ | anything; edge weights |
| Graphviz | `dot` | `.dot`, `.gv` | | ✓ | drawing, with a highlighted subgraph |
| GraphML | `graphml` | `.graphml` | | ✓ | Gephi, Cytoscape, yEd |
| adjacency lists | `lists`, `wlists` | `.txt` (`lists`) | ✓ | ✓ | plain text, one line of neighbors per vertex |

graph6 is the most compact format for dense graphs and sparse6 for sparse ones; both are standard
in combinatorics (nauty, SageMath, networkx, House of Graphs). They can also hold many graphs, one
per line: `subgraphsearch info` lists them, `search --all` searches each one, and `--index N`
picks one. Edge lists keep isolated vertices with a `# vertices: N` header.

Edge weights (integers from 1 to 255) are an edge list's third column or a Matrix Market file's
values. Formats without weights read as weight 1, and writing a weighted graph to one is an error.

## Development

See [DEVELOPMENT.md](DEVELOPMENT.md) for setting up, testing, the scripts and releasing.

## Porting notes

subgraphsearch is a port of an earlier set of C programs. See [PORTING.md](PORTING.md) for how they map onto it, how to reproduce their runs exactly and how the port is tested against them.

## License

MIT
