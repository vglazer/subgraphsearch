"""Searching for induced subgraphs with a prescribed number of edges."""

from __future__ import annotations

import json
from collections.abc import Iterable, Sequence
from dataclasses import asdict, dataclass, field

import numpy as np

from . import _core
from ._graph import Graph, WeightedGraph

NUM_BOXES = 10


@dataclass(frozen=True)
class Experiment:
    """One independent run (restart) of the search."""

    index: int
    #: Smallest conflict reached, which is that of ``vertices``: ``|edges - target|``, or for a regular search the
    #: deviation from regularity, plus the components beyond the first with ``connected``. 0 exactly on success.
    best_conflict: int
    #: Moves made: the index of the successful move, or ``max_moves`` if none succeeded.
    moves: int
    #: Whether the target was hit exactly (by a connected subgraph, with ``connected``).
    success: bool
    #: Sorted vertices (fixed ones included) of the subgraph that reached ``best_conflict``.
    vertices: tuple[int, ...]
    #: Objective value after each move, if the search recorded traces.
    trace: np.ndarray | None = field(default=None, repr=False, compare=False)


@dataclass(frozen=True)
class SearchParams:
    size: int
    edges: int
    fixed: tuple[int, ...] = ()
    experiments: int = 40
    max_moves: int = 250
    div_freq: int = 250
    div_dur: int = 4
    seed: int = 1
    rng: str = "xoshiro"
    trace: bool = False
    aspiration: bool = False
    #: For a regular-subgraph search (the original ``reg_search``): the degree; ``edges`` is then unused.
    degree: int | None = None
    #: With ``degree``: first find the right number of edges (the original ``ereg_search``).
    two_phase: bool = False
    #: Require a connected subgraph: each component beyond the first adds 1 to the conflict.
    connected: bool = False
    #: Search for induced paths on ``size`` vertices; ``edges`` is then unused.
    path: bool = False
    #: With ``path``: the two vertices the path must run between, as its ends.
    endpoints: tuple[int, int] | None = None
    #: Region kicks: see :func:`search`. 0 means off.
    kick_size: int = 0
    kick_ban: int = 20


@dataclass
class SearchResult:
    """The outcome of :func:`search`: parameters, the graph searched, and every experiment."""

    graph: Graph | WeightedGraph
    params: SearchParams
    experiments: list[Experiment]

    @property
    def target(self) -> str:
        """What was searched for: "with 12 edges", "with total weight 30", "regular of degree 3",
        each followed by ", connected" for a ``connected`` search; or, for the three shapes with a
        name, "forming a path" (or "forming a path from 0 to 127", with ``endpoints``), "forming a
        cycle" (connected and regular of degree 2) or "forming a tree" (connected, with one edge
        fewer than vertices)."""
        p = self.params
        if p.path:
            return f"forming a path from {p.endpoints[0]} to {p.endpoints[1]}" if p.endpoints else "forming a path"
        if p.connected and p.degree == 2:
            return "forming a cycle"
        if p.connected and p.degree is None and not self.weighted and p.edges == p.size - 1:
            return "forming a tree"
        if p.degree is not None:
            target = f"regular of degree {p.degree}"
        else:
            target = f"with total weight {p.edges}" if self.weighted else f"with {p.edges} edges"
        return f"{target}, connected" if p.connected else target

    @property
    def weighted(self) -> bool:
        """Whether the search was for a total weight in a :class:`WeightedGraph`."""
        return isinstance(self.graph, WeightedGraph)

    @property
    def successes(self) -> int:
        return sum(e.success for e in self.experiments)

    @property
    def success_rate(self) -> float:
        return self.successes / len(self.experiments) if self.experiments else float("nan")

    @property
    def relative_cost(self) -> float:
        """Mean moves per success, over successful experiments only."""
        s = [e.moves for e in self.experiments if e.success]
        return float(np.mean(s)) if s else float("nan")

    @property
    def true_cost(self) -> float:
        """Total moves over all experiments divided by the number of successes."""
        return sum(e.moves for e in self.experiments) / self.successes if self.successes else float("nan")

    @property
    def average_conflict(self) -> float:
        """Mean best conflict over all experiments (0 when every experiment succeeds)."""
        return float(np.mean([e.best_conflict for e in self.experiments])) if self.experiments else float("nan")

    def boxes(self) -> np.ndarray:
        """Successful experiments binned by moves needed into ``NUM_BOXES`` equal boxes over
        ``[0, max_moves]``, as ``sub_search`` reports them."""
        size = self.params.max_moves // NUM_BOXES + 1
        out = np.zeros(NUM_BOXES, dtype=np.int64)
        for e in self.experiments:
            if e.success:
                out[e.moves // size] += 1
        return out

    def solutions(self, max_conflict: int = 0, unique: bool = True) -> list[tuple[int, ...]]:
        """Vertex sets of subgraphs within ``max_conflict`` edges of the target, best first."""
        found = sorted((e for e in self.experiments if e.best_conflict <= max_conflict), key=lambda e: e.best_conflict)
        sets = [e.vertices for e in found]
        return list(dict.fromkeys(sets)) if unique else sets

    @property
    def best(self) -> Experiment | None:
        """The experiment with the smallest conflict (the earliest, on ties)."""
        return min(self.experiments, key=lambda e: (e.best_conflict, e.index), default=None)

    def traces(self) -> list[np.ndarray]:
        if not self.params.trace:
            raise ValueError("this search did not record traces; pass trace=True to search()")
        return [e.trace for e in self.experiments]

    def summary(self) -> str:
        p = self.params
        lines = [
            f"{self.successes}/{len(self.experiments)} experiments found a {p.size}-vertex subgraph {self.target}",
            (f"relative cost {self.relative_cost:.1f} moves, true cost {self.true_cost:.1f} moves, "
             f"average conflict {self.average_conflict:.3f}"),
        ]
        return "\n".join(lines)

    def to_dict(self) -> dict:
        return {
            "params": asdict(self.params),
            "experiments": [
                {k: (v.tolist() if isinstance(v, np.ndarray) else v) for k, v in asdict(e).items()}
                for e in self.experiments
            ],
        }

    def to_json(self) -> str:
        return json.dumps(self.to_dict())

    @classmethod
    def from_dict(cls, data: dict, graph: Graph | WeightedGraph) -> SearchResult:
        """Rebuilds a result from :meth:`to_dict` output or from the JSON the ``subgraphsearch search
        --json`` command writes."""
        p = dict(data["params"])
        if "record_trace" in p:  # the Rust CLI's field names
            p["trace"] = p.pop("record_trace")
        regular = p.pop("regular", None)
        if regular:
            p["degree"], p["two_phase"] = regular["degree"], regular.get("two_phase", False)
        p["fixed"] = tuple(p.get("fixed", ()))
        if p.get("endpoints") is not None:
            p["endpoints"] = tuple(p["endpoints"])
        params = SearchParams(**{k: v for k, v in p.items() if k in SearchParams.__dataclass_fields__})
        exps = [
            Experiment(
                e.get("index", i),
                e["best_conflict"],
                e["moves"],
                e["success"],
                tuple(e["vertices"]),
                None if e.get("trace") is None else np.asarray(e["trace"], dtype=np.int64),
            )
            for i, e in enumerate(data["experiments"])
        ]
        return cls(graph, params, exps)

    @classmethod
    def load(cls, path, graph: Graph | WeightedGraph) -> SearchResult:
        """Reads a result saved with :meth:`save` or ``subgraphsearch search --json``."""
        with open(path, encoding="utf-8") as f:
            return cls.from_dict(json.load(f), graph)

    def save(self, path) -> None:
        with open(path, "w", encoding="utf-8") as f:
            f.write(self.to_json())

    def __repr__(self) -> str:
        p = self.params
        target = "path=True" if p.path else f"degree={p.degree}" if p.degree is not None else f"edges={p.edges}"
        return f"SearchResult({self.successes}/{len(self.experiments)} successes, size={p.size}, {target})"

    def _repr_html_(self) -> str:
        from ._html import search_html

        return search_html(self)


def search(
    graph: Graph | WeightedGraph,
    size: int,
    edges: int | None = None,
    *,
    degree: int | None = None,
    two_phase: bool = False,
    fixed: Iterable[int] = (),
    experiments: int = 40,
    max_moves: int = 250,
    div_freq: int = 250,
    div_dur: int = 4,
    seed: int = 1,
    rng: str = "xoshiro",
    trace: bool = False,
    aspiration: bool = False,
    connected: bool = False,
    path: bool = False,
    endpoints: tuple[int, int] | None = None,
    kick_size: int = 0,
    kick_ban: int = 20,
) -> SearchResult:
    """Searches ``graph`` for induced subgraphs on ``size`` vertices with ``edges`` edges, or, in
    a :class:`WeightedGraph`, with total weight ``edges`` (the original ``wsub_search``).

    Each of ``experiments`` independent runs starts from random vertices (always including
    ``fixed``) and makes up to ``max_moves`` swaps of one subgraph vertex for one outside vertex,
    minimizing ``|edges in subgraph - edges|``. It is a tabu search: locally optimal swaps are
    chosen from a restricted neighborhood, and the two vertices just swapped are tabu (cannot be
    swapped straight back) for one move. The first ``div_dur`` of every ``div_freq`` moves are
    random ("diversification"). With ``rng="xoshiro"`` (default) experiments run in parallel and depend
    only on ``seed``; ``rng="legacy"`` reproduces the original ``sub_search`` exactly.

    Set ``trace=True`` to record the objective after every move, for :mod:`subgraphsearch.viz`.

    Pass ``degree`` instead of ``edges`` to look for subgraphs that are regular of that degree
    (the original ``reg_search``); add ``two_phase=True`` to first find the right number of edges and
    then make the subgraph regular (``ereg_search``). Regular searches need an unweighted graph.

    ``aspiration=True`` adds the aspiration criterion of the original ``asub_search``: tabu swaps,
    those involving the two vertices just swapped, are taken whenever they would beat the best
    value found so far.

    ``connected=True`` requires a connected subgraph: each component beyond the first adds 1 to the
    conflict, so only connected subgraphs that hit the target count as successes, and near misses
    include disconnected ones that hit it. Swaps are still chosen by the target alone, so this suits
    targets that connected subgraphs often meet, such as ``degree=2`` for induced cycles rather than
    unions of them. It works with every kind of search. With ``edges=size - 1`` it finds induced
    trees, since a connected graph with one edge fewer than vertices is a tree.

    Pass ``path=True`` instead of ``edges`` or ``degree`` to look for induced paths on ``size``
    vertices. It uses the regular search's moves for degree 2, aimed at the degrees of a path (two
    endpoints of degree 1, the rest 2) rather than of a cycle; the conflict is 0 exactly for an
    induced path. Like regular searches, it needs an unweighted graph.

    With ``path=True``, ``endpoints=(s, t)`` asks for induced paths that run from ``s`` to ``t``.
    The search attaches a new vertex to each of them, which an induced path can only contain as an
    end, and looks for a path two vertices longer through all four; the results leave the new
    vertices out, so every subgraph returned, near misses included, contains ``s`` and ``t``.

    ``kick_size=k`` starts each diversification cycle with a *region kick* instead of random moves:
    the ``k`` free subgraph vertices nearest a random one (in breadth-first order from it, at
    whatever distance) are swapped out, one per move, each for the best vertex to swap in, and the
    vertices removed may not return for ``kick_ban`` moves. It rebuilds one part of the subgraph at a
    time, like the large-neighborhood moves of searches for codes. The best size depends on the
    problem: for some a few vertices, for others a large part of the subgraph, so compare several,
    for example with :func:`sweep`. Edge-count searches only.
    """
    if (edges is not None) + (degree is not None) + bool(path) != 1:
        raise ValueError(
            "give exactly one of edges (an edge count or total weight), degree (for a regular subgraph) "
            "and path=True (for an induced path)"
        )
    if two_phase and degree is None:
        raise ValueError("two_phase needs degree: it searches for regular subgraphs")
    if endpoints is not None:
        endpoints = tuple(int(v) for v in endpoints)
        if len(endpoints) != 2:
            raise ValueError("endpoints takes two vertices, the ends of the path")
    fixed = tuple(int(v) for v in fixed)
    params = SearchParams(
        int(size), 0 if edges is None else int(edges), fixed, int(experiments), int(max_moves), int(div_freq), int(div_dur),
        int(seed), rng, bool(trace), bool(aspiration), None if degree is None else int(degree), bool(two_phase),
        bool(connected), bool(path), endpoints, int(kick_size), int(kick_ban),
    )
    raw = _core.search(
        graph._g, params.size, params.edges, list(fixed), params.experiments, params.max_moves, params.div_freq,
        params.div_dur, params.seed, rng, params.trace, params.aspiration, params.degree, params.two_phase,
        params.connected, params.path, None if endpoints is None else list(endpoints), params.kick_size,
        params.kick_ban,
    )
    exps = [
        Experiment(i, c, m, s, tuple(v), None if t is None else np.asarray(t, dtype=np.int64))
        for i, (c, m, s, v, t) in enumerate(raw)
    ]
    return SearchResult(graph, params, exps)


def sweep(graph: Graph | WeightedGraph, size: int, edges: int | None, parameter: str, values: Sequence, **kwargs) -> dict:
    """Runs :func:`search` once per value of ``parameter`` (e.g. ``"div_dur"``), keeping the other
    arguments fixed, and returns ``{value: SearchResult}``; plot it with
    :func:`subgraphsearch.viz.plot_sweep`."""
    return {v: search(graph, size, edges, **{**kwargs, parameter: v}) for v in values}


SHAPES = ("path", "cycle", "tree", "clique", "independent")


@dataclass
class LargestResult:
    """The outcome of :func:`largest`: every search it made, by size."""

    graph: Graph
    #: ``"path"``, ``"cycle"``, ``"tree"``, ``"clique"`` or ``"independent"``.
    shape: str
    #: Every search made, by size, in increasing order of size.
    results: dict[int, SearchResult]

    @property
    def size(self) -> int | None:
        """The largest size at which a search succeeded, or ``None`` if none did."""
        return max((k for k, r in self.results.items() if r.successes), default=None)

    @property
    def best(self) -> SearchResult | None:
        """The search at :attr:`size`, whose solutions are the largest subgraphs found."""
        return None if self.size is None else self.results[self.size]

    @property
    def next(self) -> SearchResult | None:
        """The search at the smallest size tried above :attr:`size`, which failed: its near misses
        (``next.solutions(max_conflict=...)``) are the closest the search came to anything larger."""
        above = [k for k in self.results if self.size is None or k > self.size]
        return self.results[min(above)] if above else None

    def summary(self) -> str:
        noun = {"path": "induced path", "cycle": "induced cycle", "tree": "induced tree", "clique": "clique",
                "independent": "independent set"}[self.shape]
        tried = ", ".join(f"{k} ({r.successes}/{len(r.experiments)})" for k, r in self.results.items())
        head = (f"no {noun} found" if self.size is None
                else f"largest {noun} found: {self.size} vertices, in {self.best.successes} of {len(self.best.experiments)} experiments")
        lines = [head, f"sizes tried (successes): {tried}"]
        if self.next is not None and self.next.best is not None:
            lines.append(f"closest above: {self.next.params.size} vertices, best conflict {self.next.best.best_conflict}")
        return "\n".join(lines)

    def __repr__(self) -> str:
        return f"LargestResult({self.shape}, size={self.size}, {len(self.results)} sizes tried)"


def largest(
    graph: Graph,
    shape: str,
    *,
    start: int | None = None,
    lookahead: int = 2,
    fixed: Iterable[int] = (),
    endpoints: tuple[int, int] | None = None,
    experiments: int = 40,
    max_moves: int = 250,
    div_freq: int = 250,
    div_dur: int = 4,
    seed: int = 1,
    rng: str = "xoshiro",
    trace: bool = False,
    aspiration: bool = False,
    kick_size: int = 0,
    kick_ban: int = 20,
) -> LargestResult:
    """Searches ``graph`` for the largest ``shape`` it can find: an induced ``"path"``, ``"cycle"`` or
    ``"tree"``, a ``"clique"``, or an ``"independent"`` set.

    Starting at ``start`` vertices (by default the fewest the shape and the fixed vertices allow),
    it runs :func:`search` for the shape at size after size: first doubling its steps while searches
    succeed, then bisecting between the last size that succeeded and the first that did not. A
    size counts as reachable if a search succeeds at it or at one of the next ``lookahead`` sizes,
    since some sizes can be impossible below the largest: between two hypercube vertices that differ
    in an odd number of bits, for example, every path has an even number of vertices. The other
    arguments are passed to every search; ``endpoints`` needs ``shape="path"``.

    The result keeps every search, so :attr:`LargestResult.best` holds the largest subgraphs found
    and :attr:`LargestResult.next` the near misses one size up. A heuristic search can miss
    subgraphs that exist, so the size found is a lower bound; more experiments or moves can raise it.
    """
    if shape not in SHAPES:
        raise ValueError(f"unknown shape {shape!r}: choose path, cycle, tree, clique or independent")
    if not isinstance(graph, Graph):
        raise TypeError("largest needs an unweighted Graph")
    fixed = tuple(int(v) for v in fixed)
    if endpoints is not None:
        endpoints = tuple(int(v) for v in endpoints)
        if len(endpoints) != 2:
            raise ValueError("endpoints takes two vertices, the ends of the path")
    _, sizes = _core.largest(
        graph._g, shape, None if start is None else int(start), int(lookahead), list(fixed),
        None if endpoints is None else list(endpoints), int(experiments), int(max_moves), int(div_freq),
        int(div_dur), int(seed), rng, bool(trace), bool(aspiration), int(kick_size), int(kick_ban),
    )
    results = {}
    for size, edges, raw in sizes:
        params = SearchParams(
            size, edges, fixed, int(experiments), int(max_moves), int(div_freq), int(div_dur), int(seed), rng,
            bool(trace), bool(aspiration), 2 if shape == "cycle" else None, False, shape in ("cycle", "tree"),
            shape == "path", endpoints if shape == "path" else None, int(kick_size), int(kick_ban),
        )
        exps = [
            Experiment(i, c, m, s, tuple(v), None if t is None else np.asarray(t, dtype=np.int64))
            for i, (c, m, s, v, t) in enumerate(raw)
        ]
        results[size] = SearchResult(graph, params, exps)
    return LargestResult(graph, shape, results)
