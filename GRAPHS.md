# Graphs

*Part of the [subgraphsearch documentation](README.md#documentation).*

subgraphsearch searches graphs read from files, built from edges or converted from other libraries
(networkx, igraph, SciPy, NumPy), or made by its own generators and operations. This page describes
the generators and operations and the file formats.

## Random graphs

- **exponential**: [Erdős–Rényi](https://en.wikipedia.org/wiki/Erd%C5%91s%E2%80%93R%C3%A9nyi_model);
  each pair of vertices is an edge independently with probability equal to the density. With
  `mean_degree=d` (`--mean-degree d`) instead of a density, the probability is $`d / (n - 1)`$ for
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
- **regular** (`ss.random_regular(n, d)`, `-n n --degree d`): a random $`d`$-regular graph, close
  to uniform among them; with high probability an expander nearly as good as possible.
- **hypercube** (`ss.hypercube(d)`, `--dimension d`): the
  [hypercube](https://en.wikipedia.org/wiki/Hypercube_graph) on the $`2^d`$ bit strings of length
  $`d`$, joined when they differ in one bit, up to 16 dimensions. Not random: the home of the
  snake-in-the-box and coil-in-the-box problems.
- **grid** (`ss.grid(r, c)`, `--rows r --cols c`): the $`r`$ by $`c`$ grid, vertex $`ic + j`$ in
  row $`i`$ and column $`j`$.

## Operations

Graph operations help focus a search. Searching the
[complement](https://en.wikipedia.org/wiki/Complement_graph) is equivalent, since $`k`$ vertices
with $`m`$ edges become $`k`$ vertices with $`k(k-1)/2 - m`$, and for a dense graph the complement
is sparse and cheaper to work with. Inducing a subgraph on the common neighborhood of some vertices
restricts the search to vertices adjacent to all of them, and `switch` performs Seidel switching
(see [ALGORITHM.md](ALGORITHM.md#variants)) with respect to a vertex set, such as a regular
subgraph found by the search.

Graphs also combine into larger ones: the Cartesian, tensor, strong and lexicographic products, and
the replacement and zig-zag products, which build expanders from smaller graphs
(`g.cartesian(h)` and the like; `subgraphsearch op cartesian g.g6 out.g6 --with h.g6` on the
command line).

## Graphs from vectors and words

`ss.Graph.from_vectors(vectors, inner_products)` joins two vectors when their inner product is one
of those given, and `ss.Graph.from_words(words, distances)` joins two binary words when their
Hamming distance is one of those given. Codes, Hadamard matrices and sets of equiangular lines are
then cliques or independent sets of these graphs; `ss.vectors` has helpers to make and check them.

## Incidence structures

Graphs can also be built from an
[incidence structure](https://en.wikipedia.org/wiki/Incidence_structure), a set of points with a
family of subsets called blocks, such as a
[block design](https://en.wikipedia.org/wiki/Block_design). Its *point graph* joins two points when
exactly $`k`$ blocks contain both, and its *block graph* joins two blocks when they share exactly
$`k`$ points (`ss.Graph.point_graph` and `ss.Graph.block_graph`; `subgraphsearch incidence` on the
command line, with one block per line). Many strongly regular graphs arise this way. The block graph of
all pairs from 8 points is the [triangular graph](https://en.wikipedia.org/wiki/Triangular_graph)
$`T(8)`$, with parameters $`(28, 12, 6, 4)`$, and switching it with respect to four disjoint pairs,
an independent set the search finds, gives one of the
[Chang graphs](https://en.wikipedia.org/wiki/Chang_graphs): strongly regular with the same
parameters, but not isomorphic to $`T(8)`$.

## Formats

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
