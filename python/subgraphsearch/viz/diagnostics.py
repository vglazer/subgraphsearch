"""Plots of search behavior: conflict traces, run-time distributions, conflicts, sweeps.

All functions take an optional matplotlib ``ax`` and return it (:func:`plot_traces` splits it into
one strip per experiment and returns those), so they compose into figures.
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
    """The conflict (0 when the target is reached) after every move, for a few experiments, each in
    its own strip.

    Needs a search run with ``trace=True``. Up to ``max_experiments`` experiments are drawn, half
    of them ones that fell short and half the slowest successes, since experiments that succeed
    within a few moves show little; by default, one of each. Each strip shades the range of
    conflicts in short windows of moves, with a line along the lowest, since drawing every move
    fills in a solid block. The y-axis is cut off above the conflicts reached after the initial
    descent, and dotted lines mark the start of each diversification cycle after the first.
    ``ax`` is split into the strips, which are returned as a list.
    """
    from matplotlib.ticker import MaxNLocator

    traces = result.traces()
    pairs = list(zip(result.experiments, traces))
    misses = [(e, t) for e, t in pairs if not e.success]
    hits = sorted(((e, t) for e, t in pairs if e.success), key=lambda et: -len(et[1]))
    hits = hits[: max(max_experiments - min(len(misses), max_experiments // 2), 0)]
    misses = misses[: max_experiments - len(hits)]
    shown = hits + misses
    ax = axes(ax)
    if len(shown) <= 1:
        strips = [ax]
    else:
        grid = ax.get_subplotspec().subgridspec(len(shown), 1, hspace=0.12)
        fig = ax.figure
        ax.remove()
        strips = [fig.add_subplot(grid[0])]
        strips += [fig.add_subplot(grid[i], sharex=strips[0], sharey=strips[0]) for i in range(1, len(shown))]
    longest = max((len(t) for _, t in shown), default=0)
    window = max(1, -(-longest // 120))
    tails = [np.asarray(t[len(t) // 10 :]) for _, t in shown if len(t) > 10]
    top = max(float(np.percentile(np.concatenate(tails), 99)), 3.0) if tails else None
    for strip, (e, t) in zip(strips, shown):
        if show_diversification:
            _cycle_marks(strip, result, longest)
        color = ACCENT if e.success else SECONDARY
        t = np.asarray(t)
        padded = np.concatenate([t, np.full(-len(t) % window, t[-1])]).reshape(-1, window)
        x = np.append(np.arange(len(padded)) * window, len(t))
        lo, hi = padded.min(axis=1), padded.max(axis=1)
        strip.fill_between(x, np.append(lo, lo[-1]), np.append(hi, hi[-1]), step="post", color=color, alpha=0.2, lw=0)
        strip.step(x, np.append(lo, lo[-1]), where="post", color=color, lw=1.2)
        what = f"reached the target after {e.moves:,} moves" if e.success else f"fell short by {e.best_conflict}"
        strip.text(0.99, 0.95, what, transform=strip.transAxes, ha="right", va="top", fontsize=8, color=color)
        if e.success:
            strip.plot(len(t) - 1, 0, "o", color=ACCENT, ms=4, zorder=3)
        if top is not None:
            strip.set_ylim(-0.04 * top, top * 1.08)
        strip.yaxis.set_major_locator(MaxNLocator(integer=True, nbins=4))
        strip.set_xlim(0, longest)
        for side in ("top", "right"):
            strip.spines[side].set_visible(False)
        strip.grid(axis="y", alpha=0.25, lw=0.6)
    for strip in strips[:-1]:
        strip.tick_params(labelbottom=False)
    strips[-1].set_xlabel("move")
    for strip in strips:
        strip.set_ylabel("")
    # one label centered on the strips
    strips[-1].set_ylabel(_conflict_label(result))
    strips[-1].yaxis.set_label_coords(-0.1, len(strips) * 0.56 - 0.06)
    strips[0].set_title(f"conflict after every move ({len(shown)} of {len(traces)} experiments)", fontsize=10, loc="left")
    return strips


def _shortfalls(result: SearchResult) -> str:
    """How far the experiments that fell short ended from the target, as a phrase such as
    "19 fell short: all by 1" or "10 fell short: 3 by 1, 5 by 2, 2 by 3"."""
    gaps = np.bincount([e.best_conflict for e in result.experiments if not e.success])
    missed = int(gaps.sum())
    if not missed:
        return ""
    found = [(int(d), int(n)) for d, n in enumerate(gaps) if n]
    if len(found) == 1:
        return f"{missed} fell short: all by {found[0][0]}"
    if len(found) <= 4:
        parts = [f"{n} by {d}" for d, n in found]
    else:  # the largest gaps grouped, to keep the line short
        rest = found[3:]
        parts = [f"{n} by {d}" for d, n in found[:3]] + [f"{sum(n for _, n in rest)} by {rest[0][0]}–{rest[-1][0]}"]
    return f"{missed} fell short: " + ", ".join(parts)


def plot_run_time_distribution(result: SearchResult, ax=None):
    """The run-time distribution: the fraction of experiments that succeeded within ``t`` moves.

    Its height at ``max_moves`` is the success rate; a curve still rising at ``max_moves``
    suggests allowing more moves, a curve that flattens early suggests more, shorter
    experiments instead. Dotted lines mark the start of each diversification cycle after the
    first, where successes often come in bursts. The title also says how far the experiments
    that fell short ended from the target.
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
    title = f"successes by move ({result.successes} of {total})"
    short = _shortfalls(result)
    return finish(ax, f"{title}\n{short}" if short else title)


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
    """A figure with successes by move and, if the search ran with ``trace=True``, the conflict
    after every move for a few experiments. :func:`plot_conflicts` draws how close each
    experiment got, for searches whose misses end at many different distances."""
    import matplotlib.pyplot as plt

    ncols = 2 if result.params.trace else 1
    fig, axs = plt.subplots(1, ncols, figsize=figsize or (4.8 * ncols, 3.6), layout="constrained", squeeze=False)
    plot_run_time_distribution(result, axs[0, 0])
    if result.params.trace:
        plot_traces(result, axs[0, 1])
    p = result.params
    graph = f"in a graph with {result.graph.n} vertices and {result.graph.m} edges"
    fig.suptitle(f"{p.size}-vertex subgraphs {result.target}" + (f", {graph}" if ncols == 2 else f",\n{graph}"), fontsize=11)
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
