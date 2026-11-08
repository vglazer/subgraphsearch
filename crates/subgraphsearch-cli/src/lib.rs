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
    /// Generate a random graph.
    Generate(GenerateArgs),
    /// Convert a graph between formats.
    Convert(ConvertArgs),
    /// Print statistics about a graph.
    Info(InfoArgs),
    /// Apply an operation (complement, induced subgraph, switching, ...) to a graph.
    Op(OpArgs),
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
        required_unless_present = "degree",
        conflicts_with = "degree"
    )]
    edges: Option<i64>,
    /// Instead of an edge count, look for subgraphs that are regular of this degree (the original
    /// reg_search).
    #[arg(short = 'd', long)]
    degree: Option<i64>,
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
    /// Limited aspiration (the original asub_search): also take swaps involving the two vertices just
    /// swapped when they would beat the best value so far.
    #[arg(long)]
    aspiration: bool,
    /// Search every graph in a multi-graph file (graph6 or sparse6), one after another.
    #[arg(long, conflicts_with = "index")]
    all: bool,
    /// Use edge weights (the original wsub_search): -m is then a total weight. Weights come from an
    /// edge list's third column, Matrix Market values, or weighted lists (--from wlists).
    #[arg(long, conflicts_with = "all")]
    weighted: bool,
}

#[derive(Args)]
struct GenerateArgs {
    /// Random graph model: exponential, power, geometric, or weighted (the original wggen).
    #[arg(value_parser = parse_gen_model)]
    model: GenModel,
    /// Number of vertices.
    #[arg(short = 'n', long)]
    vertices: usize,
    /// Edge density between 0 and 1, in steps of 0.001 (e.g. 0.6). For weighted graphs: the mean
    /// weight per vertex pair, in steps of 0.01, which may exceed 1.
    #[arg(short, long)]
    density: f64,
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

#[derive(Clone, Copy)]
enum GenModel {
    Unweighted(Model),
    Weighted,
}

fn parse_gen_model(s: &str) -> Result<GenModel, String> {
    if s.eq_ignore_ascii_case("weighted") {
        return Ok(GenModel::Weighted);
    }
    s.parse()
        .map(GenModel::Unweighted)
        .map_err(|e| format!("{e}, or weighted"))
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

/// What was searched for: "with 12 edges", "with total weight 30", "regular of degree 3".
fn target(params: &SearchParams, weighted: bool) -> String {
    match params.regular {
        Some(r) => format!("regular of degree {}", r.degree),
        None if weighted => format!("with total weight {}", params.edges),
        None => format!("with {} edges", params.edges),
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
    let model = match args.model {
        GenModel::Unweighted(m) => m,
        GenModel::Weighted => return generate_weighted(&args),
    };
    let permille = (args.density * 1000.0).round();
    if !(0.0..=1000.0).contains(&permille) || (permille - args.density * 1000.0).abs() > 1e-6 {
        bail!(
            "--density must be between 0 and 1 in steps of 0.001, got {}",
            args.density
        );
    }
    let density = permille as u32;
    let graph = match RngKind::from(args.rng) {
        RngKind::Legacy => {
            generate::generate(model, args.vertices, density, &mut Randu::new(args.seed))?
        }
        RngKind::Xoshiro => generate::generate(
            model,
            args.vertices,
            density,
            &mut Xoshiro::new(args.seed as u64),
        )?,
    };
    write_graph(
        &graph,
        args.output.as_deref(),
        args.format,
        &WriteOptions::default(),
    )
}

fn generate_weighted(args: &GenerateArgs) -> Result<()> {
    let percent = (args.density * 100.0).round();
    if percent < 0.0 || (percent - args.density * 100.0).abs() > 1e-6 {
        bail!(
            "--density for weighted graphs is the mean weight per vertex pair, a non-negative multiple of 0.01; got {}",
            args.density
        );
    }
    let graph = match RngKind::from(args.rng) {
        RngKind::Legacy => {
            generate::weighted(args.vertices, percent as u32, &mut Randu::new(args.seed))?
        }
        RngKind::Xoshiro => generate::weighted(
            args.vertices,
            percent as u32,
            &mut Xoshiro::new(args.seed as u64),
        )?,
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
    let needs_set = !matches!(args.operation, Operation::Complement);
    if needs_set && args.vertices.is_empty() {
        bail!("this operation needs --vertices");
    }
    let out = match args.operation {
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
        Command::Generate(a) => run_generate(a).map(|()| 0),
        Command::Convert(a) => run_convert(a).map(|()| 0),
        Command::Info(a) => run_info(a).map(|()| 0),
        Command::Op(a) => run_op(a).map(|()| 0),
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
