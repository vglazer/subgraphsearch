"""Shared colours and axes helpers. matplotlib is imported lazily so that ``import subsearch``
works without it."""

from __future__ import annotations

ACCENT = "#d6336c"  # the subgraph / successes
SECONDARY = "#4c6ef5"  # everything else that needs a colour
NEUTRAL = "#868e96"


def axes(ax=None):
    if ax is not None:
        return ax
    try:
        import matplotlib.pyplot as plt
    except ImportError as e:  # pragma: no cover
        raise ImportError("plotting needs matplotlib: pip install 'subsearch[viz]'") from e
    _, ax = plt.subplots(figsize=(5.5, 3.6), layout="constrained")
    return ax


def finish(ax, title: str):
    ax.set_title(title, fontsize=10, loc="left")
    for side in ("top", "right"):
        ax.spines[side].set_visible(False)
    ax.grid(axis="y", alpha=0.25, lw=0.6)
    return ax
