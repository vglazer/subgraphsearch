# subgraphsearch

Find [induced subgraphs](https://en.wikipedia.org/wiki/Induced_subgraph) with a given number of
vertices and edges from the command line: [cliques](https://en.wikipedia.org/wiki/Clique_(graph_theory)),
[independent sets](https://en.wikipedia.org/wiki/Independent_set_(graph_theory)) and everything in
between, [regular](https://en.wikipedia.org/wiki/Regular_graph) subgraphs, and subgraphs of a given
total weight. The command also generates random graphs, converts between graph formats, and
applies graph operations.

```sh
cargo install subgraphsearch
```

The same command comes with the Python package (`pip install subgraphsearch` or
`uv tool install subgraphsearch`), which needs no Rust toolchain.

```sh
subgraphsearch generate exponential -n 100 -d 0.6 -s 1 -o g.g6    # a random graph
subgraphsearch search g.g6 -k 11 -m 55                            # 11-cliques
subgraphsearch search g.g6 -k 30 -m 340 --max-conflict 2          # also report near misses
subgraphsearch search g.g6 -k 10 --degree 4                       # 4-regular subgraphs
subgraphsearch info g.g6                                          # statistics
subgraphsearch convert g.g6 g.dot --highlight 3,17,42             # draw a subgraph with Graphviz
subgraphsearch op complement g.g6 gc.g6                           # complements, induced subgraphs, ...
```

`subgraphsearch --help` lists every command and option. The search itself is the
[`subgraphsearch-core`](https://crates.io/crates/subgraphsearch-core) library; the
[project README](https://github.com/vglazer/subgraphsearch#readme) explains how it works and how to
choose its settings.

Licensed under the MIT license.
