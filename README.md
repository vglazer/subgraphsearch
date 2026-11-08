# subgraphsearch

[![CI](https://github.com/vglazer/subgraphsearch/actions/workflows/ci.yml/badge.svg)](https://github.com/vglazer/subgraphsearch/actions/workflows/ci.yml)

Find **induced subgraphs with a given number of vertices and edges**: cliques, independent sets
and everything in between. It can also find subgraphs that are regular of a given degree, and
subgraphs of a given total weight in weighted graphs. A fast randomized local search, written in
Rust, with a Python API, a command-line tool, random graph generators, standard graph formats and
visualization.

![Searches drawn with viz.plot_graph: an induced 12-cycle (a 2-regular subgraph) in a random graph; a dense 10-vertex, 32-edge subgraph found within the neighborhood of a fixed vertex of a random geometric graph; and one graph three times, with 6 vertices spanning 0 edges (an independent set), 7 edges, and all 15 (a clique)](https://raw.githubusercontent.com/vglazer/subgraphsearch/main/assets/overview.png)

## Install

```sh
pip install subgraphsearch              # the Python package; add [all] for plotting and interop
cargo install subgraphsearch            # the `subgraphsearch` command (needs Rust 1.87+)
```

Prebuilt wheels cover Linux, macOS and Windows for Python 3.10 and later.

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

In Jupyter, graphs and results display as summaries with small charts. The
[quickstart notebook](https://github.com/vglazer/subgraphsearch/blob/main/examples/quickstart.ipynb)
is a full tour.

- **What to search for:** `edges=` (an exact edge count), `degree=` (a regular subgraph), or a
  total weight in a `WeightedGraph`. `fixed=` names vertices the subgraph must contain, and
  `max_conflict` in `r.solutions(max_conflict=2)` also returns near misses.
- **Graphs:** `ss.Graph(n, edges)`, `ss.read(path)`, `g.write(path)`, `g.complement()`,
  `g.induced(vs)`, `g.switch(vs)`, `g.common_neighborhood(vs)`, `g.adjacency_matrix()`,
  `g.laplacian()`, `g.layout()`.
- **Random graphs:** `ss.generate("exponential" | "power" | "geometric", n, density)` and
  `ss.generate_weighted(n, mean_weight)`.
- **Weighted graphs:** `ss.WeightedGraph(n, [(u, v, w), ...])`, `ss.WeightedGraph.read(path)`.
- **Many graphs per file:** `ss.read_all(path)`, `ss.read(path, index=3)`, `ss.write_all(path, graphs)`.
- **Other libraries:** `subgraphsearch.convert.{to,from}_{networkx,igraph,scipy,numpy}`.
- **Tuning:** `ss.sweep(g, size, edges, "div_dur", [0, 4, 16])` with `viz.plot_sweep`.
- **Spectra:** `viz.laplacian_spectrum` and `viz.spectral_summary`.

## Command line

```sh
subgraphsearch generate exponential -n 100 -d 0.6 -s 1 -o g.g6    # a random graph
subgraphsearch search g.g6 -k 11 -m 55                            # 11-cliques
subgraphsearch search g.g6 -k 30 -m 340 --max-conflict 2          # also report near misses
subgraphsearch search g.g6 -k 10 --degree 4                       # 4-regular subgraphs
subgraphsearch generate weighted -n 200 -d 0.65 -o w.mtx          # a weighted graph
subgraphsearch search w.mtx --weighted -k 15 -m 140               # total weight 140
subgraphsearch search many.g6 --all -k 10 -m 30 -q                # every graph in a file

subgraphsearch info g.g6                                          # statistics
subgraphsearch convert g.g6 g.dot --highlight 3,17,42             # draw a subgraph with Graphviz
subgraphsearch op complement g.g6 gc.g6                           # complements, induced subgraphs, ...
```

`--json result.json` saves the full results, which
`python -m subgraphsearch explore g.g6 --result result.json` turns into an interactive page.
`subgraphsearch search` exits with status 2 when nothing is found.

| Option | Meaning | Default |
| --- | --- | --- |
| `-k`, `--vertices` | subgraph vertices | required |
| `-m`, `--edges` | subgraph edges (total weight, with `--weighted`) | required, unless `--degree` |
| `-d`, `--degree` | instead, a subgraph regular of this degree | |
| `--fixed` | vertices the subgraph must contain | none |
| `-n`, `--experiments` | independent restarts | 40 |
| `--moves` | maximum moves per experiment | 250 |
| `--div-freq`, `--div-dur` | make `div-dur` random moves at the start of every `div-freq` moves | 250, 4 |
| `-s`, `--seed` | random seed | 1 |
| `--weighted` | read edge weights; `-m` is then a total weight | off |

## How the search works

Finding a `k`-vertex induced subgraph with exactly `m` edges is NP-hard (it includes finding
cliques), so `subgraphsearch` uses a randomized local search. It cannot prove that no solution
exists, but in practice it finds solutions quickly when they are not too rare.

### Moves

The search minimizes the *conflict* `|e - m|`, where `e` is the number of edges the current
`k` vertices induce; a conflict of 0 is a solution. Each move swaps one subgraph vertex `u` for
one outside vertex `w`. Keeping every vertex's degree within the subgraph, `d(v)`, makes each swap
cheap to evaluate: the edge count becomes `e - d(u) + d(w) - [u ~ w]`. The search takes one of
the best swaps at random.

Two refinements make this fast and effective:

- **Restricted neighborhood.** If the last move left the edge count unchanged, only swaps
  involving a vertex adjacent to exactly one of the two vertices just swapped are examined. Every
  other swap has the same value it had before the move, so the search concentrates on the swaps
  the move changed. This improves results as well as speed, and on large dense graphs
  it skips most of the work.
- **No immediate reversal.** The two vertices just swapped are never swapped straight back, which
  stops the search cycling between the same two subgraphs.

### Diversification and restarts

Local search gets stuck in local minima. To shake it loose, the first `--div-dur` moves of every
`--div-freq` moves are made at random; these perturbations can improve the results considerably
(`--div-dur 0` turns them off). Each *experiment* is an independent restart from random vertices,
and experiments run in parallel, which makes a search roughly 10× faster than the
[original C programs](https://github.com/vglazer/USRA/tree/master/subgraph_finding) on a
multi-core machine; results still depend only on the seed.

### Reading the results

Each experiment reports the best conflict it reached and the moves it needed:

- **Success rate** and **moves to success** show how hard a target is; `viz.plot_search_summary`
  plots both, and the next section explains how to use them to tune a search.
- **Relative cost** is the mean number of moves per success, over successful experiments;
  **true cost** divides all moves made by the number of successes, so it also counts failures.
- **Average conflict** is the mean best conflict. Near misses are useful in their own right: with
  `--max-conflict n` the search doubles as an approximation algorithm, returning subgraphs
  within `n` edges of the target.

### Variants

- **Weighted graphs** (`--weighted`): edges carry integer weights and the target is a total
  weight. With all weights 1 it behaves like the plain search.
- **Regular subgraphs** (`--degree d`): the conflict becomes the total deviation from regularity,
  the sum of `|d(v) - d|` over the subgraph. Each vertex's neighbor list is kept partitioned,
  subgraph neighbors first, so that swaps stay cheap to evaluate. The intended use is finding
  regular subgraphs of strongly regular graphs: switching a strongly regular graph with respect to
  such a subgraph can produce a new, larger strongly regular graph. It can find cliques and
  independent sets too, but the plain search does that much faster.
- **Two phases** (`--degree d --two-phase`): first find a subgraph with the right number of edges
  (`k d / 2`), then make it regular using only swaps that keep the edge count. It usually finds
  regular subgraphs about 1.5 to 3.5 times faster than `--degree` alone. With `--fixed`, it
  computes the edge target from the non-fixed vertices only, as the original program did, so it
  rarely succeeds; use `--degree` alone there.
- **Aspiration** (`--aspiration`): after each move, also consider swaps involving the two vertices
  just swapped, which are normally excluded, and take one whenever it would beat the best conflict
  so far. It can help a search that stalls, but usually makes little difference.

### Random graphs and operations

- **exponential**: Erdős–Rényi; each pair of vertices is an edge independently with probability
  equal to the density.
- **geometric**: random points in the unit square, joined when within a distance chosen so that
  the disc around a point covers a fraction of the square equal to the density. There is no
  wrap-around, so vertices near the edges have fewer neighbors and the actual density is lower.
- **power**: each vertex is joined to the same number of randomly chosen higher-numbered
  vertices. Degrees spread more widely than in exponential graphs but have no heavy tail, so
  despite the name these are not scale-free graphs. Power and geometric graphs only make sense
  for densities below about 0.5.
- **weighted**: random pairs of vertices have their weight increased by 1 until the mean weight
  per pair is reached, so weights above 1 are common once it approaches 1.

Graph operations help focus a search. Searching the complement is equivalent, since `k` vertices
with `m` edges become `k` vertices with `k (k - 1) / 2 - m`, and for a dense graph the complement
is sparse and cheaper to work with. Inducing a subgraph on the common neighborhood of some
vertices restricts the search to vertices adjacent to all of them, and `switch` performs the
Seidel switching used with regular subgraphs.

To re-run experiments from the original programs, `subgraphsearch legacy` accepts their input and
reproduces their output exactly, and `--rng legacy` reproduces their results through the
interfaces above.

## Choosing a search and its settings

### Which search

| Goal | Search |
| --- | --- |
| `k` vertices with exactly `m` edges | `-k k -m m` (`size=k, edges=m`) |
| a `k`-clique, a `k`-vertex independent set | `-m` set to `k (k - 1) / 2`, or to 0 |
| the densest (or sparsest) `k` vertices | raise (or lower) `-m` until successes stop; the threshold is sharp |
| subgraphs close to a target | add `--max-conflict n` (`r.solutions(max_conflict=n)`) |
| subgraphs containing given vertices | `--fixed`, or for cliques, search their common neighborhood (below) |
| regular subgraphs of degree `d` | `--degree d --two-phase`, or `--degree d` alone with `--fixed` |
| a total weight in a weighted graph | `--weighted -m w` |

Some reformulations make a search easier:

- **Restrict to a neighborhood.** A subgraph whose vertices must all be adjacent to some vertices
  `S` (a clique through `S`, for example) lies in their common neighborhood. Searching
  `g.induced(nbrs)`, with `nbrs = g.common_neighborhood(S)`, for `k - |S|` vertices, instead of
  fixing `S` in the whole graph, is typically about 3 times faster for cliques through a given
  vertex. Vertex `i` of the result is `nbrs[i]` of the original graph.
- **Search the complement of a dense graph.** `k` vertices with `m` edges become `k` vertices
  with `k (k - 1) / 2 - m` in the complement, which is sparse when the graph is dense.
- **Find the limit by bisection.** To find the most edges `k` vertices can span, search a few
  values of `m`: the switch from always found to never found is abrupt (in a 100-vertex graph of
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
  start may need about `k` swaps to reach a solution.
- **Use more experiments to find more solutions.** The success rate estimates how hard a target is,
  and more experiments find more distinct solutions, although many repeat
  (`r.solutions()` lists each once).
- **Leave diversification at its defaults.** Short bursts of random moves make little difference.
  Bursts of `k` to `2k` random moves help long runs that stall, because they act as restarts
  within an experiment, but shorter runs with more experiments do the same more simply. `--div-dur
  0` gives a pure local search.
- **Aspiration** rarely makes a difference; try it on searches that stall.
- **Seeds:** results depend only on the seed and the settings, not on the number of threads, so a
  search can be repeated exactly; change the seed for independent runs. `--rng legacy`
  reproduces the original programs but runs sequentially, so use it only for that.

## Graph formats

| Format | Name | Extensions | Read | Write | Use |
| --- | --- | --- | --- | --- | --- |
| graph6 | `graph6` | `.g6` | ✓ | ✓ | dense or medium graphs; the default |
| sparse6 | `sparse6` | `.s6` | ✓ | ✓ | sparse graphs |
| Matrix Market | `mtx` | `.mtx` | ✓ | ✓ | SciPy, MATLAB, Julia; edge weights |
| edge list | `edgelist` | `.edges`, `.csv`, `.tsv` | ✓ | ✓ | anything; edge weights |
| Graphviz | `dot` | `.dot`, `.gv` | | ✓ | drawing, with a highlighted subgraph |
| GraphML | `graphml` | `.graphml` | | ✓ | Gephi, Cytoscape, yEd |
| sub_search lists | `lists`, `wlists` | `.txt` (`lists`) | ✓ | ✓ | the original programs' formats |

graph6 is the most compact format for dense graphs and sparse6 for sparse ones; both are standard
in combinatorics (nauty, SageMath, networkx, House of Graphs). They can also hold many graphs, one
per line: `subgraphsearch info` lists them, `search --all` searches each one, and `--index N`
picks one. Edge lists keep isolated vertices with a `# vertices: N` header.

Edge weights (integers from 1 to 255) are an edge list's third column or a Matrix Market file's
values. Formats without weights read as weight 1, and writing a weighted graph to one is an error.

## Development

```sh
cargo test --release                              # Rust tests
uv run pytest                                     # Python tests (build the CLI first)
uv run python examples/make_quickstart.py         # rebuild the quickstart notebook
uv run python examples/make_figures.py            # redraw the README figures
USRA_DIR=~/repos/USRA scripts/make_fixtures.sh    # re-record test fixtures from the C programs
```

## License

MIT
