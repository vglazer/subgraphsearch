//! `subsearch`: find induced subgraphs with a prescribed edge count, generate and convert graphs.

use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use subsearch_core::formats::{self, Format, WriteOptions};
use subsearch_core::rng::{Randu, Xoshiro};
use subsearch_core::{
    Graph, Model, RngKind, SearchParams, generate, legacy, ops, search_sequential,
};

#[derive(Parser)]
#[command(name = "subsearch", version, about, long_about = None)]
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
    /// Run the original USRA programs' interface: same stdin, same stdout.
    #[command(subcommand)]
    Legacy(LegacyCommand),
}

#[derive(Subcommand)]
enum LegacyCommand {
    /// `sub_search`: parameters, adjacency lists and fixed vertices on stdin.
    SubSearch,
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
    #[arg(long = "from", value_parser = parse_format)]
    format: Option<Format>,
}

impl GraphInput {
    fn load(&self) -> Result<Graph> {
        read_graph(&self.graph, self.format)
    }
}

#[derive(Args)]
struct SearchArgs {
    #[command(flatten)]
    input: GraphInput,
    /// Number of subgraph vertices (including fixed ones).
    #[arg(short = 'k', long)]
    vertices: usize,
    /// Desired number of subgraph edges.
    #[arg(short = 'm', long)]
    edges: i64,
    /// Vertices the subgraph must contain, e.g. 3,17,42.
    #[arg(long, value_delimiter = ',')]
    fixed: Vec<usize>,
    /// Number of independent experiments (restarts).
    #[arg(short = 'n', long, default_value_t = 10)]
    experiments: usize,
    /// Maximum moves per experiment.
    #[arg(long, default_value_t = 1000)]
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
}

#[derive(Args)]
struct GenerateArgs {
    /// Random graph model.
    #[arg(value_parser = parse_model)]
    model: Model,
    /// Number of vertices.
    #[arg(short = 'n', long)]
    vertices: usize,
    /// Edge density between 0 and 1, in steps of 0.001 (e.g. 0.6).
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
    /// Output format, if it cannot be inferred from the file extension (default: graph6).
    #[arg(long = "to", value_parser = parse_format)]
    format: Option<Format>,
}

#[derive(Args)]
struct ConvertArgs {
    #[command(flatten)]
    input: GraphInput,
    /// Output file ('-' for stdout).
    output: PathBuf,
    /// Output format, if it cannot be inferred from the file extension.
    #[arg(long = "to", value_parser = parse_format)]
    format: Option<Format>,
    /// Vertices to highlight (DOT) or flag (GraphML), e.g. a search result.
    #[arg(long, value_delimiter = ',')]
    highlight: Vec<usize>,
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
}

#[derive(Clone, Copy, ValueEnum)]
enum Operation {
    /// The complement graph.
    Complement,
    /// The subgraph induced on --vertices (renumbered in the order given).
    Induce,
    /// The subgraph induced on all vertices except --vertices.
    Remove,
    /// The subgraph induced on the common neighbourhood of --vertices.
    CommonNeighbours,
    /// The subgraph induced on the common non-neighbourhood of --vertices.
    CommonNonNeighbours,
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
    #[arg(long = "to", value_parser = parse_format)]
    format: Option<Format>,
}

fn parse_format(s: &str) -> Result<Format, String> {
    s.parse()
}

fn parse_model(s: &str) -> Result<Model, String> {
    s.parse()
}

fn read_graph(path: &Path, format: Option<Format>) -> Result<Graph> {
    if path == Path::new("-") {
        let format = format.context("reading from stdin requires --from FORMAT")?;
        let mut text = String::new();
        io::stdin().read_to_string(&mut text)?;
        return Ok(formats::parse(&text, format)?);
    }
    formats::read_file(path, format).with_context(|| format!("reading {}", path.display()))
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

fn run_search(args: SearchArgs) -> Result<ExitCode> {
    let graph = args.input.load()?;
    if let Some(t) = args.threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(t)
            .build_global()
            .ok();
    }
    let params = SearchParams {
        size: args.vertices,
        edges: args.edges,
        fixed: args.fixed,
        experiments: args.experiments,
        max_moves: args.moves,
        div_freq: args.div_freq,
        div_dur: args.div_dur,
        seed: args.seed,
        rng: args.rng.into(),
        record_trace: args.trace,
    };
    let start = std::time::Instant::now();
    let result = match params.rng {
        RngKind::Xoshiro => subsearch_core::search(&graph, &params)?,
        RngKind::Legacy => {
            let mut experiments = Vec::new();
            search_sequential(&graph, &params, |e| experiments.push(e.clone()))?;
            subsearch_core::SearchResult {
                params: params.clone(),
                experiments,
            }
        }
    };
    let elapsed = start.elapsed();

    let mut out = io::stdout().lock();
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
                    "experiment {i}: {tag} after {} moves: {}",
                    e.moves,
                    list.join(" ")
                )?;
            }
        }
    }
    let s = result.successes();
    writeln!(
        out,
        "{s}/{} experiments found a {}-vertex subgraph with {} edges ({:.2}s)",
        result.experiments.len(),
        params.size,
        params.edges,
        elapsed.as_secs_f64()
    )?;
    let fmt = |x: Option<f64>| x.map_or("-".to_string(), |x| format!("{x:.1}"));
    writeln!(
        out,
        "moves per success: {} (successful experiments), {} (all experiments); mean best conflict {:.3}",
        fmt(result.relative_cost()),
        fmt(result.true_cost()),
        result.average_conflict()
    )?;
    if s > 0 {
        let b: Vec<String> = result.boxes().iter().map(usize::to_string).collect();
        writeln!(
            out,
            "success-time distribution over {} boxes: {}",
            b.len(),
            b.join(" ")
        )?;
    }
    if let Some(path) = &args.json {
        let file =
            std::fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
        serde_json::to_writer(io::BufWriter::new(file), &result)?;
    }
    Ok(if s > 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    })
}

fn run_generate(args: GenerateArgs) -> Result<()> {
    let permille = (args.density * 1000.0).round();
    if !(0.0..=1000.0).contains(&permille) || (permille - args.density * 1000.0).abs() > 1e-6 {
        bail!(
            "--density must be between 0 and 1 in steps of 0.001, got {}",
            args.density
        );
    }
    if args.seed <= 0 {
        bail!("--seed must be a positive integer, got {}", args.seed);
    }
    let density = permille as u32;
    let graph = match RngKind::from(args.rng) {
        RngKind::Legacy => generate::generate(
            args.model,
            args.vertices,
            density,
            &mut Randu::new(args.seed),
        )?,
        RngKind::Xoshiro => generate::generate(
            args.model,
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

fn run_convert(args: ConvertArgs) -> Result<()> {
    let graph = args.input.load()?;
    let options = WriteOptions {
        highlight: &args.highlight,
    };
    write_graph(&graph, Some(&args.output), args.format, &options)
}

fn run_info(args: InfoArgs) -> Result<()> {
    let g = args.input.load()?;
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
        println!("{}", serde_json::to_string_pretty(&info)?);
        return Ok(());
    }
    println!("vertices:   {n}");
    println!("edges:      {}", g.num_edges());
    println!("density:    {:.4}", g.density());
    println!("components: {components}");
    println!("degrees:    min {min}, max {max}, mean {mean:.2}");
    if args.degrees {
        println!("degree  count");
        for (d, c) in histogram {
            println!("{d:>6}  {c}");
        }
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
        Operation::CommonNeighbours => {
            ops::induced(&g, &ops::common_neighbourhood(&g, &args.vertices)?)?
        }
        Operation::CommonNonNeighbours => {
            ops::induced(&g, &ops::common_non_neighbourhood(&g, &args.vertices)?)?
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

fn run_legacy(cmd: LegacyCommand) -> Result<ExitCode> {
    let mut input = String::new();
    if io::stdin().is_terminal() {
        eprintln!("reading parameters from stdin (Ctrl-D to finish)...");
    }
    io::stdin().read_to_string(&mut input)?;
    let mut out = io::BufWriter::new(io::stdout().lock());
    let result = match cmd {
        LegacyCommand::SubSearch => legacy::sub_search(&input, &mut out, timing_footer),
        LegacyCommand::Ggen => legacy::ggen(&input, &mut out).map(|()| 0),
    };
    out.flush()?;
    match result {
        Ok(status) => Ok(ExitCode::from(status as u8)),
        Err(e) => {
            eprintln!("{e}");
            Ok(ExitCode::FAILURE)
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Search(a) => run_search(a),
        Command::Generate(a) => run_generate(a).map(|()| ExitCode::SUCCESS),
        Command::Convert(a) => run_convert(a).map(|()| ExitCode::SUCCESS),
        Command::Info(a) => run_info(a).map(|()| ExitCode::SUCCESS),
        Command::Op(a) => run_op(a).map(|()| ExitCode::SUCCESS),
        Command::Legacy(c) => run_legacy(c),
    };
    result.unwrap_or_else(|e| {
        eprintln!("subsearch: {e:#}");
        ExitCode::FAILURE
    })
}
