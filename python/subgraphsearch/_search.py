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
    #: ``False``, ``True`` (the original programs' limited criterion) or ``"full"``: see :func:`search`.
    aspiration: bool | str = False
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
    #: The tabu tenure the search used (see :func:`search`); 1, the original programs', for results saved before it
    #: existed.
    tabu_tenure: int = 1


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
        if p.pop("full_aspiration", False):
            p["aspiration"] = "full"
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


#: The region kick size of edge-count and weight searches unless ``kick_size`` is given.
DEFAULT_KICK_SIZE = 4


def _params(used: dict) -> SearchParams:
    """The parameters the core reports a search used, as a :class:`SearchParams`."""
    return SearchParams(**used)


def _experiments(raw) -> list[Experiment]:
    """The core's (best_conflict, moves, success, vertices, trace) tuples as :class:`Experiment` objects."""
    return [
        Experiment(i, c, m, s, tuple(v), None if t is None else np.asarray(t, dtype=np.int64))
        for i, (c, m, s, v, t) in enumerate(raw)
    ]


def auto_tabu_tenure(size: int, vertices: int) -> int:
    """The tabu tenure edge-count and weight searches of ``size`` of a graph's ``vertices`` use unless
    ``tabu_tenure`` is given: a tenth of the size or an eightieth of the vertices left outside it, whichever is
    smaller, from 1 to 20. Small graphs and small subgraphs get 1, the original programs' tenure."""
    return _core.auto_tabu_tenure(int(size), int(vertices))


def _settings(kick_size: int | None, tabu_tenure: int | str | None, degree_search: bool, rng: str) -> tuple[int, int]:
    """The kick size and the tenure to pass to the core (0 for the automatic tenure), resolved by the core's
    rules for which searches take them."""
    if tabu_tenure == "auto":
        tabu_tenure = 0
    elif tabu_tenure is not None and (isinstance(tabu_tenure, str) or int(tabu_tenure) < 1):
        raise ValueError(f"tabu_tenure must be a positive number of moves or 'auto', not {tabu_tenure!r}")
    return _core.search_settings(None if kick_size is None else int(kick_size),
                                 None if tabu_tenure is None else int(tabu_tenure), degree_search, rng == "legacy")


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
    aspiration: bool | str = False,
    connected: bool = False,
    path: bool = False,
    endpoints: tuple[int, int] | None = None,
    kick_size: int | None = None,
    kick_ban: int = 20,
    tabu_tenure: int | str | None = None,
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

    ``aspiration=True`` adds the aspiration criterion of the original ``asub_search``: tabu swaps
    involving the two vertices just swapped are taken whenever they would beat the best value found
    so far. ``aspiration="full"`` considers every swap the tabu tenure forbids instead, which lets
    a long tenure keep the search from circling back without blocking improvements; it costs some
    time per move, and with tenure 1 it is the same as ``True``. Edge-count and weight searches
    only.

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

    ``kick_size=k`` starts each diversification cycle after the first with a *region kick* instead of
    random moves: the ``k`` free subgraph vertices nearest a random one (in breadth-first order from
    it, at whatever distance) are swapped out, one per move, each for the best vertex to swap in, and
    the vertices removed may not return for ``kick_ban`` moves. It rebuilds one part of the subgraph
    at a time, like the large-neighborhood moves of searches for codes. Edge-count and weight
    searches use kicks of :data:`DEFAULT_KICK_SIZE` (4) unless told otherwise, a safe size; they only
    take effect in searches longer than ``div_freq`` moves. Larger
    sizes can help much more on some problems and hurt on others, so compare several, for example
    with :func:`sweep`; ``kick_size=0`` turns kicks off. Regular and path searches have no kicks, and
    neither, by default, does ``rng="legacy"``, so that it still reproduces the original programs.
    ``tabu_tenure=t`` keeps each vertex swapped out from returning, and each vertex swapped in from
    leaving, for ``t`` moves. By default (``"auto"``), edge-count and weight searches use
    :func:`auto_tabu_tenure`: a tenth of ``size`` or an eightieth of the vertices left outside
    the subgraph, whichever is smaller, at most 20, so 1 below 20 vertices or in graphs of fewer
    than 160. This
    errs on the short side, so compare a few longer tenures with :func:`sweep`, especially on small
    graphs; 1 is the original programs' tenure, which regular and path searches and
    ``rng="legacy"`` keep.
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
    kick_size, tabu_tenure = _settings(kick_size, tabu_tenure, degree is not None or bool(path), rng)
    params = SearchParams(
        int(size), 0 if edges is None else int(edges), fixed, int(experiments), int(max_moves), int(div_freq), int(div_dur),
        int(seed), rng, bool(trace), _aspiration(aspiration), None if degree is None else int(degree), bool(two_phase),
        bool(connected), bool(path), endpoints, kick_size, int(kick_ban), tabu_tenure,
    )
    used, raw = _core.search(graph._g, params)
    return SearchResult(graph, _params(used), _experiments(raw))


def _aspiration(aspiration) -> bool | str:
    """``aspiration`` as ``SearchParams`` holds it: a bool, or ``"full"``."""
    if not isinstance(aspiration, str):
        return bool(aspiration)
    if aspiration != "full":
        raise ValueError(f"aspiration must be False, True or 'full', not {aspiration!r}")
    return aspiration


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
    aspiration: bool | str = False,
    kick_size: int | None = None,
    kick_ban: int = 20,
    tabu_tenure: int | str | None = None,
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
    kick_size, tabu_tenure = _settings(kick_size, tabu_tenure, shape in ("path", "cycle"), rng)
    if endpoints is not None:
        endpoints = tuple(int(v) for v in endpoints)
        if len(endpoints) != 2:
            raise ValueError("endpoints takes two vertices, the ends of the path")
    base = SearchParams(
        2, 0, fixed, int(experiments), int(max_moves), int(div_freq), int(div_dur), int(seed), rng, bool(trace),
        _aspiration(aspiration), endpoints=endpoints, kick_size=kick_size, kick_ban=int(kick_ban), tabu_tenure=tabu_tenure,
    )
    _, sizes = _core.largest(graph._g, shape, None if start is None else int(start), int(lookahead), base)
    results = {size: SearchResult(graph, _params(used), _experiments(raw)) for size, used, raw in sizes}
    return LargestResult(graph, shape, results)


@dataclass(frozen=True)
class ExtremeResult:
    """The outcome of :func:`densest` or :func:`sparsest`: the search, and the extreme subgraphs it found."""

    graph: Graph | WeightedGraph
    #: ``"densest"`` or ``"sparsest"``.
    kind: str
    #: The search, aimed at the most (or fewest) edges, or total weight, that any ``size`` vertices could span.
    result: SearchResult

    def _value(self, vertices) -> int:
        g = self.graph
        return g.induced_weight(vertices) if isinstance(g, WeightedGraph) else g.induced_edge_count(vertices)

    def _scored(self) -> bool:
        return isinstance(self.graph, WeightedGraph) and self.graph.has_scores

    def _rank(self, vertices) -> float:
        """How good a selection's exact score is: higher is better, for densest and sparsest alike."""
        score = self.graph.induced_score(vertices)
        return score if self.kind == "densest" else -score

    @property
    def vertices(self) -> tuple[int, ...]:
        """The vertices of the densest (or sparsest) subgraph found. On a graph made from scores
        (:meth:`WeightedGraph.from_scores`), ties in total weight go to the highest (for sparsest, the
        lowest) exact total score, as selections tied at rounded weights can differ in it; then to the
        earliest experiment."""
        best = self.result.best
        if not self._scored():
            return best.vertices
        tied = [e for e in self.result.experiments if e.best_conflict == best.best_conflict]
        return max(tied, key=lambda e: (self._rank(e.vertices), -e.index)).vertices

    @property
    def value(self) -> int:
        """Its number of edges, or total weight in a weighted graph."""
        return self._value(self.vertices)

    @property
    def score(self) -> float | None:
        """Its exact total score, on a graph made from scores (:meth:`WeightedGraph.from_scores`);
        ``None`` otherwise."""
        return self.graph.induced_score(self.vertices) if self._scored() else None

    def solutions(self) -> list[tuple[int, ...]]:
        """Every distinct vertex set found with :attr:`value`, each listed once; on a graph made
        from scores, the best exact total first."""
        found = [vs for vs in self.result.solutions(max_conflict=self.result.best.best_conflict)
                 if self._value(vs) == self.value]
        return sorted(found, key=self._rank, reverse=True) if self._scored() else found

    def summary(self) -> str:
        what = "total weight" if isinstance(self.graph, WeightedGraph) else "edges"
        score = f" (score {self.score:,.4f})" if self._scored() else ""
        best = self.result.best
        reached = sum(e.best_conflict == best.best_conflict for e in self.result.experiments)
        return (f"{self.kind} {self.result.params.size} vertices found: {self.value} {what}{score}, reached by "
                f"{reached} of {len(self.result.experiments)} experiments ({len(self.solutions())} distinct)")

    def __repr__(self) -> str:
        what = "weight" if isinstance(self.graph, WeightedGraph) else "edges"
        score = f", score={self.score:.4f}" if self._scored() else ""
        return f"ExtremeResult({self.kind}, size={self.result.params.size}, {what}={self.value}{score})"


def densest(graph: Graph | WeightedGraph, size: int, **kwargs) -> ExtremeResult:
    """Searches for the ``size`` vertices spanning the most edges, or in a :class:`WeightedGraph` the most total
    weight, without a target to aim at: the k most diverse items, the densest community.

    It runs :func:`search` aimed at the most any ``size`` vertices could span, all ``size(size - 1)/2`` pairs (at the
    largest weight, 255, in a weighted graph) or the whole graph if that is less; the conflict is then how far a
    subgraph falls short of it, so minimizing it maximizes the edges or weight. The other arguments are passed to
    :func:`search`, so ``fixed=`` and ``connected=True`` work, and so do the settings: hard instances want longer
    runs and larger region kicks, as for any search. A heuristic search can miss denser subgraphs, so the value
    found is a lower bound on the most possible.
    """
    k = int(size)
    if isinstance(graph, WeightedGraph):
        bound = min(255 * (k * (k - 1) // 2), graph.total_weight)
    else:
        bound = min(k * (k - 1) // 2, graph.m)
    return ExtremeResult(graph, "densest", search(graph, k, bound, **kwargs))


def sparsest(graph: Graph | WeightedGraph, size: int, **kwargs) -> ExtremeResult:
    """Searches for the ``size`` vertices spanning the fewest edges, or the least total weight: aimed at none, so
    the conflict is the edges or weight itself. The counterpart of :func:`densest`, with the same arguments; an
    upper bound on the fewest possible."""
    return ExtremeResult(graph, "sparsest", search(graph, int(size), 0, **kwargs))
