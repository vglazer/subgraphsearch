# subgraphsearch-core

Find [induced subgraphs](https://en.wikipedia.org/wiki/Induced_subgraph) with a given number of
vertices and edges: [cliques](https://en.wikipedia.org/wiki/Clique_(graph_theory)),
[independent sets](https://en.wikipedia.org/wiki/Independent_set_(graph_theory)) and everything in
between. It also finds [regular](https://en.wikipedia.org/wiki/Regular_graph) subgraphs and
subgraphs of a given total weight in weighted graphs. The search is a randomized
[tabu search](https://en.wikipedia.org/wiki/Tabu_search) whose experiments run in parallel, with
results that depend only on the seed.

This is the library behind the [`subgraphsearch`](https://crates.io/crates/subgraphsearch) command
and the [Python package](https://pypi.org/project/subgraphsearch/). It also has random graph
generators, graph operations (complement, induced subgraphs, common neighborhoods, Seidel
switching), and readers and writers for graph6, sparse6, Matrix Market, edge lists, DOT and
GraphML.

```rust
use subgraphsearch_core::{Graph, SearchParams, search};

// find induced 4-vertex paths (3 edges) in a 12-cycle
let g = Graph::from_edges(12, (0..12).map(|i| (i, (i + 1) % 12))).unwrap();
let result = search(&g, &SearchParams::new(4, 3)).unwrap();
let found = result.experiments.iter().find(|e| e.success).unwrap();
assert_eq!(g.induced_edge_count(&found.vertices), 3);
```

`SearchParams` sets the number of experiments and moves, fixed vertices, a regular-subgraph
target and the other settings. The [project README](https://github.com/vglazer/subgraphsearch#readme)
explains how the search works and how to choose its settings.

Licensed under the MIT license.
