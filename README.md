# subgraphsearch

[![CI](https://github.com/vglazer/subgraphsearch/actions/workflows/ci.yml/badge.svg)](https://github.com/vglazer/subgraphsearch/actions/workflows/ci.yml)
[![PyPI](https://img.shields.io/pypi/v/subgraphsearch)](https://pypi.org/project/subgraphsearch/)
[![crates.io: subgraphsearch](https://img.shields.io/crates/v/subgraphsearch?label=crates.io%3A%20subgraphsearch)](https://crates.io/crates/subgraphsearch)
[![crates.io: subgraphsearch-core](https://img.shields.io/crates/v/subgraphsearch-core?label=crates.io%3A%20subgraphsearch-core)](https://crates.io/crates/subgraphsearch-core)

Find **[induced subgraphs](https://en.wikipedia.org/wiki/Induced_subgraph) with a given number of
vertices and edges**: cycles, cliques and everything in
between. Searches are
[fast](#performance), reproducible from a seed and come with diagnostics for tuning them. Many [graph formats](#graph-formats) are supported.

A search can be required to fix particular vertices. It can also find regular subgraphs with a prescribed degree and subgraphs with a given total weight in weighted graphs. Graphs can be built from block designs and other incidence structures, as well as switched with respect to the subgraphs a search finds. Near misses are reported in addition to exact solutions, enabling use as an approximation algorithm.

See the
[quickstart notebook](https://github.com/vglazer/subgraphsearch/blob/main/examples/quickstart.ipynb)
for a complete tour.

![Searches drawn with viz.plot_graph: an induced 12-cycle (a 2-regular subgraph) in a random graph; a dense 10-vertex, 32-edge subgraph found within the neighborhood of a fixed vertex of a random geometric graph; and one graph three times, with 6 vertices spanning 0 edges (an independent set), 7 edges, and all 15 (a clique)](https://raw.githubusercontent.com/vglazer/subgraphsearch/main/assets/overview.png)

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

print(r.summary())
r.solutions()                                         # the distinct vertex sets found
viz.plot_graph(g, highlight=r.best.vertices)          # draw it
viz.plot_search_summary(r)                            # how the search went
viz.plot_graph_summary(g, highlight=r.best.vertices)  # degrees, spectrum, adjacency matrix
viz.save_html("result.html", r)                       # interactive viewer; viz.show(r) in Jupyter
```

In Jupyter, graphs and results display as summaries with small charts.

- **What to search for:** `edges=` (an exact edge count), `degree=` (a regular subgraph),
  `path=True` (an induced path), or a total weight in a `WeightedGraph`. `fixed=` names vertices the subgraph must contain,
  `connected=True` requires a connected subgraph, and `max_conflict` in
  `r.solutions(max_conflict=2)` also returns near misses.
- **Graphs:** `ss.Graph(n, edges)`, `ss.read(path)`, `g.write(path)`, `g.complement()`,
  `g.induced(vs)`, `g.switch(vs)`, `g.common_neighborhood(vs)`, `g.adjacency_matrix()`,
  `g.laplacian()`, `g.layout()`, and from an incidence structure, `ss.Graph.point_graph(blocks)`
  and `ss.Graph.block_graph(blocks)`.
- **Random graphs:** `ss.generate("exponential" | "power" | "geometric", n, density)` and
  `ss.generate_weighted(n, mean_weight)`.
- **Weighted graphs:** `ss.WeightedGraph(n, [(u, v, w), ...])`, `ss.WeightedGraph.read(path)`.
- **Many graphs per file:** `ss.read_all(path)`, `ss.read(path, index=3)`,
  `ss.write_all(path, graphs)`.
- **Other libraries:** `subgraphsearch.convert.{to,from}_{networkx,igraph,scipy,numpy}`.
- **Tuning:** `ss.sweep(g, size, edges, "div_dur", [0, 4, 16])` with `viz.plot_sweep`.
- **Spectra:** `viz.laplacian_spectrum` and `viz.spectral_summary`.
- **Designs and switching:** `ss.designs.FANO_PLANE`, `viz.plot_fano_plane()`, and
  `viz.plot_switchings(g, sets)` to draw a graph switched with respect to each of several vertex sets.

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
subgraphsearch generate exponential -n 100 -d 0.6 -s 1 -o g.g6    # a random graph
subgraphsearch search g.g6 -k 11 -m 55                            # 11-cliques
subgraphsearch search g.g6 -k 30 -m 340 --max-conflict 2          # also report near misses
subgraphsearch search g.g6 -k 10 --degree 4                       # 4-regular subgraphs
subgraphsearch search sparse.g6 -k 12 --degree 2 --connected      # induced 12-cycles
subgraphsearch search sparse.g6 -k 20 --path                      # induced 20-vertex paths
subgraphsearch generate weighted -n 200 -d 0.65 -o w.mtx          # a weighted graph
subgraphsearch search w.mtx --weighted -k 15 -m 140               # total weight 140
subgraphsearch search many.g6 --all -k 10 -m 30 -q                # every graph in a file

subgraphsearch info g.g6                                          # statistics
subgraphsearch convert g.g6 g.dot --highlight 3,17,42             # draw a subgraph with Graphviz
subgraphsearch op complement g.g6 gc.g6                           # complements, induced subgraphs, ...
subgraphsearch incidence blocks.txt --on blocks -o b.g6           # a graph from a block design

subgraphsearch search g.g6 -k 11 -m 55 --json result.json         # save the full results
python -m subgraphsearch explore g.g6 --result result.json        # explore them in an interactive page
```

`subgraphsearch search` exits with status 2 when nothing is found.

| Option | Meaning | Default |
| --- | --- | --- |
| `-k`, `--vertices` | subgraph vertices | required |
| `-m`, `--edges` | subgraph edges (total weight, with `--weighted`) | required, unless `--degree` |
| `-d`, `--degree` | instead, a subgraph regular of this degree | |
| `--path` | instead, an induced path | |
| `--connected` | require a connected subgraph | off |
| `--fixed` | vertices the subgraph must contain | none |
| `-n`, `--experiments` | independent restarts | 40 |
| `--moves` | maximum moves per experiment | 250 |
| `--div-freq`, `--div-dur` | make `div-dur` random moves at the start of every `div-freq` moves | 250, 4 |
| `-s`, `--seed` | random seed | 1 |
| `--weighted` | read edge weights; `-m` is then a total weight | off |

## How the search works

The subgraph *[induced](https://en.wikipedia.org/wiki/Induced_subgraph)* by a set of vertices
consists of those vertices and every edge of the graph between them, so choosing the vertices
determines the edges. A *[clique](https://en.wikipedia.org/wiki/Clique_(graph_theory))* is a set of
vertices that are all adjacent, an
*[independent set](https://en.wikipedia.org/wiki/Independent_set_(graph_theory))* one with no edges
at all, and a graph is *[$`d`$-regular](https://en.wikipedia.org/wiki/Regular_graph)* when every
vertex has exactly $`d`$ neighbors.

Finding a $`k`$-vertex induced subgraph with exactly $`m`$ edges is NP-hard (it includes finding
cliques), so `subgraphsearch` uses a randomized local search, specifically a *tabu search*: a
local search that forbids undoing recent moves for a while, so that it keeps exploring instead of
cycling. It cannot prove that no solution exists, but in practice it finds solutions quickly when
they are not too rare.

### Moves

Write $`S`$ for the current set of $`k`$ vertices, $`e`$ for the number of edges they induce, and
$`d_S(v)`$ for the number of neighbors a vertex $`v`$ has in $`S`$. The search minimizes the
*conflict* $`|e - m|`$; a conflict of 0 is a solution. Each move swaps one vertex $`u`$ of $`S`$
for one outside vertex $`w`$. Keeping $`d_S(v)`$ up to date for every vertex makes each swap cheap
to evaluate: the edge count becomes $`e - d_S(u) + d_S(w) - [u \sim w]`$, where $`[u \sim w]`$ is 1
if $`u`$ and $`w`$ are adjacent and 0 otherwise. Removing $`u`$ loses its $`d_S(u)`$ edges and
adding $`w`$ gains its $`d_S(w)`$, but an edge between them is counted in $`d_S(w)`$ and leaves
with $`u`$. The search takes one of the best swaps at random.

Two refinements make this fast and effective:

- **Restricted neighborhood.** If the last move left the edge count unchanged, only swaps
  involving a vertex adjacent to exactly one of the two vertices just swapped are examined. Every
  other swap has the same value it had before the move, so the search concentrates on the swaps
  the move changed. This improves results as well as speed, and on large dense graphs
  it skips most of the work.
- **Tabu moves.** The two vertices just swapped are *tabu* for the next move: the vertex just
  added cannot be removed and the vertex just removed cannot be added back. Without this the
  search would often cycle between the same two subgraphs. Moves stay tabu for a single step,
  the shortest possible *tabu tenure*.

### Diversification and restarts

Local search gets stuck in local minima. To shake it loose, the first `--div-dur` moves of every
`--div-freq` moves are made at random; these perturbations can improve the results considerably
(`--div-dur 0` turns them off). Each *experiment* is an independent restart from random vertices.
Experiments run in parallel on all cores, and results still depend only on the seed.

### Reading the results

Each experiment reports the best conflict it reached and the moves it needed:

- **Success rate** and **moves to success** show how hard a target is; `viz.plot_search_summary`
  plots both, and the next section explains how to use them to tune a search.
- **Relative cost** is the mean number of moves per success, over successful experiments;
  **true cost** divides all moves made by the number of successes, so it also counts failures.
- **Average conflict** is the mean best conflict. Near misses are useful in their own right: with
  `--max-conflict n` the search doubles as an approximation algorithm, returning subgraphs
  within $`n`$ edges of the target.

### Variants

- **Weighted graphs** (`--weighted`): edges carry integer weights and the target is a total
  weight. With all weights 1 it behaves like the plain search.
- **Regular subgraphs** (`--degree d`): the conflict becomes the total deviation from regularity,
  $`\sum_{v \in S} |d_S(v) - d|`$. Each vertex's neighbor list is kept partitioned, subgraph
  neighbors first, so that swaps stay cheap to evaluate. The intended use is finding regular
  subgraphs of strongly regular graphs. A graph is
  *[strongly regular](https://en.wikipedia.org/wiki/Strongly_regular_graph)* with parameters
  $`(n, k, \lambda, \mu)`$ when it has $`n`$ vertices, is $`k`$-regular, every two adjacent vertices
  have exactly $`\lambda`$ common neighbors, and every two non-adjacent vertices exactly $`\mu`$;
  the [5-cycle](https://en.wikipedia.org/wiki/Cycle_graph), the
  [Petersen graph](https://en.wikipedia.org/wiki/Petersen_graph) and the
  [Paley graphs](https://en.wikipedia.org/wiki/Paley_graph) are examples.
  *[Seidel switching](https://en.wikipedia.org/wiki/Two-graph#Switching_and_graphs)* a graph with
  respect to a vertex set $`S`$ swaps edges and non-edges between $`S`$ and the rest of the graph,
  leaving edges within $`S`$ and within the rest unchanged. Switching a strongly regular graph with
  respect to a suitable regular subgraph can give another strongly regular graph with the same
  parameters, sometimes one not isomorphic to the original: switching the 4×4
  [rook's graph](https://en.wikipedia.org/wiki/Rook%27s_graph), $`(16, 6, 2, 2)`$, with respect to a
  2-regular subgraph on 8 vertices gives the
  [Shrikhande graph](https://en.wikipedia.org/wiki/Shrikhande_graph). The search can find cliques
  and independent sets too, but the plain search does that much faster.
- **Two phases** (`--degree d --two-phase`): first find a subgraph with the right number of edges
  ($`kd/2`$), then make it regular using only swaps that keep the edge count. It usually finds
  regular subgraphs about 1.5 to 3.5 times faster than `--degree` alone. With `--fixed`, it
  computes the edge target from the non-fixed vertices only, so it rarely succeeds; use
  `--degree` alone there.
- **Aspiration** (`--aspiration`, edge-count and weight searches only): the tabu search's
  *aspiration criterion*, which lifts the tabu on a move that would beat the best conflict so far.
  After each move it also considers the swaps involving the two vertices just swapped, and takes one
  of them whenever it would. It can help a search that stalls, but usually makes little difference.
- **Connected subgraphs** (`--connected`, any search): each component of the subgraph beyond the
  first adds 1 to the conflict, so only connected subgraphs that hit the target count as successes.
  Swaps are still chosen by the target alone. A disconnected subgraph that hits it leaves the search
  on a plateau, which it crosses by swaps that keep the target until it reaches a connected one, so
  this works best when connected subgraphs are common among those that hit the target. It has two
  main uses. `--degree 2 --connected` finds
  [induced cycles](https://en.wikipedia.org/wiki/Induced_path) rather than unions of them: on a
  60-vertex random graph of density 0.08, all 200 of 200 experiments find a single 12-cycle, where
  `--degree 2` alone ends on one in 110. And `-k k -m k-1 --connected` finds induced
  [trees](https://en.wikipedia.org/wiki/Tree_(graph_theory)), since a connected graph on $`k`$
  vertices with $`k - 1`$ edges is a tree: on the same graph it finds 10-vertex trees in all 200
  experiments, where the edge count alone gives a tree in 25. The two-phase search's second phase
  makes no random moves, so it crosses such plateaus less often; leave out `--two-phase` here.
  Counting components after every move costs little in sparse graphs but can double the time in
  dense ones, whose dense subgraphs are connected anyway, so use it only where disconnected solutions
  turn up.
- **Paths** (`--path`, `path=True`): induced paths on $`k`$ vertices. A path is a tree, but the tree
  search rarely ends on one beyond a few vertices, so paths have a search of their own. It makes
  the regular search's moves for degree 2, but values each swap by how far the deviation from
  regularity would be from 2 rather than 0, since a path's two endpoints have degree 1 and every
  other vertex degree 2, plus how far the edge count would be from $`k - 1`$. The conflict is
  `|deviation - 2| + |edges - (k - 1)|` plus the components beyond the first, which is 0 exactly
  for an induced path. On the 60-vertex graph above it finds
  paths of 29 vertices, longer than the graph's longest induced cycle, and in hypercubes it reaches
  the known [snake-in-the-box](https://en.wikipedia.org/wiki/Snake-in-the-box) records up to the
  8-cube's 99 vertices. In the 9-cube, the lowest dimension where the longest snake is unknown, it
  finds snakes of 185 vertices in under three minutes, 7 short of the record of 192 vertices (191
  edges, reported in 2026; the previous record, 191 vertices, had stood since 2012); see
  [Performance](#performance). Fixing a 95-vertex snake of the 8-cube in one half of the 9-cube with
  `fixed=` gets closer: the search for the rest then finds snakes of 189 vertices in most
  experiments, 3 short of the record, though only some 8-cube snakes work as the half
  ([examples/snake_9cube.py](examples/snake_9cube.py)). Like regular searches, it needs an
  unweighted graph.

### Random graphs and operations

- **exponential**: [Erdős–Rényi](https://en.wikipedia.org/wiki/Erd%C5%91s%E2%80%93R%C3%A9nyi_model);
  each pair of vertices is an edge independently with probability equal to the density.
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

Graph operations help focus a search. Searching the
[complement](https://en.wikipedia.org/wiki/Complement_graph) is equivalent, since $`k`$ vertices
with $`m`$ edges become $`k`$ vertices with $`k(k-1)/2 - m`$, and for a dense graph the complement
is sparse and cheaper to work with. Inducing a subgraph on the common neighborhood of some vertices
restricts the search to vertices adjacent to all of them, and `switch` performs Seidel switching
(see [Variants](#variants)) with respect to a vertex set, such as a regular subgraph found by the
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

### Settings

Tune a search on your own graph by trying a few settings; `ss.sweep` runs one search per value of
a setting, and `viz.plot_sweep` compares them. As a rule:

- **Spend the budget on experiments, not long runs.** Most successful experiments succeed early,
  and an experiment that has stalled rarely recovers, so the defaults are many short runs: 40
  experiments of up to 250 moves. When successes come quickly, runs of 50 to 250 moves can find
  solutions 4 to 17 times faster than runs of 1000. To tune this for your target, run a pilot
  search with generous `--moves` (say 5000), look at the moves successful experiments needed (the
  success-time distribution, or `viz.plot_run_time_distribution`), and set `--moves` to two or
  three times the median, using the rest of the time for more `--experiments`. Raise `--moves` if
  successes cluster near the limit; do not go far below the size of the subgraph, since a random
  start may need about $`k`$ swaps to reach a solution.
- **Use more experiments to find more solutions.** The success rate estimates how hard a target is,
  and more experiments find more distinct solutions, although many repeat
  (`r.solutions()` lists each once).
- **Leave diversification at its defaults.** Short bursts of random moves make little difference.
  Bursts of $`k`$ to $`2k`$ random moves help long runs that stall, because they act as restarts
  within an experiment, but shorter runs with more experiments do the same more simply. `--div-dur
  0` gives a pure local search.
- **Aspiration** rarely makes a difference; try it on searches that stall.
- **Seeds:** results depend only on the seed and the settings, not on the number of threads, so a
  search can be repeated exactly; change the seed for independent runs.

## Performance

Wall-clock times on an Apple M3 Max laptop with 16 cores, using all of them, for searches started
from Python (the command runs the same engine). Settings are given as experiments × maximum moves
per experiment.

An experiment stops at its first solution, so the moves actually made, not the
maximum, determine the time.

| Search | Settings | Moves&nbsp;made | Time | Successes |
| --- | --- | --- | --- | --- |
| 11-cliques in a random graph on 100 vertices, density 0.6 | 40 × 250 (defaults) | 1,652 | 0.4&nbsp;ms | 40 of 40 |
| 30 vertices spanning 346 edges in the same graph, near the limit | 60 × 2000 | 81,539 | 14&nbsp;ms | 21 of 60 |
| 13-cliques in a random graph on 1,000 vertices, density 0.5 | 200 × 2000 | 12,042 | 12&nbsp;ms | 200 of 200 |
| 14-cliques in the same graph | 200 × 2000 | 134,267 | 135&nbsp;ms | 182 of 200 |
| 15-cliques in the same graph | 200 × 2000 | 396,104 | 394&nbsp;ms | 4 of 200 |
| 8-regular subgraphs on 20 vertices of the [Paley graph](https://en.wikipedia.org/wiki/Paley_graph) on 61 vertices | 100 × 2000, two phases | 183,319 | 49&nbsp;ms | 11 of 100 |
| 15 vertices of total weight 140 in a weighted graph on 1,000 vertices | 40 × 3000 | 634 | 1&nbsp;ms | 40 of 40 |
| induced paths on 51 vertices in the 7-cube (the [snake-in-the-box](https://en.wikipedia.org/wiki/Snake-in-the-box) record) | 400 × 2000 | 409,366 | 0.51&nbsp;s | 313 of 400 |
| induced paths on 99 vertices in the 8-cube (the record) | 400 × 20,000 | 7,951,718 | 21&nbsp;s | 3 of 400 |
| induced paths on 180 vertices in the 9-cube (the record is 192) | 100 × 200,000 | 13,226,544 | 110&nbsp;s | 44 of 100 |
| induced paths on 185 vertices in the 9-cube | 64 × 300,000 | 18,928,147 | 163&nbsp;s | 1 of 64 |
| induced paths on 189 vertices in the 9-cube through a fixed 95-vertex snake in one half | 64 × 200,000 | 4,256,688 | 36&nbsp;s | 62 of 64 |

A search that misses its target makes every move it is allowed, so its time is the cost of a move
times the number of moves. The cost of a move grows in proportion to the number of vertices, and a
little less than in proportion to the size of the subgraph.

Here are 40 × 250 = 10,000 moves looking for 30 vertices in random graphs of density 0.5:

| Graph | All 16 cores | One thread |
| --- | --- | --- |
| 100 vertices | 1.7&nbsp;ms | 20&nbsp;ms |
| 1,000 vertices | 19&nbsp;ms | 230&nbsp;ms |
| 10,000 vertices (25 million edges) | 210&nbsp;ms | 2.4&nbsp;s |

On 1,000 vertices, the same 10,000 moves take 14 ms for 20-vertex subgraphs and 61 ms for 100-vertex
subgraphs. Experiments run in parallel, so on 16 cores a search is 11 to 12 times faster than on
one thread (`--threads 1`). Once every core is busy, which 40 experiments already ensure here, the
time grows in proportion to the total number of moves: 160 experiments take four times as long.

A graph on 10,000 vertices with 25 million edges takes about 330 MB of memory. Generating it takes
0.24 s, writing it as graph6 0.05 s, and reading it back 0.28 s.

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
