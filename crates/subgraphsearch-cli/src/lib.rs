//! `subgraphsearch`: find induced subgraphs with a prescribed edge count, generate and convert graphs.

use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use subgraphsearch_core::formats::{self, Format, WriteOptions};
use subgraphsearch_core::rng::{Randu, Xoshiro};
use subgraphsearch_core::search::SearchGraph;
use subgraphsearch_core::{
    Graph, Model, RngKind, SearchParams, SearchResult, WeightedGraph, generate, legacy, ops,
    search_sequential,
};

#[derive(Parser)]
#[command(name = "subgraphsearch", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Search a graph for induced subgraphs with a given number of vertices and edges.
    Search(SearchArgs),
    /// Find the largest induced path, cycle or tree, clique or independent set a search can find.
    Largest(LargestArgs),
    /// Generate a random graph.
    Generate(GenerateArgs),
    /// Convert a graph between formats.
    Convert(ConvertArgs),
    /// Print statistics about a graph.
    Info(InfoArgs),
    /// Apply an operation (complement, induced subgraph, switching, ...) to a graph.
    Op(OpArgs),
    /// Build a graph from an incidence structure (blocks of points), such as a block design.
    Incidence(IncidenceArgs),
    /// Run the original programs' interface: same stdin, same stdout.
    #[command(subcommand)]
    Legacy(LegacyCommand),
}

#[derive(Subcommand)]
enum LegacyCommand {
    /// `sub_search`: parameters, adjacency lists and fixed vertices on stdin.
    SubSearch,
    /// `asub_search` (`sub_search` with limited aspiration): the same input as `sub_search`.
    AsubSearch,
    /// `wsub_search` (`sub_search` on weighted graphs): weights encoded in the adjacency lists.
    WsubSearch,
    /// `reg_search`: regular subgraphs; the third parameter is the degree.
    RegSearch,
    /// `ereg_search`: `reg_search` that first finds the right number of edges.
    EregSearch,
    /// `wggen`: a weighted random graph; arguments VERTICES DENSITY SEED, as on its command line.
    Wggen {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// `ggen`: parameters (and any input lists and fixed vertices) on stdin.
    Ggen,
}

#[derive(Clone, Copy, ValueEnum)]
enum RngArg {
    /// xoshiro256++ (the default).
    Xoshiro,
    /// The original programs' generator; reproduces their results exactly.
    Legacy,
}

impl From<RngArg> for RngKind {
    fn from(r: RngArg) -> Self {
        match r {
            RngArg::Xoshiro => RngKind::Xoshiro,
            RngArg::Legacy => RngKind::Legacy,
        }
    }
}

#[derive(Args)]
struct GraphInput {
    /// Graph file ('-' for stdin). Format: graph6, sparse6, mtx, edgelist or lists.
    graph: PathBuf,
    /// Input format, if it cannot be inferred from the file extension.
    #[arg(id = "from", long = "from", value_parser = parse_format)]
    format: Option<Format>,
    /// For files holding several graphs (graph6 or sparse6, one per line): which to use, from 0.
    #[arg(long)]
    index: Option<usize>,
}

impl GraphInput {
    /// The single graph to work on: the one chosen with --index, or the file's only graph.
    fn load(&self) -> Result<Graph> {
        match self.index {
            None => read_graph(&self.graph, self.format),
            Some(i) => {
                let mut graphs = read_graphs(&self.graph, self.format)?;
                if i >= graphs.len() {
                    bail!(
                        "--index {i} is out of range: {} holds {} graph(s)",
                        self.graph.display(),
                        graphs.len()
                    );
                }
                Ok(graphs.swap_remove(i))
            }
        }
    }

    /// The graph with edge weights (an edge list's third column, Matrix Market values, or
    /// weighted lists); formats without weights give every edge weight 1.
    fn load_weighted(&self) -> Result<WeightedGraph> {
        let format = self.format.or_else(|| Format::from_path(&self.graph));
        if self.index.is_some() || format.is_some_and(|f| !formats::holds_weights(f)) {
            return Ok(WeightedGraph::from_graph(&self.load()?));
        }
        if self.graph == Path::new("-") {
            let (text, format) = read_stdin(self.format)?;
            return Ok(formats::parse_weighted(&text, format)?);
        }
        formats::read_weighted_file(&self.graph, self.format)
            .with_context(|| format!("reading {}", self.graph.display()))
    }

    /// Every graph in the file (just the one chosen, with --index).
    fn load_all(&self) -> Result<Vec<Graph>> {
        match self.index {
            Some(_) => Ok(vec![self.load()?]),
            None => read_graphs(&self.graph, self.format),
        }
    }
}

#[derive(Args)]
struct SearchArgs {
    #[command(flatten)]
    input: GraphInput,
    /// Number of subgraph vertices (including fixed ones).
    #[arg(short = 'k', long)]
    vertices: usize,
    /// Desired number of subgraph edges (or total weight, with --weighted).
    #[arg(
        short = 'm',
        long,
        required_unless_present_any = ["degree", "path"],
        conflicts_with_all = ["degree", "path"]
    )]
    edges: Option<i64>,
    /// Instead of an edge count, look for subgraphs that are regular of this degree (the original
    /// reg_search).
    #[arg(short = 'd', long, conflicts_with = "path")]
    degree: Option<i64>,
    /// Instead of an edge count, look for induced paths on -k vertices.
    #[arg(long)]
    path: bool,
    /// With --path: the two vertices the path must run between, e.g. 0,127.
    #[arg(long, value_delimiter = ',', num_args = 1, requires = "path")]
    endpoints: Vec<usize>,
    /// With --degree: first find the right number of edges, then make the subgraph regular
    /// (the original ereg_search).
    #[arg(long)]
    two_phase: bool,
    /// Vertices the subgraph must contain, e.g. 3,17,42.
    #[arg(long, value_delimiter = ',')]
    fixed: Vec<usize>,
    /// Number of independent experiments (restarts).
    #[arg(short = 'n', long, default_value_t = 40)]
    experiments: usize,
    /// Maximum moves per experiment.
    #[arg(long, default_value_t = 250)]
    moves: usize,
    /// Start a burst of random moves every this many moves.
    #[arg(long, default_value_t = 250)]
    div_freq: usize,
    /// Number of random moves per burst (0 disables diversification).
    #[arg(long, default_value_t = 4)]
    div_dur: usize,
    /// Random seed.
    #[arg(short, long, default_value_t = 1)]
    seed: i64,
    /// Random number generator.
    #[arg(long, value_enum, default_value_t = RngArg::Xoshiro)]
    rng: RngArg,
    /// Also report subgraphs within this many edges of the target (0 = exact matches only).
    #[arg(long, default_value_t = 0)]
    max_conflict: i64,
    /// Write the full results (parameters, every experiment, solutions) as JSON to this file.
    #[arg(long)]
    json: Option<PathBuf>,
    /// Record the objective value after every move in the JSON output.
    #[arg(long)]
    trace: bool,
    /// Number of threads (default: all cores). Ignored with --rng legacy, which is sequential.
    #[arg(long)]
    threads: Option<usize>,
    /// Print nothing but the summary.
    #[arg(short, long)]
    quiet: bool,
    /// Aspiration criterion (the original asub_search): lift the tabu on swaps involving the two
    /// vertices just swapped when they would beat the best value so far.
    #[arg(long)]
    aspiration: bool,
    /// Require the subgraph to be connected: each component beyond the first adds 1 to the
    /// conflict. With --degree 2, finds induced cycles rather than unions of them.
    #[arg(long)]
    connected: bool,
    /// Search every graph in a multi-graph file (graph6 or sparse6), one after another.
    #[arg(long, conflicts_with = "index")]
    all: bool,
    /// Use edge weights (the original wsub_search): -m is then a total weight. Weights come from an
    /// edge list's third column, Matrix Market values, or weighted lists (--from wlists).
    #[arg(long, conflicts_with = "all")]
    weighted: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum ShapeArg {
    Path,
    Cycle,
    Tree,
    Clique,
    /// An independent set.
    Independent,
}

impl From<ShapeArg> for subgraphsearch_core::Shape {
    fn from(s: ShapeArg) -> Self {
        use subgraphsearch_core::Shape;
        match s {
            ShapeArg::Path => Shape::Path,
            ShapeArg::Cycle => Shape::Cycle,
            ShapeArg::Tree => Shape::Tree,
            ShapeArg::Clique => Shape::Clique,
            ShapeArg::Independent => Shape::Independent,
        }
    }
}

#[derive(Args)]
struct LargestArgs {
    #[command(flatten)]
    input: GraphInput,
    /// What to grow.
    #[arg(long, value_enum)]
    shape: ShapeArg,
    /// With --shape path: the two vertices the path must run between, e.g. 0,127.
    #[arg(long, value_delimiter = ',', num_args = 1)]
    endpoints: Vec<usize>,
    /// Vertices the subgraph must contain, e.g. 3,17,42.
    #[arg(long, value_delimiter = ',')]
    fixed: Vec<usize>,
    /// The size to start from (default: the fewest vertices the shape allows).
    #[arg(long)]
    start: Option<usize>,
    /// Count a size as reachable if a search succeeds at it or at one of this many sizes above.
    #[arg(long, default_value_t = 2)]
    lookahead: usize,
    /// Number of independent experiments (restarts) at each size.
    #[arg(short = 'n', long, default_value_t = 40)]
    experiments: usize,
    /// Maximum moves per experiment.
    #[arg(long, default_value_t = 250)]
    moves: usize,
    /// Start a burst of random moves every this many moves.
    #[arg(long, default_value_t = 250)]
    div_freq: usize,
    /// Number of random moves per burst (0 disables diversification).
    #[arg(long, default_value_t = 4)]
    div_dur: usize,
    /// Random seed.
    #[arg(short, long, default_value_t = 1)]
    seed: i64,
    /// Random number generator.
    #[arg(long, value_enum, default_value_t = RngArg::Xoshiro)]
    rng: RngArg,
    /// Write every search made (parameters and experiments, by size) as JSON to this file.
    #[arg(long)]
    json: Option<PathBuf>,
    /// Number of threads (default: all cores).
    #[arg(long)]
    threads: Option<usize>,
    /// Print only the largest subgraph found, not every size tried.
    #[arg(short, long)]
    quiet: bool,
}

#[derive(Args)]
struct GenerateArgs {
    /// Random graph model: exponential, power, geometric, or weighted (the original wggen); or a
    /// standard graph: hypercube (with --dimension) or grid (with --rows and --cols).
    #[arg(value_parser = parse_gen_model)]
    model: GenModel,
    /// Number of vertices (random graphs).
    #[arg(short = 'n', long)]
    vertices: Option<usize>,
    /// Edge density between 0 and 1, in steps of 0.001 (e.g. 0.6). For weighted graphs: the mean
    /// weight per vertex pair, in steps of 0.01, which may exceed 1.
    #[arg(short, long, conflicts_with = "mean_degree")]
    density: Option<f64>,
    /// Instead of --density, for exponential graphs: the mean number of neighbors (any value up to
    /// n - 1). The graph is generated in time proportional to its edges, which suits large sparse
    /// graphs; needs the default --rng.
    #[arg(long)]
    mean_degree: Option<f64>,
    /// Hypercube: the number of dimensions (2^dimension vertices, up to 16 dimensions).
    #[arg(long)]
    dimension: Option<usize>,
    /// Grid: the number of rows.
    #[arg(long)]
    rows: Option<usize>,
    /// Grid: the number of columns.
    #[arg(long)]
    cols: Option<usize>,
    /// Random seed (positive).
    #[arg(short, long, default_value_t = 1)]
    seed: i64,
    /// Random number generator. 'legacy' reproduces ggen's graphs exactly.
    #[arg(long, value_enum, default_value_t = RngArg::Xoshiro)]
    rng: RngArg,
    /// Output file ('-' or omitted for stdout).
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Output format, if it cannot be inferred from the file extension (default: graph6, or an
    /// edge list for weighted graphs).
    #[arg(id = "to", long = "to", value_parser = parse_format)]
    format: Option<Format>,
}

#[derive(Args)]
struct IncidenceArgs {
    /// File of blocks ('-' for stdin): one block per line, its points as numbers separated by
    /// spaces or commas; blank lines and lines starting with '#' are skipped.
    blocks: PathBuf,
    /// The graph's vertices: 'points' (adjacent when exactly K blocks contain both) or 'blocks'
    /// (adjacent when they share exactly K points).
    #[arg(long, value_enum, default_value_t = IncidenceOn::Points)]
    on: IncidenceOn,
    /// The number of shared blocks or points that makes two vertices adjacent.
    #[arg(short, default_value_t = 1)]
    k: usize,
    /// Number of points (default: one more than the largest listed).
    #[arg(long)]
    points: Option<usize>,
    /// Output file ('-' or omitted for stdout).
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Output format, if it cannot be inferred from the file extension (default: graph6).
    #[arg(id = "to", long = "to", value_parser = parse_format)]
    format: Option<Format>,
}

#[derive(Clone, Copy, ValueEnum)]
enum IncidenceOn {
    Points,
    Blocks,
}

#[derive(Clone, Copy)]
enum GenModel {
    Unweighted(Model),
    Weighted,
    Hypercube,
    Grid,
}

fn parse_gen_model(s: &str) -> Result<GenModel, String> {
    match s.to_ascii_lowercase().as_str() {
        "weighted" => return Ok(GenModel::Weighted),
        "hypercube" => return Ok(GenModel::Hypercube),
        "grid" => return Ok(GenModel::Grid),
        _ => {}
    }
    s.parse()
        .map(GenModel::Unweighted)
        .map_err(|e| format!("{e}, weighted, hypercube or grid"))
}

#[derive(Args)]
struct ConvertArgs {
    #[command(flatten)]
    input: GraphInput,
    /// Output file ('-' for stdout).
    output: PathBuf,
    /// Output format, if it cannot be inferred from the file extension.
    #[arg(id = "to", long = "to", value_parser = parse_format)]
    format: Option<Format>,
    /// Vertices to highlight (DOT) or flag (GraphML), e.g. a search result.
    #[arg(long, value_delimiter = ',')]
    highlight: Vec<usize>,
    /// Keep edge weights (the output format must be able to store them).
    #[arg(long)]
    weighted: bool,
}

#[derive(Args)]
struct InfoArgs {
    #[command(flatten)]
    input: GraphInput,
    /// Print the full degree distribution.
    #[arg(long)]
    degrees: bool,
    /// Print as JSON.
    #[arg(long)]
    json: bool,
    /// Also report edge weights.
    #[arg(long)]
    weighted: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum Operation {
    /// The complement graph.
    Complement,
    /// The subgraph induced on --vertices (renumbered in the order given).
    Induce,
    /// The subgraph induced on all vertices except --vertices.
    Remove,
    /// The subgraph induced on the common neighborhood of --vertices.
    CommonNeighbors,
    /// The subgraph induced on the common non-neighborhood of --vertices.
    CommonNonNeighbors,
    /// Seidel switching with respect to --vertices.
    Switch,
    /// The Cartesian product with the graph in --with; vertex (u, v) is u * |with| + v.
    Cartesian,
    /// The tensor (direct) product with the graph in --with.
    Tensor,
    /// The strong product with the graph in --with.
    Strong,
    /// The lexicographic product with the graph in --with.
    Lexicographic,
    /// The replacement product of a d-regular graph with the graph in --with, on d vertices.
    Replacement,
    /// The zig-zag product of a D-regular graph with the regular graph in --with, on D vertices.
    Zigzag,
}

#[derive(Args)]
struct OpArgs {
    /// The operation.
    #[arg(value_enum)]
    operation: Operation,
    #[command(flatten)]
    input: GraphInput,
    /// Output file ('-' for stdout).
    output: PathBuf,
    /// The vertex set the operation uses, e.g. 1,5,9.
    #[arg(long, value_delimiter = ',')]
    vertices: Vec<usize>,
    /// For products: the second graph's file.
    #[arg(long = "with")]
    with: Option<PathBuf>,
    /// Output format, if it cannot be inferred from the file extension.
    #[arg(id = "to", long = "to", value_parser = parse_format)]
    format: Option<Format>,
}

fn parse_format(s: &str) -> Result<Format, String> {
    s.parse()
}

fn read_stdin(format: Option<Format>) -> Result<(String, Format)> {
    let format = format.context("reading from stdin requires --from FORMAT")?;
    let mut text = String::new();
    io::stdin().read_to_string(&mut text)?;
    Ok((text, format))
}

fn run_incidence(args: IncidenceArgs) -> Result<()> {
    let text = if args.blocks == Path::new("-") {
        let mut text = String::new();
        io::stdin().read_to_string(&mut text)?;
        text
    } else {
        std::fs::read_to_string(&args.blocks)
            .with_context(|| format!("reading {}", args.blocks.display()))?
    };
    let blocks =
        ops::parse_blocks(&text).with_context(|| format!("reading {}", args.blocks.display()))?;
    let incidence = ops::Incidence::from_blocks(args.points, &blocks)?;
    let graph = match args.on {
        IncidenceOn::Points => incidence.symbol_graph(args.k),
        IncidenceOn::Blocks => incidence.set_graph(args.k),
    };
    write_graph(
        &graph,
        args.output.as_deref(),
        args.format,
        &WriteOptions::default(),
    )
}

fn read_graph(path: &Path, format: Option<Format>) -> Result<Graph> {
    if path == Path::new("-") {
        let (text, format) = read_stdin(format)?;
        return Ok(formats::parse(&text, format)?);
    }
    formats::read_file(path, format).with_context(|| format!("reading {}", path.display()))
}

fn read_graphs(path: &Path, format: Option<Format>) -> Result<Vec<Graph>> {
    if path == Path::new("-") {
        let (text, format) = read_stdin(format)?;
        return Ok(formats::parse_all(&text, format)?);
    }
    formats::read_file_all(path, format).with_context(|| format!("reading {}", path.display()))
}

fn write_graph(
    g: &Graph,
    path: Option<&Path>,
    format: Option<Format>,
    options: &WriteOptions,
) -> Result<()> {
    match path {
        None => write_stdout(g, format.unwrap_or(Format::Graph6), options),
        Some(p) if p == Path::new("-") => {
            write_stdout(g, format.unwrap_or(Format::Graph6), options)
        }
        Some(p) => formats::write_file(g, p, format, options)
            .with_context(|| format!("writing {}", p.display())),
    }
}

fn write_stdout(g: &Graph, format: Format, options: &WriteOptions) -> Result<()> {
    io::stdout().write_all(formats::to_string(g, format, options).as_bytes())?;
    Ok(())
}

fn run_search(args: SearchArgs) -> Result<u8> {
    if args.two_phase && args.degree.is_none() {
        bail!("--two-phase needs --degree: it searches for regular subgraphs");
    }
    let graphs = if args.weighted {
        Vec::new()
    } else if args.all {
        args.input.load_all()?
    } else {
        vec![args.input.load()?]
    };
    if let Some(t) = args.threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(t)
            .build_global()
            .ok();
    }
    let params = SearchParams {
        size: args.vertices,
        edges: args.edges.unwrap_or(0),
        fixed: args.fixed.clone(),
        experiments: args.experiments,
        max_moves: args.moves,
        div_freq: args.div_freq,
        div_dur: args.div_dur,
        seed: args.seed,
        rng: args.rng.into(),
        record_trace: args.trace,
        aspiration: args.aspiration,
        connected: args.connected,
        path: args.path,
        endpoints: match args.endpoints[..] {
            [] => None,
            [s, t] => Some([s, t]),
            _ => bail!("--endpoints takes two vertices, e.g. --endpoints 0,127"),
        },
        regular: args.degree.map(|degree| subgraphsearch_core::Regular {
            degree,
            two_phase: args.two_phase,
        }),
    };
    let mut out = io::stdout().lock();
    if args.weighted {
        let graph = args.input.load_weighted()?;
        let start = std::time::Instant::now();
        let result = run_one(&graph, &params)?;
        report(&mut out, &result, start.elapsed(), "", &args)?;
        write_json(&args, &[(0, result.clone())])?;
        return Ok(if result.successes() > 0 { 0 } else { 2 });
    }
    let mut results = Vec::with_capacity(graphs.len());
    let mut graphs_with_success = 0;
    for (index, graph) in graphs.iter().enumerate() {
        let prefix = if args.all {
            format!("graph {index}: ")
        } else {
            String::new()
        };
        let start = std::time::Instant::now();
        let result = match run_one(graph, &params) {
            Ok(r) => r,
            // with --all, a graph too small for the subgraph is reported and skipped
            Err(e) if args.all => {
                writeln!(out, "{prefix}skipped: {e}")?;
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        report(&mut out, &result, start.elapsed(), &prefix, &args)?;
        graphs_with_success += usize::from(result.successes() > 0);
        results.push((index, result));
    }
    if args.all {
        writeln!(
            out,
            "{graphs_with_success}/{} graphs contain a {}-vertex subgraph {}",
            graphs.len(),
            params.size,
            target(&params, false)
        )?;
    }
    write_json(&args, &results)?;
    Ok(if graphs_with_success > 0 { 0 } else { 2 })
}

fn run_largest(args: LargestArgs) -> Result<u8> {
    let graph = args.input.load()?;
    if let Some(t) = args.threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(t)
            .build_global()
            .ok();
    }
    let mut base = SearchParams::new(2, 0);
    base.fixed = args.fixed.clone();
    base.endpoints = match args.endpoints[..] {
        [] => None,
        [s, t] => Some([s, t]),
        _ => bail!("--endpoints takes two vertices, e.g. --endpoints 0,127"),
    };
    base.experiments = args.experiments;
    base.max_moves = args.moves;
    base.div_freq = args.div_freq;
    base.div_dur = args.div_dur;
    base.seed = args.seed;
    base.rng = args.rng.into();
    let shape = subgraphsearch_core::Shape::from(args.shape);
    let start = std::time::Instant::now();
    let found = subgraphsearch_core::largest(&graph, shape, &base, args.start, args.lookahead)?;
    let mut out = io::stdout().lock();
    if !args.quiet {
        for r in &found.results {
            writeln!(
                out,
                "{} vertices: {}/{} experiments succeeded, best conflict {}",
                r.params.size,
                r.successes(),
                r.experiments.len(),
                r.experiments
                    .iter()
                    .map(|e| e.best_conflict)
                    .min()
                    .unwrap_or(0)
            )?;
        }
    }
    let noun = match shape {
        subgraphsearch_core::Shape::Clique => "clique".to_string(),
        subgraphsearch_core::Shape::Independent => "independent set".to_string(),
        s => format!("induced {}", s.name()),
    };
    match found.size {
        Some(size) => {
            let best = found
                .results
                .iter()
                .find(|r| r.params.size == size)
                .and_then(|r| r.experiments.iter().find(|e| e.success))
                .expect("a successful search at the largest size");
            let list: Vec<String> = best.vertices.iter().map(usize::to_string).collect();
            writeln!(
                out,
                "largest {noun} found: {size} vertices ({:.2}s): {}",
                start.elapsed().as_secs_f64(),
                list.join(" ")
            )?;
        }
        None => writeln!(
            out,
            "no {noun} found ({:.2}s)",
            start.elapsed().as_secs_f64()
        )?,
    }
    if let Some(path) = &args.json {
        let file =
            std::fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
        serde_json::to_writer(io::BufWriter::new(file), &found)?;
    }
    Ok(if found.size.is_some() { 0 } else { 2 })
}

/// One search: in parallel with xoshiro, sequentially (as the C programs) with the legacy rng.
fn run_one<G: SearchGraph>(
    graph: &G,
    params: &SearchParams,
) -> Result<SearchResult, subgraphsearch_core::Error> {
    match params.rng {
        RngKind::Xoshiro => subgraphsearch_core::search(graph, params),
        RngKind::Legacy => {
            let mut experiments = Vec::new();
            search_sequential(graph, params, |e| experiments.push(e.clone())).map(|()| {
                SearchResult {
                    params: params.clone(),
                    experiments,
                }
            })
        }
    }
}

/// Writes --json output: one result, or with --all a list tagged with graph indices.
fn write_json(args: &SearchArgs, results: &[(usize, SearchResult)]) -> Result<()> {
    let Some(path) = &args.json else {
        return Ok(());
    };
    let file =
        std::fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let writer = io::BufWriter::new(file);
    if args.all {
        let all: Vec<_> = results
            .iter()
            .map(|(i, r)| serde_json::json!({"graph": i, "params": r.params, "experiments": r.experiments}))
            .collect();
        serde_json::to_writer(writer, &all)?;
    } else if let Some((_, r)) = results.first() {
        serde_json::to_writer(writer, r)?;
    }
    Ok(())
}

/// What was searched for: "with 12 edges", "with total weight 30", "regular of degree 3", each
/// followed by ", connected" for a connected search; or, for the three shapes with a name, "forming
/// a path" (or "forming a path from 0 to 127", with endpoints), "forming a cycle" (connected and
/// regular of degree 2) or "forming a tree" (connected, with one edge fewer than vertices).
fn target(params: &SearchParams, weighted: bool) -> String {
    if params.path {
        return match params.endpoints {
            Some([s, t]) => format!("forming a path from {s} to {t}"),
            None => "forming a path".into(),
        };
    }
    let target = match params.regular {
        Some(r) if params.connected && r.degree == 2 => return "forming a cycle".into(),
        None if params.connected && !weighted && params.edges == params.size as i64 - 1 => {
            return "forming a tree".into();
        }
        Some(r) => format!("regular of degree {}", r.degree),
        None if weighted => format!("with total weight {}", params.edges),
        None => format!("with {} edges", params.edges),
    };
    if params.connected {
        target + ", connected"
    } else {
        target
    }
}

fn report(
    out: &mut impl Write,
    result: &subgraphsearch_core::SearchResult,
    elapsed: std::time::Duration,
    prefix: &str,
    args: &SearchArgs,
) -> Result<()> {
    let params = &result.params;
    if !args.quiet {
        for (i, e) in result.experiments.iter().enumerate() {
            if e.best_conflict <= args.max_conflict {
                let tag = if e.success {
                    "found".to_string()
                } else {
                    format!("off by {}", e.best_conflict)
                };
                let list: Vec<String> = e.vertices.iter().map(usize::to_string).collect();
                writeln!(
                    out,
                    "{prefix}experiment {i}: {tag} after {} moves: {}",
                    e.moves,
                    list.join(" ")
                )?;
            }
        }
    }
    let s = result.successes();
    writeln!(
        out,
        "{prefix}{s}/{} experiments found a {}-vertex subgraph {} ({:.2}s)",
        result.experiments.len(),
        params.size,
        target(params, args.weighted),
        elapsed.as_secs_f64()
    )?;
    if args.all && args.quiet {
        return Ok(());
    }
    let fmt = |x: Option<f64>| x.map_or("-".to_string(), |x| format!("{x:.1}"));
    writeln!(
        out,
        "{prefix}moves per success: {} (successful experiments), {} (all experiments); mean best conflict {:.3}",
        fmt(result.relative_cost()),
        fmt(result.true_cost()),
        result.average_conflict()
    )?;
    if s > 0 {
        let b: Vec<String> = result.boxes().iter().map(usize::to_string).collect();
        writeln!(
            out,
            "{prefix}success-time distribution over {} boxes: {}",
            b.len(),
            b.join(" ")
        )?;
    }
    Ok(())
}

fn run_generate(args: GenerateArgs) -> Result<()> {
    if args.seed <= 0 {
        bail!("--seed must be a positive integer, got {}", args.seed);
    }
    let random = matches!(args.model, GenModel::Unweighted(_) | GenModel::Weighted);
    if random && args.vertices.is_none() {
        bail!("random graphs need -n, the number of vertices");
    }
    if random && (args.dimension.is_some() || args.rows.is_some() || args.cols.is_some()) {
        bail!("--dimension, --rows and --cols are for hypercube and grid");
    }
    if !random && (args.vertices.is_some() || args.density.is_some() || args.mean_degree.is_some())
    {
        bail!("hypercube and grid take --dimension, or --rows and --cols, not -n or a density");
    }
    let graph = match args.model {
        GenModel::Weighted => return generate_weighted(&args),
        GenModel::Hypercube => {
            let Some(d) = args.dimension else {
                bail!("hypercube needs --dimension");
            };
            generate::hypercube(d)?
        }
        GenModel::Grid => {
            let (Some(rows), Some(cols)) = (args.rows, args.cols) else {
                bail!("grid needs --rows and --cols");
            };
            generate::grid(rows, cols)
        }
        GenModel::Unweighted(model) => {
            let n = args.vertices.unwrap_or_default();
            match (args.density, args.mean_degree) {
                (None, Some(mean_degree)) => {
                    if model != Model::Exponential || matches!(args.rng, RngArg::Legacy) {
                        bail!("--mean-degree needs the exponential model and the default --rng");
                    }
                    generate::sparse_random(n, mean_degree, &mut Xoshiro::new(args.seed as u64))?
                }
                (Some(d), None) => {
                    let permille = (d * 1000.0).round();
                    if !(0.0..=1000.0).contains(&permille) || (permille - d * 1000.0).abs() > 1e-6 {
                        bail!("--density must be between 0 and 1 in steps of 0.001, got {d}");
                    }
                    let density = permille as u32;
                    match RngKind::from(args.rng) {
                        RngKind::Legacy => {
                            generate::generate(model, n, density, &mut Randu::new(args.seed))?
                        }
                        RngKind::Xoshiro => generate::generate(
                            model,
                            n,
                            density,
                            &mut Xoshiro::new(args.seed as u64),
                        )?,
                    }
                }
                _ => bail!("give --density or --mean-degree"),
            }
        }
    };
    write_graph(
        &graph,
        args.output.as_deref(),
        args.format,
        &WriteOptions::default(),
    )
}

fn generate_weighted(args: &GenerateArgs) -> Result<()> {
    let n = args.vertices.unwrap_or_default();
    let Some(density) = args.density else {
        bail!("weighted graphs need --density, the mean weight per vertex pair");
    };
    let percent = (density * 100.0).round();
    if percent < 0.0 || (percent - density * 100.0).abs() > 1e-6 {
        bail!(
            "--density for weighted graphs is the mean weight per vertex pair, a non-negative multiple of 0.01; got {density}"
        );
    }
    let graph = match RngKind::from(args.rng) {
        RngKind::Legacy => generate::weighted(n, percent as u32, &mut Randu::new(args.seed))?,
        RngKind::Xoshiro => {
            generate::weighted(n, percent as u32, &mut Xoshiro::new(args.seed as u64))?
        }
    };
    write_weighted(
        &graph,
        args.output.as_deref(),
        args.format,
        &WriteOptions::default(),
    )
}

fn write_weighted(
    g: &WeightedGraph,
    path: Option<&Path>,
    format: Option<Format>,
    options: &WriteOptions,
) -> Result<()> {
    match path {
        Some(p) if p != Path::new("-") => formats::write_weighted_file(g, p, format, options)
            .with_context(|| format!("writing {}", p.display())),
        _ => {
            let text = formats::to_string_weighted(g, format.unwrap_or(Format::EdgeList), options)?;
            io::stdout().write_all(text.as_bytes())?;
            Ok(())
        }
    }
}

fn run_convert(args: ConvertArgs) -> Result<()> {
    if args.weighted {
        let g = args.input.load_weighted()?;
        return write_weighted(
            &g,
            Some(&args.output),
            args.format,
            &WriteOptions {
                highlight: &args.highlight,
            },
        );
    }
    let mut graphs = args.input.load_all()?;
    if graphs.len() == 1 {
        let options = WriteOptions {
            highlight: &args.highlight,
        };
        return write_graph(
            &graphs.pop().unwrap(),
            Some(&args.output),
            args.format,
            &options,
        );
    }
    // a multi-graph file: convert every graph, which only graph6 and sparse6 can hold
    if !args.highlight.is_empty() {
        bail!("--highlight needs a single graph; choose one with --index");
    }
    let to_stdout = args.output == Path::new("-");
    let format = args
        .format
        .or_else(|| Format::from_path(&args.output))
        .or(to_stdout.then_some(Format::Graph6));
    let format =
        format.context("cannot tell the output format from the file extension; use --to")?;
    let text = formats::to_string_all(&graphs, format)
        .map_err(|e| anyhow::anyhow!("{e} (or choose one graph with --index)"))?;
    if to_stdout {
        io::stdout().write_all(text.as_bytes())?;
    } else {
        std::fs::write(&args.output, text)
            .with_context(|| format!("writing {}", args.output.display()))?;
    }
    Ok(())
}

fn run_info(args: InfoArgs) -> Result<()> {
    let mut out = io::stdout().lock();
    if args.weighted {
        return info_weighted(&args);
    }
    let graphs = args.input.load_all()?;
    if graphs.len() != 1 {
        return info_table(&graphs, args.json);
    }
    let g = &graphs[0];
    let degrees = g.degrees();
    let (_, components) = g.components();
    let n = g.num_vertices();
    let mut histogram = std::collections::BTreeMap::new();
    for &d in &degrees {
        *histogram.entry(d).or_insert(0usize) += 1;
    }
    let (min, max) = (
        degrees.iter().min().copied().unwrap_or(0),
        degrees.iter().max().copied().unwrap_or(0),
    );
    let mean = if n > 0 {
        2.0 * g.num_edges() as f64 / n as f64
    } else {
        0.0
    };
    if args.json {
        let info = serde_json::json!({
            "vertices": n,
            "edges": g.num_edges(),
            "density": g.density(),
            "components": components,
            "degree": {"min": min, "max": max, "mean": mean},
            "degree_counts": histogram.iter().map(|(d, c)| [*d, *c]).collect::<Vec<_>>(),
        });
        writeln!(out, "{}", serde_json::to_string_pretty(&info)?)?;
        return Ok(());
    }
    writeln!(out, "vertices:   {n}")?;
    writeln!(out, "edges:      {}", g.num_edges())?;
    writeln!(out, "density:    {:.4}", g.density())?;
    writeln!(out, "components: {components}")?;
    writeln!(out, "degrees:    min {min}, max {max}, mean {mean:.2}")?;
    if args.degrees {
        writeln!(out, "degree  count")?;
        for (d, c) in histogram {
            writeln!(out, "{d:>6}  {c}")?;
        }
    }
    Ok(())
}

fn info_weighted(args: &InfoArgs) -> Result<()> {
    let mut out = io::stdout().lock();
    let g = args.input.load_weighted()?;
    let mut counts = std::collections::BTreeMap::new();
    for (_, _, w) in g.edges() {
        *counts.entry(w).or_insert(0usize) += 1;
    }
    let structure = g.structure();
    let m = g.num_edges();
    let mean = if m > 0 {
        g.total_weight() as f64 / m as f64
    } else {
        0.0
    };
    if args.json {
        let info = serde_json::json!({
            "vertices": g.num_vertices(),
            "edges": m,
            "total_weight": g.total_weight(),
            "density": structure.density(),
            "components": structure.components().1,
            "weight": {"min": counts.keys().next(), "max": counts.keys().last(), "mean": mean},
            "weight_counts": counts.iter().map(|(w, c)| [usize::from(*w), *c]).collect::<Vec<_>>(),
        });
        writeln!(out, "{}", serde_json::to_string_pretty(&info)?)?;
        return Ok(());
    }
    writeln!(out, "vertices:     {}", g.num_vertices())?;
    writeln!(out, "edges:        {m}")?;
    writeln!(out, "total weight: {}", g.total_weight())?;
    writeln!(out, "density:      {:.4}", structure.density())?;
    writeln!(out, "components:   {}", structure.components().1)?;
    writeln!(
        out,
        "weights:      min {}, max {}, mean {mean:.2}",
        counts.keys().next().copied().unwrap_or(0),
        counts.keys().last().copied().unwrap_or(0)
    )?;
    if args.degrees {
        writeln!(out, "weight  edges")?;
        for (w, c) in counts {
            writeln!(out, "{w:>6}  {c}")?;
        }
    }
    Ok(())
}

/// One line per graph, for multi-graph files.
fn info_table(graphs: &[Graph], json: bool) -> Result<()> {
    let mut out = io::stdout().lock();
    let rows: Vec<_> = graphs
        .iter()
        .map(|g| {
            let d = g.degrees();
            (
                g.num_vertices(),
                g.num_edges(),
                g.density(),
                g.components().1,
                d.iter().min().copied().unwrap_or(0),
                d.iter().max().copied().unwrap_or(0),
            )
        })
        .collect();
    if json {
        let list: Vec<_> = rows
            .iter()
            .enumerate()
            .map(|(i, &(n, m, density, comps, dmin, dmax))| {
                serde_json::json!({"graph": i, "vertices": n, "edges": m, "density": density, "components": comps, "degree": {"min": dmin, "max": dmax}})
            })
            .collect();
        writeln!(out, "{}", serde_json::to_string_pretty(&list)?)?;
        return Ok(());
    }
    writeln!(out, "{} graphs", graphs.len())?;
    writeln!(
        out,
        "{:>6} {:>9} {:>10} {:>8} {:>10} {:>11}",
        "graph", "vertices", "edges", "density", "components", "degrees"
    )?;
    for (i, (n, m, density, comps, dmin, dmax)) in rows.into_iter().enumerate() {
        let degrees = if dmin == dmax {
            format!("{dmin} (regular)")
        } else {
            format!("{dmin}-{dmax}")
        };
        writeln!(
            out,
            "{i:>6} {n:>9} {m:>10} {density:>8.4} {comps:>10} {degrees:>11}"
        )?;
    }
    Ok(())
}

fn run_op(args: OpArgs) -> Result<()> {
    let g = args.input.load()?;
    let is_product = matches!(
        args.operation,
        Operation::Cartesian
            | Operation::Tensor
            | Operation::Strong
            | Operation::Lexicographic
            | Operation::Replacement
            | Operation::Zigzag
    );
    let needs_set = !is_product && !matches!(args.operation, Operation::Complement);
    if needs_set && args.vertices.is_empty() {
        bail!("this operation needs --vertices");
    }
    let other = match (&args.with, is_product) {
        (Some(path), true) => Some(read_graph(path, None)?),
        (None, true) => bail!("a product needs the second graph: --with FILE"),
        (Some(_), false) => bail!("--with is for products"),
        (None, false) => None,
    };
    let h = || other.as_ref().expect("checked above");
    let out = match args.operation {
        Operation::Cartesian => ops::product(&g, h(), ops::Product::Cartesian),
        Operation::Tensor => ops::product(&g, h(), ops::Product::Tensor),
        Operation::Strong => ops::product(&g, h(), ops::Product::Strong),
        Operation::Lexicographic => ops::product(&g, h(), ops::Product::Lexicographic),
        Operation::Replacement => ops::replacement(&g, h())?,
        Operation::Zigzag => ops::zigzag(&g, h())?,
        Operation::Complement => ops::complement(&g),
        Operation::Induce => ops::induced(&g, &args.vertices)?,
        Operation::Remove => ops::induced(&g, &ops::rest(&g, &args.vertices)?)?,
        Operation::CommonNeighbors => {
            ops::induced(&g, &ops::common_neighborhood(&g, &args.vertices)?)?
        }
        Operation::CommonNonNeighbors => {
            ops::induced(&g, &ops::common_non_neighborhood(&g, &args.vertices)?)?
        }
        Operation::Switch => ops::switch(&g, &args.vertices)?,
    };
    write_graph(
        &out,
        Some(&args.output),
        args.format,
        &WriteOptions::default(),
    )
}

/// `"T> %.2f seconds H> %s\n"`: user CPU time and host name, as `sub_search.c` prints them.
#[cfg(unix)]
fn timing_footer() -> String {
    // SAFETY: getrusage and gethostname only write into the buffers passed to them.
    unsafe {
        let mut usage: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut usage);
        let seconds = usage.ru_utime.tv_sec as f64 + usage.ru_utime.tv_usec as f64 / 1e6;
        // sub_search.c used a 16-byte buffer, so long host names were truncated
        let mut name = [0u8; 16];
        libc::gethostname(name.as_mut_ptr().cast(), name.len());
        let len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
        format!(
            "T> {seconds:.2} seconds H> {}\n",
            String::from_utf8_lossy(&name[..len])
        )
    }
}

/// Elsewhere (the C programs were Unix-only): elapsed time since start-up, and the host name
/// from the environment, truncated as on Unix.
#[cfg(not(unix))]
fn timing_footer() -> String {
    let seconds = START.get().map_or(0.0, |t| t.elapsed().as_secs_f64());
    let host: String = std::env::var("COMPUTERNAME")
        .unwrap_or_default()
        .chars()
        .take(15)
        .collect();
    format!("T> {seconds:.2} seconds H> {host}\n")
}

#[cfg(not(unix))]
static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

fn run_legacy(cmd: LegacyCommand) -> Result<u8> {
    if let LegacyCommand::Wggen { args } = &cmd {
        let mut out = io::BufWriter::new(io::stdout().lock());
        let result = legacy::wggen(args, &mut out);
        out.flush()?;
        return Ok(match result {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("{e}");
                1
            }
        });
    }
    let mut input = String::new();
    if io::stdin().is_terminal() {
        eprintln!("reading parameters from stdin (Ctrl-D to finish)...");
    }
    io::stdin().read_to_string(&mut input)?;
    let mut out = io::BufWriter::new(io::stdout().lock());
    let result = match cmd {
        LegacyCommand::SubSearch => legacy::sub_search(&input, &mut out, timing_footer),
        LegacyCommand::AsubSearch => legacy::asub_search(&input, &mut out, timing_footer),
        LegacyCommand::WsubSearch => legacy::wsub_search(&input, &mut out, timing_footer),
        LegacyCommand::RegSearch => legacy::reg_search(&input, &mut out, timing_footer),
        LegacyCommand::EregSearch => legacy::ereg_search(&input, &mut out, timing_footer),
        LegacyCommand::Wggen { .. } => unreachable!(),
        LegacyCommand::Ggen => legacy::ggen(&input, &mut out).map(|()| 0),
    };
    out.flush()?;
    match result {
        Ok(status) => Ok(status as u8),
        Err(e) => {
            eprintln!("{e}");
            Ok(1)
        }
    }
}

fn is_broken_pipe(e: &anyhow::Error) -> bool {
    e.chain().any(|cause| {
        cause
            .downcast_ref::<io::Error>()
            .is_some_and(|e| e.kind() == io::ErrorKind::BrokenPipe)
    })
}

/// Runs the command line given by `args` (the program name first, as in `std::env::args_os`) and
/// returns the process exit status: 0 on success, 2 when a search finds nothing or the arguments
/// are invalid, 1 on other errors.
/// Output goes to the process's stdout and stderr. It never exits the process itself, so it can
/// also run inside another program (the Python package's `subgraphsearch` command).
pub fn run<I, T>(args: I) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    #[cfg(not(unix))]
    START.get_or_init(std::time::Instant::now);
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(e) => {
            // as clap's own exit: --help and --version print to stdout and succeed, and usage
            // errors print to stderr with status 2
            let _ = e.print();
            return e.exit_code() as u8;
        }
    };
    let result = match cli.command {
        Command::Search(a) => run_search(a),
        Command::Largest(a) => run_largest(a),
        Command::Generate(a) => run_generate(a).map(|()| 0),
        Command::Convert(a) => run_convert(a).map(|()| 0),
        Command::Info(a) => run_info(a).map(|()| 0),
        Command::Op(a) => run_op(a).map(|()| 0),
        Command::Incidence(a) => run_incidence(a).map(|()| 0),
        Command::Legacy(c) => run_legacy(c),
    };
    match result.and_then(|status| Ok(io::stdout().flush().map(|()| status)?)) {
        Ok(status) => status,
        // the reader went away (`subgraphsearch ... | head`): stop quietly, as other tools do
        Err(e) if is_broken_pipe(&e) => 0,
        Err(e) => {
            eprintln!("subgraphsearch: {e:#}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_definition_is_consistent() {
        // clap checks argument names, ids and conflicts here, rather than only in debug builds
        // at parse time
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
