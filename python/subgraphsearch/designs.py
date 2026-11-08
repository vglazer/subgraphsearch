"""Small incidence structures (block designs) to build graphs from with
:meth:`Graph.point_graph <subgraphsearch.Graph.point_graph>` and
:meth:`Graph.block_graph <subgraphsearch.Graph.block_graph>`.

>>> import subgraphsearch as ss
>>> ss.Graph.point_graph(ss.designs.FANO_PLANE).num_edges   # every two points share a line: K7
21
"""

from __future__ import annotations

#: The Fano plane, the smallest projective plane: 7 points and 7 lines of 3 points, every two points
#: on exactly one line and every two lines meeting in exactly one point. The labels match
#: :func:`subgraphsearch.viz.plot_fano_plane`: 0, 1 and 2 are the corners of a triangle, 3, 4 and 5
#: the midpoints of its sides and 6 its center, and the last line is the circle through the midpoints.
FANO_PLANE: tuple[tuple[int, int, int], ...] = (
    (0, 1, 3),
    (1, 2, 4),
    (0, 2, 5),
    (0, 4, 6),
    (1, 5, 6),
    (2, 3, 6),
    (3, 4, 5),
)

__all__ = ["FANO_PLANE"]
