"""Searching for induced subgraphs with a prescribed number of edges."""

from __future__ import annotations

import json
from collections.abc import Iterable, Sequence
from dataclasses import asdict, dataclass, field

import numpy as np

from . import _core
from ._graph import Graph

NUM_BOXES = 10


@dataclass(frozen=True)
class Experiment:
    """One independent run (restart) of the search."""

    index: int
    #: Smallest ``|edges - target|`` reached.
    best_conflict: int
    #: Moves made: the index of the successful move, or ``max_moves`` if none succeeded.
    moves: int
    #: Whether the target edge count was hit exactly.
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
    experiments: int = 10
    max_moves: int = 1000
    div_freq: int = 250
    div_dur: int = 4
    seed: int = 1
    rng: str = "xoshiro"
    trace: bool = False
    aspiration: bool = False


@dataclass
class SearchResult:
    """The outcome of :func:`search`: parameters, the graph searched, and every experiment."""

    graph: Graph
    params: SearchParams
    experiments: list[Experiment]

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
            f"{self.successes}/{len(self.experiments)} experiments found a {p.size}-vertex subgraph with {p.edges} edges",
            f"relative cost {self.relative_cost:.1f} moves, true cost {self.true_cost:.1f} moves, "
            f"average conflict {self.average_conflict:.3f}",
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
    def from_dict(cls, data: dict, graph: Graph) -> SearchResult:
        """Rebuilds a result from :meth:`to_dict` output or from the JSON the ``subsearch search
        --json`` command writes."""
        p = dict(data["params"])
        if "record_trace" in p:  # the Rust CLI's field name
            p["trace"] = p.pop("record_trace")
        p["fixed"] = tuple(p.get("fixed", ()))
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
    def load(cls, path, graph: Graph) -> SearchResult:
        """Reads a result saved with :meth:`save` or ``subsearch search --json``."""
        with open(path, encoding="utf-8") as f:
            return cls.from_dict(json.load(f), graph)

    def save(self, path) -> None:
        with open(path, "w", encoding="utf-8") as f:
            f.write(self.to_json())

    def __repr__(self) -> str:
        return f"SearchResult({self.successes}/{len(self.experiments)} successes, size={self.params.size}, edges={self.params.edges})"

    def _repr_html_(self) -> str:
        from ._html import search_html

        return search_html(self)


def search(
    graph: Graph,
    size: int,
    edges: int,
    *,
    fixed: Iterable[int] = (),
    experiments: int = 10,
    max_moves: int = 1000,
    div_freq: int = 250,
    div_dur: int = 4,
    seed: int = 1,
    rng: str = "xoshiro",
    trace: bool = False,
    aspiration: bool = False,
) -> SearchResult:
    """Searches ``graph`` for induced subgraphs on ``size`` vertices with ``edges`` edges.

    Each of ``experiments`` independent runs starts from random vertices (always including
    ``fixed``) and makes up to ``max_moves`` swaps of one subgraph vertex for one outside vertex,
    minimising ``|edges in subgraph - edges|``. Locally optimal swaps are chosen from a
    restricted neighbourhood; the first ``div_dur`` of every ``div_freq`` moves are random
    ("diversification"). With ``rng="xoshiro"`` (default) experiments run in parallel and depend
    only on ``seed``; ``rng="legacy"`` reproduces USRA ``sub_search`` exactly.

    Set ``trace=True`` to record the objective after every move, for :mod:`subsearch.viz`.

    ``aspiration=True`` adds the limited aspiration of USRA's ``asub_search``: swaps involving
    the two vertices just swapped, which the restricted search otherwise excludes, are taken
    whenever they would beat the best value found so far.
    """
    fixed = tuple(int(v) for v in fixed)
    params = SearchParams(int(size), int(edges), fixed, int(experiments), int(max_moves), int(div_freq), int(div_dur), int(seed), rng, bool(trace), bool(aspiration))
    raw = _core.search(graph._g, params.size, params.edges, list(fixed), params.experiments, params.max_moves, params.div_freq, params.div_dur, params.seed, rng, params.trace, params.aspiration)
    exps = [
        Experiment(i, c, m, s, tuple(v), None if t is None else np.asarray(t, dtype=np.int64))
        for i, (c, m, s, v, t) in enumerate(raw)
    ]
    return SearchResult(graph, params, exps)


def sweep(graph: Graph, size: int, edges: int, parameter: str, values: Sequence, **kwargs) -> dict:
    """Runs :func:`search` once per value of ``parameter`` (e.g. ``"div_dur"``), keeping the other
    arguments fixed, and returns ``{value: SearchResult}``; plot it with
    :func:`subsearch.viz.plot_sweep`."""
    return {v: search(graph, size, edges, **{**kwargs, parameter: v}) for v in values}
