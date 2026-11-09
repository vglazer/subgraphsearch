"""Plots of search behavior: objective traces, run-time distributions, conflicts, sweeps.

All functions take an optional matplotlib ``ax`` and return it, so they compose into figures.
"""

from __future__ import annotations

from collections.abc import Mapping

import numpy as np

from .._search import SearchResult
from ._style import ACCENT, NEUTRAL, SECONDARY, axes, finish


def plot_traces(result: SearchResult, ax=None, max_experiments: int = 12, show_diversification: bool = True):
    """The objective ``|edges - target|`` after every move, one line per experiment.

    Needs a search run with ``trace=True``. Shaded bands mark the random (diversification)
    moves at the start of each ``div_freq`` cycle; a dot marks each success.
    """
    traces = result.traces()
    ax = axes(ax)
    p = result.params
    longest = max((len(t) for t in traces), default=0)
    if show_diversification and p.div_dur > 0:
        for start in range(0, longest, p.div_freq):
            ax.axvspan(start, start + p.div_dur, color=NEUTRAL, alpha=0.12, lw=0)
    shown = traces[:max_experiments]
    for e, t in zip(result.experiments, shown):
        color = ACCENT if e.success else SECONDARY
        ax.plot(np.arange(len(t)), t, color=color, lw=0.9, alpha=0.75)
        if e.success:
            ax.plot(len(t) - 1, 0, "o", color=ACCENT, ms=4)
    ax.set_yscale("symlog", linthresh=1)
    ax.set_xlabel("move")
    if p.path:
        ax.set_ylabel("conflict (0 for a path)")
    else:
        label = "deviation from regular" if p.degree is not None else "|edges − target|"
        ax.set_ylabel(f"{label} + extra components" if p.connected else label)
    hidden = len(traces) - len(shown)
    title = f"objective per move ({len(shown)} of {len(traces)} experiments)" if hidden else "objective per move"
    return finish(ax, title)


def plot_run_time_distribution(result: SearchResult, ax=None):
    """The run-time distribution: the fraction of experiments that succeeded within ``t`` moves.

    Its height at ``max_moves`` is the success rate; a curve still rising at ``max_moves``
    suggests allowing more moves, a curve that flattens early suggests more, shorter
    experiments instead.
    """
    ax = axes(ax)
    p = result.params
    moves = np.sort([e.moves for e in result.experiments if e.success])
    total = len(result.experiments)
    # when every experiment succeeded the move budget is irrelevant: zoom to the moves used
    xmax = max(p.max_moves, 1) if len(moves) < total or not len(moves) else max(int(moves[-1] * 1.15), 10)
    if len(moves):
        x = np.concatenate([[0], moves, [xmax]])
        y = np.concatenate([[0], np.arange(1, len(moves) + 1) / total, [len(moves) / total]])
        ax.step(x, y, where="post", color=ACCENT, lw=1.6)
    ax.set_xlim(0, xmax)
    ax.set_ylim(0, 1.02)
    ax.set_xlabel("moves")
    ax.set_ylabel("P(success within moves)")
    return finish(ax, f"run-time distribution ({result.successes}/{total} successes)")


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
    """Histogram of each experiment's best conflict (0 = success)."""
    ax = axes(ax)
    c = np.bincount([e.best_conflict for e in result.experiments]) if result.experiments else np.zeros(1, int)
    colors = [ACCENT] + [SECONDARY] * (len(c) - 1)
    ax.bar(np.arange(len(c)), c, color=colors, edgecolor="white", lw=0.5)
    p = result.params
    ax.set_xlabel("best conflict" if p.connected or p.path or p.degree is not None else "best |edges − target|")
    ax.set_ylabel("experiments")
    ax.set_xticks(np.arange(len(c)) if len(c) <= 15 else ax.get_xticks())
    return finish(ax, f"best conflict (mean {result.average_conflict:.2f})")


def plot_search_summary(result: SearchResult, figsize=None):
    """A figure with the run-time distribution, the conflict histogram and, if recorded, traces."""
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
