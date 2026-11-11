# Algorithm

*Part of the [subgraphsearch documentation](README.md#documentation).*

subgraphsearch finds *[induced subgraphs](https://en.wikipedia.org/wiki/Induced_subgraph)* with a
prescribed number of vertices and edges. The subgraph induced by a set of vertices consists of those
vertices and every edge of the graph between them, so choosing the vertices determines the edges.
Finding $`k`$ vertices that span exactly $`m`$ edges is NP-hard, since it includes finding
[cliques](https://en.wikipedia.org/wiki/Clique_problem), so the search is a heuristic: a randomized
*[tabu search](https://en.wikipedia.org/wiki/Tabu_search) with perturbations*. It cannot prove
that no solution exists, but it finds solutions quickly when they are not too rare, and the closest
subgraphs it reaches when they are.

The method comes from a set of C programs written in 2002 at the University of Toronto under the
supervision of Rudi Mathon, which built on Fred Glover's tabu search to construct combinatorial
designs ([the original programs and reports](https://github.com/vglazer/USRA/tree/master/subgraph_finding)).
The same structure, a tabu search interrupted by perturbations, was later proposed independently as
*iterated tabu search* by G. Palubeckis ("Iterated tabu search for the maximum diversity problem",
*Applied Mathematics and Computation* 189, 2007), now among the strongest methods for that problem;
subgraphsearch's programs predate it by five years.

This page describes the method. [TUNING.md](TUNING.md) covers choosing its settings, and
[BENCHMARKS.md](BENCHMARKS.md) shows how it compares with other methods. Parameters are given by their
Python names, with the command-line options in parentheses on first mention.

## Objective and moves

Write $`S`$ for the current set of $`k`$ vertices, $`e`$ for the number of edges they induce, and
$`d_S(v)`$ for the number of neighbors a vertex $`v`$ has in $`S`$. The search minimizes the
*conflict* $`|e - m|`$; a conflict of 0 is a solution. Each move swaps one vertex $`u`$ of $`S`$
for one outside vertex $`w`$. Keeping $`d_S(v)`$ up to date for every vertex makes each swap cheap
to evaluate: the edge count becomes $`e - d_S(u) + d_S(w) - [u \sim w]`$, where $`[u \sim w]`$ is 1
if $`u`$ and $`w`$ are adjacent and 0 otherwise. Removing $`u`$ loses its $`d_S(u)`$ edges and
adding $`w`$ gains its $`d_S(w)`$, but an edge between them is counted in $`d_S(w)`$ and leaves
with $`u`$. Each move takes one of the best swaps at random.

Two refinements make this fast and effective:

- **Restricted neighborhood.** If the last move left the edge count unchanged, only swaps
  involving a vertex adjacent to exactly one of the two vertices just swapped are examined. Every
  other swap has the same value it had before the move, so the search concentrates on the swaps
  the move changed. This improves the search as well as its speed, and on large dense graphs it
  skips most of the work.
- **Tabu moves.** The two vertices just swapped are *tabu* for the next move: the vertex just
  added cannot be removed and the vertex just removed cannot be added back, so the search does not
  cycle between the same two subgraphs. Moves stay tabu for a single step, the shortest possible
  *tabu tenure*.

## Perturbations

A local search gets stuck in local minima, so the search is perturbed at regular intervals. Every
`div_freq` (`--div-freq`) moves a *diversification cycle* begins, and the first `div_dur`
(`--div-dur`) moves of the first cycle are made at random (`div_dur=0` turns this off).

In later cycles the random moves are replaced by a *region kick*: the search takes the `kick_size`
(`--kick-size`) subgraph vertices nearest a random subgraph vertex, in breadth-first order from it, and swaps each
out, one per move, for the best vertex to swap in by the usual move value. The vertices removed may
not return for `kick_ban` (`--kick-ban`) moves, 20 by default. A kick rebuilds one part of the subgraph at a time
while leaving the rest in place, much as the large-neighborhood moves of searches for codes do,
where random moves would scatter changes across the whole subgraph.

Edge-count and weight searches use kicks of 4 vertices by default, a size that rebuilds little at
a time. Larger kicks rebuild more and suit some problems but not others, so the size is a setting
to compare; `kick_size=0` keeps random moves in every cycle. Regular and path searches use random
moves only, as does the legacy random number generator by default, so that it reproduces the
original programs. A search no longer than `div_freq` moves, such as one with the default settings,
never reaches a second cycle and so is the same with or without kicks.

## Experiments, threads and seeds

Each *experiment* is an independent restart from random vertices, which always include any fixed
vertices, and makes up to `max_moves` (`--moves`) moves, stopping at its first solution; a search
runs `experiments` (`--experiments`) of them. Many short experiments
usually do better than a few long ones, so the defaults are 40 experiments of up to 250 moves.
Experiments run in parallel on all cores, each with its own random stream derived from the seed,
so results depend only on the seed and the settings, never on the number of threads. With the
legacy random number generator, a variant of RANDU, experiments run one after another, as in the
original programs, and reproduce their results exactly (see [PORTING.md](PORTING.md)).

## Variants

- **Weighted graphs** (a `WeightedGraph`, `--weighted`). Edges carry integer weights from 1 to 255, and the target is the total
  weight of the subgraph's edges. The moves are the same, with $`d_S(v)`$ the total weight of
  $`v`$'s edges into $`S`$; with all weights 1 it is the plain search.
- **Fixed vertices** (`fixed`, `--fixed`). Vertices given are part of every subgraph and are never swapped
  out.
- **Regular subgraphs** (`degree=d`, `--degree d`). The conflict becomes the total deviation from regularity,
  $`\sum_{v \in S} |d_S(v) - d|`$. Each vertex's neighbor list is kept partitioned, subgraph
  neighbors first, so that swaps stay cheap to evaluate. The intended use is finding regular
  subgraphs of [strongly regular graphs](https://en.wikipedia.org/wiki/Strongly_regular_graph),
  where [Seidel switching](https://en.wikipedia.org/wiki/Two-graph#Switching_and_graphs) with
  respect to a suitable regular subgraph can give another strongly regular graph with the same
  parameters.
- **Two phases** (`two_phase=True`, `--two-phase`). The search first finds a subgraph with the right number of edges, $`kd/2`$, then
  makes it regular using only swaps that keep the edge count. With fixed vertices it computes the
  edge target from the non-fixed vertices only, a quirk of the original program, so it rarely
  succeeds there; use the one-phase search with fixed vertices.
- **Aspiration** (`aspiration=True`, `--aspiration`). The tabu search's *aspiration criterion* lifts the tabu on a move that would beat
  the best conflict so far: after each move the search also considers the swaps involving the two
  vertices just swapped, and takes one of them whenever it would. Edge-count and weight searches
  only.
- **Connected subgraphs** (`connected=True`, `--connected`). Each component of the subgraph beyond the first adds 1 to the conflict,
  so only connected subgraphs that hit the target count as successes. Swaps are still chosen by the
  target alone: a disconnected subgraph that hits it leaves the search on a plateau, which it
  crosses by swaps that keep the target until it reaches a connected one. This suits targets that
  connected subgraphs often meet. With degree 2 it finds
  [induced cycles](https://en.wikipedia.org/wiki/Induced_path) rather than unions of them, and with
  $`k - 1`$ edges it finds induced [trees](https://en.wikipedia.org/wiki/Tree_(graph_theory)), since
  a connected graph on $`k`$ vertices with $`k - 1`$ edges is a tree. Components are counted after
  every move, which costs about the sum of the subgraph's degrees in the whole graph.
- **Paths** (`path=True`, `--path`). Induced paths on $`k`$ vertices have a search of their own, since the tree search
  rarely ends on a path beyond a few vertices. It makes the regular search's moves for degree 2,
  but values each swap by how far the deviation from regularity would be from 2 rather than 0,
  since a path's two endpoints have degree 1 and every other vertex degree 2, plus how far the edge
  count would be from $`k - 1`$. The conflict, `|deviation - 2| + |edges - (k - 1)|` plus the
  components beyond the first, is 0 exactly for an induced path.
- **Paths between two vertices** (`endpoints=(s, t)`, `--endpoints s,t`). For induced paths from $`s`$ to $`t`$, the search attaches a new
  vertex to each of $`s`$ and $`t`$, which an induced path can only contain as an end, fixes all
  four, and looks for a path two vertices longer. The results leave the new vertices out, so every
  subgraph returned, near misses included, contains $`s`$ and $`t`$.

## Finding the largest size

`ss.largest` (`subgraphsearch largest`) finds the largest induced path, cycle or tree, clique or independent set the search can
reach. Starting at the smallest size the shape and the fixed vertices allow, it doubles its steps
while searches succeed, then bisects between the last size that succeeded and the first that did
not. Some sizes can be impossible below the largest (between two hypercube vertices that differ in
an odd number of bits, every path has an even number of vertices), so a size counts as reachable if
a search succeeds at it or at one of the next `lookahead` sizes, 2 by default. The size found is a
lower bound, like any heuristic's.

## Reading the results

Each experiment reports the best conflict it reached, the subgraph that reached it and the moves
it made. Over a search:

- The **success rate** and the distribution of **moves to success** show how hard a target is.
  The distribution guides the choice of `max_moves`: if successes cluster early, runs can be
  shorter; if they cluster near the limit, longer.
- The **relative cost** is the mean number of moves per success, over successful experiments; the
  **true cost** divides all moves made by the number of successes, so it also counts failures.
- The **average conflict** is the mean best conflict, 0 when every experiment succeeds.
- **Near misses**, the subgraphs within a few edges of the target, make the search an
  approximation algorithm: aiming at an unreachable target, such as a clique of the size wanted,
  finds the densest subgraphs the search can reach.

## Data structures and limits

A graph is held as an adjacency matrix and as adjacency lists. The matrix has one bit per pair of
vertices in an unweighted graph and one byte, the edge's weight, in a weighted one; it gives
constant-time adjacency tests during move evaluation, read a row at a time. The lists bound the work
of updating $`d_S(v)`$ after a swap by the degrees of the two vertices swapped. The lists take most
of the memory of a dense graph: one on 10,000 vertices with 25 million edges takes about 330 MB. The
matrix dominates in large sparse graphs: one bit per pair of vertices is about 1.25 GB at 100,000
vertices, and a weighted graph's byte per pair about 10 GB. Each thread keeps its own $`d_S(v)`$
array and move lists, so the memory of a search beyond the graph grows with the number of vertices
times the number of threads.

The cost of a move grows in proportion to the number of vertices, and a little less than in
proportion to the size of the subgraph; a search that misses its target makes every move it is
allowed. Experiments are independent, so the time divides almost evenly among cores.

Regular, cycle and path searches also keep, in every thread, where each vertex's neighbors are in
its neighbor list. On graphs of up to 2,048 vertices they use a table with an entry for every pair
of vertices, as the original programs do; on larger ones, an entry for every edge, so their memory
grows with the number of edges rather than with the square of the number of vertices. Positions are
32-bit integers, 4 bytes per edge end in every thread, which adds up in dense graphs: a graph on
10,000 vertices with 25 million edges needs about 200 MB per thread, and a search can use fewer
threads (`--threads`) when memory is short. The original programs stored positions in 16 bits,
which limited these searches to 32,767 vertices; subgraphsearch has no such limit, so a path search
runs in the 16-cube, with 65,536 vertices, as in smaller ones. No search has a fixed limit on the
number of vertices: memory is the limit.
