subgraphsearch
==============

Find induced subgraphs of a given size that hit a prescribed target: a number of edges, a degree
for every vertex, or a total weight. This is the reference for the Python package; the
`README <https://github.com/vglazer/subgraphsearch#readme>`_ introduces it, the
`quickstart notebook <https://github.com/vglazer/subgraphsearch/blob/main/examples/quickstart.ipynb>`_
tours it, and the guides cover
`how the search works <https://github.com/vglazer/subgraphsearch/blob/main/ALGORITHM.md>`_,
`how to tune it <https://github.com/vglazer/subgraphsearch/blob/main/TUNING.md>`_,
`graphs and formats <https://github.com/vglazer/subgraphsearch/blob/main/GRAPHS.md>`_ and
`benchmarks <https://github.com/vglazer/subgraphsearch/blob/main/BENCHMARKS.md>`_. The Rust
library has its own reference on `docs.rs <https://docs.rs/subgraphsearch-core>`_.

.. code-block:: python

   import subgraphsearch as ss

   g = ss.generate("exponential", 100, 0.6, seed=1)
   r = ss.search(g, size=11, edges=55)   # 11-cliques
   print(r.summary())

.. toctree::
   :maxdepth: 2

   api
