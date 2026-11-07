//! Python bindings: the `subsearch._core` extension module. The public Python API lives in the
//! pure-Python `subsearch` package, which wraps these.

use std::path::PathBuf;
use std::sync::Arc;

use pyo3::exceptions::{PyIOError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use subsearch_core::formats::{self, Format, WriteOptions};
use subsearch_core::rng::{Randu, RngKind, Xoshiro};
use subsearch_core::{Error, Graph, Model, SearchParams, generate, layout, legacy, ops};

fn err(e: Error) -> PyErr {
    match e {
        Error::Io(io) => PyIOError::new_err(io.to_string()),
        other => PyValueError::new_err(other.to_string()),
    }
}

fn parse<T: std::str::FromStr<Err = String>>(s: &str) -> PyResult<T> {
    s.parse().map_err(PyValueError::new_err)
}

/// An immutable simple undirected graph on vertices 0..n.
#[pyclass(module = "subsearch._core", name = "Graph", frozen)]
struct PyGraph {
    inner: Arc<Graph>,
}

impl PyGraph {
    fn wrap(g: Graph) -> Self {
        PyGraph { inner: Arc::new(g) }
    }

    fn check(&self, v: usize) -> PyResult<()> {
        let n = self.inner.num_vertices();
        if v >= n {
            return Err(PyValueError::new_err(format!(
                "vertex {v} is out of range [0, {n})"
            )));
        }
        Ok(())
    }
}

#[pymethods]
impl PyGraph {
    /// Graph(n, edges): a graph on n vertices with the given (u, v) edges.
    #[new]
    fn new(n: usize, edges: Vec<(usize, usize)>) -> PyResult<Self> {
        Ok(Self::wrap(Graph::from_edges(n, edges).map_err(err)?))
    }

    /// A graph from edges packed as little-endian uint32 (u, v) pairs, e.g. numpy's tobytes().
    #[staticmethod]
    fn from_edge_bytes(n: usize, data: &[u8]) -> PyResult<Self> {
        if data.len() % 8 != 0 {
            return Err(PyValueError::new_err(
                "edge data must be a whole number of uint32 pairs",
            ));
        }
        let word = |c: &[u8]| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as usize;
        let edges = data.chunks_exact(8).map(|c| (word(&c[..4]), word(&c[4..])));
        Ok(Self::wrap(Graph::from_edges(n, edges).map_err(err)?))
    }

    /// Reads a graph file; the format is inferred from the extension unless given.
    #[staticmethod]
    #[pyo3(signature = (path, format = None))]
    fn read(py: Python<'_>, path: PathBuf, format: Option<&str>) -> PyResult<Self> {
        let format = format.map(parse::<Format>).transpose()?;
        let g = py
            .detach(|| formats::read_file(&path, format))
            .map_err(err)?;
        Ok(Self::wrap(g))
    }

    /// Parses a graph from text in the given format.
    #[staticmethod]
    fn parse(text: &str, format: &str) -> PyResult<Self> {
        Ok(Self::wrap(
            formats::parse(text, parse(format)?).map_err(err)?,
        ))
    }

    /// Serialises the graph; `highlight` marks vertices in DOT and GraphML output.
    #[pyo3(signature = (format, highlight = Vec::new()))]
    fn dumps(&self, format: &str, highlight: Vec<usize>) -> PyResult<String> {
        Ok(formats::to_string(
            &self.inner,
            parse(format)?,
            &WriteOptions {
                highlight: &highlight,
            },
        ))
    }

    /// Writes the graph to a file; the format is inferred from the extension unless given.
    #[pyo3(signature = (path, format = None, highlight = Vec::new()))]
    fn write(&self, path: PathBuf, format: Option<&str>, highlight: Vec<usize>) -> PyResult<()> {
        let format = format.map(parse::<Format>).transpose()?;
        formats::write_file(
            &self.inner,
            &path,
            format,
            &WriteOptions {
                highlight: &highlight,
            },
        )
        .map_err(err)
    }

    #[getter]
    fn num_vertices(&self) -> usize {
        self.inner.num_vertices()
    }

    #[getter]
    fn num_edges(&self) -> usize {
        self.inner.num_edges()
    }

    #[getter]
    fn density(&self) -> f64 {
        self.inner.density()
    }

    /// Edges (u, v) with u < v.
    fn edges(&self) -> Vec<(usize, usize)> {
        self.inner.edges().collect()
    }

    /// Edges packed as little-endian uint32 (u, v) pairs, for numpy.frombuffer.
    fn edge_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        let mut data = Vec::with_capacity(self.inner.num_edges() * 8);
        for (u, v) in self.inner.edges() {
            data.extend((u as u32).to_le_bytes());
            data.extend((v as u32).to_le_bytes());
        }
        PyBytes::new(py, &data)
    }

    fn neighbors(&self, u: usize) -> PyResult<Vec<u32>> {
        self.check(u)?;
        Ok(self.inner.neighbors(u).to_vec())
    }

    fn degrees(&self) -> Vec<usize> {
        self.inner.degrees()
    }

    fn has_edge(&self, u: usize, v: usize) -> PyResult<bool> {
        self.check(u)?;
        self.check(v)?;
        Ok(self.inner.has_edge(u, v))
    }

    /// (component index per vertex, number of components).
    fn components(&self) -> (Vec<usize>, usize) {
        self.inner.components()
    }

    fn induced_edge_count(&self, vertices: Vec<usize>) -> PyResult<usize> {
        ops::rest(&self.inner, &vertices).map_err(err)?;
        Ok(self.inner.induced_edge_count(&vertices))
    }

    fn complement(&self) -> Self {
        Self::wrap(ops::complement(&self.inner))
    }

    fn induced(&self, vertices: Vec<usize>) -> PyResult<Self> {
        Ok(Self::wrap(
            ops::induced(&self.inner, &vertices).map_err(err)?,
        ))
    }

    fn switch(&self, vertices: Vec<usize>) -> PyResult<Self> {
        Ok(Self::wrap(
            ops::switch(&self.inner, &vertices).map_err(err)?,
        ))
    }

    fn common_neighbourhood(&self, vertices: Vec<usize>) -> PyResult<Vec<usize>> {
        ops::common_neighbourhood(&self.inner, &vertices).map_err(err)
    }

    fn common_non_neighbourhood(&self, vertices: Vec<usize>) -> PyResult<Vec<usize>> {
        ops::common_non_neighbourhood(&self.inner, &vertices).map_err(err)
    }

    /// Force-directed layout: one (x, y) in the unit square per vertex.
    #[pyo3(signature = (iterations = 150, seed = 1))]
    fn spring_layout(&self, py: Python<'_>, iterations: usize, seed: u64) -> Vec<(f64, f64)> {
        let g = Arc::clone(&self.inner);
        py.detach(move || layout::spring(&g, iterations, seed))
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        formats::write_graph6(&self.inner).hash(&mut h);
        h.finish()
    }
}

/// A random graph from `model` with `density` per mille.
#[pyfunction]
fn generate_graph(model: &str, n: usize, density: u32, seed: i64, rng: &str) -> PyResult<PyGraph> {
    let model: Model = parse(model)?;
    let g = match parse::<RngKind>(rng)? {
        RngKind::Legacy => generate::generate(model, n, density, &mut Randu::new(seed)),
        RngKind::Xoshiro => generate::generate(model, n, density, &mut Xoshiro::new(seed as u64)),
    };
    Ok(PyGraph::wrap(g.map_err(err)?))
}

type ExperimentTuple = (i64, usize, bool, Vec<usize>, Option<Vec<u32>>);

/// Runs a search; returns one (best_conflict, moves, success, vertices, trace) per experiment.
#[pyfunction]
#[pyo3(signature = (graph, size, edges, fixed, experiments, max_moves, div_freq, div_dur, seed, rng, trace))]
#[allow(clippy::too_many_arguments)]
fn search(
    py: Python<'_>,
    graph: &PyGraph,
    size: usize,
    edges: i64,
    fixed: Vec<usize>,
    experiments: usize,
    max_moves: usize,
    div_freq: usize,
    div_dur: usize,
    seed: i64,
    rng: &str,
    trace: bool,
) -> PyResult<Vec<ExperimentTuple>> {
    let params = SearchParams {
        size,
        edges,
        fixed,
        experiments,
        max_moves,
        div_freq,
        div_dur,
        seed,
        rng: parse(rng)?,
        record_trace: trace,
    };
    let g = Arc::clone(&graph.inner);
    let result = py
        .detach(move || subsearch_core::search(&g, &params))
        .map_err(err)?;
    Ok(result
        .experiments
        .into_iter()
        .map(|e| (e.best_conflict, e.moves, e.success, e.vertices, e.trace))
        .collect())
}

/// Runs the original `sub_search` program's interface on `stdin`; returns (stdout, exit status).
#[pyfunction]
fn legacy_sub_search(py: Python<'_>, stdin: String) -> PyResult<(String, i32)> {
    py.detach(move || {
        let mut out = Vec::new();
        let start = std::time::Instant::now();
        let status = legacy::sub_search(&stdin, &mut out, || {
            format!(
                "T> {:.2} seconds H> python\n",
                start.elapsed().as_secs_f64()
            )
        })?;
        Ok((String::from_utf8(out).unwrap(), status))
    })
    .map_err(err)
}

/// Runs the original `ggen` program's interface on `stdin`; returns its stdout.
#[pyfunction]
fn legacy_ggen(stdin: &str) -> PyResult<String> {
    let mut out = Vec::new();
    legacy::ggen(stdin, &mut out).map_err(err)?;
    Ok(String::from_utf8(out).unwrap())
}

/// Names of the supported file formats, and whether each can be read.
#[pyfunction]
fn formats_list() -> Vec<(&'static str, bool)> {
    Format::ALL
        .iter()
        .map(|(f, name)| (*name, f.readable()))
        .collect()
}

#[pymodule]
mod _core {
    #[pymodule_export]
    use super::{PyGraph, formats_list, generate_graph, legacy_ggen, legacy_sub_search, search};

    #[pymodule_init]
    fn init(m: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
        use pyo3::types::PyModuleMethods;
        m.add("__version__", env!("CARGO_PKG_VERSION"))
    }
}
