"""Building blocks for searches over vectors and words: the candidates, conversions, and checks.

- :func:`binary_words` lists binary words, all of them or those of a given weight, for
  :meth:`Graph.from_words` and :meth:`Graph.from_vectors`.
- :func:`signs` turns 0/1 words into +-1 vectors (0 to +1, 1 to -1); words ``t`` places apart become
  vectors with inner product ``n - 2t``.
- :func:`min_distance` and :func:`is_hadamard` check what a search found.
- :func:`supports` writes a code as the positions of its 1s, the form tables of codes use.

>>> import subgraphsearch as ss
>>> words = ss.vectors.binary_words(4, weight=2)
>>> len(words), words[0].tolist()
(6, [1, 1, 0, 0])
>>> ss.vectors.min_distance(words)
2
"""

from __future__ import annotations

import itertools
from math import comb

import numpy as np


def binary_words(n: int, weight: int | None = None) -> np.ndarray:
    """All binary words of length ``n``, or those with ``weight`` 1s, as the rows of a ``uint8`` array.

    All words come in the order of the integers they spell, entry ``i`` being bit ``i``, so word
    ``k`` is the binary expansion of ``k``; words of a given weight come in the order of their
    positions of 1s, lexicographically.
    """
    n = int(n)
    if not 0 <= n <= 64:
        raise ValueError(f"word length must be between 0 and 64, got {n}")
    if weight is None:
        if n > 26:
            raise ValueError(f"2^{n} words are too many to list")
        k = np.arange(2**n, dtype=np.uint64)[:, None]
        return ((k >> np.arange(n, dtype=np.uint64)) & 1).astype(np.uint8)
    weight = int(weight)
    if not 0 <= weight <= n:
        raise ValueError(f"weight must be between 0 and {n}, got {weight}")
    out = np.zeros((comb(n, weight), n), dtype=np.uint8)
    for row, ones in enumerate(itertools.combinations(range(n), weight)):
        out[row, list(ones)] = 1
    return out


def signs(words) -> np.ndarray:
    """0/1 words as +-1 vectors: 0 becomes +1 and 1 becomes -1, so words ``t`` places apart become
    vectors with inner product ``n - 2t``."""
    return 1 - 2 * np.asarray(words, dtype=np.int64)


def min_distance(words) -> int:
    """The smallest Hamming distance between two of the words (rows of 0s and 1s): the minimum
    distance of the code they form. Needs at least two words."""
    a = np.asarray(words, dtype=np.int64)
    if len(a) < 2:
        raise ValueError("a minimum distance needs at least two words")
    s = 1 - 2 * a
    distances = (a.shape[1] - s @ s.T) // 2
    return int(distances[np.triu_indices(len(a), 1)].min())


def is_hadamard(h) -> bool:
    """Whether ``h`` is a Hadamard matrix: square, with entries +-1 and ``h @ h.T == n * I``."""
    h = np.asarray(h)
    if h.ndim != 2 or h.shape[0] != h.shape[1] or not np.isin(h, (1, -1)).all():
        return False
    n = h.shape[0]
    return bool((h.astype(np.int64) @ h.T.astype(np.int64) == n * np.eye(n, dtype=np.int64)).all())


def supports(words, base: int = 1) -> list[tuple[int, ...]]:
    """Each word as the positions of its 1s, counted from ``base``: the usual way of writing down a
    constant-weight code, one word per line."""
    return [tuple(int(i) + base for i in np.flatnonzero(w)) for w in np.asarray(words)]
