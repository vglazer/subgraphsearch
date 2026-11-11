# Benchmarks

*Part of the [subgraphsearch documentation](README.md#documentation).*

subgraphsearch is one general search: it finds any number of vertices and edges, total weights,
and induced paths, cycles, trees and cliques with the same method. Below it is set beside the
methods one would otherwise use, on problems with public benchmarks or known records. Times are wall-clock on an Apple M3 Max
laptop with 16 cores; subgraphsearch's are for all its experiments together.

![Left: CP-SAT's time to find k vertices spanning exactly m edges grows to seconds and then past its 10-second limit as random graphs grow from 20 to 59 vertices. Middle: subgraphsearch finds the same targets in about a millisecond or less. Right: on eight maximum diversity benchmark instances, subgraphsearch reaches every best known value, while CP-SAT after 10 seconds falls up to 8% short](assets/comparison.png)

## Exact edge counts

No specialized solver finds $`k`$ vertices spanning exactly $`m`$ edges, so the alternatives are
general solvers, each stopped after 10 seconds:

- OR-Tools' [CP-SAT](https://developers.google.com/optimization/cp/cp_solver), a constraint
  solver, on 16 cores, with one yes-or-no variable per vertex and the edge count as half the sum
  of the chosen vertices' degrees inside the set;
- the open-source mixed-integer programming solvers [SCIP](https://www.scipopt.org),
  [HiGHS](https://highs.dev) and [CBC](https://github.com/coin-or/Cbc) (CBC on 16 cores), through
  PuLP, with the standard formulation: a yes-or-no variable per vertex and per edge, the edge's
  variable 1 exactly when both its ends are chosen.

The graphs are random, with density 0.6; the target is the most edges subgraphsearch finds among
30% of the vertices, the hardest kind of target.

![Left: the times of CP-SAT, SCIP, HiGHS and CBC on graphs of 20 to 59 vertices. All four find solutions at 20 vertices in under a second; at 29 vertices the MIP solvers take 0.8 to 3 seconds; from 38 vertices on only CP-SAT finds anything within 10 seconds, at 38 and 50 vertices. Right: subgraphsearch finds every target in under a millisecond](assets/benchmarks_edges.png)

| Graph | Target | subgraphsearch | CP-SAT | SCIP | HiGHS | CBC |
| --- | --- | --- | --- | --- | --- | --- |
| 20 vertices | 6 vertices, 15 edges | 0.13&nbsp;ms | 25&nbsp;ms | 0.10&nbsp;s | 0.35&nbsp;s | 0.51&nbsp;s |
| 29 vertices | 9 vertices, 34 edges | 0.13&nbsp;ms | 0.41&nbsp;s | 0.78&nbsp;s | 3.0&nbsp;s | 2.6&nbsp;s |
| 38 vertices | 11 vertices, 52 edges | 0.15&nbsp;ms | 4.5&nbsp;s | nothing | nothing | nothing |
| 44 vertices | 13 vertices, 70 edges | 0.17&nbsp;ms | nothing | nothing | nothing | nothing |
| 50 vertices | 15 vertices, 91 edges | 0.83&nbsp;ms | 8.8&nbsp;s | nothing | nothing | nothing |
| 59 vertices | 18 vertices, 129 edges | 0.31&nbsp;ms | nothing | nothing | nothing | nothing |

"Nothing" means no solution within 10 seconds.

subgraphsearch ran 60 experiments of up to 2,000 moves, and every one succeeded. What the general
solvers can do and subgraphsearch cannot is prove a target impossible: one edge more on the 30-vertex graph of the
same kind, CP-SAT proves in 0.9 s that none exists.

## Maximum diversity

The [maximum diversity problem](https://grafo.etsii.urjc.es/optsicom/mdp.html) picks $`k`$ of
$`n`$ items so that the total distance between the chosen ones is as large as possible: a weighted
search, with the items as vertices and the distances as edge weights. Its standard benchmark,
MDPLIB, lists best known values found by two-hour runs of a scatter search made for the problem;
the specialist heuristics that lead on it, such as iterated tabu search, reach them too. Its SOM-b
instances have integer distances from 0 to 9. Here subgraphsearch aims at each best known value
with 60 experiments of up to 20,000 moves and `kick_size=16`. The general solvers maximize the
total distance for 10 seconds, with the same formulations as above, and the table gives the best
total each found. HiGHS and CBC, which found nothing at 300 and 400 items, were not run on the
largest instances.

![For each of fourteen MDPLIB instances, how far each method's best total after 10 seconds falls below the best known value: subgraphsearch matches every one; CP-SAT falls 0 to 9% short; SCIP, HiGHS and CBC fall 5% to 38% short where they find a solution, and HiGHS and CBC find none on the 300- and 400-item instances](assets/benchmarks_diversity.png)

| Items | Chosen | Best known | subgraphsearch | CP-SAT | SCIP | HiGHS | CBC |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 100 | 10 | 333 | 60 of 60 in 2&nbsp;ms | 329 | 206 | 209 | 221 |
| 100 | 20 | 1,195 | 60 of 60 in 16&nbsp;ms | 1,171 | 847 | 915 | 853 |
| 100 | 30 | 2,457 | 60 of 60 in 10&nbsp;ms | 2,457 | 2,101 | 1,911 | 1,940 |
| 100 | 40 | 4,142 | 60 of 60 in 3&nbsp;ms | 4,127 | 3,665 | 3,881 | 3,534 |
| 200 | 40 | 4,450 | 56 of 60 in 183&nbsp;ms | 4,263 | 3,830 | 3,582 | 3,426 |
| 300 | 60 | 9,689 | 59 of 60 in 0.42&nbsp;s | 9,297 | 8,350 | none | none |
| 400 | 40 | 4,658 | 39 of 60 in 0.70&nbsp;s | 4,374 | 3,635 | none | none |
| 400 | 80 | 16,956 | 41 of 60 in 1.1&nbsp;s | 15,595 | not run | not run | not run |
| 500 | 50 | 7,141 | 28 of 60 in 1.1&nbsp;s | 6,636 | 5,638 | not run | not run |
| 500 | 100 | 26,258 | 59 of 60 in 0.9&nbsp;s | 23,802 | 23,300 | not run | not run |
| 500 | 150 | 56,572 | 60 of 60 in 0.5&nbsp;s | 52,257 | 52,137 | not run | not run |
| 500 | 200 | 97,344 | 59 of 60 in 1.2&nbsp;s | 91,945 | 92,174 | not run | not run |

CP-SAT comes within 0.4% to 9.4% of the best known values, and the mixed-integer solvers within 5%
to 38%, where they find a solution at all. None of them can show that any value is optimal: CP-SAT's
proved upper bounds, for example, lie 17% to 60% above the best known values.

The result does not depend on SOM-b's small integer distances. MDPLIB's GKD-c instances hold
Euclidean distances between random points and its MDG-a instances random real distances from 0 to
10. subgraphsearch's weights are integers, so for it and CP-SAT the distances were scaled (by 10
and by 25) and rounded; subgraphsearch then aimed above any reachable total and kept the heaviest
selection it found. Every method's selection is scored with the original distances.

| Instance | Items | Chosen | Best known | subgraphsearch | CP-SAT | SCIP |
| --- | --- | --- | --- | --- | --- | --- |
| GKD-c 1 | 500 | 50 | 19,485.19 | 19,485.19 in all 60 runs, 1.5&nbsp;s | 19,329.0 | 15,997.4 |
| MDG-a 1 | 500 | 50 | 7,833.83 | 7,833.83 in all 60 runs, 1.6&nbsp;s | 7,294.0 | 6,229.7 |

subgraphsearch's totals differ from the best known values by less than 0.003, below the precision
of the distances as published, so it matches them; CP-SAT falls 0.8% and 6.9% short, SCIP 18% and
20%.

## Maximum cliques

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

## Snake-in-the-box

The longest induced paths in the $`n`$-dimensional hypercube, *snakes*, are a long-studied search
problem, with records set by specialized searches.

| Hypercube | Longest known snake | subgraphsearch |
| --- | --- | --- |
| 7-cube | 51 vertices, the longest possible | 51, in 313 of 400 experiments, 0.5&nbsp;s |
| 8-cube | 99 vertices, the longest possible | 99, in 3 of 400 experiments, 21&nbsp;s |
| 9-cube | 192 vertices (2026) | 185 in under 3 minutes; 189 through a fixed 95-vertex snake in one half, in 36&nbsp;s |

It reaches the records up to the 8-cube with its general path search, and comes within 3 vertices
of the 9-cube's.

## In short

subgraphsearch is the tool for subgraphs with a prescribed number of vertices and edges or a
prescribed total weight: there it is fast where general solvers stall, and on maximum diversity it
matches the best published values. For maximum cliques, a dedicated clique solver is the better
choice. The [quickstart notebook](https://github.com/vglazer/subgraphsearch/blob/main/examples/quickstart.ipynb)
runs several of these searches, and [examples/data](examples/data/README.md) holds the benchmark
instances and their licenses.
