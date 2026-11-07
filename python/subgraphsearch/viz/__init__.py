"""Visualisation.

- Search behaviour: :func:`plot_traces`, :func:`plot_run_time_distribution`,
  :func:`plot_success_times`, :func:`plot_conflicts`, :func:`plot_search_summary`,
  :func:`plot_sweep`.
- Graph structure: :func:`plot_degree_distribution`, :func:`plot_spectrum`,
  :func:`plot_adjacency`, :func:`plot_graph_summary`, plus :func:`laplacian_spectrum` and
  :func:`spectral_summary` for the numbers.
- Interactive: :func:`to_html`, :func:`save_html` and :func:`show` (Jupyter) produce a
  self-contained page for exploring a graph and the subgraphs a search found.

The matplotlib functions need ``pip install 'subgraphsearch[viz]'``; the interactive page needs
nothing extra.
"""

from .diagnostics import (
    plot_conflicts,
    plot_run_time_distribution,
    plot_search_summary,
    plot_success_times,
    plot_sweep,
    plot_traces,
)
from .interactive import save_html, show, to_html
from .spectral import (
    laplacian_spectrum,
    plot_adjacency,
    plot_degree_distribution,
    plot_graph_summary,
    plot_spectrum,
    spectral_summary,
)

__all__ = [
    "laplacian_spectrum",
    "plot_adjacency",
    "plot_conflicts",
    "plot_degree_distribution",
    "plot_graph_summary",
    "plot_run_time_distribution",
    "plot_search_summary",
    "plot_spectrum",
    "plot_success_times",
    "plot_sweep",
    "plot_traces",
    "save_html",
    "show",
    "spectral_summary",
    "to_html",
]
