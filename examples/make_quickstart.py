"""Builds and executes examples/quickstart.ipynb, and writes an output-free copy into the package for
`python -m subgraphsearch quickstart` (run: uv run python examples/make_quickstart.py)."""

import copy
from pathlib import Path

import nbformat
import nbformat.sign
from nbclient import NotebookClient

W = "https://en.wikipedia.org/wiki/"


def md(text: str):
    return nbformat.v4.new_markdown_cell(text.strip())


def code(text: str):
    return nbformat.v4.new_code_cell(text.strip())


cells = [
    md(f"""
# subgraphsearch quickstart

`subgraphsearch` finds **induced [cycles]({W}Cycle_(graph_theory)), [paths]({W}Induced_path),
[trees]({W}Tree_(graph_theory)) and [cliques]({W}Clique_(graph_theory))** in a graph, or any
[induced subgraph]({W}Induced_subgraph) with a given number of vertices and edges. It also finds
[regular]({W}Regular_graph) subgraphs, connected subgraphs and subgraphs of a given total weight. The method
is a randomized [tabu search]({W}Tabu_search).

The notebook starts with searches of one dense random graph: cliques, harder targets, near misses and fixed
vertices, then how to save, repeat and tune searches. It then turns to regular subgraphs, cycles and trees,
and induced paths, including paths between two given vertices. It combines regular subgraphs with switching
and block designs to construct strongly regular graphs, and finds Hadamard matrices, equiangular lines and
error-correcting codes as cliques of vectors. It ends with weighted graphs, views of a graph's structure, and
file formats.
"""),
    code("""
import subgraphsearch as ss
from subgraphsearch import viz

ss.__version__
"""),
    md(f"""
## A graph

`ss.generate` makes random graphs; `"exponential"` is the [Erdős–Rényi]({W}Erd%C5%91s%E2%80%93R%C3%A9nyi_model)
model, in which each pair of vertices is an edge with probability equal to the density. `ss.read` reads
graph6, sparse6, Matrix Market, edge lists and adjacency lists. In a notebook, a graph displays as a summary
with its degree distribution and, when it is small, a drawing.
"""),
    code("""
g = ss.generate("exponential", 100, 0.6, seed=1)   # 100 vertices, density 0.6
g
"""),
    md(f"""
## Kinds of search

Every search picks $k$ vertices and swaps one of them for an outside vertex at each move, trying to drive a
*conflict* to 0. What the conflict measures depends on what you ask for:

| You want | Call | Conflict |
| --- | --- | --- |
| $k$ vertices spanning exactly $m$ edges | `ss.search(g, k, m)` | $\\lvert e - m \\rvert$, with $e$ the edges among them |
| $k$ vertices whose edges weigh $m$ in total | `ss.search(w, k, m)` on a `WeightedGraph` | $\\lvert \\text{{weight}} - m \\rvert$ |
| a [$d$-regular]({W}Regular_graph) subgraph on $k$ vertices | `ss.search(g, k, degree=d)` | $\\sum_{{v \\in S}} \\lvert d_S(v) - d \\rvert$ |
| the same, in two phases | `ss.search(g, k, degree=d, two_phase=True)` | first $\\lvert e - kd/2 \\rvert$, then regularity |
| any of these, connected | add `connected=True` | plus 1 for each component beyond the first |
| an [induced path]({W}Induced_path) on $k$ vertices | `ss.search(g, k, path=True)` | $\\lvert \\text{{deviation}} - 2 \\rvert + \\lvert e - (k - 1) \\rvert$, plus extra components |
| the same, from $s$ to $t$ | add `endpoints=(s, t)` | the same, for a path two vertices longer |

Here $d_S(v)$ is the number of neighbors $v$ has among the chosen vertices $S$. Any of these can also
require some vertices with `fixed=`, return near misses with `solutions(max_conflict=...)`, and record the
conflict after every move with `trace=True`. Edge-count and weight searches can also use `aspiration=True`,
which lets the search undo its last swap when that would beat the best conflict so far (the *aspiration
criterion* of a tabu search). The sections below try each one.
"""),
    md("""
## Cliques and independent sets

An 11-vertex clique has all 11 · 10 / 2 = 55 edges, so this asks for 11 vertices spanning 55 edges. Each of
the 40 *experiments* (independent restarts) reports the best subgraph it found.
"""),
    code("""
cliques = ss.search(g, size=11, edges=55)
cliques
"""),
    md("""
Many experiments find the same clique; `solutions()` lists each distinct vertex set once, and
`viz.plot_graph` draws one.
"""),
    code("""
print(len(cliques.solutions()), "distinct 11-cliques")
viz.plot_graph(g, cliques.best.vertices, title="an 11-clique");
"""),
    md(f"""
An [independent set]({W}Independent_set_(graph_theory)) is the opposite: vertices with no edges among them,
so the target is 0. It is also a clique of the complement, `g.complement()`, and the two searches find the
same sets. This graph is dense, so its independent sets are smaller than its cliques: the search finds 7
vertices every time and 8 never. `is_independent()` checks a set (and `is_clique()` a clique), on the subgraph
it induces.
"""),
    code("""
independent = ss.search(g, 7, 0)
print(independent.successes, "of 40 experiments found a 7-vertex independent set")
print("the same as 7-cliques of the complement:", independent.solutions() == ss.search(g.complement(), 7, 21).solutions())
print("each induces no edges:", all(g.induced(vs).is_independent() for vs in independent.solutions()))
"""),
    md("""
## A harder target

Any edge count can be the target. For 30 vertices of this graph, 346 edges is near the limit of what the
search finds, and it succeeds only some of the time. `trace=True` records the *conflict*, the distance from
the target edge count, after every move.
"""),
    code("""
r = ss.search(g, size=30, edges=346, experiments=60, max_moves=2000, div_freq=300, div_dur=8, seed=5, trace=True)
print(r.summary())
"""),
    code("""
viz.plot_search_summary(r);
"""),
    md("""
The **run-time distribution** (left) is the fraction of experiments that succeeded within a given number of
moves, the middle panel counts the best conflict each experiment reached, and the **traces** (right) show the
conflict after each move for some of the experiments. Here 19 of the 21 successes came within 100 moves, and
the rest of the 2000-move budget added only two: many short experiments usually beat a few long ones.

Success falls off sharply around the limit: one edge fewer is found by every experiment, and a few more by
none.
"""),
    code("""
settings = {"experiments": 60, "max_moves": 2000, "div_freq": 300, "div_dur": 8, "seed": 5}
{m: ss.search(g, 30, m, **settings).successes for m in (345, 346, 350)}
"""),
    md("""
## Near misses and approximate solutions

The search minimizes the conflict, so every experiment ends with the best subgraph it reached, even when it
misses the target. `solutions(max_conflict=c)` returns the distinct subgraphs within $c$ edges of the target
(`--max-conflict c` on the command line), and each one's conflict is exact: it is how far that subgraph's
edge count is from the target. This turns the search into an approximation algorithm:

- **When a target is out of reach,** the near misses are the closest subgraphs found. No experiment finds 30
  vertices spanning 350 edges, but several find 349.
- **To find the densest $k$ vertices,** aim at a target the graph cannot reach, such as all $k(k-1)/2$ edges,
  and take the best near miss: its edge count is the most the search found. Aiming at 0 edges finds the
  sparsest $k$ vertices the same way.

The search cannot show that nothing closer exists, so the best near miss is a lower bound on the true
optimum, not a proof of it.
"""),
    code("""
near = ss.search(g, 30, 350, **settings).solutions(max_conflict=1)
print("within one edge of 350:", [g.induced_edge_count(vs) for vs in near])

densest = ss.search(g, 30, 30 * 29 // 2, **settings).best   # aim at a 30-clique
print("aiming at a 30-clique, the densest 30 vertices found span", g.induced_edge_count(densest.vertices), "edges")
"""),
    md("""
## Fixed vertices and neighborhoods

`fixed=` names vertices the subgraph must contain, here cliques through the vertex of highest degree. A
clique through a vertex also lies in its neighborhood, so searching the subgraph induced on the neighborhood
for one vertex fewer finds the same cliques, with a smaller graph to search.
"""),
    code("""
v = int(g.degrees().argmax())
through_v = ss.search(g, 11, 55, fixed=[v])

nbrs = g.common_neighborhood([v])
in_nbhd = ss.search(g.induced(nbrs), 10, 45)
cliques_via_nbhd = [sorted([v, *(int(nbrs[i]) for i in vs)]) for vs in in_nbhd.solutions()]

print(f"vertex {v} has {len(nbrs)} neighbors")
print("with fixed=[v]:", through_v.successes, "of 40 experiments found an 11-clique through it")
print("in its neighborhood:", in_nbhd.successes, "of 40 found one")
assert all(g.induced_edge_count(c) == 55 for c in cliques_via_nbhd)
"""),
    md("""
## Saving and repeating results

`result.save(path)` writes a result as JSON, and `ss.SearchResult.load(path, g)` reads it back given the
graph. Results depend only on the seed and the settings, not on the number of threads, so a search can be
repeated exactly; change the seed for independent runs.
"""),
    code("""
import pathlib
import tempfile

result = ss.search(g, 30, 330, experiments=8, seed=2)
path = pathlib.Path(tempfile.mkdtemp()) / "result.json"
result.save(path)
assert ss.SearchResult.load(path, g).solutions() == result.solutions()
print("the same seed gives the same solutions:", ss.search(g, 30, 330, experiments=8, seed=2).solutions() == result.solutions())
"""),
    md("""
## Tuning

`ss.sweep` runs one search per value of a setting, and `viz.plot_sweep` compares them. Here the setting is
`div_dur`, the number of random moves at the start of every `div_freq` moves.
"""),
    code("""
sweep = ss.sweep(g, 30, 340, "div_dur", [0, 4, 16, 64], experiments=40, max_moves=1500, div_freq=300)
viz.plot_sweep(sweep, parameter="div_dur");
"""),
    md("""
Aspiration is worth comparing the same way. On the hard 346-edge target from earlier it raises the number of
successes; on many targets it makes little difference. So is `kick_size`, which replaces the random moves
of each diversification cycle by rebuilding the part of the subgraph nearest a random vertex. The best size
depends on the problem, a few vertices for some and a large part of the subgraph for others, so it is worth
comparing several; the error-correcting codes below show it at work.
"""),
    code("""
for aspiration in (False, True):
    res = ss.search(g, 30, 346, aspiration=aspiration, **settings)
    print(f"aspiration={aspiration}: {res.successes}/60 successes, {res.true_cost:.0f} moves per success")
"""),
    md(f"""
## Regular subgraphs

`degree=` asks for subgraphs in which every vertex has exactly that many neighbors within the subgraph. A
graph is [strongly regular]({W}Strongly_regular_graph) with parameters $(n, k, \\lambda, \\mu)$ when it has $n$
vertices, is $k$-regular, every two adjacent vertices have exactly $\\lambda$ common neighbors, and every two
non-adjacent vertices exactly $\\mu$. The [Paley graph]({W}Paley_graph) on 61 vertices is one; here are
4-regular subgraphs on 10 of its vertices. The conflict is the total deviation from regularity,
$\\sum_{{v \\in S}} \\lvert d_S(v) - d \\rvert$, which is 0 exactly when every chosen vertex has $d$
neighbors among the others.
"""),
    code("""
q = 61
squares = {x * x % q for x in range(1, q)}
paley = ss.Graph(q, [(i, j) for i in range(q) for j in range(i + 1, q) if (j - i) % q in squares])
reg = ss.search(paley, 10, degree=4, experiments=40, max_moves=2000, div_freq=300, div_dur=8)
print("degrees within the subgraph:", set(paley.induced(reg.best.vertices).degrees().tolist()))
reg
"""),
    md("""
A $d$-regular subgraph on $k$ vertices has exactly $kd/2$ edges. With `two_phase=True` the search first finds
$k$ vertices with that many edges, then makes them regular using only swaps that keep the edge count. Those
swaps are cheaper to evaluate, so although each experiment succeeds less often on hard targets, two phases
usually find solutions faster. Compare the time per success on a harder target, 8-regular subgraphs on 20
vertices (with `fixed=`, use `degree=` alone: the two-phase edge target then counts only the non-fixed
vertices, so it rarely succeeds):
"""),
    code("""
import time

for two_phase in (False, True):
    start = time.perf_counter()
    res = ss.search(paley, 20, degree=8, two_phase=two_phase, experiments=100, max_moves=2000, div_freq=300, div_dur=8, seed=3)
    elapsed = time.perf_counter() - start
    print(f"two_phase={two_phase}: {res.successes}/100 successes, {1000 * elapsed / max(res.successes, 1):.1f} ms per success")
"""),
    md(f"""
## Cycles and trees

Cycles are the most important special case of regular subgraphs. Every vertex of an
[induced cycle]({W}Induced_path) has exactly two neighbors on it, so `degree=2` searches for them. An
edge-count search is no substitute: a $k$-cycle spans $k$ edges, but so do many other subgraphs, such as a
triangle with a path hanging off it. A 2-regular graph is a disjoint union of cycles, though, not always a
single one, so cycles get special support: `connected=True` adds 1 to the conflict for each component beyond
the first, so only single cycles count as successes. Below 6 vertices it changes nothing, since the smallest
union of two cycles is two triangles. Here are 12-vertex searches of a sparse random graph, the one in the
README figure:
"""),
    code("""
import collections

sparse = ss.generate("exponential", 60, 0.08, seed=3)
by_counts = ss.search(sparse, 12, 12, experiments=100, seed=1).solutions()
regular = sum(set(sparse.induced(vs).degrees().tolist()) == {2} for vs in by_counts)
print(f"12 vertices spanning 12 edges: {regular} of {len(by_counts)} are 2-regular")


def cycle_lengths(h):
    \"\"\"The lengths of the cycles a 2-regular graph is made of.\"\"\"
    labels, _ = h.components()
    return tuple(sorted(collections.Counter(labels).values()))


unions = ss.search(sparse, 12, degree=2, experiments=200, seed=1).solutions()
print("degree=2, by cycle lengths:", collections.Counter(cycle_lengths(sparse.induced(vs)) for vs in unions).most_common(4))
cycles = ss.search(sparse, 12, degree=2, connected=True, experiments=200, seed=1)
print(f"degree=2, connected=True: {cycles.successes} of 200 experiments found a single 12-cycle")
viz.plot_graph(sparse, cycles.best.vertices, layout=sparse.layout("spring", seed=2), title="an induced 12-cycle");   # a seed that draws it uncrossed
"""),
    md("""
`connected=True` combines with the other options: `fixed=` gives cycles through chosen vertices, and raising
$k$ until successes stop finds the longest induced cycles the search can reach. Here they get rare at 28
vertices and none are found at 30. As with near misses, the longest one found is a lower bound, not a proof
that none is longer. Leave out `two_phase=True` for connected searches: its second phase makes no random
moves, so it gets stuck on unions of cycles more often.
"""),
    code("""
def induced_cycles(k, **kwargs):
    \"\"\"The distinct induced k-cycles of the sparse graph that the search finds.\"\"\"
    return ss.search(sparse, k, degree=2, connected=True, experiments=200, seed=1, **kwargs).solutions()


print("10-cycles through vertex 0:", len(induced_cycles(10, fixed=[0])))
print("distinct k-cycles found, by k:", {k: len(induced_cycles(k)) for k in (24, 26, 28, 30)})
"""),
    md("""
`viz.show` displays a search's solutions on an interactive page: pick one from the menu to highlight it, drag
to pan, scroll to zoom, hover over a vertex for details and click it to pin it. Here the menu steps through
distinct induced 20-cycles. `viz.save_html` writes the same page to a file, and on the command line,
`python -m subgraphsearch explore graph.g6 --result result.json` makes one from a saved result.
"""),
    code("""
viz.show(ss.search(sparse, 20, degree=2, connected=True, seed=1))
"""),
    md(f"""
`connected=True` works with edge-count searches too, and there it finds induced
[trees]({W}Tree_(graph_theory)): a connected graph on $k$ vertices with $k - 1$ edges is a tree. The edge count
alone is not enough, since $k - 1$ edges can also form a forest that contains a cycle, as `is_tree()` on the
induced subgraph shows. Raising $k$ as for cycles finds large induced trees, a classic hard problem; in this
graph they grow larger than its induced cycles.
"""),
    code("""
for k in (10, 20):
    plain = ss.search(sparse, k, k - 1, experiments=200, seed=1)
    trees = sum(sparse.induced(e.vertices).is_tree() for e in plain.experiments if e.success)
    print(f"{k} vertices spanning {k - 1} edges: {trees} of 200 experiments ended on a tree")
induced_trees = {k: ss.search(sparse, k, k - 1, connected=True, experiments=200, seed=1).successes for k in (10, 20, 30, 35, 40)}
print("with connected=True, experiments that found a k-vertex tree, by k:", induced_trees)
"""),
    md(f"""
## Induced paths

The shortest path between two vertices is always an induced path, and breadth-first search finds it at once.
Long induced paths are another matter: finding the longest is NP-hard, and exact searches, which must rule
out every alternative, give out on graphs of modest size. In the hypercubes below, the longest induced path
is known exactly only up to dimension 8. A heuristic search that finds long paths quickly, and reports how
close it came when it does not, fills that gap.

A path is a tree whose vertices all have degree at most 2, but beyond a few vertices the tree search rarely
ends on one. `path=True` searches for induced [paths]({W}Path_graph) directly. It makes the regular search's
moves for degree 2, but aims at a deviation from regularity of 2 instead of 0, for the path's two endpoints
of degree 1, and at $k - 1$ edges, so it is drawn to paths rather than cycles; its conflict is 0 exactly for
an induced path. In this graph it finds paths of 29 vertices, longer than its longest induced cycle:
"""),
    code("""
trees = ss.search(sparse, 12, 11, connected=True, experiments=200, seed=1).solutions()
print("12-vertex trees that are paths:", sum(sparse.induced(vs).is_path() for vs in trees), "of", len(trees))
paths = {k: ss.search(sparse, k, path=True, experiments=200, seed=1).successes for k in (12, 20, 25, 29, 31)}
print("path=True, experiments that found a k-vertex path, by k:", paths)
"""),
    md("""
Trying sizes by hand, as above, is how to find the longest path the search can reach, and `ss.largest` does it
for you. Given a shape, `"path"`, `"cycle"`, `"tree"`, `"clique"` or `"independent"`, it doubles the size while
searches succeed, then bisects between the last success and the first failure, and keeps every search it made:
`.size` is the largest size found, `.best` the search at that size, and `.next` the closest failure above it,
with its near misses. Here it finds a path of 30 vertices, a size the loop above skipped.
"""),
    code("""
longest = ss.largest(sparse, "path", experiments=200, seed=1)
print(longest.summary())
print("largest cycle and tree:", ss.largest(sparse, "cycle", experiments=200, seed=1).size,
      ss.largest(sparse, "tree", experiments=200, seed=1).size)
"""),
    md(f"""
In the $n$-dimensional [hypercube]({W}Hypercube_graph), whose vertices are the $n$-bit strings and whose edges
join strings that differ in one bit, the longest induced paths are the [snake-in-the-box]({W}Snake-in-the-box)
problem, and the longest induced cycles the coil-in-the-box problem. Their known records make a good test:
the longest snake in the 7-cube has 51 vertices, and the path search finds one in under a second. In the
9-cube, the lowest dimension where the longest snake is unknown, the same search finds snakes of 185 vertices
in under three minutes, 7 short of the record of 192 vertices (191 edges), reported in 2026. Splitting the
9-cube into two 8-cubes gets closer: fixing a 95-vertex 8-cube snake in one half with `fixed=` leaves the
search only the rest to place, and it then finds snakes of 189 vertices in most experiments, 3 short of the
record, though only some 8-cube snakes work as the half. The script
[snake_9cube.py](https://github.com/vglazer/subgraphsearch/blob/main/examples/snake_9cube.py) reproduces one
in under a minute.
"""),
    code("""
cube = ss.hypercube(7)   # vertices 0 to 127, joined when they differ in one bit
snakes = ss.search(cube, 51, path=True, experiments=400, max_moves=2000, div_freq=300, div_dur=8)
print(snakes.summary())
"""),
    md("""
The longest induced cycles, *coils*, are found with the cycle search from the previous section, `degree=2`
with `connected=True`. Their lengths are known exactly up to the 8-cube too: the longest coil in the 7-cube
has 48 vertices, which the search finds in under a second, and the longest in the 8-cube has 96, which it
finds in 4 of 200 experiments of up to 20,000 moves, in about 10 seconds.
"""),
    code("""
coils = ss.search(cube, 48, degree=2, connected=True, experiments=400, max_moves=2000, div_freq=300, div_dur=8)
print(coils.summary())
"""),
    md("""
`endpoints=(s, t)` asks for induced paths that run from $s$ to $t$. The search attaches a new vertex to each
of them, which an induced path can only contain as an end, looks for a path two vertices longer through all
four, and leaves the new vertices out of the results. Here are paths between opposite corners of the 7-cube,
0 = 0000000 and 127 = 1111111. Each step along a path flips one bit, and the two corners differ in all seven,
an odd number, so a path between them has an even number of vertices: the search finds none of 47.
"""),
    code("""
corner_paths = {}
for k in (40, 44, 47, 48):
    corner_paths[k] = ss.search(cube, k, path=True, endpoints=(0, 127), experiments=200, max_moves=5000, div_freq=300, div_dur=8)
    print(f"{k} vertices from 0 to 127: {corner_paths[k].successes} of 200 experiments")
"""),
    md("""
Near misses come back as for any search, with `solutions(max_conflict=...)`, and every one of them contains
both endpoints. With endpoints, a near miss of conflict 1 has the right number of vertices, but only some of
them lie on a path from $s$ to $t$: the rest form a separate cycle. Leaving the cycle out still gives a path
from $s$ to $t$, though, as here, it can be a good deal shorter than the one asked for.
"""),
    code("""
for vs in corner_paths[48].solutions(max_conflict=1):
    h = cube.induced(list(vs))
    labels, pieces = h.components()
    if pieces > 1:
        sizes = sorted((int((labels == c).sum()) for c in range(pieces)), reverse=True)
        print(f"a near miss: a path of {sizes[0]} vertices from 0 to 127, and a separate cycle of {sizes[1]}")
        break
"""),
    md(f"""
## Switching

[Seidel switching]({W}Two-graph#Switching_and_graphs) with respect to a vertex set $S$ swaps edges and
non-edges between $S$ and the rest of the graph. Switching a strongly regular graph with respect to a
suitable regular subgraph can give a different strongly regular graph with the same parameters. The 4×4
[rook's graph]({W}Rook%27s_graph), the [Cartesian product]({W}Cartesian_product_of_graphs) of two complete
graphs on 4 vertices, with parameters $(16, 6, 2, 2)$, switched with respect to an induced
8-cycle gives the [Shrikhande graph]({W}Shrikhande_graph), the only other graph with those parameters. Here
the difference between a cycle and a union of cycles matters: switching with respect to two 4-cycles, also a
2-regular subgraph on 8 vertices, gives the rook's graph back, so the search asks for a connected one.
"""),
    code("""
import networkx as nx

from subgraphsearch import convert


def srg_parameters(h):
    \"\"\"(n, k, λ, μ) if h is strongly regular, else None.\"\"\"
    A = h.adjacency_matrix().astype(int)
    common, n = A @ A, len(A)
    degrees = set(A.sum(axis=1).tolist())
    lam = {int(common[i, j]) for i in range(n) for j in range(i + 1, n) if A[i, j]}
    mu = {int(common[i, j]) for i in range(n) for j in range(i + 1, n) if not A[i, j]}
    return (n, *degrees, *lam, *mu) if len(degrees) == len(lam) == len(mu) == 1 else None


k4 = ss.Graph(4).complement()       # the complete graph on 4 vertices
rook = k4.cartesian(k4)              # squares of a 4 x 4 board, joined when in the same row or column
eight_cycle = ss.search(rook, 8, degree=2, connected=True).best.vertices
switched = rook.switch(eight_cycle)
print("rook's graph:", srg_parameters(rook), "| switched:", srg_parameters(switched))
print("isomorphic:", nx.is_isomorphic(convert.to_networkx(rook), convert.to_networkx(switched)))
"""),
    md(f"""
## Block designs

An [incidence structure]({W}Incidence_structure) is a set of points with a family of subsets called
*blocks*. In a [block design]({W}Block_design) with parameters $(v, k, \\lambda)$ there are $v$ points,
every block has $k$ of them, and every two points lie together in exactly $\\lambda$ blocks. Two graphs
come from any incidence structure: the *point graph* joins two points that lie together in some number of
blocks (`ss.Graph.point_graph`), and the *block graph* joins two blocks that share some number of points
(`ss.Graph.block_graph`), one by default in both cases.

The [Fano plane]({W}Fano_plane) is the smallest projective plane, a $(7, 3, 1)$ design: 7 points and 7 lines
of 3 points each. In the usual picture the points are a triangle's corners, the midpoints of its sides and its
center; six lines are straight, and the seventh is the circle through the midpoints.
"""),
    code("""
fano = ss.designs.FANO_PLANE   # the 7 lines, labeled as in the picture
print(fano)
viz.plot_fano_plane();
"""),
    md("""
Every two points lie on exactly one line and every two lines meet in exactly one point, so both of its graphs
are complete:
"""),
    code("""
print("point graph:", ss.Graph.point_graph(fano), "| block graph:", ss.Graph.block_graph(fano))
"""),
    md(f"""
### From a design to strongly regular graphs

Designs, regular subgraph searches and switching combine into a way to construct
[strongly regular graphs]({W}Strongly_regular_graph):

1. **Build a graph from a design** with `ss.Graph.block_graph` or `ss.Graph.point_graph`, and check that it is
   strongly regular.
2. **Search it for regular subgraphs** with `ss.search(g, k, degree=d)`, or for independent sets with
   `ss.search(g, k, 0)`.
3. **Switch** with respect to each subgraph found (`g.switch(vertices)`), and keep the switched graphs that are
   still strongly regular but not isomorphic to the original or to each other.

As an example, take every pair of 8 points as a block. Its block graph joins two pairs that share a point: the
[triangular graph]({W}Triangular_graph) $T(8)$, strongly regular with parameters $(28, 12, 6, 4)$. Every
triangular graph $T(n)$ is the only graph with its parameters except $T(8)$, which shares them with the three
[Chang graphs]({W}Chang_graphs), so these steps have something to find.
"""),
    code("""
# 1. build a graph from a design: the block graph of all pairs of 8 points
pairs = [(i, j) for i in range(8) for j in range(i + 1, 8)]
t8 = ss.Graph.block_graph(pairs)
print("T(8):", srg_parameters(t8))
"""),
    md("""
Each Chang graph is $T(8)$ switched with respect to a set of pairs: four disjoint pairs, eight pairs forming an
8-cycle on the points, or a triangle plus a 5-cycle. In $T(8)$ these are independent sets of 4 vertices and
2-regular subgraphs on 8 vertices, so the search looks for both:
"""),
    code("""
# 2. search for regular subgraphs: independent sets of 4 vertices, and 2-regular subgraphs on 8
independent = ss.search(t8, 4, 0, experiments=20).solutions()
two_regular = ss.search(t8, 8, degree=2, experiments=200).solutions()
print(len(independent), "independent sets and", len(two_regular), "2-regular subgraphs found")
"""),
    md("""
Switching with respect to each one gives a graph to check. To tell the switched graphs apart, the cell compares
the sizes of their maximal cliques. Relabeling vertices cannot change them, so graphs whose clique sizes differ
cannot be isomorphic. This takes milliseconds, where testing isomorphism directly takes about 0.1 seconds per
pair of these graphs:
"""),
    code("""
# 3. switch, and keep the strongly regular graphs not isomorphic to T(8) or to each other
def clique_sizes(h):
    \"\"\"The sizes of all maximal cliques, sorted: isomorphic graphs have the same.\"\"\"
    return sorted(len(c) for c in nx.find_cliques(convert.to_networkx(h)))


def described(vertices):
    \"\"\"What the pairs at these vertices of T(8) form on the 8 points.\"\"\"
    on_points = nx.Graph([pairs[v] for v in vertices])
    cycles = tuple(sorted(len(c) for c in nx.connected_components(on_points)))
    return {(2, 2, 2, 2): "4 disjoint pairs", (8,): "an 8-cycle", (3, 5): "a triangle and a 5-cycle"}.get(cycles, f"cycles {cycles}")


seen = [clique_sizes(t8)]
chang = []   # (description, switched vertices, graph), one per isomorphism class
for vertices in independent + two_regular:
    switched = t8.switch(vertices)
    if srg_parameters(switched) != (28, 12, 6, 4):
        continue
    sizes = clique_sizes(switched)
    if sizes not in seen:
        seen.append(sizes)
        chang.append((described(vertices), vertices, switched))

for how, _, h in chang:
    print(f"switching T(8) with respect to {how}: {srg_parameters(h)}, not isomorphic to T(8) or the others")
"""),
    md("""
The top row draws each set of pairs on the 8 points, and the bottom row the graph it gives, with the switched
vertices first on the ring. Switching leaves the edges among them alone, so they still form the same cycles.
"""),
    code("""
viz.plot_switchings(
    t8, [vertices for _, vertices, _ in chang],
    graph_title="T(8): pairs that share a point", titles=[f"Chang graph {i}" for i in range(1, len(chang) + 1)],
    above=[ss.Graph(8, pairs)] + [ss.Graph(8, [pairs[v] for v in vertices]) for _, vertices, _ in chang],
    above_titles=["all 28 pairs of 8 points"] + [f"switch with respect to {how}" for how, _, _ in chang],
);
"""),
    md(f"""
## Spectral bounds

A graph's eigenvalues limit the subgraphs it can contain. In a $k$-regular graph on $n$ vertices whose
adjacency matrix has smallest eigenvalue $\\lambda$, an independent set has at most $n(-\\lambda)/(k - \\lambda)$
vertices: the [Hoffman bound]({W}Hoffman%27s_ratio_bound). In a strongly regular graph, and more generally in a
distance-regular one such as a hypercube, a clique has at most $1 - k/\\lambda$ vertices: the Delsarte bound. A search can only show what a graph contains, never what it
lacks, but the bounds supply the other half: when the search finds a set as large as a bound allows, it has
found the largest there is.

Here are the largest independent sets and cliques `ss.largest` finds in a few strongly regular graphs and the
5-cube, against the two bounds. The Clebsch graph is the 4-cube with each vertex also joined to the one
opposite it.
"""),
    code("""
import itertools

import matplotlib.pyplot as plt
import numpy as np


def paley(q):
    squares = {x * x % q for x in range(1, q)}
    return ss.Graph(q, [(u, v) for u in range(q) for v in range(u + 1, q) if (v - u) % q in squares])


def bounds(g):
    eigenvalues = np.linalg.eigvalsh(g.adjacency_matrix(dtype=float))
    k, low = eigenvalues[-1], eigenvalues[0]
    return g.n * -low / (k - low), 1 - k / low       # Hoffman (independent sets), Delsarte (cliques)


five = list(itertools.combinations(range(5), 2))
spectral = {
    "Petersen": ss.Graph(10, [(i, j) for i, j in itertools.combinations(range(10), 2) if not set(five[i]) & set(five[j])]),
    "T(8)": t8,
    "rook's 6 x 6": ss.Graph(6).complement().cartesian(ss.Graph(6).complement()),
    "5-cube": ss.hypercube(5),
    "Clebsch": ss.Graph(16, ss.hypercube(4).edge_list() + [(u, u ^ 15) for u in range(8)]),
    "Paley 61": paley(61),
    "Paley 197": paley(197),
}
largest = {}
for name, graph in spectral.items():
    hoffman, delsarte = bounds(graph)
    largest[name] = (ss.largest(graph, "independent", experiments=64, max_moves=2000),
                     ss.largest(graph, "clique", experiments=64, max_moves=2000))
    independent, clique = (r.size for r in largest[name])
    print(f"{name:13} independent set {independent:2} (Hoffman: at most {hoffman:5.2f})   clique {clique:2} (Delsarte: at most {delsarte:5.2f})")
"""),
    md("""
In the first four graphs every set found is as large as its bound allows, rounded down to a whole number, so it
is as large as possible, and the sets are familiar ones. In the Petersen graph, whose vertices are the pairs
from 5 points joined when disjoint, the 4 pairs through a point, and 2 disjoint pairs, since 5 points have no
3 disjoint pairs. In $T(8)$, whose vertices are the pairs from 8 points joined when they meet, 4 disjoint
pairs and the 7 pairs through a point. In the rook's graph, 6 squares no two in a row or column, the pattern
of a permutation, and a whole row or column. In the 5-cube, the 16 words of even weight.

Meeting the Hoffman bound has a consequence one can check: every vertex outside such an independent set $S$
has exactly $k\\lvert S \\rvert / (n - \\lvert S \\rvert)$ neighbors in it, the same number for all. The Clebsch graph's
independent sets fall one short of its bound, and there the counts differ. The Paley graphs, the standard
"random-looking" strongly regular graphs, fall far short of both bounds; how large their cliques really are is
a well-known open problem.
"""),
    code("""
def neighbors_inside(g, vertices):
    inside = set(vertices)
    return sorted({sum(int(w) in inside for w in g.neighbors(v)) for v in range(g.n) if v not in inside})


for name in ("Petersen", "T(8)", "rook's 6 x 6", "5-cube", "Clebsch"):
    print(f"{name:13} vertices outside the independent set have", neighbors_inside(spectral[name], largest[name][0].best.best.vertices),
          "neighbors in it")

board = lambda vertices: np.isin(np.arange(36), vertices).reshape(6, 6).astype(int)
independent, clique = (r.best.best.vertices for r in largest["rook's 6 x 6"])
fig, axs = plt.subplots(1, 2, figsize=(7, 3.6), layout="constrained")
viz.plot_sign_matrix(board(independent), axs[0], title="largest independent set: a permutation")
viz.plot_sign_matrix(board(clique), axs[1], title="largest clique: a whole column (or row)");
"""),
    md(f"""
## Edges and eigenvalues

The bounds above are the extreme cases of a statement about every vertex set. In a $d$-regular graph on $n$
vertices with adjacency eigenvalues $d = \\theta_1 \\ge \\theta_2 \\ge \\dots \\ge \\theta_n$, a set $S$ of $k$ vertices spans
$e(S)$ edges with

$$\\frac{{d k^2}}{{2n}} + \\theta_n \\frac{{k (n - k)}}{{2n}} \\;\\le\\; e(S) \\;\\le\\; \\frac{{d k^2}}{{2n}} + \\theta_2 \\frac{{k (n - k)}}{{2n}}.$$

The first term is what a random set of $k$ vertices spans on average. The upper bound limits how few edges can
leave a set, since each of its vertices has $d$ edges in all: at least $(d - \\theta_2)\\,k(n - k)/n$ of them, the
easy half of [Cheeger's inequality]({W}Cheeger_constant_(graph_theory)) on the graph's
[expansion]({W}Expander_graph). At $e(S) = 0$ the lower bound is the Hoffman bound. Bounding the deviation from
random on both sides at once, by the larger of $\\theta_2$ and $\\lvert \\theta_n \\rvert$, is the
[expander mixing lemma]({W}Expander_mixing_lemma).

Finding the most or fewest edges $k$ vertices can span is exactly what the search does: ask for $k$ vertices
with $m$ edges, starting at a bound and moving inward until a search succeeds (not every count in between is
possible). Below, for each $k$, the deviation from random is divided by $k(n - k)/(2n)$, so the two bounds become
horizontal lines at $\\theta_2$ and $\\theta_n$, and the points are the extremes the search finds.
"""),
    code("""
def extreme_edges(graph, k, start, step):
    # the first edge count the search finds, starting at a bound and moving inward
    m = start
    while ss.search(graph, k, m, experiments=32, max_moves=2000).successes == 0:
        m += step
    return m


mixing = {
    "rook's graph, 8 x 8": ss.Graph(8).complement().cartesian(ss.Graph(8).complement()),
    "6-cube": ss.hypercube(6),
    "circulant: 64 on a circle, 4 each side": ss.Graph(64, [(u, (u + s) % 64) for u in range(64) for s in range(1, 5)]),
    "random 8-regular, 64 vertices": ss.random_regular(64, 8, seed=1),
    "Paley graph, 61 vertices": paley(61),
}
fig, axs = plt.subplots(1, 5, figsize=(18.5, 3.6), layout="constrained", sharey=True)
for ax, (name, graph) in zip(axs, mixing.items()):
    n = graph.n
    eigenvalues = np.linalg.eigvalsh(graph.adjacency_matrix(dtype=float))
    d, second, smallest = eigenvalues[-1], eigenvalues[-2], eigenvalues[0]
    sizes, fewest, most = range(4, n - 3, 4), [], []
    for k in sizes:
        random_set, scale = d * k * k / (2 * n), k * (n - k) / (2 * n)
        high = extreme_edges(graph, k, int(np.floor(random_set + second * scale + 1e-9)), -1)
        low = extreme_edges(graph, k, max(0, int(np.ceil(random_set + smallest * scale - 1e-9))), 1)
        most.append((high - random_set) / scale)
        fewest.append((low - random_set) / scale)
    for line in (second, smallest):
        ax.axhline(line, color="#d6336c", lw=1.2)
    ax.axhline(0, color="#868e96", lw=0.8, ls=":")
    ax.plot(sizes, most, "o", sizes, fewest, "o", color="#4c6ef5", ms=4)
    ax.set_title(f"{name}\\nd = {d:.0f}, lines at θ₂ = {second:.2f} and θₙ = {smallest:.2f}", fontsize=10, loc="left")
    ax.set_xlabel("vertices k")
axs[0].set_ylabel("edges above a random set,\\nper k(n - k)/(2n)");
"""),
    md("""
Where a point sits on a line, the search has found a set as uneven as the eigenvalues allow, so the bound is
sharp there. In the rook's graph that happens at every multiple of 8, on both lines: whole rows span the most
edges, and squares forming permutation patterns the fewest. In the 6-cube it happens at half the cube, 32
vertices: a 5-dimensional subcube on the upper line, the words of even weight on the lower one.

The last three panels have the same degree, or about it, and run from a poor expander to good ones. In the
circulant, each vertex joined to its 4 nearest neighbors on either side around a circle, $\\theta_2$ is almost $d$:
the upper line is near the top, and the search finds sets of every size, arcs of the circle, with almost all
their edges inside and only a few leaving. That is the other half of Cheeger's inequality at work: a small gap
between $d$ and $\\theta_2$ means some set has few edges leaving it, and the search finds one. A random 8-regular
graph is the opposite. Its eigenvalues other than $d$ are about as small as they can be, within the Ramanujan
bound $2\\sqrt{{d - 1}}$, so its lines are close together and every set spans nearly the number of edges a random
set would: that is what it means for a graph to be an expander. Its points even stay well inside its lines,
so here the eigenvalues understate how close to random its sets are. The Paley graph, the standard
pseudo-random graph, is an expander too, and in it the points come close to both lines.
"""),
    md(f"""
## Vectors with prescribed inner products

Many objects in linear algebra are sets of vectors with prescribed inner products, and so are cliques in a
graph on the candidate vectors. `ss.Graph.from_vectors(vectors, inner_products)` builds that graph: one vertex
per row, two joined when their inner product is one of `inner_products`. It computes the inner products in
parallel straight into the adjacency matrix, so even graphs with hundreds of millions of edges take seconds.

A [Hadamard matrix]({W}Hadamard_matrix) of order $n$ is an $n \\times n$ matrix of $\\pm 1$ entries with pairwise
orthogonal rows, $H H^T = n I$. Negating columns and then rows makes its first row and column all $+1$, and
then its other $n - 1$ rows are $\\pm 1$ vectors that start with $+1$, sum to 0 (being orthogonal to the first
row) and are pairwise orthogonal. So in the graph on all such vectors, joined when orthogonal, a Hadamard
matrix of order $n$ is a clique on $n - 1$ vertices. For order 12 the graph has 462 vertices, and the search
finds a clique at once; `examples/hadamard.py` also does orders 8, 16 and 20. Choosing orthogonal rows one at a
time, at random, also works at orders 12 and 16, but at order 20 it gets stuck after 10 of the 18 rows about a
third of the time, while the search finds all 18 every time.
"""),
    code("""
n = 12
# +1 followed by the 11-entry +-1 vectors with six -1s: all candidates for the rows after the first
rest = ss.vectors.signs(ss.vectors.binary_words(n - 1, weight=n // 2))
rows = np.hstack([np.ones((len(rest), 1), dtype=int), rest])
orthogonal = ss.Graph.from_vectors(rows, [0])

found = ss.search(orthogonal, n - 1, (n - 1) * (n - 2) // 2, experiments=16, max_moves=1000)
h = np.vstack([np.ones(n, dtype=int), rows[list(found.best.vertices)]])
print(f"{orthogonal.n} vectors; {found.successes} of 16 experiments found a clique of {n - 1}")
print("a Hadamard matrix:", ss.vectors.is_hadamard(h))
"""),
    md(f"""
The matrix the search found looks random, on the left below ($+1$ dark, $-1$ white). But reordering the rows
and columns of a Hadamard matrix, or negating some of them, gives another one, and up to those changes there
is only one Hadamard matrix of order 12: [Paley's]({W}Paley_construction), built from the squares modulo 11.
Below its first row, Paley's matrix is $I + Q$ next to a column of $-1$s, where $Q_{{ij}}$ is $+1$ when $j - i$
is a nonzero square mod 11, $-1$ when it is not a square, and 0 when $j = i$. $Q$ depends only on $j - i$, so
each row is the one above shifted by one place: the diagonal stripes on the right.

Finding the reordering and the signs that turn the search's matrix into Paley's is a graph isomorphism
problem. Give each row and each column two vertices, one for each sign, join a row vertex to a column vertex
when the entry with those signs is $+1$, and join each row's or column's two vertices to each other. Then two
matrices are equivalent exactly when their graphs are isomorphic, and an isomorphism maps each row and column,
with its sign, to its place in the other matrix.
"""),
    code("""
# Paley's matrix of order 12, from the squares mod 11
squares = {x * x % 11 for x in range(1, 11)}
Q = np.array([[0 if i == j else 1 if (j - i) % 11 in squares else -1 for j in range(11)] for i in range(11)])
paley = np.eye(n, dtype=int)
paley[0, 1:] = 1
paley[1:, 0] = -1
paley[1:, 1:] += Q
print("Paley's matrix is a Hadamard matrix:", ss.vectors.is_hadamard(paley))


def signed(m):
    # two vertices per row and per column, one for each sign
    g = nx.Graph()
    for i in range(n):
        g.add_edge(("row", i, 1), ("row", i, -1), kind="pair")
        g.add_edge(("col", i, 1), ("col", i, -1), kind="pair")
    for i, j in itertools.product(range(n), repeat=2):
        for a, b in itertools.product((1, -1), repeat=2):
            if a * b * m[i, j] == 1:
                g.add_edge(("row", i, a), ("col", j, b))
    nx.set_node_attributes(g, {v: v[0] for v in g}, "side")
    return g


same_side = lambda x, y: x["side"] == y["side"]   # rows go to rows, columns to columns
match = next(nx.algorithms.isomorphism.GraphMatcher(signed(h), signed(paley), node_match=same_side).isomorphisms_iter())
rows_to, cols_to = np.zeros((n, n), dtype=int), np.zeros((n, n), dtype=int)
for i in range(n):
    _, k, sign = match[("row", i, 1)]
    rows_to[k, i] = sign           # row i of h becomes row k, times sign
    _, k, sign = match[("col", i, 1)]
    cols_to[i, k] = sign           # column i of h becomes column k, times sign
print("reordered and negated, the search's matrix is Paley's:", (rows_to @ h @ cols_to == paley).all())

fig, axs = plt.subplots(1, 2, figsize=(8.4, 4.2), layout="constrained")
viz.plot_sign_matrix(h, axs[0], title="as found by the search")
viz.plot_sign_matrix(rows_to @ h @ cols_to, axs[1], title="rows and columns reordered, some negated:\\nPaley's matrix, its 11 x 11 core circulant");
"""),
    md(f"""
[Equiangular lines]({W}Equiangular_lines) are lines through the origin with the same angle between every pair.
Lines given by vectors of equal length are equiangular when all their inner products are $\\pm c$, so they too
are cliques. Taking one $\\pm 1$ vector of length 6 per line (the 32 that start with $+1$) and asking for inner
products $\\pm 2$, that is $\\lvert \\cos \\rvert = 1/3$, `ss.largest` finds 16 lines spanning all 6 dimensions:
the most equiangular lines there can be in $\\mathbb{{R}}^6$.

Their *Seidel matrix* $S$ holds the signs of the inner products, with 0 on the diagonal, so the Gram matrix of
the unit vectors is $I + S/3$. That matrix is positive semidefinite of rank 6, which means $S$ has smallest
eigenvalue $-3$ with multiplicity $16 - 6 = 10$, as printed below. In general, $n$ lines at
$\\lvert \\cos \\rvert = a$ span $n - m$ dimensions, where $m$ is the multiplicity of the eigenvalue $-1/a$.
"""),
    code("""
d = 6
signs = ss.vectors.signs(ss.vectors.binary_words(d - 1))
lines = np.hstack([np.ones((len(signs), 1), dtype=int), signs])   # the 32 +-1 vectors starting with +1
equiangular = ss.largest(ss.Graph.from_vectors(lines, [-2, 2]), "clique", experiments=64, max_moves=3000)
chosen = lines[list(equiangular.best.best.vertices)]
gram = np.abs(chosen @ chosen.T)
print(equiangular.size, "lines spanning", np.linalg.matrix_rank(chosen), "dimensions")
print("every pair at |cos| = 2/6:", (gram[np.triu_indices(len(chosen), 1)] == 2).all())
seidel = np.sign(chosen @ chosen.T) - np.eye(len(chosen), dtype=int)
values, counts = np.unique(np.round(np.linalg.eigvalsh(seidel)).astype(int), return_counts=True)
print("Seidel eigenvalues:", ", ".join(f"{v} ({c} times)" for v, c in zip(values, counts)))
"""),
    md("""
A line has two direction vectors, $v$ and $-v$, and choosing the other one negates its row and column of $S$:
that is Seidel switching, from the switching section above, and it does not change the lines. Choosing
directions so that line 0 makes an acute angle with every other line turns the first row and column of $S$ all
dark ($+1$), on the left below. The other 15 lines then have a structure of their own: joined when they make an
acute angle, they form the line graph of $K_6$, a strongly regular graph. So each of them can be labeled by
one of the 15 edges of $K_6$, as on the right, and two of the lines make an acute angle exactly when their edges
share an endpoint; otherwise the angle is obtuse.
"""),
    code("""
first = np.sign(chosen @ chosen[0])
first[0] = 1
switched = chosen * first[:, None]        # line 0 now at an acute angle to every other line
seidel = np.sign(switched @ switched.T) - np.eye(16, dtype=int)

# lines 1 to 15, joined when acute, are the line graph of K6: match each line to an edge of K6
acute = nx.Graph([(i, j) for i in range(1, 16) for j in range(i + 1, 16) if seidel[i, j] == 1])
edge_of = next(nx.algorithms.isomorphism.GraphMatcher(acute, nx.line_graph(nx.complete_graph(6))).isomorphisms_iter())
print("acute exactly when the edges share an endpoint:",
      all((seidel[i, j] == 1) == bool(set(edge_of[i]) & set(edge_of[j])) for i in range(1, 16) for j in range(i + 1, 16)))

fig, axs = plt.subplots(1, 2, figsize=(9.8, 4.4), layout="constrained")
viz.plot_sign_matrix(seidel, axs[0], title="Seidel matrix after switching:\\n+1 dark (acute), -1 white (obtuse)")
corner = {p: (np.cos(np.pi / 2 + np.pi * p / 3), np.sin(np.pi / 2 + np.pi * p / 3)) for p in range(6)}
for line, (a, b) in edge_of.items():
    (x0, y0), (x1, y1) = corner[a], corner[b]
    axs[1].plot([x0, x1], [y0, y1], color="#d6336c", lw=1.2, alpha=0.7)
    t = 0.32 if abs(a - b) == 3 else 0.5   # keep the three diameters' labels apart
    axs[1].annotate(str(line), ((1 - t) * x0 + t * x1, (1 - t) * y0 + t * y1), ha="center", va="center", fontsize=8,
                    bbox={"boxstyle": "circle,pad=0.15", "fc": "white", "ec": "#d6336c", "lw": 0.6})
axs[1].scatter(*zip(*corner.values()), s=70, color="#868e96", zorder=3)
axs[1].set_aspect("equal")
axs[1].axis("off")
axs[1].set_title("lines 1 to 15 as the edges of K6:\\nacute exactly when the edges share an endpoint", fontsize=10, loc="left");
"""),
    md(f"""
## Error-correcting codes

A binary [code]({W}Error_correction_code) of length $n$ and minimum distance $d$ is a set of $n$-bit words,
any two of which differ in at least $d$ places. Then a word sent with up to $(d - 1)/2$ bits flipped is still
closer to the word sent than to any other, so the receiver can correct those errors, and up to $d - 1$ flipped
bits are always noticed, since they cannot turn one word into another. The more words, the more messages; the
largest number of words possible, $A(n, d)$, is known only for small $n$ and $d$.

Codes are cliques too: in the graph on all $2^n$ words, joined when they differ in at least $d$ places, a code
is a clique, and $A(n, d)$ is the size of the largest. `ss.Graph.from_words(words, distances)` builds that graph.
They are also vectors with prescribed inner products: written as $\\pm 1$ vectors, two words that differ in $t$
places have inner product $n - 2t$, which is how the decoding below finds the nearest word. For length 11 and distance 4 the most
words a code can have is 72, and the search finds such a code in a fraction of a second; `examples/codes.py`
finds the largest codes for lengths 9 to 12 and distances 4 and 6.
"""),
    code("""
n, d = 11, 4
words = ss.vectors.binary_words(n)                        # all 2048 words, as rows of 0s and 1s
far_apart = ss.Graph.from_words(words, range(d, n + 1))   # joined when at distance d or more
found = ss.search(far_apart, 72, 72 * 71 // 2, experiments=16, max_moves=2000)
print(f"{found.successes} of 16 experiments found a code of 72 words")
print("minimum distance:", ss.vectors.min_distance(words[list(found.best.vertices)]))
code = ss.vectors.signs(words[list(found.best.vertices)])    # as +-1 vectors, for decoding by inner products

# send each word with bits flipped, and decode to the nearest word of the code
rng = np.random.default_rng(1)
def received(word, flips):
    out = word.copy()
    out[rng.choice(n, flips, replace=False)] *= -1
    return out
one = [received(w, 1) for w in code]
corrected = sum(np.argmax(code @ r) == i for i, r in enumerate(one))    # nearest word: largest inner product
two = [received(w, 2) for w in code]
noticed = sum(not (code @ r == n).any() for r in two)                   # not equal to any word of the code
print(f"one bit flipped: {corrected} of 72 words decoded correctly")
print(f"two bits flipped: {noticed} of 72 noticed as errors")
"""),
    md(f"""
A code can be searched the other way round too: as an independent set in the graph of words that are *too
close*, joined when they differ in fewer than $d$ places. That graph is sparse, and distances in it mean
something: the codewords nearest a codeword lie in one region of the space of words. This is where a *region
kick* can pay off. With `kick_size=k`, at the start of each diversification cycle the search removes the $k$
codewords nearest a random one, instead of making random moves, and refills the gap with the best words it can
find, so it rebuilds one region of the code at a time, the kind of move searches for codes use.

How large a region to rebuild depends on the problem, so the size is a setting to compare. Here it is for the
72-word code above and for a [constant-weight code]({W}Constant-weight_code): 69 words of length 15, each with
seven 1s, at distance at least 6 from one another, also the most there can be:
"""),
    code("""
weight_7 = ss.vectors.binary_words(15, weight=7)
codes = {
    "72 words of length 11": (ss.Graph.from_words(words, range(1, 4)), 72),       # joined when less than 4 apart
    "69 words of weight 7": (ss.Graph.from_words(weight_7, range(1, 6)), 69),     # joined when less than 6 apart
}
print(f"{'':12}" + "".join(f"{name:>30}" for name in codes))
for kick_size in (0, 4, 32):
    row = []
    for too_close, size in codes.values():
        res = ss.search(too_close, size, 0, kick_size=kick_size, experiments=64, max_moves=5000, div_freq=300, div_dur=8)
        row.append(f"{res.successes:2}/64 found, {res.true_cost:6.0f} moves")
    print(f"kick_size={kick_size:<2}" + "".join(f"{cell:>30}" for cell in row))
"""),
    md("""
Each code wants what the other cannot stand. The 72-word code gains from rebuilding large regions: kicks of 32
codewords, nearly half the code, find it in every experiment with a seventh of the work. The constant-weight code
is best with small kicks of 4 and is wrecked by large ones. So compare a few sizes on a problem, as for any
setting, before relying on one.
"""),
    md("""
## Weighted graphs

In a `WeightedGraph` every edge has an integer weight, and the target is the total weight of the subgraph's
edges. This graph has 200 vertices and a mean weight of 0.65 per pair of vertices; the search looks for 15
vertices whose edges weigh 140 in total.
"""),
    code("""
w = ss.generate_weighted(200, 0.65, seed=1)
ss.search(w, 15, 140, experiments=40, max_moves=3000, div_freq=300, div_dur=10)
"""),
    md(f"""
## Graph structure

`viz.plot_graph_summary` shows three views of a graph: its [degree distribution]({W}Degree_distribution) next to
that of an Erdős–Rényi graph of the same density, its Laplacian spectrum, and its adjacency matrix with a
subgraph's vertices moved to the top-left block. Here it is for the dense graph from the start, with the
30-vertex, 346-edge subgraph found earlier. This section's tools also accept a `WeightedGraph`, and use its
edges without their weights.
"""),
    code("""
viz.plot_graph_summary(g, highlight=r.best.vertices);
"""),
    md(f"""
The [Laplacian]({W}Laplacian_matrix) of a graph is $L = D - A$, where $D$ is the diagonal matrix of degrees
and $A$ the adjacency matrix. Its eigenvalues $\\lambda_0 \\le \\lambda_1 \\le \\dots$ are never negative,
$\\lambda_0$ is always 0, and the number of eigenvalues equal to 0 is the number of connected components. So the
second-smallest, $\\lambda_1$, called the [algebraic connectivity]({W}Algebraic_connectivity), is positive
exactly when the graph is connected. It also measures how well connected the graph is. *Fiedler's bound*
(1973) says that unless the graph is complete, $\\lambda_1 \\le \\kappa$, where $\\kappa$ is the
[vertex connectivity]({W}Connectivity_(graph_theory)), the fewest vertices whose removal disconnects the graph.
So removing fewer than $\\lambda_1$ vertices can never disconnect it.

The middle panel is a histogram of all 100 eigenvalues. The lone bar at 0 is $\\lambda_0$, and the red line
marks $\\lambda_1$, far from 0, so this graph is connected and hard to break apart. The vertex connectivity
can never exceed the smallest degree, since removing a vertex's neighbors cuts it off, so $\\kappa$ lies
between $\\lambda_1$ and the smallest degree. Here that means between 46 and 48, and computing it gives 48. Two separate 4-cycles, by contrast, have two zero eigenvalues and $\\lambda_1 = 0$:
"""),
    code("""
fiedler = viz.spectral_summary(g)["algebraic_connectivity"]
kappa = nx.node_connectivity(convert.to_networkx(g))
print(f"Fiedler's bound: algebraic connectivity {fiedler:.1f} <= vertex connectivity {kappa} "
      f"<= smallest degree {g.degrees().min()}")

two_cycles = ss.Graph(8, [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4)])
for name, h in [("g", g), ("two 4-cycles", two_cycles)]:
    eigenvalues = viz.laplacian_spectrum(h)
    print(f"{name}: {sum(abs(eigenvalues) < 1e-9)} zero eigenvalue(s), algebraic connectivity "
          f"{viz.spectral_summary(h)['algebraic_connectivity']:.1f}")
"""),
    md(f"""
Despite its name, the `"power"` model has no heavy tail, so its graphs are not
[scale-free]({W}Scale-free_network); the comparison makes that plain:
"""),
    code("""
viz.plot_degree_distribution(ss.generate("power", 1500, 0.2, seed=52));
"""),
    md("""
## Formats and other libraries

Graphs read and write graph6, sparse6, Matrix Market, edge lists, DOT and GraphML, and convert to and
from networkx, igraph, SciPy and NumPy.
"""),
    code("""
path = pathlib.Path(tempfile.mkdtemp()) / "g.g6"
g.write(path)
assert ss.read(path) == g
print(convert.to_networkx(g), "|", repr(convert.to_scipy(g)))
print("formats:", ss.formats())
"""),
]

def main() -> None:
    nb = nbformat.v4.new_notebook(cells=cells, metadata={"kernelspec": {"name": "python3", "display_name": "Python 3 (ipykernel)", "language": "python"}})
    # the packaged copy has no outputs: about 12 KB instead of over a megabyte of embedded images, and
    # its cells are meant to be run anyway
    packaged = Path(__file__).resolve().parents[1] / "python" / "subgraphsearch" / "quickstart.ipynb"
    nbformat.write(copy.deepcopy(nb), packaged)
    print(packaged, f"{packaged.stat().st_size // 1024} KB")
    NotebookClient(nb, timeout=600, kernel_name="python3").execute()
    # Sign the notebook, as `jupyter trust` would, so that its saved outputs (inline charts, the
    # interactive viewer) display on this machine instead of being stripped as untrusted HTML.
    # Elsewhere, re-running the cells or `jupyter trust quickstart.ipynb` does the same.
    nbformat.sign.NotebookNotary().sign(nb)
    out = Path(__file__).with_name("quickstart.ipynb")
    nbformat.write(nb, out)
    print(out, f"{out.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
