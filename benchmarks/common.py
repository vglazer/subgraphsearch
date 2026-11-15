"""Shared helpers for the benchmark scripts: instance loading, data fetching and timing."""

from __future__ import annotations

import io
import statistics
import time
import urllib.request
import zipfile
from pathlib import Path

import numpy as np

import subgraphsearch as ss

ROOT = Path(__file__).resolve().parents[1]
DATA = Path(__file__).resolve().parent / "data"   # downloaded instances (not committed)
MDPLIB_ZIP = "https://zenodo.org/api/records/13749317/files/mdplib_2010.zip/content"


def timed(fn, repeats: int = 1):
    """Runs ``fn`` ``repeats`` times; returns its last result and the median time in seconds."""
    times, result = [], None
    for _ in range(repeats):
        start = time.perf_counter()
        result = fn()
        times.append(time.perf_counter() - start)
    return result, statistics.median(times)


def dimacs() -> dict[str, tuple[ss.Graph, int]]:
    """The eight DIMACS clique graphs in examples/data, by name, with their clique numbers."""
    graphs = ss.parse_all((ROOT / "examples/data/dimacs.g6").read_text(encoding="utf-8"), "graph6")
    names = ["C125.9", "C250.9", "p_hat300-3", "MANN_a27", "hamming8-4", "keller4", "brock200_2", "brock400_2"]
    return dict(zip(names, zip(graphs, [34, 44, 36, 126, 16, 11, 12, 29])))


def mdplib_path(name: str) -> Path:
    """An MDPLIB instance file: from examples/data/mdplib if it is there, else downloaded once into
    benchmarks/data by HTTP range requests on MDPLIB's Zenodo archive (679 MB, never fetched whole)."""
    local = ROOT / "examples/data/mdplib" / f"{name}.txt"
    if local.exists():
        return local
    path = DATA / f"{name}.txt"
    if not path.exists():
        DATA.mkdir(exist_ok=True)
        with zipfile.ZipFile(io.BufferedReader(_RangeFile(MDPLIB_ZIP), buffer_size=1 << 20)) as z:
            member = next(m for m in z.namelist() if m.endswith(f"/{name}.txt"))
            path.write_bytes(z.read(member))
    return path


def read_mdplib(name: str, real: bool = False):
    """``(n, k, triples)`` of an MDPLIB instance: items, items to choose, and ``(i, j, distance)`` rows."""
    with open(mdplib_path(name), encoding="utf-8") as f:
        n, k = map(int, f.readline().split())
        triples = np.loadtxt(f, dtype=float if real else np.int64)
    return n, k, triples


def integer_instance(name: str) -> tuple[ss.WeightedGraph, int]:
    """An MDPLIB instance with integer distances as a weighted graph, and the number of items to choose."""
    n, k, t = read_mdplib(name)
    return ss.WeightedGraph(n, t[t[:, 2] > 0]), k


class _RangeFile(io.RawIOBase):
    """A read-only, seekable view of a remote file, fetched by HTTP range requests."""

    def __init__(self, url: str):
        self.url, self.pos = url, 0
        with urllib.request.urlopen(urllib.request.Request(url, method="HEAD"), timeout=60) as r:
            self.size = int(r.headers["Content-Length"])

    def readable(self) -> bool:
        return True

    def seekable(self) -> bool:
        return True

    def tell(self) -> int:
        return self.pos

    def seek(self, offset: int, whence: int = 0) -> int:
        self.pos = offset if whence == 0 else self.pos + offset if whence == 1 else self.size + offset
        return self.pos

    def readinto(self, buffer) -> int:
        if self.pos >= self.size or not len(buffer):
            return 0
        end = min(self.pos + len(buffer), self.size) - 1
        request = urllib.request.Request(self.url, headers={"Range": f"bytes={self.pos}-{end}"})
        with urllib.request.urlopen(request, timeout=60) as r:
            data = r.read()
        buffer[: len(data)] = data
        self.pos += len(data)
        return len(data)
