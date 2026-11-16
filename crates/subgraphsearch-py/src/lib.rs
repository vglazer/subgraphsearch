//! Python bindings: the `subgraphsearch._core` extension module. The public Python API lives in the
//! pure-Python `subgraphsearch` package, which wraps these.

use std::path::PathBuf;
use std::sync::Arc;

use pyo3::exceptions::{PyIOError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyTuple};
use subgraphsearch_core::formats::{self, Format, WriteOptions};
use subgraphsearch_core::rng::{Randu, RngKind, Xoshiro};
use subgraphsearch_core::{
    Error, Graph, Model, SearchGraph, SearchParams, WeightedGraph, generate, layout, legacy, ops,
};

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
#[pyclass(module = "subgraphsearch._core", name = "Graph", frozen)]
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
        if !data.len().is_multiple_of(8) {
            return Err(PyValueError::new_err(
                "edge data must be a whole number of uint32 pairs",
            ));
        }
        let word = |c: &[u8]| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as usize;
        let edges = data.chunks_exact(8).map(|c| (word(&c[..4]), word(&c[4..])));
        Ok(Self::wrap(Graph::from_edges(n, edges).map_err(err)?))
    }

    /// The graph on vectors packed as little-endian int64 rows of length `dim`, joined when their
    /// inner product is in `allowed`; see `Graph::from_vectors`.
    #[staticmethod]
    fn from_vector_bytes(
        py: Python<'_>,
        data: &[u8],
        dim: usize,
        allowed: Vec<i64>,
    ) -> PyResult<Self> {
        if !data.len().is_multiple_of(8) {
            return Err(PyValueError::new_err(
                "vector data must be a whole number of int64 entries",
            ));
        }
        let entries: Vec<i64> = data
            .chunks_exact(8)
            .map(|c| i64::from_le_bytes(c.try_into().expect("8 bytes")))
            .collect();
        let g = py
            .detach(move || Graph::from_vectors(&entries, dim, &allowed))
            .map_err(err)?;
        Ok(Self::wrap(g))
    }

    /// The graph on binary words packed as little-endian uint64 integers, joined when their Hamming
    /// distance is in `distances`; see `Graph::from_words`.
    #[staticmethod]
    fn from_word_bytes(py: Python<'_>, data: &[u8], distances: Vec<u32>) -> PyResult<Self> {
        if !data.len().is_multiple_of(8) {
            return Err(PyValueError::new_err(
                "word data must be a whole number of uint64 entries",
            ));
        }
        let words: Vec<u64> = data
            .chunks_exact(8)
            .map(|c| u64::from_le_bytes(c.try_into().expect("8 bytes")))
            .collect();
        Ok(Self::wrap(
            py.detach(move || Graph::from_words(&words, &distances)),
        ))
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

    /// Reads every graph in a file (graph6 and sparse6 hold one per line).
    #[staticmethod]
    #[pyo3(signature = (path, format = None))]
    fn read_all(py: Python<'_>, path: PathBuf, format: Option<&str>) -> PyResult<Vec<Self>> {
        let format = format.map(parse::<Format>).transpose()?;
        let graphs = py
            .detach(|| formats::read_file_all(&path, format))
            .map_err(err)?;
        Ok(graphs.into_iter().map(Self::wrap).collect())
    }

    /// Parses every graph in `text`.
    #[staticmethod]
    fn parse_all(text: &str, format: &str) -> PyResult<Vec<Self>> {
        let graphs = formats::parse_all(text, parse(format)?).map_err(err)?;
        Ok(graphs.into_iter().map(Self::wrap).collect())
    }

    /// Serializes the graph; `highlight` marks vertices in DOT and GraphML output.
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

    /// A product with `other`: "cartesian", "tensor", "strong" or "lexicographic".
    fn product(&self, py: Python<'_>, other: PyRef<'_, PyGraph>, kind: &str) -> PyResult<Self> {
        let kind: ops::Product = kind.parse().map_err(err)?;
        let (g, h) = (Arc::clone(&self.inner), Arc::clone(&other.inner));
        Ok(Self::wrap(py.detach(move || ops::product(&g, &h, kind))))
    }

    fn replacement(&self, other: PyRef<'_, PyGraph>) -> PyResult<Self> {
        Ok(Self::wrap(
            ops::replacement(&self.inner, &other.inner).map_err(err)?,
        ))
    }

    fn zigzag(&self, other: PyRef<'_, PyGraph>) -> PyResult<Self> {
        Ok(Self::wrap(
            ops::zigzag(&self.inner, &other.inner).map_err(err)?,
        ))
    }

    fn common_neighborhood(&self, vertices: Vec<usize>) -> PyResult<Vec<usize>> {
        ops::common_neighborhood(&self.inner, &vertices).map_err(err)
    }

    fn common_non_neighborhood(&self, vertices: Vec<usize>) -> PyResult<Vec<usize>> {
        ops::common_non_neighborhood(&self.inner, &vertices).map_err(err)
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

/// An immutable undirected graph with integer edge weights between 1 and 255.
#[pyclass(module = "subgraphsearch._core", name = "WeightedGraph", frozen)]
struct PyWeightedGraph {
    inner: Arc<WeightedGraph>,
}

impl PyWeightedGraph {
    fn wrap(g: WeightedGraph) -> Self {
        PyWeightedGraph { inner: Arc::new(g) }
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
impl PyWeightedGraph {
    /// WeightedGraph(n, edges): edges are (u, v, weight) triples.
    #[new]
    fn new(n: usize, edges: Vec<(usize, usize, u32)>) -> PyResult<Self> {
        Ok(Self::wrap(
            WeightedGraph::from_weighted_edges(n, edges).map_err(err)?,
        ))
    }

    /// Every edge of an unweighted graph, with weight 1.
    #[staticmethod]
    fn from_graph(g: &PyGraph) -> Self {
        Self::wrap(WeightedGraph::from_graph(&g.inner))
    }

    #[staticmethod]
    #[pyo3(signature = (path, format = None))]
    fn read(py: Python<'_>, path: PathBuf, format: Option<&str>) -> PyResult<Self> {
        let format = format.map(parse::<Format>).transpose()?;
        let g = py
            .detach(|| formats::read_weighted_file(&path, format))
            .map_err(err)?;
        Ok(Self::wrap(g))
    }

    #[staticmethod]
    fn parse(text: &str, format: &str) -> PyResult<Self> {
        Ok(Self::wrap(
            formats::parse_weighted(text, parse(format)?).map_err(err)?,
        ))
    }

    #[pyo3(signature = (format, highlight = Vec::new()))]
    fn dumps(&self, format: &str, highlight: Vec<usize>) -> PyResult<String> {
        formats::to_string_weighted(
            &self.inner,
            parse(format)?,
            &WriteOptions {
                highlight: &highlight,
            },
        )
        .map_err(err)
    }

    #[pyo3(signature = (path, format = None, highlight = Vec::new()))]
    fn write(&self, path: PathBuf, format: Option<&str>, highlight: Vec<usize>) -> PyResult<()> {
        let format = format.map(parse::<Format>).transpose()?;
        formats::write_weighted_file(
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
    fn total_weight(&self) -> u64 {
        self.inner.total_weight()
    }

    /// Edges (u, v, weight) with u < v.
    fn edges(&self) -> Vec<(usize, usize, u8)> {
        self.inner.edges().collect()
    }

    fn weight(&self, u: usize, v: usize) -> PyResult<u8> {
        self.check(u)?;
        self.check(v)?;
        Ok(self.inner.weight(u, v))
    }

    fn neighbors(&self, u: usize) -> PyResult<Vec<u32>> {
        self.check(u)?;
        Ok(self.inner.neighbors(u).to_vec())
    }

    fn induced_weight(&self, vertices: Vec<usize>) -> PyResult<u64> {
        let structure = self.inner.structure();
        ops::rest(&structure, &vertices).map_err(err)?;
        Ok(self.inner.induced_weight(&vertices))
    }

    /// The unweighted graph with the same edges.
    fn structure(&self) -> PyGraph {
        PyGraph::wrap(self.inner.structure())
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

/// A weighted random graph as the original wggen makes it, `density` in percent (mean weight per
/// vertex pair times 100).
#[pyfunction]
fn generate_weighted(n: usize, density: u32, seed: i64, rng: &str) -> PyResult<PyWeightedGraph> {
    let g = match parse::<RngKind>(rng)? {
        RngKind::Legacy => generate::weighted(n, density, &mut Randu::new(seed)),
        RngKind::Xoshiro => generate::weighted(n, density, &mut Xoshiro::new(seed as u64)),
    };
    Ok(PyWeightedGraph::wrap(g.map_err(err)?))
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

/// The `dimension`-dimensional hypercube.
#[pyfunction]
fn hypercube_graph(dimension: usize) -> PyResult<PyGraph> {
    Ok(PyGraph::wrap(generate::hypercube(dimension).map_err(err)?))
}

/// The `rows` by `cols` grid.
#[pyfunction]
fn grid_graph(rows: usize, cols: usize) -> PyGraph {
    PyGraph::wrap(generate::grid(rows, cols))
}

/// An Erdős–Rényi random graph with the given mean degree, generated in time proportional to its
/// edges.
#[pyfunction]
fn sparse_random_graph(py: Python<'_>, n: usize, mean_degree: f64, seed: i64) -> PyResult<PyGraph> {
    let g = py
        .detach(move || generate::sparse_random(n, mean_degree, &mut Xoshiro::new(seed as u64)))
        .map_err(err)?;
    Ok(PyGraph::wrap(g))
}

/// A random regular graph of the given degree.
#[pyfunction]
fn random_regular_graph(py: Python<'_>, n: usize, degree: usize, seed: i64) -> PyResult<PyGraph> {
    let g = py
        .detach(move || generate::random_regular(n, degree, &mut Xoshiro::new(seed as u64)))
        .map_err(err)?;
    Ok(PyGraph::wrap(g))
}

/// The graph of an incidence structure: on its points (adjacent when exactly `k` blocks contain
/// both) or, with `on_blocks`, on its blocks (adjacent when they share exactly `k` points).
#[pyfunction]
#[pyo3(signature = (blocks, k, on_blocks, points = None))]
fn incidence_graph(
    py: Python<'_>,
    blocks: Vec<Vec<usize>>,
    k: usize,
    on_blocks: bool,
    points: Option<usize>,
) -> PyResult<PyGraph> {
    let g = py
        .detach(move || {
            let incidence = subgraphsearch_core::ops::Incidence::from_blocks(points, &blocks)?;
            Ok::<_, subgraphsearch_core::Error>(if on_blocks {
                incidence.set_graph(k)
            } else {
                incidence.symbol_graph(k)
            })
        })
        .map_err(err)?;
    Ok(PyGraph::wrap(g))
}

type ExperimentTuple = (i64, usize, bool, Vec<usize>, Option<Vec<u32>>);

/// `SearchParams.aspiration` in Python: `False`, `True` (the original, limited criterion) or
/// `"full"`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AspirationParam {
    Off,
    Limited,
    Full,
}

impl<'a, 'py> FromPyObject<'a, 'py> for AspirationParam {
    type Error = PyErr;

    fn extract(ob: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        if let Ok(on) = ob.extract::<bool>() {
            return Ok(if on { Self::Limited } else { Self::Off });
        }
        match ob.extract::<String>().as_deref() {
            Ok("full") => Ok(Self::Full),
            _ => Err(PyValueError::new_err(
                "aspiration must be False, True or \"full\"",
            )),
        }
    }
}

/// The Python package's `SearchParams`, read attribute by attribute. A `tabu_tenure` of 0 asks for
/// the automatic tenure.
#[derive(FromPyObject)]
struct Params {
    size: usize,
    edges: i64,
    fixed: Vec<usize>,
    experiments: usize,
    max_moves: usize,
    div_freq: usize,
    div_dur: usize,
    seed: i64,
    rng: String,
    trace: bool,
    aspiration: AspirationParam,
    degree: Option<i64>,
    two_phase: bool,
    connected: bool,
    path: bool,
    endpoints: Option<[usize; 2]>,
    kick_size: usize,
    kick_ban: usize,
    tabu_tenure: usize,
}

impl Params {
    fn core(self) -> PyResult<SearchParams> {
        Ok(SearchParams {
            size: self.size,
            edges: self.edges,
            fixed: self.fixed,
            experiments: self.experiments,
            max_moves: self.max_moves,
            div_freq: self.div_freq,
            div_dur: self.div_dur,
            kick_size: self.kick_size,
            kick_ban: self.kick_ban,
            tabu_tenure: self.tabu_tenure,
            seed: self.seed,
            rng: parse(&self.rng)?,
            record_trace: self.trace,
            aspiration: self.aspiration == AspirationParam::Limited,
            full_aspiration: self.aspiration == AspirationParam::Full,
            connected: self.connected,
            path: self.path,
            endpoints: self.endpoints,
            regular: self.degree.map(|degree| subgraphsearch_core::Regular {
                degree,
                two_phase: self.two_phase,
            }),
        })
    }
}

/// The parameters a search used, as keyword arguments of the Python package's `SearchParams`.
fn params_dict<'py>(py: Python<'py>, p: &SearchParams) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("size", p.size)?;
    d.set_item("edges", p.edges)?;
    d.set_item("fixed", PyTuple::new(py, &p.fixed)?)?;
    d.set_item("experiments", p.experiments)?;
    d.set_item("max_moves", p.max_moves)?;
    d.set_item("div_freq", p.div_freq)?;
    d.set_item("div_dur", p.div_dur)?;
    d.set_item("seed", p.seed)?;
    d.set_item(
        "rng",
        match p.rng {
            RngKind::Legacy => "legacy",
            RngKind::Xoshiro => "xoshiro",
        },
    )?;
    d.set_item("trace", p.record_trace)?;
    if p.full_aspiration {
        d.set_item("aspiration", "full")?;
    } else {
        d.set_item("aspiration", p.aspiration)?;
    }
    d.set_item("degree", p.regular.map(|r| r.degree))?;
    d.set_item("two_phase", p.regular.is_some_and(|r| r.two_phase))?;
    d.set_item("connected", p.connected)?;
    d.set_item("path", p.path)?;
    d.set_item("endpoints", p.endpoints.map(|[s, t]| (s, t)))?;
    d.set_item("kick_size", p.kick_size)?;
    d.set_item("kick_ban", p.kick_ban)?;
    d.set_item("tabu_tenure", p.tabu_tenure)?;
    Ok(d)
}

fn experiment_tuples(experiments: Vec<subgraphsearch_core::Experiment>) -> Vec<ExperimentTuple> {
    experiments
        .into_iter()
        .map(|e| (e.best_conflict, e.moves, e.success, e.vertices, e.trace))
        .collect()
}

/// Runs a search with the Python package's `SearchParams`; returns the parameters it used (as
/// `SearchParams` keyword arguments) and one (best_conflict, moves, success, vertices, trace) per
/// experiment.
#[pyfunction]
fn search<'py>(
    py: Python<'py>,
    graph: &Bound<'py, PyAny>,
    params: Params,
) -> PyResult<(Bound<'py, PyDict>, Vec<ExperimentTuple>)> {
    let params = params.core()?;
    let result = if let Ok(g) = graph.extract::<PyRef<'_, PyGraph>>() {
        run_search(py, Arc::clone(&g.inner), params)?
    } else if let Ok(g) = graph.extract::<PyRef<'_, PyWeightedGraph>>() {
        run_search(py, Arc::clone(&g.inner), params)?
    } else {
        return Err(PyValueError::new_err(
            "graph must be a Graph or a WeightedGraph",
        ));
    };
    Ok((
        params_dict(py, &result.params)?,
        experiment_tuples(result.experiments),
    ))
}

/// One size searched by `largest`: (size, the parameters used, experiments as `search` returns them).
type SizeTuple<'py> = (usize, Bound<'py, PyDict>, Vec<ExperimentTuple>);

/// Searches for the largest induced path, cycle, tree or clique, with everything but the shape's
/// own settings from the `SearchParams` given; returns (the largest size found, one `SizeTuple`
/// per size searched, in increasing order).
#[pyfunction]
fn largest<'py>(
    py: Python<'py>,
    graph: PyRef<'py, PyGraph>,
    shape: &str,
    start: Option<usize>,
    lookahead: usize,
    params: Params,
) -> PyResult<(Option<usize>, Vec<SizeTuple<'py>>)> {
    let shape: subgraphsearch_core::Shape = shape.parse().map_err(err)?;
    let base = params.core()?;
    let g = Arc::clone(&graph.inner);
    let found = py
        .detach(move || subgraphsearch_core::largest(&g, shape, &base, start, lookahead))
        .map_err(err)?;
    let sizes = found
        .results
        .into_iter()
        .map(|r| {
            Ok((
                r.params.size,
                params_dict(py, &r.params)?,
                experiment_tuples(r.experiments),
            ))
        })
        .collect::<PyResult<_>>()?;
    Ok((found.size, sizes))
}

/// The automatic tabu tenure: see `subgraphsearch_core::auto_tabu_tenure`.
#[pyfunction]
fn auto_tabu_tenure(size: usize, vertices: usize) -> usize {
    subgraphsearch_core::auto_tabu_tenure(size, vertices)
}

fn run_search<G: SearchGraph + Send + 'static>(
    py: Python<'_>,
    graph: Arc<G>,
    params: SearchParams,
) -> PyResult<subgraphsearch_core::SearchResult> {
    py.detach(move || subgraphsearch_core::search(&*graph, &params))
        .map_err(err)
}

/// Runs the original `sub_search` program's interface on `stdin`; returns (stdout, exit status).
#[pyfunction]
fn legacy_sub_search(py: Python<'_>, stdin: String) -> PyResult<(String, i32)> {
    run_legacy_search(py, stdin, "sub")
}

/// Runs the original `asub_search` program's interface on `stdin`; returns (stdout, exit status).
#[pyfunction]
fn legacy_asub_search(py: Python<'_>, stdin: String) -> PyResult<(String, i32)> {
    run_legacy_search(py, stdin, "asub")
}

/// Runs the original `wsub_search` program's interface on `stdin`; returns (stdout, exit status).
#[pyfunction]
fn legacy_wsub_search(py: Python<'_>, stdin: String) -> PyResult<(String, i32)> {
    run_legacy_search(py, stdin, "wsub")
}

/// Runs the original `reg_search` program's interface on `stdin`; returns (stdout, exit status).
#[pyfunction]
fn legacy_reg_search(py: Python<'_>, stdin: String) -> PyResult<(String, i32)> {
    run_legacy_search(py, stdin, "reg")
}

/// Runs the original `ereg_search` program's interface on `stdin`; returns (stdout, exit status).
#[pyfunction]
fn legacy_ereg_search(py: Python<'_>, stdin: String) -> PyResult<(String, i32)> {
    run_legacy_search(py, stdin, "ereg")
}

/// Runs the original `wggen` program with these command-line arguments; returns its stdout.
#[pyfunction]
fn legacy_wggen(args: Vec<String>) -> PyResult<String> {
    let mut out = Vec::new();
    legacy::wggen(&args, &mut out).map_err(err)?;
    Ok(String::from_utf8(out).unwrap())
}

fn run_legacy_search(
    py: Python<'_>,
    stdin: String,
    program: &'static str,
) -> PyResult<(String, i32)> {
    py.detach(move || {
        let mut out = Vec::new();
        let start = std::time::Instant::now();
        let footer = || {
            format!(
                "T> {:.2} seconds H> python\n",
                start.elapsed().as_secs_f64()
            )
        };
        let status = match program {
            "asub" => legacy::asub_search(&stdin, &mut out, footer)?,
            "wsub" => legacy::wsub_search(&stdin, &mut out, footer)?,
            "reg" => legacy::reg_search(&stdin, &mut out, footer)?,
            "ereg" => legacy::ereg_search(&stdin, &mut out, footer)?,
            _ => legacy::sub_search(&stdin, &mut out, footer)?,
        };
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

/// Serializes several graphs, one per line (graph6 and sparse6 only, for more than one).
#[pyfunction]
fn dumps_all(graphs: Vec<PyRef<'_, PyGraph>>, format: &str) -> PyResult<String> {
    let owned: Vec<Graph> = graphs.iter().map(|g| (*g.inner).clone()).collect();
    formats::to_string_all(&owned, parse(format)?).map_err(err)
}

/// The kick size and tabu tenure a search uses: see `subgraphsearch_core::search_settings`.
#[pyfunction]
fn search_settings(
    kick_size: Option<usize>,
    tabu_tenure: Option<usize>,
    degree_search: bool,
    legacy: bool,
) -> PyResult<(usize, usize)> {
    subgraphsearch_core::search_settings(kick_size, tabu_tenure, degree_search, legacy).map_err(err)
}

/// Names of the supported file formats, and whether each can be read.
#[pyfunction]
fn formats_list() -> Vec<(&'static str, bool)> {
    Format::ALL
        .iter()
        .map(|(f, name)| (*name, f.readable()))
        .collect()
}

/// Runs the `subgraphsearch` command line (`argv`, program name first) on the process's stdin,
/// stdout and stderr, without the GIL; returns the exit status.
#[pyfunction]
fn cli_main(py: Python<'_>, argv: Vec<String>) -> u8 {
    py.detach(move || subgraphsearch::run(argv))
}

#[pymodule]
mod _core {
    #[pymodule_export]
    use super::{
        PyGraph, PyWeightedGraph, auto_tabu_tenure, cli_main, dumps_all, formats_list,
        generate_graph, generate_weighted, grid_graph, hypercube_graph, incidence_graph, largest,
        legacy_asub_search, legacy_ereg_search, legacy_ggen, legacy_reg_search, legacy_sub_search,
        legacy_wggen, legacy_wsub_search, random_regular_graph, search, search_settings,
        sparse_random_graph,
    };

    #[pymodule_init]
    fn init(m: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
        use pyo3::types::PyModuleMethods;
        m.add("__version__", env!("CARGO_PKG_VERSION"))
    }
}
