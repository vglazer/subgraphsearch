"""The original USRA programs' interfaces, reproduced byte for byte.

These take exactly the text the C programs read on stdin and return what they printed, which is
useful for re-running old experiments or scripts.

>>> out = ggen("2 10 0 500 42 0 0 0")
>>> out.splitlines()[0]
'2 10 0 500 42  0 0  0'
"""

from __future__ import annotations

from . import _core


def sub_search(stdin: str) -> tuple[str, int]:
    """Runs ``sub_search``: parameters, '-1'-terminated adjacency lists and any fixed vertices.
    Returns ``(stdout, exit status)``. The final ``T> ... H> ...`` timing line reports elapsed
    time and the host name ``python`` instead of CPU time and the real host name."""
    return _core.legacy_sub_search(stdin)


def asub_search(stdin: str) -> tuple[str, int]:
    """Runs ``asub_search`` (``sub_search`` with limited aspiration), which reads and prints the
    same things as :func:`sub_search`."""
    return _core.legacy_asub_search(stdin)


def wsub_search(stdin: str) -> tuple[str, int]:
    """Runs ``wsub_search`` (``sub_search`` on weighted graphs): entry ``x`` in adjacency list
    ``i`` is the edge ``{i, x % v}`` with weight ``x // v + 1``, and the target is a total weight."""
    return _core.legacy_wsub_search(stdin)


def wggen(args: str | list[str]) -> str:
    """Runs ``wggen`` with its command-line arguments (``"VERTICES DENSITY SEED"`` or a list)
    and returns its output. Raises ``ValueError`` with ``wggen``'s message on invalid arguments."""
    return _core.legacy_wggen(args.split() if isinstance(args, str) else [str(a) for a in args])


def ggen(stdin: str) -> str:
    """Runs ``ggen`` (parameters, plus any input lists and fixed vertices) and returns its output.
    Raises ``ValueError`` with ``ggen``'s error message on invalid input."""
    return _core.legacy_ggen(stdin)
