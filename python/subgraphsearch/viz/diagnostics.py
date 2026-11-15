"""Plots of search behavior: conflict traces, run-time distributions, conflicts, sweeps.

All functions take an optional matplotlib ``ax`` and return it, so they compose into figures.
"""

from __future__ import annotations

from collections.abc import Mapping

import numpy as np

from .._search import SearchResult
from ._style import ACCENT, NEUTRAL, SECONDARY, axes, finish


def _cycle_marks(ax, result: SearchResult, until: int) -> None:
    """Dotted lines where each diversification cycle after the first starts: with a region kick, or
    with random moves when kicks are off."""
    p = result.params
    if p.div_dur == 0 or p.div_freq == 0:
        return
    label = "region kick" if p.kick_size > 0 else "random moves"
    for i, start in enumerate(range(p.div_freq, until, p.div_freq)):
        ax.axvline(start, color=NEUTRAL, lw=0.8, ls=":", label=label if i == 0 else None)


def _conflict_label(result: SearchResult) -> str:
    p = result.params
    if p.path:
        label = "conflict (0 for a path)"
    elif p.degree is not None:
        label = "deviation from regular"
    else:
        label = "distance from the target"
    return f"{label} + extra components" if p.connected else label


def plot_traces(result: SearchResult, ax=None, max_experiments: int = 2, show_diversification: bool = True):
    """The conflict (0 when the target is reached) after every move, for a few experiments.

    Needs a search run with ``trace=True``. Up to ``max_experiments`` experiments are drawn, half
    of them ones that fell short and half the slowest successes, since experiments that succeed
    within a few moves show little; by default, one of each. The y-axis is cut off above the conflicts reached after the
    initial descent, and dotted lines mark the start of each diversification cycle after the first.
    """
    traces = result.traces()
    ax = axes(ax)
    pairs = list(zip(result.experiments, traces))
    misses = [(e, t) for e, t in pairs if not e.success]
    hits = sorted(((e, t) for e, t in pairs if e.success), key=lambda et: -len(et[1]))
    hits = hits[: max(max_experiments - min(len(misses), max_experiments // 2), 0)]
    misses = misses[: max_experiments - len(hits)]
    shown = sorted(hits + misses, key=lambda et: et[0].index)
    longest = max((len(t) for _, t in shown), default=0)
    if show_diversification:
        _cycle_marks(ax, result, longest)
    labelled = set()
    for e, t in shown:
        color = ACCENT if e.success else SECONDARY
        label = "reached the target" if e.success else "fell short"
        ax.plot(np.arange(len(t)), t, color=color, lw=0.9, alpha=0.8, label=None if label in labelled else label)
        labelled.add(label)
        if e.success:
            ax.plot(len(t) - 1, 0, "o", color=ACCENT, ms=4)
    # cut off the initial descent, so the moves near the target are visible
    tails = [np.asarray(t[len(t) // 10 :]) for _, t in shown if len(t) > 10]
    if tails:
        top = max(float(np.percentile(np.concatenate(tails), 99)), 3.0)
        ax.set_ylim(-0.04 * top, top * 1.08)
    from matplotlib.ticker import MaxNLocator

    ax.yaxis.set_major_locator(MaxNLocator(integer=True))
    ax.set_xlabel("move")
    ax.set_ylabel(_conflict_label(result))
    if shown:
        ax.legend(fontsize=8, loc="upper center", bbox_to_anchor=(0.5, -0.2), ncol=3, frameon=False)
    return finish(ax, f"conflict after every move ({len(shown)} of {len(traces)} experiments)")


def plot_run_time_distribution(result: SearchResult, ax=None):
    """The run-time distribution: the fraction of experiments that succeeded within ``t`` moves.

    Its height at ``max_moves`` is the success rate; a curve still rising at ``max_moves``
    suggests allowing more moves, a curve that flattens early suggests more, shorter
    experiments instead. Dotted lines mark the start of each diversification cycle after the
    first, where successes often come in bursts.
    """
    ax = axes(ax)
    p = result.params
    moves = np.sort([e.moves for e in result.experiments if e.success])
    total = len(result.experiments)
    # when every experiment succeeded the move budget is irrelevant: zoom to the moves used
    xmax = max(p.max_moves, 1) if len(moves) < total or not len(moves) else max(int(moves[-1] * 1.15), 10)
    _cycle_marks(ax, result, xmax)
    if len(moves):
        x = np.concatenate([[0], moves, [xmax]])
        y = np.concatenate([[0], np.arange(1, len(moves) + 1) / total, [len(moves) / total]])
        ax.step(x, y, where="post", color=ACCENT, lw=1.6)
    ax.set_xlim(0, xmax)
    ax.set_ylim(0, 1.02)
    ax.set_xlabel("move")
    ax.set_ylabel("fraction of experiments that succeeded")
    if ax.get_legend_handles_labels()[0]:
        ax.legend(frameon=False, fontsize=8, loc="lower right")
    return finish(ax, f"successes by move ({result.successes} of {total})")


def plot_success_times(result: SearchResult, ax=None, bins: int | None = None):
    """Histogram of the moves successful experiments needed (``sub_search``'s "box
    distribution" when ``bins`` is left at its default of 10 equal boxes)."""
    ax = axes(ax)
    p = result.params
    if bins is None:
        counts = result.boxes()
        width = p.max_moves // len(counts) + 1
        edges = np.arange(len(counts) + 1) * width
    else:
        moves = [e.moves for e in result.experiments if e.success]
        counts, edges = np.histogram(moves, bins=bins, range=(0, max(p.max_moves, 1)))
    ax.bar(edges[:-1], counts, width=np.diff(edges), align="edge", color=ACCENT, edgecolor="white", lw=0.5)
    ax.set_xlabel("moves to success")
    ax.set_ylabel("experiments")
    return finish(ax, "moves to success")


def plot_conflicts(result: SearchResult, ax=None):
    """How close each experiment came to the target: the number of experiments by best conflict,
    from "hit" (conflict 0, a success) up."""
    ax = axes(ax)
    c = np.bincount([e.best_conflict for e in result.experiments]) if result.experiments else np.zeros(1, int)
    x = np.arange(len(c))
    ax.bar(x, c, color=[ACCENT] + [SECONDARY] * (len(c) - 1), edgecolor="white", lw=0.5)
    if len(c) <= 20:
        for xi, n in zip(x, c):
            if n:
                ax.text(xi, n, str(n), ha="center", va="bottom", fontsize=8)
        ax.set_ylim(top=max(c.max(), 1) * 1.15)
    if len(c) <= 8:
        ax.set_xticks(x, ["hit"] + [f"{d} off" for d in x[1:]])
        ax.set_xlabel("closest each experiment came to the target")
    else:
        ax.set_xlabel("closest each experiment came to the target (0 = hit)")
    ax.set_ylabel("experiments")
    return finish(ax, "how close each experiment got")


def plot_search_summary(result: SearchResult, figsize=None):
    """A figure with successes by move, how close each experiment got and, if the search ran with
    ``trace=True``, the conflict after every move for a few experiments."""
    import matplotlib.pyplot as plt

    has_traces = result.params.trace
    ncols = 3 if has_traces else 2
    fig, axs = plt.subplots(1, ncols, figsize=figsize or (4.2 * ncols, 3.4), layout="constrained")
    plot_run_time_distribution(result, axs[0])
    plot_conflicts(result, axs[1])
    if has_traces:
        plot_traces(result, axs[2])
    p = result.params
    fig.suptitle(f"{p.size}-vertex subgraphs {result.target}, in a graph with {result.graph.n} vertices and {result.graph.m} edges", fontsize=11)
    return fig


def plot_sweep(results: Mapping, ax=None, parameter: str | None = None):
    """Success rate and mean moves per success against a swept parameter, from
    :func:`subgraphsearch.sweep`'s ``{value: SearchResult}``."""
    ax = axes(ax)
    values = list(results)
    rate = [results[v].success_rate for v in values]
    cost = [results[v].true_cost for v in values]
    x = np.arange(len(values))
    ax.plot(x, rate, "o-", color=ACCENT, label="success rate")
    ax.set_ylim(0, 1.02)
    ax.set_ylabel("success rate", color=ACCENT)
    ax.set_xticks(x, [str(v) for v in values])
    ax.set_xlabel(parameter or "parameter")
    twin = ax.twinx()
    twin.plot(x, cost, "s--", color=SECONDARY, label="moves per success")
    twin.set_ylabel("moves per success (true cost)", color=SECONDARY)
    twin.spines["top"].set_visible(False)
    return finish(ax, f"effect of {parameter}" if parameter else "parameter sweep")
