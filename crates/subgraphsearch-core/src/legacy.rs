//! Byte-for-byte emulation of the original `sub_search` and `ggen` programs: the same
//! stdin parameters, the same algorithms and random numbers, and the same output.
//!
//! The only intended differences are input checks the C programs lacked (`sub_search` read
//! vertex numbers unchecked, for example); those report an error instead of misbehaving.

use std::io::{self, Write};

use crate::Error;
use crate::formats::{self, Format};
use crate::generate;
use crate::graph::{Graph, GraphBuilder, WeightedGraph};
use crate::ops;
use crate::regular;
use crate::rng::{Randu, RngKind};
use crate::search::{self, Experiment, NUM_BOXES, Regular, SearchParams};

/// Reads integers the way C's `scanf("%d")` does: skip whitespace, an optional sign, then digits.
/// In particular `"5-1"` reads as `5` followed by `-1`, which `ggen`'s output relies on.
pub struct Scanner<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Scanner<'a> {
    pub fn new(input: &'a str) -> Self {
        Scanner {
            bytes: input.as_bytes(),
            pos: 0,
        }
    }

    /// The next integer, or `None` at end of input or on a non-integer (as `scanf` would fail).
    pub fn next_int(&mut self) -> Option<i64> {
        let b = self.bytes;
        while self.pos < b.len() && b[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
        let start = self.pos;
        let mut p = self.pos;
        if p < b.len() && (b[p] == b'-' || b[p] == b'+') {
            p += 1;
        }
        let digits = p;
        while p < b.len() && b[p].is_ascii_digit() {
            p += 1;
        }
        if p == digits {
            return None;
        }
        self.pos = p;
        std::str::from_utf8(&b[start..p]).ok()?.parse().ok()
    }
}

/// Formats `x` like C's `printf("%g", x)` (6 significant digits).
pub fn format_g(x: f64) -> String {
    if x.is_nan() {
        return if x.is_sign_negative() {
            "-nan".into()
        } else {
            "nan".into()
        };
    }
    if x.is_infinite() {
        return if x < 0.0 { "-inf".into() } else { "inf".into() };
    }
    if x == 0.0 {
        return if x.is_sign_negative() {
            "-0".into()
        } else {
            "0".into()
        };
    }
    const P: i32 = 6;
    // the exponent after rounding to P significant digits
    let sci = format!("{:.*e}", (P - 1) as usize, x);
    let (mantissa, exp) = sci.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let strip = |s: &str| -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s.to_string()
        }
    };
    if !(-4..P).contains(&exp) {
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{}e{}{:02}", strip(mantissa), sign, exp.abs())
    } else {
        strip(&format!("{:.*}", (P - 1 - exp) as usize, x))
    }
}

fn read_int(sc: &mut Scanner) -> Result<i64, Error> {
    sc.next_int()
        .ok_or_else(|| Error::Legacy("unexpected end of input or non-integer input".into()))
}

fn read_vertex(sc: &mut Scanner, n: usize, allow_end: bool) -> Result<Option<usize>, Error> {
    let x = read_int(sc)?;
    if allow_end && x == -1 {
        return Ok(None);
    }
    if x < 0 || x >= n as i64 {
        return Err(Error::Legacy(format!(
            "vertex {x} is out of range [0, {n})"
        )));
    }
    Ok(Some(x as usize))
}

/// Reads `n` '-1'-terminated adjacency lists (either triangle, or both). Self-loops are either
/// ignored (`ggen` never outputs them, so they cannot affect its output) or rejected (they would
/// change `sub_search`'s degree bookkeeping, which this port does not emulate).
fn read_adjacency(sc: &mut Scanner, n: usize, ignore_self_loops: bool) -> Result<Graph, Error> {
    let mut b = GraphBuilder::new(n);
    for i in 0..n {
        while let Some(v) = read_vertex(sc, n, true)? {
            if v == i {
                if ignore_self_loops {
                    continue;
                }
                return Err(Error::Legacy(format!(
                    "self-loop at vertex {v} is not supported"
                )));
            }
            b.set(i, v, true);
        }
    }
    Ok(b.build())
}

/// The exit status a legacy program finished with, after writing its output.
pub type Status = i32;

/// Runs `sub_search` on `input` (its complete stdin), writing its stdout to `out`.
///
/// `footer` supplies the final `"T> ... seconds H> host\n"` line (CPU time and host name), which
/// the caller measures. Problems the C program reported on stdout (an invalid subgraph size or
/// diversification frequency) are written to `out` with status 1; malformed input is an `Err`.
pub fn sub_search<W: Write>(
    input: &str,
    out: &mut W,
    footer: impl FnOnce() -> String,
) -> Result<Status, Error> {
    run_search(input, out, footer, Variant::Plain)
}

/// Runs `asub_search` (`sub_search` with limited aspiration); see [`sub_search`]. The two
/// programs read and print exactly the same things.
pub fn asub_search<W: Write>(
    input: &str,
    out: &mut W,
    footer: impl FnOnce() -> String,
) -> Result<Status, Error> {
    run_search(input, out, footer, Variant::Aspiration)
}

/// Runs `wsub_search` (`sub_search` on weighted graphs); see [`sub_search`]. Edge weights are
/// encoded in the adjacency lists: entry `x` in row `i` is the edge `{i, x % v}` with weight
/// `x / v + 1`, and the target `e_sub` is a total weight.
pub fn wsub_search<W: Write>(
    input: &str,
    out: &mut W,
    footer: impl FnOnce() -> String,
) -> Result<Status, Error> {
    run_search(input, out, footer, Variant::Weighted)
}

/// Runs `reg_search`: the same input as [`sub_search`], except that the third parameter is the
/// degree the subgraph should be regular of, not an edge count.
pub fn reg_search<W: Write>(
    input: &str,
    out: &mut W,
    footer: impl FnOnce() -> String,
) -> Result<Status, Error> {
    run_search(input, out, footer, Variant::Regular)
}

/// Runs `ereg_search`: `reg_search` that first finds the right number of edges; see
/// [`crate::regular`].
pub fn ereg_search<W: Write>(
    input: &str,
    out: &mut W,
    footer: impl FnOnce() -> String,
) -> Result<Status, Error> {
    run_search(input, out, footer, Variant::TwoPhase)
}

/// Reads `n` weighted '-1'-terminated adjacency lists in `wsub_search`'s encoding. As in the C
/// program, a pair listed twice keeps the last weight given.
fn read_weighted_adjacency(sc: &mut Scanner, n: usize) -> Result<WeightedGraph, Error> {
    let mut weights = vec![0u8; n * n];
    for i in 0..n {
        loop {
            let x = read_int(sc)?;
            if x == -1 {
                break;
            }
            if x < 0 {
                return Err(Error::Legacy(format!("invalid adjacency entry {x}")));
            }
            let (u, w) = ((x % n as i64) as usize, x / n as i64 + 1);
            if u == i {
                return Err(Error::Legacy(format!(
                    "self-loop at vertex {u} is not supported"
                )));
            }
            // wsub_search kept weights in a signed char
            if w > i64::from(generate::MAX_GENERATED_WEIGHT) {
                return Err(Error::Legacy(format!(
                    "entry {x} encodes weight {w}, more than the original program could store (127)"
                )));
            }
            weights[i * n + u] = w as u8;
            weights[u * n + i] = w as u8;
        }
    }
    Ok(WeightedGraph::from_matrix(n, weights))
}

/// Runs `wggen` with the given command-line arguments (excluding the program name), writing its
/// stdout to `out`. Errors carry the message `wggen` prints on stderr before exiting with status 1.
pub fn wggen<W: Write>(args: &[String], out: &mut W) -> Result<(), Error> {
    if args.len() != 3 {
        return Err(Error::Legacy(
            "usage: wggen [ number of vertices ] [ density ] [ seed ] ".into(),
        ));
    }
    let (v, d, seed) = (atoi(&args[0]) as i32, atoi(&args[1]) as i32, atoi(&args[2]));
    if v < 1 || d < 0 || seed <= 0 {
        return Err(Error::Legacy(format!(
            "wggen: need v >= 1, density >= 0 and seed >= 1, got {v} {d} {seed}"
        )));
    }
    let g =
        generate::weighted(v as usize, d as u32, &mut Randu::new(seed)).map_err(|e| match e {
            Error::Invalid(msg) => Error::Legacy(format!("wggen: {msg}")),
            other => other,
        })?;
    write!(
        out,
        "{}",
        formats::to_string_weighted(&g, Format::WeightedLists, &formats::WriteOptions::default())?
    )?;
    write!(out, "\nV = {v}, E = {}\n", g.total_weight())?;
    Ok(())
}

/// C's `atoi`/`atol`: optional whitespace and sign, then digits; anything else gives 0.
fn atoi(s: &str) -> i64 {
    let mut sc = Scanner::new(s);
    sc.next_int().unwrap_or(0)
}

fn run_search<W: Write>(
    input: &str,
    out: &mut W,
    footer: impl FnOnce() -> String,
    variant: Variant,
) -> Result<Status, Error> {
    let mut sc = Scanner::new(input);
    let mut p = [0i64; 10];
    for x in p.iter_mut() {
        *x = sc.next_int().ok_or_else(|| {
            Error::Legacy(
                "expected 10 integer parameters: v v_sub e_sub v_fix num_exps num_moves div_freq div_dur seed show_sols".into(),
            )
        })?;
    }
    let [
        v,
        v_sub,
        e_sub,
        v_fix,
        num_exps,
        num_mov,
        div_freq,
        div_dur,
        seed,
        show_sols,
    ] = p;
    writeln!(
        out,
        "{v} {v_sub} {e_sub} {v_fix}  {num_exps} {num_mov} {div_freq} {div_dur} {seed}  {show_sols}"
    )?;

    if v_sub < 2 || v_sub <= v_fix || v_sub > v - 1 {
        writeln!(out, "{v_sub} is not a valid subgraph size")?;
        return Ok(1);
    }
    if div_freq < 1 {
        writeln!(out, "{div_freq} is not a valid diversification frequency")?;
        return Ok(1);
    }
    if v_fix < 0 {
        return Err(Error::Legacy(format!(
            "number of fixed vertices must be non-negative, got {v_fix}"
        )));
    }
    let n = v as usize;
    let weighted = variant == Variant::Weighted;
    let (plain, weights) = if weighted {
        (None, Some(read_weighted_adjacency(&mut sc, n)?))
    } else {
        (Some(read_adjacency(&mut sc, n, false)?), None)
    };
    let mut fixed = Vec::with_capacity(v_fix as usize);
    for _ in 0..v_fix {
        fixed.push(read_vertex(&mut sc, n, false)?.unwrap());
    }

    let params = SearchParams {
        size: v_sub as usize,
        edges: e_sub,
        fixed,
        experiments: num_exps.max(0) as usize,
        max_moves: num_mov.max(0) as usize,
        div_freq: div_freq as usize,
        div_dur: div_dur.max(0) as usize,
        seed,
        rng: RngKind::Legacy,
        record_trace: false,
        aspiration: variant == Variant::Aspiration,
        connected: false,
        regular: match variant {
            Variant::Regular => Some(Regular {
                degree: e_sub,
                two_phase: false,
            }),
            Variant::TwoPhase => Some(Regular {
                degree: e_sub,
                two_phase: true,
            }),
            _ => None,
        },
    };
    let run = Run {
        params,
        show_sols,
        v_sub,
        num_mov,
        num_exps,
    };
    let sorted = |g: &Graph, k: usize| -> Vec<(usize, i64)> {
        g.neighbors(k)
            .iter()
            .map(|&u| (u as usize, i64::from(u)))
            .collect()
    };
    match (plain, weights) {
        // reg_search and ereg_search print neighbors in the order of their partitioned lists
        (Some(g), _) if run.params.regular.is_some() => {
            report_experiments(&run, out, footer, |report| {
                regular::search_sequential_as_c(&g, &run.params, |e, lists| {
                    report(e, &|k| {
                        lists[k]
                            .iter()
                            .map(|&u| (u as usize, i64::from(u)))
                            .collect()
                    })
                })
            })
        }
        (Some(g), _) => report_experiments(&run, out, footer, |report| {
            search::search_sequential(&g, &run.params, |e| report(e, &|k| sorted(&g, k)))
        }),
        // wsub_search prints edge {k, u} of weight w as u + (w - 1) * v
        (_, Some(g)) => report_experiments(&run, out, footer, |report| {
            search::search_sequential(&g, &run.params, |e| {
                report(e, &|k| {
                    g.neighbors(k)
                        .iter()
                        .map(|&u| {
                            (
                                u as usize,
                                i64::from(u) + (i64::from(g.weight(k, u as usize)) - 1) * v,
                            )
                        })
                        .collect()
                })
            })
        }),
        _ => unreachable!(),
    }
}

/// For a vertex, its neighbors in the order the C program printed them, each with the number
/// printed for it.
type Neighbors<'a> = &'a dyn Fn(usize) -> Vec<(usize, i64)>;

/// Which of the search programs to emulate.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    Plain,
    Aspiration,
    Weighted,
    Regular,
    TwoPhase,
}

/// What [`report_experiments`] needs besides the graph.
struct Run {
    params: SearchParams,
    show_sols: i64,
    v_sub: i64,
    num_mov: i64,
    num_exps: i64,
}

/// Runs the experiments and prints them as the C programs do. `entry(k, u)` is how the neighbor
/// `u` of `k` appears in printed adjacency lists. As in the C programs, the printed `E =` counts
/// edges, even for weighted graphs.
fn report_experiments<W: Write>(
    run: &Run,
    out: &mut W,
    footer: impl FnOnce() -> String,
    drive: impl FnOnce(&mut dyn FnMut(&Experiment, Neighbors)) -> Result<(), Error>,
) -> Result<Status, Error> {
    let Run {
        params,
        show_sols,
        v_sub,
        num_mov,
        num_exps,
    } = run;
    let (show_sols, v_sub, num_mov, num_exps) = (*show_sols, *v_sub, *num_mov, *num_exps);
    let threshold = show_sols.unsigned_abs() as i64;
    let mut io_result = Ok(());
    let mut i = 0usize;
    let (mut succ_exp, mut succ_mov, mut tot_mov, mut glob_vals) = (0i64, 0i64, 0i64, 0i64);
    let mut boxes = [0i64; NUM_BOXES];
    let boxsize = num_mov / NUM_BOXES as i64 + 1;
    let _ = params;
    drive(&mut |e, neighbors| {
        if io_result.is_err() {
            return;
        }
        io_result = (|| -> io::Result<()> {
            glob_vals += e.best_conflict;
            tot_mov += e.moves as i64;
            if e.success {
                succ_exp += 1;
                succ_mov += e.moves as i64;
                boxes[(e.moves as i64 / boxsize) as usize] += 1;
            }
            if i.is_multiple_of(25) {
                writeln!(out)?;
            }
            write!(out, "{} ", e.best_conflict)?;
            if e.best_conflict < threshold {
                write!(out, "\n\n")?;
                for &k in &e.vertices {
                    write!(out, "{k} ")?;
                }
                write!(out, "\n\n")?;
                if show_sols < 0 {
                    let mut degree = 0;
                    for &k in &e.vertices {
                        for (u, printed) in neighbors(k) {
                            if e.vertices.binary_search(&u).is_ok() {
                                degree += 1;
                                if u > k {
                                    write!(out, " {printed}")?;
                                }
                            }
                        }
                        writeln!(out, "-1")?;
                    }
                    write!(out, "\nV = {}, E = {}\n\n", v_sub, degree / 2)?;
                }
            }
            out.flush()
        })();
        i += 1;
    })?;
    io_result?;

    write!(out, "\n\n{succ_exp} : ")?;
    let divisor = succ_exp.max(1);
    for b in boxes {
        write!(out, "{b} ")?;
    }
    write!(
        out,
        "\n{} {} {} {}",
        succ_mov / divisor,
        tot_mov / divisor,
        format_g(glob_vals as f64 / num_exps as f64),
        footer()
    )?;
    Ok(0)
}

/// Runs `ggen` on `input` (its complete stdin), writing its stdout to `out`. Errors carry the
/// message `ggen` prints on stderr before exiting with status 1.
pub fn ggen<W: Write>(input: &str, out: &mut W) -> Result<(), Error> {
    const ADJ_LIST: i64 = 0;
    const INC_LIST: i64 = 1;
    let die = |msg: String| Err(Error::Legacy(msg));

    let mut sc = Scanner::new(input);
    let mut p = [0i64; 8];
    for x in p.iter_mut() {
        *x = match sc.next_int() {
            Some(x) => x,
            None => {
                return die(
                    "expected 8 integer parameters: graph_type v num_sets density seed num_fixed fixed_type compl".into(),
                );
            }
        };
    }
    let [
        graph_type,
        v,
        num_sets,
        density,
        seed,
        num_fixed,
        fixed_type,
        compl,
    ] = p;
    writeln!(
        out,
        "{graph_type} {v} {num_sets} {density} {seed}  {num_fixed} {fixed_type}  {compl}"
    )?;

    if !(0..=4).contains(&graph_type) {
        return die(format!("{graph_type} is not a valid graph type"));
    }
    if v < 0 {
        return die(format!("number of vertices must be non-negative, got {v}"));
    }
    if graph_type >= 2 {
        if !(0..=1000).contains(&density) {
            return die(format!("density must be between 0 and 1000, got {density}"));
        }
        if seed <= 0 {
            return die(format!("seed must be a positive integer, got {seed}"));
        }
    } else if graph_type == INC_LIST && density < 0 {
        return die(format!(
            "density (intersection size) must be non-negative, got {density}"
        ));
    }
    if num_fixed < 0 {
        return die(format!(
            "number of fixed vertices must be non-negative, got {num_fixed}"
        ));
    }
    if num_fixed > 0 && ![1, -1, 2, -2, 3].contains(&fixed_type) {
        return die(format!("{fixed_type} is not a valid fixed type"));
    }
    if compl != 0 && compl != 1 {
        return die(format!("complementation flag must be 0 or 1, got {compl}"));
    }

    let n = v as usize;
    let mut graph = match graph_type {
        ADJ_LIST => read_adjacency(&mut sc, n, true)?,
        INC_LIST => {
            let sets_count = num_sets.unsigned_abs() as usize;
            let mut sets = Vec::with_capacity(sets_count);
            for _ in 0..sets_count {
                let mut set = Vec::new();
                while let Some(s) = read_vertex(&mut sc, n, true)? {
                    set.push(s);
                }
                sets.push(set);
            }
            let inc = ops::Incidence::new(n, &sets)?;
            if num_sets > 0 {
                inc.symbol_graph(density as usize)
            } else {
                inc.set_graph(density as usize)
            }
        }
        t => {
            let model = match t {
                2 => generate::Model::Exponential,
                3 => generate::Model::Power,
                _ => generate::Model::Geometric,
            };
            generate::generate(model, n, density as u32, &mut Randu::new(seed))?
        }
    };

    if num_fixed > 0 {
        let v = graph.num_vertices();
        if num_fixed as usize > v {
            return die(format!(
                "{num_fixed} fixed vertices requested, but the graph has only {v} vertices"
            ));
        }
        let mut fixed = Vec::with_capacity(num_fixed as usize);
        let mut seen = vec![false; v];
        for _ in 0..num_fixed {
            let f = read_vertex(&mut sc, v, false)?.unwrap();
            if std::mem::replace(&mut seen[f], true) {
                return die(format!("fixed vertex {f} is listed twice"));
            }
            fixed.push(f);
        }
        let sub_verts = match fixed_type {
            1 | 3 => fixed.clone(),
            -1 => ops::rest(&graph, &fixed)?,
            2 => ops::common_neighborhood(&graph, &fixed)?,
            _ => ops::common_non_neighborhood(&graph, &fixed)?,
        };
        write!(out, "\n{}\n", sub_verts.len())?;
        for s in &sub_verts {
            write!(out, " {s}")?;
        }
        write!(out, "\n\n")?;
        graph = if fixed_type == 3 {
            ops::switch(&graph, &fixed)?
        } else {
            ops::induced(&graph, &sub_verts)?
        };
    }

    let n = graph.num_vertices();
    let complemented = compl == 1;
    let mut e = 0;
    for i in 0..n {
        for j in i + 1..n {
            if graph.has_edge(i, j) != complemented {
                write!(out, " {j}")?;
                e += 1;
            }
        }
        writeln!(out, "-1")?;
    }
    write!(out, "\nV = {n}, E = {e}\n")?;
    write_degree_spectrum(out, &graph, complemented)?;
    Ok(())
}

/// `ggen`'s degree spectrum: `"degree: count"` pairs, in its idiosyncratic line breaking.
fn write_degree_spectrum<W: Write>(
    out: &mut W,
    graph: &Graph,
    complemented: bool,
) -> io::Result<()> {
    let n = graph.num_vertices();
    let mut counts = vec![0usize; n];
    for i in 0..n {
        let d = if complemented {
            n - 1 - graph.degree(i)
        } else {
            graph.degree(i)
        };
        counts[d] += 1;
    }
    let mut j = 0;
    for (i, &c) in counts.iter().enumerate() {
        if c > 0 {
            j += 1;
            write!(out, "{i:<4}: {c:<4} ")?;
        }
        if j % 7 == 0 {
            j += 1;
            writeln!(out)?;
        }
    }
    writeln!(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scanner_reads_like_scanf() {
        let mut sc = Scanner::new(" 3 5-1\n-1 +7 x");
        let got: Vec<_> = std::iter::from_fn(|| sc.next_int()).collect();
        assert_eq!(got, vec![3, 5, -1, -1, 7]);
    }

    #[test]
    fn format_g_matches_printf() {
        let cases = [
            (0.0, "0"),
            (1.0, "1"),
            (2.5, "2.5"),
            (1.0 / 3.0, "0.333333"),
            (123456.0, "123456"),
            (1234567.0, "1.23457e+06"),
            (0.0001, "0.0001"),
            (0.00001234, "1.234e-05"),
            (999999.5, "1e+06"),
            (-2.25, "-2.25"),
            (f64::NAN, "nan"),
        ];
        for (x, want) in cases {
            assert_eq!(format_g(x), want, "{x}");
        }
    }

    #[test]
    fn degree_spectrum_handles_four_digit_degrees() {
        let g = crate::ops::complement(&Graph::empty(1001));
        let mut out = Vec::new();
        write_degree_spectrum(&mut out, &g, false).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "\n1000: 1001 \n");
    }
}
