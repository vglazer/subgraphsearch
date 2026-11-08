"""Spectral and structural views of a graph: Laplacian spectra, degree distributions and
adjacency-matrix heatmaps that put a found subgraph in its own block."""

from __future__ import annotations

from collections.abc import Iterable
from math import lgamma

import numpy as np

from .._graph import Graph, structure_of
from ._style import ACCENT, NEUTRAL, SECONDARY, axes, finish

#: Above this many vertices, dense eigendecomposition is refused (it needs O(n^2) memory and
#: O(n^3) time); pass ``k`` to compute only the extreme eigenvalues with SciPy instead.
DENSE_LIMIT = 6000


def laplacian_spectrum(graph: Graph, normalized: bool = False, k: int | None = None) -> np.ndarray:
    """Eigenvalues of the Laplacian, ascending.

    With ``k`` set, only the ``k`` smallest are computed, with SciPy's sparse solver, which
    scales to much larger graphs.
    """
    graph = structure_of(graph)
    if k is not None:
        import scipy.sparse as sp
        from scipy.sparse.linalg import eigsh

        from ..convert import to_scipy

        a = to_scipy(graph).astype(float)
        d = np.asarray(a.sum(axis=1)).ravel()
        if normalized:
            s = np.where(d > 0, 1 / np.sqrt(np.where(d > 0, d, 1)), 0)
            lap = sp.diags_array((d > 0).astype(float)) - sp.diags_array(s) @ a @ sp.diags_array(s)
        else:
            lap = sp.diags_array(d) - a
        # shift-invert around a small negative number finds the smallest eigenvalues reliably
        vals = eigsh(lap.tocsc(), k=min(k, graph.n - 1), sigma=-1e-3, which="LM", return_eigenvectors=False)
        return np.sort(vals)
    if graph.n > DENSE_LIMIT:
        raise ValueError(f"{graph.n} vertices is too many for a dense eigendecomposition; pass k=... for the k smallest eigenvalues")
    return np.linalg.eigvalsh(graph.laplacian(normalized))


def spectral_summary(graph: Graph) -> dict:
    """Connected components (by search, exactly), the algebraic connectivity (Fiedler value,
    the second-smallest Laplacian eigenvalue, positive iff the graph is connected) and the
    largest Laplacian eigenvalue."""
    graph = structure_of(graph)
    _, components = graph.components()
    vals = laplacian_spectrum(graph) if graph.n > 0 else np.zeros(0)
    return {
        "components": components,
        "algebraic_connectivity": float(vals[1]) if graph.n > 1 and components == 1 else 0.0,
        "largest_eigenvalue": float(vals[-1]) if graph.n else 0.0,
        "trace": float(vals.sum()),
        "edges": graph.m,
    }


def plot_spectrum(graph: Graph, ax=None, normalized: bool = False, bins: int = 40):
    """Histogram of the Laplacian eigenvalues, marking the algebraic connectivity."""
    graph = structure_of(graph)
    ax = axes(ax)
    vals = laplacian_spectrum(graph, normalized)
    ax.hist(vals, bins=bins, color=SECONDARY, edgecolor="white", lw=0.4)
    summary = spectral_summary(graph) if not normalized else None
    if graph.n > 1:
        fiedler = vals[1]
        ax.axvline(fiedler, color=ACCENT, lw=1.4)
        label = f"λ₁ = {fiedler:.3g}" + (f"  ({summary['components']} components)" if summary and summary["components"] > 1 else "")
        ax.annotate(label, (fiedler, ax.get_ylim()[1] * 0.92), xytext=(4, 0), textcoords="offset points", color=ACCENT, fontsize=9)
    ax.set_xlabel("normalized Laplacian eigenvalue" if normalized else "Laplacian eigenvalue")
    ax.set_ylabel("count")
    return finish(ax, "Laplacian spectrum")


def _binomial_pmf(trials: int, p: float, k: np.ndarray) -> np.ndarray:
    """Binomial probabilities, computed in log space so large ``trials`` do not overflow."""
    lg = np.vectorize(lgamma)
    log_pmf = lgamma(trials + 1) - lg(k + 1) - lg(trials - k + 1) + k * np.log(p) + (trials - k) * np.log1p(-p)
    return np.exp(log_pmf)


def plot_degree_distribution(graph: Graph, ax=None, compare_random: bool = True, log: bool = False):
    """Degree counts, optionally against the binomial distribution an Erdős–Rényi graph with the
    same density would have, which makes skewed or heavy-tailed degrees stand out."""
    graph = structure_of(graph)
    ax = axes(ax)
    d = graph.degrees()
    lo, hi = (int(d.min()), int(d.max())) if len(d) else (0, 0)
    # one bar per degree for narrow ranges; about 40 bins otherwise
    width = max(1, int(np.ceil((hi - lo + 1) / 40)))
    edges = np.arange(lo, hi + width + 1, width)
    counts, _ = np.histogram(d, bins=edges)
    ax.bar(edges[:-1], counts, width=width, align="edge", color=SECONDARY, edgecolor="white", lw=0.3, label="observed")
    if compare_random and graph.n > 1 and 0 < graph.density < 1:
        k = np.arange(graph.n)
        expected = graph.n * _binomial_pmf(graph.n - 1, graph.density, k) * width
        a, b = max(min(lo, int(graph.density * (graph.n - 1)) - 4 * int(np.sqrt(graph.n)) ) - 5, 0), min(hi + 6, graph.n)
        ax.plot(k[a:b] + width / 2, expected[a:b], color=ACCENT, lw=1.4, label="Erdős–Rényi, same density")
        ax.legend(frameon=False, fontsize=8)
    if log:
        ax.set_yscale("log")
    ax.set_xlabel("degree")
    ax.set_ylabel("vertices")
    return finish(ax, f"degree distribution (mean {d.mean() if len(d) else 0:.1f})")


def _order(graph: Graph, highlight: list[int], order: str) -> np.ndarray:
    rest = np.setdiff1d(np.arange(graph.n), highlight)
    if order == "index":
        return np.concatenate([highlight, rest]).astype(int)
    if order == "degree":
        d = graph.degrees()
        return np.concatenate([sorted(highlight, key=lambda v: -d[v]), rest[np.argsort(-d[rest], kind="stable")]]).astype(int)
    if order == "spectral":
        _, vecs = np.linalg.eigh(graph.laplacian(normalized=True))
        f = vecs[:, 1]
        return np.concatenate([sorted(highlight, key=lambda v: f[v]), rest[np.argsort(f[rest], kind="stable")]]).astype(int)
    raise ValueError(f"unknown order {order!r} (expected index, degree or spectral)")


def plot_adjacency(graph: Graph, highlight: Iterable[int] = (), ax=None, order: str = "degree"):
    """The adjacency matrix as an image, with ``highlight`` (e.g. a search result) moved to the
    top-left so the subgraph appears as a block. ``order`` sorts the remaining vertices by
    ``"degree"``, by ``"spectral"`` position (the Fiedler vector, which reveals clusters) or by
    ``"index"``."""
    graph = structure_of(graph)
    from matplotlib.colors import ListedColormap

    ax = axes(ax)
    h = sorted(int(v) for v in highlight)
    perm = _order(graph, h, order)
    a = graph.adjacency_matrix(dtype=np.int8)[np.ix_(perm, perm)].astype(float)
    k = len(h)
    if k:
        a[:k, :k] *= 2  # subgraph edges get their own color
    ax.imshow(a, cmap=ListedColormap(["#ffffff", NEUTRAL, ACCENT]), vmin=0, vmax=2, interpolation="nearest")
    if k:
        for pos in (k - 0.5,):
            ax.axhline(pos, color=ACCENT, lw=0.8)
            ax.axvline(pos, color=ACCENT, lw=0.8)
    ax.set_xticks([])
    ax.set_yticks([])
    title = f"adjacency ({k}-vertex subgraph first)" if k else f"adjacency (ordered by {order})"
    ax.set_title(title, fontsize=10, loc="left")
    return ax


def plot_graph_summary(graph: Graph, highlight: Iterable[int] = (), figsize=None):
    """A figure with the degree distribution, the Laplacian spectrum and the adjacency matrix."""
    graph = structure_of(graph)
    import matplotlib.pyplot as plt

    fig, axs = plt.subplots(1, 3, figsize=figsize or (13, 3.8), layout="constrained")
    plot_degree_distribution(graph, axs[0])
    plot_spectrum(graph, axs[1])
    plot_adjacency(graph, highlight, axs[2])
    fig.suptitle(f"{graph.n} vertices, {graph.m} edges, density {graph.density:.3f}", fontsize=11)
    return fig
