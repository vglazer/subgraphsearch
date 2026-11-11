# Tuning

*Part of the [subgraphsearch documentation](README.md#documentation).*

This page covers which search to use for a goal, reformulations that make a search easier, and how to
set a search's parameters. [ALGORITHM.md](ALGORITHM.md) describes what the parameters do. Parameters
are given by their Python names, with the command-line options in parentheses.

## Which search

| Goal | Search |
| --- | --- |
| $`k`$ vertices with exactly $`m`$ edges | `size=k, edges=m` (`-k k -m m`) |
| a $`k`$-clique, a $`k`$-vertex independent set | `edges` set to $`k(k-1)/2`$, or to 0 |
| the densest (or sparsest) $`k`$ vertices, or the heaviest (lightest) in a weighted graph | `ss.densest(g, k)` (`ss.sparsest(g, k)`); on the command line, aim `-m` at $`k(k-1)/2`$ (or 0) and read the best near miss |
| subgraphs close to a target | `r.solutions(max_conflict=n)` (`--max-conflict n`) |
| subgraphs containing given vertices | `fixed=` (`--fixed`), or for cliques, search their common neighborhood (below) |
| regular subgraphs of degree $`d`$ | `degree=d, two_phase=True` (`--degree d --two-phase`), or `degree=d` alone with fixed vertices |
| induced cycles on $`k`$ vertices | `degree=2, connected=True` (`--degree 2 --connected`) |
| induced paths on $`k`$ vertices | `path=True` (`--path`), between two vertices with `endpoints=(s, t)` (`--endpoints s,t`) |
| induced trees on $`k`$ vertices | `edges=k-1, connected=True` (`-m k-1 --connected`) |
| a total weight in a weighted graph | `ss.search(w, k, total)` on a `WeightedGraph` (`--weighted -m total`) |
| the largest path, cycle, tree, clique or independent set | `ss.largest(g, shape)` (`subgraphsearch largest --shape shape`) |

Some reformulations make a search easier:

- **Restrict to a neighborhood.** A subgraph whose vertices must all be adjacent to some vertices
  $`F`$ (a clique through $`F`$, for example) lies in their common neighborhood. Searching
  `g.induced(nbrs)`, with `nbrs = g.common_neighborhood(F)`, for $`k - |F|`$ vertices, instead of
  fixing $`F`$ in the whole graph, is typically about 3 times faster for cliques through a given
  vertex. Vertex `i` of the result is `nbrs[i]` of the original graph.
- **Search the complement of a dense graph.** $`k`$ vertices with $`m`$ edges become $`k`$ vertices
  with $`k(k-1)/2 - m`$ in the complement, which is sparse when the graph is dense.
- **Let `densest` find the most edges** $`k`$ vertices can span (`ss.densest(g, k)`). Aiming at
  exact values just below that limit also works, since the switch from always found to never found
  is abrupt, and finds more distinct subgraphs at a given value.
- **Let `largest` find the size** of the largest path, cycle, tree, clique or independent set (see
  [ALGORITHM.md](ALGORITHM.md#finding-the-largest-size)). The size it finds is a lower bound;
  more experiments or moves can raise it, and `lookahead` matters where some sizes below the
  largest are impossible.

## Settings

Tune a search on your own graph by trying a few settings; `ss.sweep` runs one search per value of
a setting, and `viz.plot_sweep` compares them. As a rule:

- **Start with many short runs.** Easy targets are found early, so the defaults are 40
  experiments (`experiments`, `-n`) of up to 250 moves (`max_moves`, `--moves`). To tune this for
  your target, run a pilot search with generous `max_moves` (say 5000), look at the moves
  successful experiments needed (`viz.plot_run_time_distribution`), and set `max_moves` to two or
  three times the median, using the rest of the time for more experiments. Do not go far below the
  size of the subgraph, since a random start may need about $`k`$ swaps to reach a solution.
- **Give hard targets longer runs and compare kick sizes.** Runs longer than `div_freq`
  (`--div-freq`) moves make [region kicks](ALGORITHM.md#perturbations), of 4 vertices by default,
  which keep finding solutions where an unperturbed run would stall. Larger kicks (`kick_size`,
  `--kick-size`) can help much more on some problems and less on others, so compare a few sizes
  with `ss.sweep`; `kick_size=0` turns them off.
- **Use more experiments to find more solutions.** The success rate estimates how hard a target is,
  and more experiments find more distinct solutions, although many repeat (`r.solutions()` lists
  each once).
- **Random moves** (`div_dur`, `--div-dur`): the defaults, 4 every 250 moves, rarely need
  changing; `div_dur=0` gives a pure local search.
- **Aspiration** (`aspiration=True`, `--aspiration`) rarely makes a difference; try it on searches
  that stall.
- **Seeds** (`seed`, `-s`): results depend only on the seed and the settings
  ([ALGORITHM.md](ALGORITHM.md#experiments-threads-and-seeds)); change the seed for independent
  runs.
