# Architecture

*Part of the [subgraphsearch documentation](README.md#documentation).*

subgraphsearch is a Rust library with two front ends: a command-line program, and a Python package
that wraps the library through a compiled extension. All searching, graph construction and file
handling happen in Rust; Python adds a friendlier API, plotting, notebook display and conversions to
other libraries. This page describes how the pieces fit together, for anyone changing the code.
[DEVELOPMENT.md](DEVELOPMENT.md) covers building, testing and releasing.

```mermaid
flowchart TB
    pyuser(["Python: import subgraphsearch as ss"])
    cmd(["the subgraphsearch command"])
    rustuser(["Rust: subgraphsearch-core"])

    subgraph pkg["python/subgraphsearch"]
        api["_search.py, _graph.py<br/>search, largest, densest, Graph<br/>resolve defaults, build results"]
        extras["viz/, _html.py, convert.py, vectors.py, designs.py<br/>pure Python: drawing, notebooks, other libraries"]
        clipy["_cli.py"]
    end

    ext["subgraphsearch._core (crates/subgraphsearch-py)<br/>PyO3 bindings; releases the GIL"]
    cli["crates/subgraphsearch-cli: run(argv)<br/>parses options, resolves defaults"]
    bin["main.rs"]

    subgraph core["crates/subgraphsearch-core"]
        entry["search(graph, params)"]
        counting["search.rs<br/>the counting search"]
        degree["regular.rs<br/>the degree search"]
        largest["largest.rs<br/>size after size"]
        support["graph.rs, formats.rs, generate.rs, ops.rs,<br/>layout.rs, rng.rs, legacy.rs"]
    end

    pyuser --> api
    pyuser --> extras
    extras -. "graphs, layouts" .-> ext
    api --> ext
    cmd -- "pip install" --> clipy --> ext
    cmd -- "cargo install" --> bin --> cli
    ext -- "cli_main" --> cli
    ext --> entry
    ext --> largest
    ext --> support
    cli --> entry
    cli --> largest
    cli --> support
    rustuser --> entry
    rustuser --> largest
    rustuser --> support
    largest --> entry
    entry -- "edges or weight" --> counting
    entry -- "degree= or path=True" --> degree
```

## The crates

Rust code comes in *crates*, Rust's packages. The three in `crates/` form one Cargo *workspace*:
they are built together and share one version number.

- **`subgraphsearch-core`** is everything that computes. Its only dependencies are `rayon`, which
  spreads loops across cores, `serde`, which saves and loads data, and `thiserror`, which defines
  error types. It is published to crates.io, Rust's package index, for use from Rust.
  - `search.rs`: the counting search, for edge counts and weights, whose move loop `Workspace::run`
    drives named steps (`start_experiment`, `choose_move`, `apply_swap`, `list_candidates`,
    `bound_rows`, `collect_best_moves`); `regular.rs`: the degree search, for regular
    subgraphs, cycles and paths (see [ALGORITHM.md](ALGORITHM.md#two-searches));
    `largest.rs`: the search for the largest size of a shape.
  - `graph.rs`: `Graph` (a bit matrix and adjacency lists) and `WeightedGraph` (a byte matrix).
    Searches are written once for any type that implements the `SearchGraph` *trait*, Rust's
    form of an interface (much like an abstract base class in Python), and both graph types do.
  - `generate.rs`, `ops.rs`, `layout.rs`: random graphs, operations and products, and the
    force-directed layout behind the drawings.
  - `formats.rs`: every file format. `rng.rs`: xoshiro256++ and the original programs' RANDU
    variant. `legacy.rs`: byte-for-byte emulation of the original C programs.
- **`subgraphsearch`** (`crates/subgraphsearch-cli`) is the command-line program. Its `lib.rs`
  holds the whole program behind `run(argv) -> exit code`, and `main.rs` only calls it, so the
  Python package can embed the same program.
- **`subgraphsearch-py`** builds the extension module `subgraphsearch._core`, with PyO3 for the
  bindings and maturin for the packaging (see the next section).

## PyO3 and maturin

The extension involves two tools with separate jobs: PyO3 decides what Python can call, and
maturin turns the result into something pip can install.

**[PyO3](https://pyo3.rs) is a Rust library: the bindings are Rust code** in
`crates/subgraphsearch-py/src/lib.rs`. Its *attributes*, the `#[...]` lines above a type or
function, generate the code Python needs to create Rust objects and call Rust functions.
- `#[pyclass(frozen)]` makes `Graph` and `WeightedGraph` Python types, each holding an `Arc` of the
  Rust graph: a reference-counted pointer, so that copies share one graph, which is freed when the
  last copy goes. Frozen means immutable, so they need no locking and can be shared across
  threads.
- `#[pyfunction]`, with `#[pyo3(signature = ...)]` for keyword arguments and defaults, exposes each
  function. PyO3 converts the arguments (integers, strings, lists, bytes) and the results (tuples
  and lists) on its own, and the `err` helper maps the core's errors to `IOError` and `ValueError`.
- `#[pymodule] mod _core` lists what the module exports.
- `py.detach(...)` releases the GIL around every long Rust call. The GIL, Python's global
  interpreter lock, lets only one thread at a time run Python code, which is why pure Python cannot
  spread a computation over several cores. A search in Rust touches no Python objects while it runs,
  so it can give the lock up: other Python threads keep running meanwhile, and the search itself
  runs on every core through rayon, as Python threads could not.
- Two Cargo *features*, build options, of PyO3 shape the build. `abi3-py310` compiles against
  Python's *stable ABI*, the part of Python's C interface that stays compatible from one version to
  the next, so one compiled module serves every Python from 3.10 on instead of one module per
  version. `extension-module` leaves Python's own library out of the module, since the interpreter
  that imports the module already provides it. A module built that way cannot be linked into a
  standalone program, so plain `cargo build` and `cargo test` skip this crate: it is not one of the
  workspace's *default members*.

**[maturin](https://www.maturin.rs) is the build tool: the packaging is configuration**, in
`pyproject.toml` and the release workflow.
- It is the package's *build backend* (`[build-system]`), the tool that pip and uv call to build
  the package from source, so `uv sync`, `pip install` from source and the release workflow all
  build through it. It calls `cargo` with the options a Python extension needs.
- `[tool.maturin]` points it at the crate (`manifest-path`) and at the pure-Python package
  (`python-source = "python"`), and names the compiled module `subgraphsearch._core`. maturin puts
  the compiled module inside `python/subgraphsearch`, so a wheel holds a single package with both.
- The package takes its version from the crate (`dynamic = ["version"]`), so
  `scripts/bump_version.py` changes only the Cargo manifests.
- In `.github/workflows/release.yml`, `PyO3/maturin-action` builds a *wheel*, a ready-to-install
  package with the compiled module inside, per platform. For Linux there are two kinds, on x86-64
  and ARM: *manylinux* wheels, built against an old version of glibc, the C library of most
  distributions, so that they run on any newer one, and *musllinux* wheels for distributions built
  on the musl C library instead, such as Alpine. Then macOS on Intel and Apple silicon, and Windows
  on x64 and ARM. Each wheel is built on a machine of its own architecture and smoke-tested there
  (the macOS Intel wheel under Rosetta, the musl wheels in an Alpine container). A source
  distribution covers other platforms, and needs a Rust toolchain to install.

To change what Python can call, edit `lib.rs`; to change how the package is built or shipped, edit
`pyproject.toml` or the release workflow.

## The Python package

`python/subgraphsearch` is the package users import; maturin places the extension inside it as
`_core`.

- **Thin wrappers.** `_graph.py` and `_search.py` define `Graph`, `WeightedGraph`, `search`,
  `largest`, `densest`, `sparsest` and the result classes. A Python `Graph` holds a `_core.Graph`,
  which shares an immutable Rust graph through an `Arc`, so copies and searches never duplicate it.
  Large inputs, such as NumPy edge arrays and the vectors behind `Graph.from_vectors`, cross into
  Rust as byte buffers rather than Python lists.
- **Pure-Python additions.** `viz/` draws graphs, search diagnostics, spectra and matrices with
  Matplotlib and writes interactive HTML pages; `_html.py` gives graphs and results their notebook
  display; `convert.py` converts to and from networkx, igraph and SciPy; `vectors.py` and
  `designs.py` are helpers for codes, Hadamard matrices and block designs. These import their
  optional dependencies only when used.
- **Two commands.** The `subgraphsearch` command installed with the package (`_cli.py`) calls
  `_core.cli_main`, which is the Rust program's `run`, so the Python and Rust installs offer the
  identical program. `python -m subgraphsearch` (`__main__.py`) adds the Python-only commands:
  `explore`, `report` and `quickstart`.

## A search, end to end

`ss.search(g, 30, 346)` in Python:

1. `_search.search` checks the arguments, has the core's `search_settings` resolve the kick size
   and tabu tenure for the kind of search, and passes its `SearchParams` to `_core.search`.
2. The binding reads that object field by field into the core's `SearchParams` (the `Params`
   struct, with PyO3's `FromPyObject`, which copies a Python object's attributes into a Rust
   struct of the same field names), and runs `subgraphsearch_core::search` inside
   `py.detach`, releasing the GIL, so other Python threads keep running meanwhile.
3. The core runs the experiments in parallel on rayon's threads, one per core, each with its own random
   stream derived from the seed, so results depend only on the seed and the settings.
4. The binding returns the parameters the core used, with the automatic tenure resolved, and each
   experiment as a plain tuple; Python builds the `SearchParams`, `Experiment` and
   `SearchResult` objects from them. `largest` works the same way, and reports each size's own
   parameters, so Python never re-derives what the core decided.

The command line follows the same path from step 2: its `search` and `largest` share one
`Settings` struct of options, whose `apply` fills in the `SearchParams` through the same
`search_settings`. `subgraphsearch search --json` writes the core's serde
form of a result, and `SearchResult.load` reads it as well as Python's own saved results, so
`python -m subgraphsearch explore` can show a search run from the command line.

## Performance

The time goes into each move's search for the best swaps (`collect_best_moves` and the functions
it calls), and that work is bound by memory access, not arithmetic: every candidate is reached
through `u_list`, then its degree in `sub_degs` and its entry in the adjacency row, at scattered
positions in memory. *SIMD* instructions, which process several values at once, help when the
values lie side by side; loading scattered ones in one instruction, a *gather*, is something ARM's
vector instructions cannot do and x86's AVX2 does slowly, so wider vectors have little to speed
up. And since bounded rows and buckets, most swaps are never evaluated at all. A build for the newest instructions of the CPU it runs on
(`RUSTFLAGS="-C target-cpu=native"`) ran 0.98 to 1.05 times as fast as the portable build on six
workloads on an Apple M3, so the wheels lose nothing by targeting baseline CPUs there. Gains come
from evaluating fewer swaps, not from evaluating them faster; when changing the scan, compare
builds with alternating runs, since single timings vary by 5% to 10%.

One place could still gain on x86-64. Portable builds target the baseline x86-64 processor, which
lacks POPCNT, the instruction that counts the 1 bits in a word, so `count_ones` becomes a sequence
of bit operations. It is used in `Graph::from_words` (all pairs of binary words, for codes) and
`induced_edge_count`. That has not been measured: an x86-64 runner timing those against a build
for x86-64-v3, a later baseline that has POPCNT and AVX2, would show whether it is worth adding
*runtime dispatch*, which chooses the faster code while running, on processors that have it.

## Conventions

- **Defaults that depend on the kind of search live in the core.** `search_settings` decides the
  kick size and tabu tenure (none and 1 for the degree search and the legacy generator) and
  rejects settings a search cannot take; Python and the CLI both call it, so they cannot
  disagree. `SearchParams::new` gives Rust users the counting search's defaults.
- **Saved results stay loadable.** Results are saved as JSON through
  [serde](https://serde.rs), Rust's serialization library, and files saved before a field existed
  lack it, which serde rejects unless the field has a default. So a new `SearchParams` field
  needs one (`#[serde(default = ...)]`) equal to what searches did before it existed, and the
  Python `SearchParams` dataclass a matching default, so older results load as they ran. For
  example, `tabu_tenure` loads as 1, the tenure every search used before it was a setting, though
  new searches choose theirs automatically.
- **The legacy generator reproduces the C programs exactly.** Changes to the search must keep
  `rng="legacy"` byte-identical; the *golden tests* in `crates/subgraphsearch-cli/tests/golden.rs`
  compare its output with output recorded from the original programs ([PORTING.md](PORTING.md)).
- **Speed-ups must not change results.** The same seed gives the same experiments, so an
  optimization must make exactly the same moves. The move scan in `search.rs` has three layers
  that skip work without changing it: bounds that rule out whole rows, buckets of columns by
  degree that narrow a row to the columns that can matter, and a pause that stops building
  buckets when they do not help. `skipping_rows_changes_no_result` compares every kind of search
  against a plain scan of every swap; keep it passing, and extend it for any new shortcut.

## Adding a search setting

A new setting touches every layer, in this order:

1. The core: a `SearchParams` field with a serde default (see Conventions), used by the search,
   and unit tests; if its default depends on the kind of search, decide it in `search_settings`.
2. The binding: a field of the `Params` struct and an entry in `params_dict`, in
   `crates/subgraphsearch-py/src/lib.rs`.
3. Python: a field of the `SearchParams` dataclass with the same name and default, the argument of
   `search` (and `largest`) in `_search.py`, and its docstring.
4. The CLI: a field of the shared `Settings` struct, set in `Settings::apply`, in
   `crates/subgraphsearch-cli/src/lib.rs`; or of `SearchArgs` alone if only `search` takes it.
5. Tests in `python/tests`, including one through the command line, and the documentation:
   [ALGORITHM.md](ALGORITHM.md), [TUNING.md](TUNING.md) and the README's option table.
