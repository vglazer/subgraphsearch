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
and induced paths, including paths between two given vertices, and combines regular subgraphs with switching and block designs to construct strongly regular graphs.
It ends with weighted graphs, views of a graph's structure, and file formats.
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
successes; on many targets it makes little difference.
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
[rook's graph]({W}Rook%27s_graph), with parameters $(16, 6, 2, 2)$, switched with respect to an induced
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


rook = ss.Graph(16, [(a, b) for a in range(16) for b in range(a + 1, 16) if a // 4 == b // 4 or a % 4 == b % 4])
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
