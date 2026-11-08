# Development

## Setup

You need [Rust](https://rustup.rs) and [uv](https://docs.astral.sh/uv/getting-started/installation/).
Then:

```sh
git clone https://github.com/vglazer/subgraphsearch
cd subgraphsearch
uv sync --all-extras                              # build the Python package, with every extra and the dev dependencies
```

## Tests and linting

```sh
cargo test --release                              # Rust tests, including the golden fixtures from the C programs
cargo build --release -p subgraphsearch           # the CLI, which a Python test round-trips through
uv run pytest                                     # Python tests
uv run ruff check .                               # lint the Python code
cargo fmt --all --check                           # check the Rust formatting
cargo clippy --release --all-targets -- -D warnings          # lint the Rust code
cargo clippy --release -p subgraphsearch-py -- -D warnings   # and the Python bindings, not a default member
```

CI runs all of these on Linux, macOS and Windows. The Python tests also run the quickstart notebook,
check that both copies of it are up to date and draw the README figure. CI runs them with
`-X warn_default_encoding -W error::EncodingWarning`, so every text file read or written must name its
encoding.

The fixtures in `tests/fixtures` record the original programs' output; see [PORTING.md](PORTING.md)
for how they are made and what they test.

## Scripts

```sh
uv run python examples/make_quickstart.py         # rebuild the quickstart notebooks
uv run python examples/make_figures.py            # redraw the README figures
```

- **The quickstart notebook** is generated from `examples/make_quickstart.py`, in two copies: the
  executed one in `examples` and an output-free one in the package, which
  `python -m subgraphsearch quickstart` hands to users. Edit the script and rerun it rather than
  editing either notebook; a test fails if they are out of date.
- **`scripts/make_pypi_readme.py`** checks that the README converts for PyPI, which cannot render its
  math. CI runs it on every push.

## Releasing

1. Set the version of every crate: `python scripts/bump_version.py X.Y.Z`. The Python package takes
   its version from the crates.
2. Commit and push.
3. [Create a release](https://github.com/vglazer/subgraphsearch/releases/new) on GitHub with a new
   tag `v` followed by that version, e.g. `v1.0.2`.
4. The [release workflow](.github/workflows/release.yml) then runs the CI tests, builds wheels for
   Linux, macOS and Windows and a source distribution, and publishes them to PyPI, then the crates
   to crates.io.

To rehearse a release, run the release workflow by hand (Actions > Release > Run workflow). It
publishes a development release such as `1.0.2.dev7` to TestPyPI instead.
