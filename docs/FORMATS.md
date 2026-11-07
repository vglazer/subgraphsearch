# Graph formats

USRA's programs exchanged graphs as '-1'-terminated adjacency lists: line *i* lists the neighbours
*j > i* of vertex *i*. The format is compact and simple, but nothing else reads it. `subsearch`
still reads and writes it (`lists`, `.txt`) for compatibility, and adds the standard formats below.

| Format | Name | Extensions | Read | Write | Best for |
|---|---|---|---|---|---|
| graph6 | `graph6` | `.g6` | ✓ | ✓ | dense or medium graphs (one or many per file); the default output |
| sparse6 | `sparse6` | `.s6` | ✓ | ✓ | sparse graphs (one or many per file) |
| Matrix Market | `mtx` | `.mtx` | ✓ | ✓ | SciPy, MATLAB, Julia, numerical code |
| edge list | `edgelist` | `.edges`, `.csv`, `.tsv` | ✓ | ✓ | spreadsheets, shell tools, anything |
| USRA lists | `lists` | `.txt` | ✓ | ✓ | the original C programs |
| Graphviz | `dot` | `.dot`, `.gv` | | ✓ | drawing with Graphviz, highlighting a subgraph |
| GraphML | `graphml` | `.graphml` | | ✓ | Gephi, Cytoscape, yEd |

## Sizes

The same graphs in each format (generated with `subsearch generate ... -s 1`):

| Graph | Edges | graph6 | sparse6 | USRA lists | edge list | Matrix Market | GraphML |
|---|---:|---:|---:|---:|---:|---:|---:|
| exponential, 100 vertices, density 0.6 | 2,969 | **0.8 KB** | 3.9 KB | 9.0 KB | 16.8 KB | 17.0 KB | 112 KB |
| geometric, 790 vertices, 0.15 | 38,440 | **51 KB** | 69 KB | 152 KB | 290 KB | 290 KB | 1.5 MB |
| power, 2,500 vertices, 0.2 | 615,771 | **508 KB** | 1.3 MB | 2.9 MB | 5.5 MB | 5.5 MB | 24 MB |
| exponential, 5,000 vertices, 0.002 | 25,028 | 2.0 MB | **58 KB** | 136 KB | 234 KB | 234 KB | 1.1 MB |

graph6 stores one bit per vertex pair, so its size depends only on the number of vertices
(`n(n-1)/12` bytes). sparse6 stores roughly `log2(n)` bits per edge. As a rule of thumb, graph6 is
smaller once the density exceeds about `log2(n) / n`: around 7% at 100 vertices, 0.25% at 5,000.

## Recommendations

- **Use graph6** for the dense, medium-sized graphs `sub_search` is typically run on. It is the
  standard in combinatorics: nauty's `geng` produces it; SageMath, networkx and igraph read it;
  and collections such as [House of Graphs](https://houseofgraphs.org/) and Brendan McKay's and
  Ted Spence's lists of strongly regular graphs are distributed in it.
- **Use sparse6** for large sparse graphs.
- **Keep families of graphs in one graph6 or sparse6 file**, one graph per line, which is how
  `geng` and graph catalogues deliver them. `subsearch info many.g6` lists them,
  `subsearch search many.g6 --all ...` searches each in turn, `--index N` picks one, and
  `subsearch convert many.g6 many.s6` converts them all. In Python: `ss.read_all`,
  `ss.read(path, index=N)` and `ss.write_all`. A command that needs one graph refuses a
  multi-graph file unless given `--index`, rather than silently using the first graph.
- **Use an edge list** for exchange with tools that know nothing about graphs. `subsearch` writes
  a `# vertices: N` header, so isolated vertices survive the round trip.
- **Use Matrix Market** to move adjacency matrices into numerical code
  (`scipy.io.mmread`). The reader accepts `pattern`, `integer` and `real` matrices, `general` or
  `symmetric`; any non-zero off-diagonal entry is an edge.
- **Use DOT or GraphML** to draw. Both take a vertex set to highlight
  (`subsearch convert g.g6 g.dot --highlight 3,17,42`), which colours a found subgraph.

## Not supported, and why

- **Weighted graphs** (`wsub_search`'s format) are planned together with the weighted search.
- **JSON node-link** (d3, networkx `node_link_data`): use `subsearch.convert.to_networkx` and
  networkx's own JSON support. networkx is changing its default key from `links` to `edges`, so a
  fixed writer here would soon mismatch.
- **Reading DOT and GraphML** would need full parsers for formats that are mostly used for
  output; convert with networkx (`read_graphml`, `nx_pydot.read_dot`) and
  `subsearch.convert.from_networkx` instead.
