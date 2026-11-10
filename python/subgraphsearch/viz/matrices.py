"""Matrices of vectors with prescribed inner products: their sign patterns."""

from __future__ import annotations

import numpy as np

from ._style import axes

DARK = "#212529"  # +1
LIGHT = "#dee2e6"  # 0


def plot_sign_matrix(matrix, ax=None, title: str | None = None):
    """A matrix with entries +1, -1 and 0, such as a Hadamard, weighing or Seidel matrix, as an
    image: +1 dark, -1 white, 0 light gray."""
    from matplotlib.colors import ListedColormap

    m = np.sign(np.asarray(matrix))
    if m.ndim != 2:
        raise ValueError(f"expected a 2-D matrix, got shape {m.shape}")
    ax = axes(ax)
    ax.imshow(m, cmap=ListedColormap(["#ffffff", LIGHT, DARK]), vmin=-1, vmax=1, interpolation="nearest")
    ax.set_xticks([])
    ax.set_yticks([])
    for side in ax.spines.values():
        side.set_color(LIGHT)
    rows, cols = m.shape
    ax.set_title(title or f"{rows} x {cols} sign pattern (+1 dark, -1 white)", fontsize=10, loc="left")
    return ax
