# Benchmarks

*Part of the [subgraphsearch documentation](README.md#documentation).*

subgraphsearch is one general search for vertex and edge counts, total weights, and induced paths,
cycles, trees and cliques. Here it is set beside the methods one would otherwise use, on problems
with public benchmarks or known records. Times are wall-clock on a 16-core Apple M3 Max laptop;
subgraphsearch's are for all its experiments together, with default settings unless stated. The
scripts in [benchmarks](benchmarks/README.md) reproduce every measurement.

![Left: CP-SAT's time to find k vertices spanning exactly m edges grows to seconds and then past its 10-second limit as random graphs grow from 20 to 59 vertices. Middle: subgraphsearch finds the same targets in about a millisecond or less. Right: on eight maximum diversity benchmark instances, subgraphsearch reaches every best known value, while CP-SAT after 10 seconds falls up to 8% short](assets/comparison.png)

## Exact edge counts

*Reproduce with [benchmarks/edge_counts.py](benchmarks/edge_counts.py).*

No specialized solver finds $`k`$ vertices spanning exactly $`m`$ edges, so the alternatives are
general solvers, each stopped after 10 seconds: OR-Tools'
[CP-SAT](https://developers.google.com/optimization/cp/cp_solver) on 16 cores, with a yes-or-no
variable per vertex and the edge count as half the sum of the chosen vertices' degrees within the
set, and the mixed-integer solvers [SCIP](https://www.scipopt.org), [HiGHS](https://highs.dev)
and [CBC](https://github.com/coin-or/Cbc) (CBC on 16 cores) through PuLP, with a variable per
vertex and per edge. The graphs are random with density 0.6, and the target is the most edges
subgraphsearch finds among 30% of the vertices, the hardest kind of target.

![Left: the times of CP-SAT, SCIP, HiGHS and CBC on graphs of 20 to 59 vertices. All four find solutions at 20 vertices in under a second; at 29 vertices the MIP solvers take 0.8 to 3 seconds; from 38 vertices on only CP-SAT finds anything within 10 seconds, at 38 and 50 vertices. Right: subgraphsearch finds every target in under a millisecond](assets/benchmarks_edges.png)

| Graph | Target | subgraphsearch | CP-SAT | SCIP | HiGHS | CBC |
| --- | --- | --- | --- | --- | --- | --- |
| 20 vertices | 6 vertices, 15 edges | 0.15&nbsp;ms | 25&nbsp;ms | 0.10&nbsp;s | 0.35&nbsp;s | 0.51&nbsp;s |
| 29 vertices | 9 vertices, 34 edges | 0.19&nbsp;ms | 0.41&nbsp;s | 0.78&nbsp;s | 3.0&nbsp;s | 2.6&nbsp;s |
| 38 vertices | 11 vertices, 52 edges | 0.21&nbsp;ms | 4.5&nbsp;s | nothing | nothing | nothing |
| 44 vertices | 13 vertices, 70 edges | 0.16&nbsp;ms | nothing | nothing | nothing | nothing |
| 50 vertices | 15 vertices, 91 edges | 0.77&nbsp;ms | 8.8&nbsp;s | nothing | nothing | nothing |
| 59 vertices | 18 vertices, 129 edges | 0.38&nbsp;ms | nothing | nothing | nothing | nothing |

"Nothing" means no solution within 10 seconds. subgraphsearch ran 60 experiments of up to 2,000
moves, and all succeeded. Given 60 seconds, CP-SAT also solves the 44-, 47- and 56-vertex graphs
(in 9.2, 44 and 12.5 s) but still nothing at 53 or 59 vertices. What a general solver can do and
subgraphsearch cannot is prove a target impossible: on a 30-vertex graph of the same kind, CP-SAT
proves in 0.9 s that no 9 vertices span one edge more than the most subgraphsearch finds.

## Maximum diversity

*Reproduce with [benchmarks/diversity.py](benchmarks/diversity.py).*

The [maximum diversity problem](https://grafo.etsii.urjc.es/optsicom/mdp.html) picks $`k`$ of
$`n`$ items so that the total distance between the chosen ones is as large as possible: a weighted
search, with the items as vertices and the distances as edge weights. Its standard benchmark,
MDPLIB, lists best known values from two-hour runs of heuristics made for the problem. Its SOM-b
instances have integer distances from 0 to 9. subgraphsearch aims at each best known value with 60
experiments of up to 20,000 moves and `kick_size=16`; the general solvers maximize the total for 10
seconds with the formulations above, and the table gives the best total each found (HiGHS and CBC,
which found nothing at 300 and 400 items, were not run on the largest).

![For each of fourteen MDPLIB instances, how far each method's best total after 10 seconds falls below the best known value: subgraphsearch matches every one; CP-SAT falls 0 to 9% short; SCIP, HiGHS and CBC fall 5% to 38% short where they find a solution, and HiGHS and CBC find none on the 300- and 400-item instances](assets/benchmarks_diversity.png)

| Items | Chosen | Best known | subgraphsearch | CP-SAT | SCIP | HiGHS | CBC |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 100 | 10 | 333 | 60 of 60 in 2&nbsp;ms | 329 | 206 | 209 | 221 |
| 100 | 20 | 1,195 | 60 of 60 in 5&nbsp;ms | 1,171 | 847 | 915 | 853 |
| 100 | 30 | 2,457 | 60 of 60 in 5&nbsp;ms | 2,457 | 2,101 | 1,911 | 1,940 |
| 100 | 40 | 4,142 | 60 of 60 in 2&nbsp;ms | 4,127 | 3,665 | 3,881 | 3,534 |
| 200 | 40 | 4,450 | 60 of 60 in 43&nbsp;ms | 4,263 | 3,830 | 3,582 | 3,426 |
| 300 | 60 | 9,689 | 60 of 60 in 30&nbsp;ms | 9,297 | 8,350 | none | none |
| 400 | 40 | 4,658 | 60 of 60 in 47&nbsp;ms | 4,374 | 3,635 | none | none |
| 400 | 80 | 16,956 | 57 of 60 in 0.11&nbsp;s | 15,595 | not run | not run | not run |
| 500 | 50 | 7,141 | 56 of 60 in 0.16&nbsp;s | 6,636 | 5,638 | not run | not run |
| 500 | 100 | 26,258 | 60 of 60 in 36&nbsp;ms | 23,802 | 23,300 | not run | not run |
| 500 | 150 | 56,572 | 60 of 60 in 27&nbsp;ms | 52,257 | 52,137 | not run | not run |
| 500 | 200 | 97,344 | 60 of 60 in 49&nbsp;ms | 91,945 | 92,174 | not run | not run |

CP-SAT ends 0.4% to 9.4% short of the best known values, and the mixed-integer solvers 5% to 38%
where they find anything; none can show a value optimal (CP-SAT's upper bounds lie 17% to 60%
above). subgraphsearch does not need the targets: `ss.densest`, aiming at no particular total,
reaches every best known value in all 60 of 60 experiments of up to 50,000 moves, and never exceeds
one, consistent with the values being optimal.

**Real-valued distances.** MDPLIB's GKD-c instances hold Euclidean distances and its MDG-a
instances random real distances from 0 to 10. `ss.WeightedGraph.from_scores` rounds them to integer
weights (the largest becomes 255), CP-SAT got them scaled by 10 and 25, and every selection is
scored with the original distances. subgraphsearch ran `ss.densest` as above.

| Instance | Items | Chosen | Best known | subgraphsearch | CP-SAT | SCIP |
| --- | --- | --- | --- | --- | --- | --- |
| GKD-c 1 | 500 | 50 | 19,485.19 | 19,485.19 in 30 of 60 runs, the rest within 0.43, 1.2&nbsp;s | 19,329.0 | 15,997.4 |
| MDG-a 1 | 500 | 50 | 7,833.83 | 7,833.83 in all 60 runs, 0.36&nbsp;s | 7,294.0 | 6,229.7 |

Matching is to within 0.003, the precision of the published distances. The GKD-c misses are
rounding: some slightly lighter selections tie with the best one at integer weights. Among tied
selections, `ss.densest` returns the one with the highest exact total, so its result is the best
known value whenever any of its runs reaches it, as at least one does here.

**Against specialists.** These instances are easy for the heuristics made for the problem, whose
papers test on 2000 to 5000 items. On MDG-a_21 (200 of 2000 items, integer distances 0 to 10, best
known value 114,271), OBMA (Y. Zhou, J.-K. Hao and B. Duval, *IEEE Transactions on Evolutionary
Computation* 21, 2017) reaches the best known value in 22 of 30 runs of 20 s, on one core of a
2.5 GHz Xeon E5-2670, a 2012 processor slower than one core here. Given the same 20 s per run on one
core here, subgraphsearch reaches it:

| Settings | Moves in 20 s | Runs reaching 114,271 | Median run | Worst run |
| --- | ---: | ---: | ---: | ---: |
| default | 1.8 million | 7 of 30 | 2 short | 8 short |
| `aspiration="full"` | 1.5 million | 27 of 30 | reaches it | 6 short |

No run exceeds it. The default [tabu tenure](ALGORITHM.md#objective-and-moves), 20 here, matters
most: with the original tenure of 1, the median run of 200,000 moves ends about 300 short, against
19 with it. The full [aspiration criterion](ALGORITHM.md#variants) then lets swaps the tenure
forbids through whenever they beat the best total so far, which turns most near misses into hits;
it costs about 15% of the moves. With one instance, and OBMA's result from slower hardware, the
comparison shows the two in the same range rather than one ahead. On three
more 2000-item instances, MDG-a_22 to 24, at the same budgets, the full criterion raised the median
run on all three. On MDG-a_22 it raised the runs reaching MDPLIB's best value from 13 to 28 of 30;
on MDG-a_24 its worst run ended lower, 31 below MDPLIB's value where every default run exceeded it.
Both settings beat MDPLIB's values for MDG-a_23 and 24, from 2-hour runs of an earlier heuristic, by
up to 72.

## Maximum cliques

*Reproduce with [benchmarks/cliques.py](benchmarks/cliques.py).*

Maximum cliques have specialists of their own. On eight graphs of the
[DIMACS benchmark](https://networkrepository.com/dimacs.php), subgraphsearch aims at each known
clique number with 32 experiments of up to 20,000 moves. Beside it are DLS-MC, a local search made
for cliques, from its published results (W. Pullan and H. H. Hoos, *Journal of Artificial
Intelligence Research* 25, 2006; mean time per run on one core of a 2.2 GHz Pentium IV, several
times slower than today's processors), and igraph's exact clique number, stopped after 90 seconds.

| Graph | Vertices | Largest clique | subgraphsearch | DLS-MC | igraph (exact) |
| --- | --- | --- | --- | --- | --- |
| C125.9 | 125 | 34 | 32 of 32 in 3&nbsp;ms | 0.1&nbsp;ms | over 90&nbsp;s |
| C250.9 | 250 | 44 | 32 of 32 in 10&nbsp;ms | 0.9&nbsp;ms | over 90&nbsp;s |
| p_hat300-3 | 300 | 36 | 32 of 32 in 6&nbsp;ms | 0.7&nbsp;ms | over 90&nbsp;s |
| MANN_a27 | 378 | 126 | 17 of 32 in 153&nbsp;ms | 48&nbsp;ms | over 90&nbsp;s |
| hamming8-4 | 256 | 16 | 32 of 32 in 1&nbsp;ms | under 0.1&nbsp;ms | 18&nbsp;s |
| keller4 | 171 | 11 | 32 of 32 in 1&nbsp;ms | under 0.1&nbsp;ms | 1.8&nbsp;s |
| brock200_2 | 200 | 12 | 0 of 32 (11 at most) in 83&nbsp;ms | 24&nbsp;ms | 0.3&nbsp;s |
| brock400_2 | 400 | 29 | 0 of 32 (25 at most) in 0.20&nbsp;s | 0.48&nbsp;s | over 90&nbsp;s |

subgraphsearch is far faster than the exact search, but DLS-MC is faster still and solves the
*brock* graphs, built to hide their largest clique from local search, where subgraphsearch ends on
a smaller one. A [tabu tenure](TUNING.md#settings) of 5 instead of the default 3 finds MANN_a27's
clique in 30 of 32 experiments; no tenure from 1 to 20 gets closer than one edge on brock200_2 or
six on brock400_2.

## Steiner triple systems

*Reproduce with [benchmarks/steiner.py](benchmarks/steiner.py).*

A Steiner triple system on $`v`$ points is a set of triples containing every pair of points
exactly once: an independent set of $`v(v-1)/6`$ vertices in the graph of all
$`\binom{v}{3}`$ triples, joined when they share a pair. A method made for these designs,
D. R. Stinson's hill-climbing (*Annals of Discrete Mathematics* 26, 1985), builds one directly: it
keeps adding a triple through an uncovered pair, removing whichever triple already covered one of
its other pairs. Each method here builds one system, and every system was checked pair by pair:
subgraphsearch with one experiment and `tabu_tenure=2` (median of 3 runs), and Stinson's method
written in plain Python, on one core (median of 5).

| Points | Triples | subgraphsearch: graph, then search | Stinson's hill-climbing |
| --- | --- | --- | --- |
| 31 | 155 | 0.02&nbsp;s, then 0.01&nbsp;s | 0.9&nbsp;ms |
| 49 | 392 | 0.30&nbsp;s, then 0.19&nbsp;s | 2.6&nbsp;ms |
| 61 | 610 | 1.1&nbsp;s, then 1.1&nbsp;s | 4.0&nbsp;ms |
| 81 | 1,080 | 6.3&nbsp;s, then 15&nbsp;s | 7.3&nbsp;ms |

The specialist is thousands of times faster and goes much further: 999 points, 166,167 triples, in
2.3 s. subgraphsearch's graph has a vertex for every triple, so beyond about 100 points it no
longer fits in memory. Steiner triple systems show the general search finding a classical design
knowing nothing about designs, not a way to build them in practice.

## Snake-in-the-box

*Reproduce with [benchmarks/hypercubes.py](benchmarks/hypercubes.py).*

The longest induced paths in the $`n`$-dimensional hypercube, *snakes*, are a long-studied search
problem, with records set by specialized searches.

| Hypercube | Longest known snake | subgraphsearch |
| --- | --- | --- |
| 7-cube | 51 vertices, the longest possible | 51, in 313 of 400 experiments, 0.44&nbsp;s |
| 8-cube | 99 vertices, the longest possible | 99, in 3 of 400 experiments, 21&nbsp;s |
| 9-cube | 192 vertices (2026) | 185 in 2.5 minutes; 189 through a fixed 95-vertex snake in one half, in 35&nbsp;s |

The general path search reaches the records up to the 8-cube; the 9-cube's record came from a
specialized annealing search.

## In short

subgraphsearch is the tool for subgraphs with a prescribed number of vertices and edges or total
weight, where no specialized solver exists: it finds them in milliseconds where general solvers
stall, and matches the best published maximum diversity values up to 500 items. At 2000 items,
with `aspiration="full"`, it reaches the best known value about as often as a heuristic made for
the problem, in the same time. Where specialists exist elsewhere they win at scale: on maximum
cliques, on Steiner triple systems, and on the 9-cube's snake. The [quickstart notebook](https://github.com/vglazer/subgraphsearch/blob/main/examples/quickstart.ipynb)
runs several of these searches, and [examples/data](examples/data/README.md) holds the benchmark
instances and their licenses.
