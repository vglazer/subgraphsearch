//! Randomized tabu search for vertex-induced subgraphs with a prescribed edge count.
//!
//! This is a faithful port of the original `sub_search.c`. Each *experiment* starts from
//! a random set of `size` vertices (always including any fixed vertices) and makes up to
//! `max_moves` *moves*, each swapping one subgraph vertex for one outside vertex, trying to drive
//! the objective `|e - target|` to zero, where `e` is the number of edges the subgraph induces.
//!
//! - Normally a move is chosen at random among the best swaps in a *restricted* neighborhood:
//!   when the edge count did not change on the last move, only rows for vertices adjacent to
//!   exactly one of the two vertices just swapped get a full scan; the rest only look at such
//!   columns.
//! - The vertices just swapped are *tabu* for [`SearchParams::tabu_tenure`] moves: the vertex just
//!   added cannot be removed and the vertex just removed cannot be added back. The original
//!   programs' tenure was 1, a single move; by default it is a tenth of the subgraph size.
//! - For the first `div_dur` moves of every `div_freq` (and whenever no optimal move is known),
//!   a uniformly random swap is made instead ("diversification").
//!
//! - With [`SearchParams::aspiration`] (the original `asub_search`), an *aspiration criterion*
//!   lifts the tabu: the swaps involving the two vertices just swapped are also examined after
//!   each move, and one is taken whenever it would beat the best value so far. With
//!   [`SearchParams::full_aspiration`], every swap the tabu tenure forbids is examined the same
//!   way; with tenure 1 the two are the same.
//!
//! - With [`SearchParams::connected`], every component of the subgraph beyond the first adds 1 to
//!   the conflict, so an experiment succeeds only on a connected subgraph that hits the target.
//!   Moves are still chosen by the target alone: a disconnected subgraph that hits it leaves the
//!   search on a plateau, which it walks across until it reaches a connected one.
//!
//! With [`RngKind::Legacy`] the search consumes random numbers exactly as the C programs do, so
//! the same seed gives the same experiments, move for move (see [`search_sequential`]).

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::graph::{Graph, WeightedGraph, bit};
use crate::rng::{Randu, RngKind, SearchRng, Xoshiro};

/// Number of "boxes" in the distribution of moves needed by successful experiments.
pub const NUM_BOXES: usize = 10;

/// A graph the search can run on: [`Graph`] (`sub_search`), where every edge counts 1, or
/// [`WeightedGraph`] (`wsub_search`), where an edge counts its weight and the target is a total
/// weight. The search is compiled separately for each, so the unweighted case pays nothing for
/// weights.
pub trait SearchGraph: Sync {
    /// A row of the adjacency (or weight) matrix.
    type Row<'a>: AdjacencyRow
    where
        Self: 'a;

    fn num_vertices(&self) -> usize;
    /// Neighbors of `u`, ascending.
    fn neighbors(&self, u: usize) -> &[u32];
    fn adjacency_row(&self, u: usize) -> Self::Row<'_>;
    /// What the edge `{u, v}` contributes to the subgraph's value: its weight, or 0 or 1.
    fn edge_value(&self, u: usize, v: usize) -> i64;
    /// The graph as an unweighted [`Graph`], if it is one (regular searches need one).
    fn as_graph(&self) -> Option<&Graph> {
        None
    }
    /// [`SearchGraph::edge_value`] for a known neighbor `v` of `u`.
    fn neighbor_value(&self, u: usize, v: usize) -> i64 {
        self.edge_value(u, v)
    }
    /// The initial "best value so far" of an experiment: the sum of the degrees in
    /// `sub_search`, the total weight in `wsub_search`.
    fn objective_bound(&self) -> i64;
    /// An upper bound on [`AdjacencyRow::value`] in `u`'s row: 1 in an unweighted graph, the
    /// largest weight at `u` in a weighted one. The search uses it to skip rows that cannot hold a
    /// best move; the default, a bound too large to skip any, is always correct.
    fn max_row_value(&self, _u: usize) -> i64 {
        NO_ROW_BOUND
    }
}

/// [`SearchGraph::max_row_value`]'s default: large enough to bound any row, small enough that the
/// search's arithmetic with it cannot overflow.
const NO_ROW_BOUND: i64 = i64::MAX / 4;

/// In tests, makes every row's bound 0 and turns off the buckets, so the search scans every
/// column of every row, as a reference.
#[cfg(test)]
static NO_ROW_SKIPPING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Constant-time access to one row of an adjacency or weight matrix.
pub trait AdjacencyRow: Copy {
    fn value(self, v: usize) -> i64;
}

impl AdjacencyRow for &[u64] {
    #[inline(always)]
    fn value(self, v: usize) -> i64 {
        bit(self, v) as i64
    }
}

impl AdjacencyRow for &[u8] {
    #[inline(always)]
    fn value(self, v: usize) -> i64 {
        i64::from(self[v])
    }
}

impl SearchGraph for Graph {
    type Row<'a> = &'a [u64];

    fn num_vertices(&self) -> usize {
        Graph::num_vertices(self)
    }
    fn neighbors(&self, u: usize) -> &[u32] {
        Graph::neighbors(self, u)
    }
    fn adjacency_row(&self, u: usize) -> &[u64] {
        self.row(u)
    }
    fn edge_value(&self, u: usize, v: usize) -> i64 {
        i64::from(self.has_edge(u, v))
    }
    #[inline(always)]
    fn neighbor_value(&self, _u: usize, _v: usize) -> i64 {
        1
    }
    fn objective_bound(&self) -> i64 {
        2 * self.num_edges() as i64
    }
    #[inline(always)]
    fn max_row_value(&self, _u: usize) -> i64 {
        1
    }
    fn as_graph(&self) -> Option<&Graph> {
        Some(self)
    }
}

impl SearchGraph for WeightedGraph {
    type Row<'a> = &'a [u8];

    fn num_vertices(&self) -> usize {
        WeightedGraph::num_vertices(self)
    }
    fn neighbors(&self, u: usize) -> &[u32] {
        WeightedGraph::neighbors(self, u)
    }
    fn adjacency_row(&self, u: usize) -> &[u8] {
        self.row(u)
    }
    fn edge_value(&self, u: usize, v: usize) -> i64 {
        i64::from(self.weight(u, v))
    }
    fn objective_bound(&self) -> i64 {
        self.total_weight() as i64
    }
    #[inline(always)]
    fn max_row_value(&self, u: usize) -> i64 {
        i64::from(self.max_weight_at(u))
    }
}

/// Search parameters. See the module documentation for what they mean.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchParams {
    /// Number of subgraph vertices, including fixed ones.
    pub size: usize,
    /// Desired number of subgraph edges.
    pub edges: i64,
    /// Vertices that must belong to the subgraph.
    #[serde(default)]
    pub fixed: Vec<usize>,
    /// Number of independent experiments.
    pub experiments: usize,
    /// Maximum number of moves per experiment.
    pub max_moves: usize,
    /// Diversification frequency: a cycle of random moves starts every `div_freq` moves (>= 1).
    pub div_freq: usize,
    /// Diversification duration: the number of random moves at the start of each cycle (0 = none).
    pub div_dur: usize,
    /// With a positive size `k`, each diversification cycle starts with a *region kick* instead of
    /// random moves: the `k` free subgraph vertices nearest a random one (in breadth-first order from
    /// it, at whatever distance) are swapped out, one per move, each for the best vertex to swap in,
    /// and the vertices removed may not return for `kick_ban` moves. It rebuilds one part of the
    /// subgraph at a time, as large-neighborhood moves in searches for codes do. The first cycle
    /// keeps its random moves. [`SearchParams::new`] sets [`DEFAULT_KICK_SIZE`], a safe default;
    /// larger sizes can help much more on some problems and hurt on others, so compare a few, as
    /// with `div_dur`. 0 keeps the random moves throughout, and is what
    /// results saved before kicks existed used. Edge-count and weight searches only: regular and
    /// path searches ignore it. To reproduce the original programs' runs with
    /// [`RngKind::Legacy`], set it to 0.
    #[serde(default)]
    pub kick_size: usize,
    /// How many moves a vertex removed by a region kick stays out (see `kick_size`).
    #[serde(default = "default_kick_ban")]
    pub kick_ban: usize,
    /// The *tabu tenure*: for how many moves a vertex just swapped out may not be swapped back in,
    /// and a vertex just swapped in may not be swapped out. [`AUTO_TABU_TENURE`] (0), which
    /// [`SearchParams::new`] sets, chooses [`auto_tabu_tenure`] of the subgraph size and the
    /// number of vertices left outside it, and results record the tenure chosen. 1 is the shortest tenure, the original programs', and what
    /// results saved before the tenure existed used; to reproduce the original programs' runs with
    /// [`RngKind::Legacy`], set it to 1. Edge-count and weight searches only: regular and path
    /// searches ignore it.
    #[serde(default = "default_tabu_tenure")]
    pub tabu_tenure: usize,
    /// Random seed.
    pub seed: i64,
    /// Random number generator.
    pub rng: RngKind,
    /// Record the objective value after every move (memory: experiments x moves).
    #[serde(default)]
    pub record_trace: bool,
    /// An aspiration criterion, as in the original `asub_search`: after each move, also consider
    /// the swaps involving the two vertices just swapped (which are otherwise tabu), and take one
    /// of them whenever it would beat the best value so far.
    #[serde(default)]
    pub aspiration: bool,
    /// The full aspiration criterion: after each move, also consider every swap the tabu tenure
    /// forbids, not only those involving the two vertices just swapped, and take one of them
    /// whenever it would beat the best value so far. It lets a long tenure keep the search from
    /// circling back without blocking improvements, at some cost per move. With a tenure of 1 it is
    /// the same as `aspiration`, whose code it then uses, so legacy runs are unchanged. Edge-count
    /// and weight searches only.
    #[serde(default)]
    pub full_aspiration: bool,
    /// Require a connected subgraph: each component beyond the first adds 1 to the conflict. The
    /// components are counted after every move, at a cost of about the sum of the subgraph's
    /// degrees in the whole graph.
    #[serde(default)]
    pub connected: bool,
    /// Search for induced paths on `size` vertices instead, with the regular search's moves aimed at
    /// the degrees of a path (see [`crate::regular`]); the `edges` target is then ignored, and
    /// `regular` must be `None`. Needs an unweighted [`Graph`].
    #[serde(default)]
    pub path: bool,
    /// With `path`: the two vertices the path must run between, as its two ends. The search adds a
    /// new vertex adjacent to each, which an induced path can only contain as an end, and fixes all
    /// four; the results leave the new vertices out (see [`crate::regular`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoints: Option<[usize; 2]>,
    /// Search for regular induced subgraphs instead (the original `reg_search` and `ereg_search`); the
    /// `edges` target is then ignored. Needs an unweighted [`Graph`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regular: Option<Regular>,
}

/// A regular-subgraph search: see [`crate::regular`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Regular {
    /// The degree every subgraph vertex should have within the subgraph.
    pub degree: i64,
    /// First find the right number of edges, then make the subgraph regular (`ereg_search`).
    #[serde(default)]
    pub two_phase: bool,
}

fn default_kick_ban() -> usize {
    20
}

/// The region kick size [`SearchParams::new`] sets: small enough to be a safe default; compare
/// larger sizes on hard problems.
pub const DEFAULT_KICK_SIZE: usize = 4;

fn default_tabu_tenure() -> usize {
    1
}

/// The `tabu_tenure` that chooses [`auto_tabu_tenure`], and the one [`SearchParams::new`] sets.
pub const AUTO_TABU_TENURE: usize = 0;

/// The tabu tenure [`AUTO_TABU_TENURE`] chooses for a subgraph of `size` of the graph's `vertices`:
/// a tenth of the size or an eightieth of the vertices left outside it, whichever is smaller,
/// at least 1 and at most [`MAX_AUTO_TABU_TENURE`]. The best tenure grows with both, and too long a
/// tenure hems the search in, so this errs on the short side: small graphs get 1, the original
/// programs' tenure. Compare others with [`SearchParams::tabu_tenure`].
pub fn auto_tabu_tenure(size: usize, vertices: usize) -> usize {
    (size / 10)
        .min(vertices.saturating_sub(size) / 80)
        .clamp(1, MAX_AUTO_TABU_TENURE)
}

/// The longest tenure [`auto_tabu_tenure`] chooses: longer tenures rarely help, and on large
/// subgraphs of structured graphs they stop the search from finding anything.
pub const MAX_AUTO_TABU_TENURE: usize = 20;

/// The kick size and tabu tenure a search uses, from those requested (`None` for the default, and
/// [`AUTO_TABU_TENURE`] for the automatic tenure): the one place that knows which searches take
/// them. The counting search takes both, by default kicks of [`DEFAULT_KICK_SIZE`] and the
/// automatic tenure. The degree search (`degree_search`: regular subgraphs, cycles and paths) and
/// the legacy random number generator (`legacy`) default to no kicks and a tenure of 1, the
/// original programs'; the degree search accepts no others.
pub fn search_settings(
    kick_size: Option<usize>,
    tabu_tenure: Option<usize>,
    degree_search: bool,
    legacy: bool,
) -> Result<(usize, usize), Error> {
    let not_for_degree = |what: &str| {
        Err(Error::Invalid(format!(
            "{what} for the counting search only, not for regular subgraphs, paths or cycles"
        )))
    };
    let kick_size = match kick_size {
        Some(k) if k > 0 && degree_search => return not_for_degree("region kicks are"),
        Some(k) => k,
        None if degree_search || legacy => 0,
        None => DEFAULT_KICK_SIZE,
    };
    let tabu_tenure = match tabu_tenure {
        Some(t) if t != 1 && degree_search => {
            return not_for_degree("a tabu tenure other than 1 is");
        }
        Some(t) => t,
        None if degree_search || legacy => 1,
        None => AUTO_TABU_TENURE,
    };
    Ok((kick_size, tabu_tenure))
}

impl SearchParams {
    /// The tabu tenure a search of a graph on `vertices` vertices uses: `tabu_tenure`, or
    /// [`auto_tabu_tenure`] for [`AUTO_TABU_TENURE`].
    pub fn resolved_tabu_tenure(&self, vertices: usize) -> usize {
        if self.tabu_tenure == AUTO_TABU_TENURE {
            auto_tabu_tenure(self.size, vertices)
        } else {
            self.tabu_tenure
        }
    }
}

impl SearchParams {
    /// Parameters with the given subgraph size and edge count and otherwise reasonable defaults:
    /// 40 experiments of up to 250 moves (many short runs usually beat a few long ones), 4 random
    /// moves every 250, region kicks of [`DEFAULT_KICK_SIZE`] from the second cycle on, seed 1,
    /// xoshiro.
    pub fn new(size: usize, edges: i64) -> Self {
        SearchParams {
            size,
            edges,
            fixed: Vec::new(),
            experiments: 40,
            max_moves: 250,
            div_freq: 250,
            div_dur: 4,
            kick_size: DEFAULT_KICK_SIZE,
            kick_ban: default_kick_ban(),
            tabu_tenure: AUTO_TABU_TENURE,
            seed: 1,
            rng: RngKind::Xoshiro,
            record_trace: false,
            aspiration: false,
            full_aspiration: false,
            connected: false,
            path: false,
            endpoints: None,
            regular: None,
        }
    }

    /// Checks the parameters against a graph on `n` vertices, with the same rules as the C program
    /// plus checks it omitted (fixed vertices must be distinct and in range).
    pub fn validate(&self, n: usize) -> Result<(), Error> {
        if self.size < 2 || self.size <= self.fixed.len() || self.size + 1 > n {
            return Err(Error::InvalidSubgraphSize(self.size as i64));
        }
        if self.div_freq < 1 {
            return Err(Error::InvalidDiversificationFrequency(self.div_freq as i64));
        }
        let mut seen = vec![false; n];
        for &v in &self.fixed {
            if v >= n {
                return Err(Error::VertexOutOfRange {
                    vertex: v as i64,
                    n,
                });
            }
            if std::mem::replace(&mut seen[v], true) {
                return Err(Error::DuplicateVertex(v));
            }
        }
        if let Some([s, t]) = self.endpoints {
            if !self.path {
                return Err(Error::Invalid("endpoints need a path search".into()));
            }
            for v in [s, t] {
                if v >= n {
                    return Err(Error::VertexOutOfRange {
                        vertex: v as i64,
                        n,
                    });
                }
            }
            if s == t {
                return Err(Error::Invalid(format!(
                    "a path's two endpoints must differ, got {s} twice"
                )));
            }
            let extra = [s, t].iter().filter(|&&v| !seen[v]).count();
            if self.size <= self.fixed.len() + extra {
                return Err(Error::InvalidSubgraphSize(self.size as i64));
            }
        }
        Ok(())
    }
}

/// Counts the components of the current subgraph, for [`SearchParams::connected`].
pub(crate) struct Components {
    /// The pass that last reached each vertex; a new pass needs no clearing.
    seen: Vec<u32>,
    pass: u32,
    stack: Vec<usize>,
}

impl Components {
    pub(crate) fn new(n: usize) -> Self {
        Components {
            seen: vec![0; n],
            pass: 0,
            stack: Vec::new(),
        }
    }

    /// The number of components beyond the first among `members`, the vertices with
    /// `in_sub[v] != 0`, with each vertex's neighbors in the whole graph given by `neighbors`.
    pub(crate) fn extra<'g>(
        &mut self,
        members: impl Iterator<Item = usize>,
        in_sub: &[u8],
        neighbors: impl Fn(usize) -> &'g [u32],
    ) -> i64 {
        if self.pass == u32::MAX {
            self.seen.fill(0);
            self.pass = 0;
        }
        self.pass += 1;
        let pass = self.pass;
        let mut components = 0;
        for start in members {
            if self.seen[start] == pass {
                continue;
            }
            components += 1;
            self.seen[start] = pass;
            self.stack.push(start);
            while let Some(u) = self.stack.pop() {
                for &x in neighbors(u) {
                    let x = x as usize;
                    if in_sub[x] != 0 && self.seen[x] != pass {
                        self.seen[x] = pass;
                        self.stack.push(x);
                    }
                }
            }
        }
        (components - 1).max(0)
    }
}

/// The outcome of one experiment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Experiment {
    /// The smallest conflict reached, which is that of `vertices`: `|e - target|`, or for a regular
    /// search the deviation from regularity, plus the components beyond the first with
    /// [`SearchParams::connected`]. 0 exactly when the experiment succeeded.
    pub best_conflict: i64,
    /// Moves made: the index of the successful move, or `max_moves` if none succeeded.
    pub moves: usize,
    /// Whether a subgraph with exactly the target edge count was found.
    pub success: bool,
    /// Vertices (ascending, fixed ones included) of the subgraph that reached `best_conflict`.
    pub vertices: Vec<usize>,
    /// Objective value after each move, if requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<Vec<u32>>,
}

/// The outcome of a search: its parameters and one [`Experiment`] per experiment.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub params: SearchParams,
    pub experiments: Vec<Experiment>,
}

impl SearchResult {
    /// Number of successful experiments.
    pub fn successes(&self) -> usize {
        self.experiments.iter().filter(|e| e.success).count()
    }

    /// The distribution of moves needed by successful experiments over [`NUM_BOXES`] equal boxes.
    pub fn boxes(&self) -> [usize; NUM_BOXES] {
        let size = self.params.max_moves / NUM_BOXES + 1;
        let mut boxes = [0; NUM_BOXES];
        for e in self.experiments.iter().filter(|e| e.success) {
            boxes[e.moves / size] += 1;
        }
        boxes
    }

    /// Mean moves per success, over successful experiments only ("relative cost").
    pub fn relative_cost(&self) -> Option<f64> {
        let s = self.successes();
        (s > 0).then(|| {
            self.experiments
                .iter()
                .filter(|e| e.success)
                .map(|e| e.moves)
                .sum::<usize>() as f64
                / s as f64
        })
    }

    /// Total moves over all experiments divided by the number of successes ("true cost").
    pub fn true_cost(&self) -> Option<f64> {
        let s = self.successes();
        (s > 0).then(|| self.experiments.iter().map(|e| e.moves).sum::<usize>() as f64 / s as f64)
    }

    /// The best experiment of a densest or sparsest search of `graph`: the smallest conflict; then,
    /// if the graph was made from scores ([`WeightedGraph::from_scores`]), the highest total score
    /// (`highest`) or the lowest, since selections tied at rounded weights can differ in their
    /// exact totals; then the earliest. `None` without experiments.
    pub fn best_by_score(&self, graph: &WeightedGraph, highest: bool) -> Option<usize> {
        let least = self.experiments.iter().map(|e| e.best_conflict).min()?;
        let tied =
            (0..self.experiments.len()).filter(|&i| self.experiments[i].best_conflict == least);
        let score = |i: usize| {
            graph
                .induced_score(&self.experiments[i].vertices)
                .unwrap_or(0.0)
        };
        tied.reduce(|best, i| {
            let (a, b) = (score(best), score(i));
            if (highest && b > a) || (!highest && b < a) {
                i
            } else {
                best
            }
        })
    }

    /// Mean best objective value over all experiments ("average conflict").
    pub fn average_conflict(&self) -> f64 {
        if self.experiments.is_empty() {
            return f64::NAN;
        }
        self.experiments
            .iter()
            .map(|e| e.best_conflict)
            .sum::<i64>() as f64
            / self.experiments.len() as f64
    }
}

/// Runs a search. With [`RngKind::Xoshiro`], experiments run in parallel, each with its own random
/// stream, so results depend only on the seed. With [`RngKind::Legacy`], experiments run one after
/// another and match the C program exactly.
pub fn search<G: SearchGraph>(graph: &G, params: &SearchParams) -> Result<SearchResult, Error> {
    if params.regular.is_some() || params.path {
        // regular and path searches have a tenure of 1
        let mut result = crate::regular::search(unweighted(graph)?, params)?;
        result.params.tabu_tenure = 1;
        return Ok(result);
    }
    params.validate(graph.num_vertices())?;
    let experiments = match params.rng {
        RngKind::Legacy => {
            let mut out = Vec::with_capacity(params.experiments);
            search_sequential(graph, params, |e| out.push(e.clone()))?;
            out
        }
        RngKind::Xoshiro => (0..params.experiments)
            .into_par_iter()
            .map_init(
                || Workspace::new(graph, params),
                |ws, i| {
                    ws.reset(params);
                    let mut rng = Xoshiro::for_stream(params.seed as u64, i as u64);
                    ws.run(graph, params, &mut rng)
                },
            )
            .collect(),
    };
    // record the tenure the search used, not "automatic"
    let mut params = params.clone();
    params.tabu_tenure = params.resolved_tabu_tenure(graph.num_vertices());
    Ok(SearchResult {
        params,
        experiments,
    })
}

/// Runs the experiments one at a time, calling `on_experiment` after each.
///
/// With [`RngKind::Legacy`] this reproduces the C program exactly: a single random stream shared
/// by all experiments, and state (the permutation used to pick starting subgraphs, the best
/// subgraph seen) carried over from one experiment to the next. With [`RngKind::Xoshiro`] the
/// results equal those of [`search`].
pub fn search_sequential<G: SearchGraph, F>(
    graph: &G,
    params: &SearchParams,
    mut on_experiment: F,
) -> Result<(), Error>
where
    F: FnMut(&Experiment),
{
    if params.regular.is_some() || params.path {
        return crate::regular::search_sequential(unweighted(graph)?, params, on_experiment);
    }
    params.validate(graph.num_vertices())?;
    let mut ws = Workspace::new(graph, params);
    match params.rng {
        RngKind::Legacy => {
            let mut rng = Randu::new(params.seed);
            for _ in 0..params.experiments {
                on_experiment(&ws.run(graph, params, &mut rng));
            }
        }
        RngKind::Xoshiro => {
            for i in 0..params.experiments {
                ws.reset(params);
                let mut rng = Xoshiro::for_stream(params.seed as u64, i as u64);
                on_experiment(&ws.run(graph, params, &mut rng));
            }
        }
    }
    Ok(())
}

/// The smaller of `best` and the best value of swapping a subgraph vertex, whose row of the
/// adjacency matrix is `row`, for each of `cols`, stopping early at `bound`, below which the row
/// cannot go. With [`scan_row`], this is the search's hot loop.
#[inline(always)]
fn row_min<R: AdjacencyRow>(
    row: R,
    cols: &[usize],
    sub_degs: &[i32],
    degree: i64,
    bound: i64,
    best: i64,
) -> i64 {
    let mut best = best;
    for &t in cols {
        let val = (degree - row.value(t) + i64::from(sub_degs[t])).abs();
        if val < best {
            best = val;
            if best <= bound {
                break;
            }
        }
    }
    best
}

/// Appends to `sub_opt`/`rest_opt` every swap of `vert` for one of `cols` with value `best`, up
/// to their length, which caps how many are kept, as in `sub_search.c`.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn scan_row<R: AdjacencyRow>(
    row: R,
    cols: &[usize],
    sub_degs: &[i32],
    degree: i64,
    vert: usize,
    best: i64,
    opt_mov: &mut usize,
    sub_opt: &mut [usize],
    rest_opt: &mut [usize],
) {
    let cap = sub_opt.len().min(rest_opt.len());
    let mut count = *opt_mov;
    for &t in cols {
        if count >= cap {
            break;
        }
        let val = (degree - row.value(t) + i64::from(sub_degs[t])).abs();
        if val == best {
            sub_opt[count] = vert;
            rest_opt[count] = t;
            count += 1;
        }
    }
    *opt_mov = count;
}

/// The candidate columns for a move, as `Workspace::list_candidates` lists them in `u_list`: how
/// many are active (they come first) and how many in all, and the range of their degrees into the
/// subgraph.
struct Columns {
    active: usize,
    all: usize,
    active_range: (i64, i64),
    all_range: (i64, i64),
}

impl Columns {
    /// The range of degrees among the first `w_cols` columns: all of them, or the active ones.
    #[inline]
    fn range(&self, w_cols: usize) -> (i64, i64) {
        if w_cols == self.all {
            self.all_range
        } else {
            self.active_range
        }
    }
}

/// The candidate columns bucketed by their degree into the subgraph, so that a row can visit only
/// the columns whose degree lets a swap reach a given value. Results are exactly those of scanning
/// the columns in order: buckets keep the columns' order, and ties are listed by position.
#[derive(Default)]
struct Buckets {
    /// Degree of bucket 0.
    base: i64,
    start: Vec<u32>,
    pos: Vec<u32>,
    fill: Vec<u32>,
    /// Each column's degree, by position.
    deg: Vec<i64>,
    gather: Vec<u32>,
    /// Whether bucketing is paying off: rows whose moves were collected from buckets, rows tried,
    /// and the move before which buckets are not built (when their degrees are too crowded for
    /// the buckets to narrow anything down, building them only costs time).
    collected: u32,
    tried: u32,
    paused_until: usize,
}

impl Buckets {
    /// Buckets `cols` by degree, if their degrees span few enough values to pay off; returns
    /// whether it did.
    fn build(&mut self, cols: &[usize], sub_degs: &[i32], lo: i64, hi: i64) -> bool {
        let span = (hi - lo + 1) as usize;
        if span > 2 * cols.len() + 64 {
            return false;
        }
        self.base = lo;
        self.start.clear();
        self.start.resize(span + 1, 0);
        self.deg.clear();
        for &t in cols {
            let d = i64::from(sub_degs[t]);
            self.deg.push(d);
            self.start[(d - lo) as usize + 1] += 1;
        }
        for b in 0..span {
            self.start[b + 1] += self.start[b];
        }
        self.fill.clear();
        self.fill.extend_from_slice(&self.start[..span]);
        self.pos.clear();
        self.pos.resize(cols.len(), 0);
        for (p, &d) in self.deg.iter().enumerate() {
            let b = (d - lo) as usize;
            self.pos[self.fill[b] as usize] = p as u32;
            self.fill[b] += 1;
        }
        true
    }

    /// Starts an experiment with bucketing on.
    fn reset(&mut self) {
        (self.collected, self.tried, self.paused_until) = (0, 0, 0);
    }

    /// After a move made with buckets: once enough rows have tried them, pauses bucketing for a
    /// while if they mostly could not narrow the columns down.
    fn review(&mut self, j: usize) {
        if self.tried >= 256 {
            if self.collected * 2 < self.tried {
                self.paused_until = j + 1024;
            }
            (self.collected, self.tried) = (0, 0);
        }
    }

    /// The positions in degree `d`'s bucket, ascending.
    #[inline]
    fn bucket(&self, d: i64) -> &[u32] {
        let b = (d - self.base) as usize;
        &self.pos[self.start[b] as usize..self.start[b + 1] as usize]
    }

    /// [`row_min`] over the first `w_cols` columns, whose degrees lie in `range`, visiting buckets
    /// in order of the least value their swaps can have and stopping once none can beat `best`.
    #[allow(clippy::too_many_arguments)]
    fn row_min<R: AdjacencyRow>(
        &self,
        row: R,
        u_list: &[usize],
        w_cols: usize,
        (lo, hi): (i64, i64),
        degree: i64,
        wmax: i64,
        bound: i64,
        mut best: i64,
    ) -> i64 {
        // a swap with a column of degree d has value |degree + d - w| for some w in 0..=wmax: 0 is
        // possible for d in [-degree, -degree + wmax], and the least value grows by 1 per step away
        let visit = |d: i64, best: &mut i64| -> bool {
            for &p in self.bucket(d) {
                let p = p as usize;
                if p >= w_cols {
                    break;
                }
                let val = (degree + d - row.value(u_list[p])).abs();
                if val < *best {
                    *best = val;
                    if *best <= bound {
                        return true;
                    }
                }
            }
            false
        };
        let (za, zb) = ((-degree).max(lo), (-degree).saturating_add(wmax).min(hi));
        let (mut left, mut right) = if za <= zb {
            for d in za..=zb {
                if visit(d, &mut best) {
                    return best;
                }
            }
            (za - 1, zb + 1)
        } else if -degree > hi {
            (hi, hi + 1)
        } else {
            (lo - 1, lo)
        };
        loop {
            let from_left = if left >= lo {
                -(degree + left)
            } else {
                i64::MAX
            };
            let from_right = if right <= hi {
                degree + right - wmax
            } else {
                i64::MAX
            };
            let least = from_left.min(from_right);
            if least == i64::MAX || least >= best {
                return best;
            }
            let d = if from_left <= from_right {
                left -= 1;
                left + 1
            } else {
                right += 1;
                right - 1
            };
            if visit(d, &mut best) {
                return best;
            }
        }
    }

    /// [`scan_row`] over the first `w_cols` columns, visiting only the buckets whose swaps can have
    /// value `best`, and listing those swaps in order of position. Returns false, having done
    /// nothing, when those buckets hold too many columns for this to pay off.
    #[allow(clippy::too_many_arguments)]
    fn collect_row<R: AdjacencyRow>(
        &mut self,
        row: R,
        u_list: &[usize],
        w_cols: usize,
        (lo, hi): (i64, i64),
        degree: i64,
        wmax: i64,
        best: i64,
        vert: usize,
        opt_mov: &mut usize,
        sub_opt: &mut [usize],
        rest_opt: &mut [usize],
    ) -> bool {
        // |degree + d - w| = best needs d - w = best - degree or -best - degree, w in 0..=wmax
        let window = |c: i64| (c.max(lo), c.saturating_add(wmax).min(hi));
        let (a, b) = (window(-best - degree), window(best - degree));
        let spans: &[(i64, i64)] = &if a.1 + 1 >= b.0 {
            [(a.0, b.1), (1, 0)]
        } else {
            [a, b]
        };
        let total: usize = spans
            .iter()
            .filter(|s| s.0 <= s.1)
            .map(|&(x, y)| {
                (self.start[(y - self.base) as usize + 1] - self.start[(x - self.base) as usize])
                    as usize
            })
            .sum();
        self.tried += 1;
        if total * 4 > w_cols {
            return false;
        }
        self.collected += 1;
        self.gather.clear();
        for &(x, y) in spans {
            for d in x..=y {
                let b = (d - self.base) as usize;
                for &p in &self.pos[self.start[b] as usize..self.start[b + 1] as usize] {
                    if p as usize >= w_cols {
                        break;
                    }
                    self.gather.push(p);
                }
            }
        }
        self.gather.sort_unstable();
        let cap = sub_opt.len().min(rest_opt.len());
        for &p in &self.gather {
            if *opt_mov >= cap {
                break;
            }
            let p = p as usize;
            let t = u_list[p];
            if (degree + self.deg[p] - row.value(t)).abs() == best {
                sub_opt[*opt_mov] = vert;
                rest_opt[*opt_mov] = t;
                *opt_mov += 1;
            }
        }
        true
    }
}

fn unweighted<G: SearchGraph>(graph: &G) -> Result<&Graph, Error> {
    graph
        .as_graph()
        .ok_or_else(|| Error::Invalid("regular and path searches need an unweighted graph".into()))
}

/// Search state. Field names follow `sub_search.c`.
struct Workspace {
    /// Number of free (non-fixed) subgraph vertices.
    v_sub: usize,
    /// Number of vertices outside the subgraph.
    v_rest: usize,
    /// Sum of all degrees (twice the edge count); an upper bound on the objective.
    e: i64,
    /// Degree of every vertex with respect to the current subgraph.
    sub_degs: Vec<i32>,
    /// Free subgraph vertices and outside vertices, in no particular order.
    sub_verts: Vec<usize>,
    rest_verts: Vec<usize>,
    /// Position of every vertex in `sub_verts` or `rest_verts`.
    vert_inds: Vec<usize>,
    /// Membership of the current subgraph (fixed vertices always set) and of the best one.
    sub_ch: Vec<u8>,
    opt_ch: Vec<u8>,
    /// Marks active rows and columns.
    u_ch: Vec<u8>,
    /// Candidate columns: active ones first, then the inactive ones.
    u_list: Vec<usize>,
    /// Optimal moves found in the current neighborhood.
    sub_opt: Vec<usize>,
    rest_opt: Vec<usize>,
    /// Aspiration moves (with `params.aspiration`), as in `asub_search.c`.
    sub_asp: Vec<usize>,
    rest_asp: Vec<usize>,
    /// Non-fixed vertices; a partial shuffle of it picks each starting subgraph.
    rand_perm: Vec<usize>,
    /// With `params.connected`.
    components: Option<Components>,
    /// With a region kick (`params.kick_size`): the move until which each vertex may not return,
    /// whether each vertex is fixed, and the vertices still to be kicked out.
    banned_until: Vec<usize>,
    fixed: Vec<bool>,
    kick_queue: Vec<usize>,
    /// The tabu tenure, and with one above 1, the move until which each removed vertex may not
    /// return and each added vertex may not leave.
    tenure: usize,
    tabu_in: Vec<usize>,
    tabu_out: Vec<usize>,
    /// With `params.full_aspiration`: the outside vertices that may not return yet, rebuilt each move.
    tabu_cols: Vec<usize>,
    /// The rows to scan in a move: vertex, candidate columns, row constant and a lower bound on
    /// the row's best value.
    rows: Vec<(usize, usize, i64, i64)>,
    /// The candidate columns' positions bucketed by subgraph degree: bucket `b` (degree
    /// `all_lo + b`) is `bucket_pos[bucket_start[b]..bucket_start[b + 1]]`, in increasing position.
    buckets: Buckets,
}

impl Workspace {
    fn new<G: SearchGraph>(graph: &G, params: &SearchParams) -> Self {
        let v = graph.num_vertices();
        let v_fix = params.fixed.len();
        let mut ws = Workspace {
            v_sub: params.size - v_fix,
            v_rest: v - params.size,
            // a bound on the conflict: the graph's edge count or total weight, as in the original
            // programs, unless the target lies above it, when the conflict can reach the target.
            // The legacy rng keeps the original bound, whose searches then never record a best
            // subgraph and move at random (see PORTING.md)
            e: {
                let bound = graph.objective_bound();
                if params.edges > bound && params.rng != RngKind::Legacy {
                    params.edges + 1
                } else {
                    bound
                }
            },
            sub_degs: vec![0; v],
            sub_verts: vec![0; params.size - v_fix],
            rest_verts: vec![0; v - params.size],
            vert_inds: vec![0; v],
            sub_ch: vec![0; v],
            opt_ch: vec![0; v],
            u_ch: vec![0; v],
            u_list: vec![0; v - params.size],
            sub_opt: vec![0; v],
            rest_opt: vec![0; v],
            sub_asp: if params.aspiration || params.full_aspiration {
                vec![0; v]
            } else {
                Vec::new()
            },
            rest_asp: if params.aspiration || params.full_aspiration {
                vec![0; v]
            } else {
                Vec::new()
            },
            rand_perm: Vec::with_capacity(v - v_fix),
            components: params.connected.then(|| Components::new(v)),
            banned_until: Vec::new(),
            fixed: Vec::new(),
            kick_queue: Vec::new(),
            tenure: params.resolved_tabu_tenure(v),
            tabu_cols: Vec::new(),
            tabu_in: Vec::new(),
            tabu_out: Vec::new(),
            rows: Vec::with_capacity(params.size),
            buckets: Buckets::default(),
        };
        if ws.tenure > 1 {
            ws.tabu_in = vec![0; v];
            ws.tabu_out = vec![0; v];
        }
        if params.kick_size > 0 {
            ws.banned_until = vec![0; v];
            ws.fixed = vec![false; v];
            for &f in &params.fixed {
                ws.fixed[f] = true;
            }
        }
        ws.reset(params);
        ws
    }

    /// For a region kick: the `size` free subgraph vertices nearest a random free subgraph vertex,
    /// itself included, found by breadth-first search from it (fewer if it reaches no more). Each
    /// layer of the search is visited in random order, so that among vertices equally far from the
    /// center none is favored: in a clique, for instance, all are at distance 1.
    fn region<G: SearchGraph, R: SearchRng>(
        &self,
        graph: &G,
        size: usize,
        rng: &mut R,
    ) -> Vec<usize> {
        let center = self.sub_verts[rng.below(self.v_sub as u32) as usize];
        let mut seen = std::collections::HashSet::from([center]);
        let (mut layer, mut out) = (vec![center], vec![center]);
        while out.len() < size && !layer.is_empty() {
            let mut next = Vec::new();
            for &u in &layer {
                for &w in graph.neighbors(u) {
                    let w = w as usize;
                    if seen.insert(w) {
                        next.push(w);
                    }
                }
            }
            for i in (1..next.len()).rev() {
                let j = rng.below(i as u32 + 1) as usize;
                next.swap(i, j);
            }
            out.extend(
                next.iter()
                    .copied()
                    .filter(|&w| self.sub_ch[w] != 0 && !self.fixed[w])
                    .take(size - out.len()),
            );
            layer = next;
        }
        out
    }

    /// For a region kick: the best vertex to swap in for `sub_vert`, by the usual move value, among
    /// the outside vertices not banned at move `j`; ties are broken at random.
    fn best_swap_in<G: SearchGraph, R: SearchRng>(
        &self,
        graph: &G,
        new_min_val: i64,
        sub_vert: usize,
        j: usize,
        rng: &mut R,
    ) -> Option<usize> {
        let degree = new_min_val - i64::from(self.sub_degs[sub_vert]);
        let (mut best, mut count, mut choice) = (i64::MAX, 0u32, None);
        for &r in &self.rest_verts[..self.v_rest] {
            if self.banned_until[r] > j {
                continue;
            }
            let val = (degree + i64::from(self.sub_degs[r]) - graph.edge_value(sub_vert, r)).abs();
            if val < best {
                (best, count, choice) = (val, 1, Some(r));
            } else if val == best {
                count += 1;
                if rng.below(count) == 0 {
                    choice = Some(r);
                }
            }
        }
        choice
    }

    /// Lists the aspiration moves: swaps involving the two vertices just swapped (`rest_vert` now
    /// inside the subgraph, `sub_vert` now outside) that would beat the best value so far,
    /// `g_opt_val`. At most `v` are kept. Mirrors `asub_search.c`.
    fn aspiration_moves<G: SearchGraph>(
        &mut self,
        graph: &G,
        new_min_val: i64,
        g_opt_val: i64,
        sub_vert: usize,
        rest_vert: usize,
    ) -> usize {
        let cap = self.sub_asp.len();
        let mut asp_mov = 0;
        // swap rest_vert back out for any outside vertex
        let degree = new_min_val - i64::from(self.sub_degs[rest_vert]);
        let row = graph.adjacency_row(rest_vert);
        for k in 0..self.v_rest {
            let vert = self.rest_verts[k];
            let val = (degree - row.value(vert) + i64::from(self.sub_degs[vert])).abs();
            if val < g_opt_val && asp_mov < cap {
                self.sub_asp[asp_mov] = rest_vert;
                self.rest_asp[asp_mov] = vert;
                asp_mov += 1;
            }
        }
        // swap sub_vert back in for any subgraph vertex
        let degree = new_min_val + i64::from(self.sub_degs[sub_vert]);
        let row = graph.adjacency_row(sub_vert);
        for k in 0..self.v_sub {
            let vert = self.sub_verts[k];
            if vert == rest_vert {
                continue;
            }
            let val = (degree - row.value(vert) - i64::from(self.sub_degs[vert])).abs();
            if val < g_opt_val && asp_mov < cap {
                self.sub_asp[asp_mov] = vert;
                self.rest_asp[asp_mov] = sub_vert;
                asp_mov += 1;
            }
        }
        asp_mov
    }

    /// Lists the full aspiration moves (with `params.full_aspiration`): the swaps the tabu tenure
    /// forbids at move `j` that would beat the best value so far, `g_opt_val`. Those are each
    /// subgraph vertex that may not leave yet with every outside vertex, and each outside vertex
    /// that may not return yet with every subgraph vertex. Rows are skipped by the same kind of
    /// bound as in [`Workspace::bound_rows`]. At most `v` are kept.
    fn full_aspiration_moves<G: SearchGraph>(
        &mut self,
        graph: &G,
        new_min_val: i64,
        g_opt_val: i64,
        j: usize,
    ) -> usize {
        let cap = self.sub_asp.len();
        let mut asp_mov = 0;
        // the few outside vertices that may not return yet, and the degree ranges among all outside
        // vertices and among those, for bounds like `bound_rows`'
        let (mut all_lo, mut all_hi, mut tabu_lo, mut tabu_hi) =
            (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
        let mut tabu_cols = std::mem::take(&mut self.tabu_cols);
        tabu_cols.clear();
        for l in 0..self.v_rest {
            let t = self.rest_verts[l];
            let d = i64::from(self.sub_degs[t]);
            (all_lo, all_hi) = (all_lo.min(d), all_hi.max(d));
            if self.tabu_in[t] > j {
                tabu_cols.push(t);
                (tabu_lo, tabu_hi) = (tabu_lo.min(d), tabu_hi.max(d));
            }
        }
        for k in 0..self.v_sub {
            let s = self.sub_verts[k];
            let tabu_row = self.tabu_out[s] > j;
            if !tabu_row && tabu_cols.is_empty() {
                continue;
            }
            let degree = new_min_val - i64::from(self.sub_degs[s]);
            let (lo, hi) = if tabu_row {
                (all_lo, all_hi)
            } else {
                (tabu_lo, tabu_hi)
            };
            let (low, high) = (degree + lo - graph.max_row_value(s), degree + hi);
            let bound = if high < 0 {
                -high
            } else if low > 0 {
                low
            } else {
                0
            };
            #[cfg(test)]
            let bound = if NO_ROW_SKIPPING.load(std::sync::atomic::Ordering::Relaxed) {
                0
            } else {
                bound
            };
            if bound >= g_opt_val {
                continue;
            }
            let row = graph.adjacency_row(s);
            let mut consider = |t: usize, asp_mov: &mut usize| {
                let val = (degree - row.value(t) + i64::from(self.sub_degs[t])).abs();
                if val < g_opt_val && *asp_mov < cap {
                    self.sub_asp[*asp_mov] = s;
                    self.rest_asp[*asp_mov] = t;
                    *asp_mov += 1;
                }
            };
            if tabu_row {
                for &t in &self.rest_verts[..self.v_rest] {
                    consider(t, &mut asp_mov);
                }
            } else {
                for &t in &tabu_cols {
                    consider(t, &mut asp_mov);
                }
            }
        }
        self.tabu_cols = tabu_cols;
        asp_mov
    }

    /// Restores the state of a fresh workspace.
    fn reset(&mut self, params: &SearchParams) {
        self.sub_ch.fill(0);
        self.opt_ch.fill(0);
        for &f in &params.fixed {
            self.sub_ch[f] = 1;
        }
        self.rand_perm.clear();
        self.rand_perm
            .extend((0..self.sub_ch.len()).filter(|&i| self.sub_ch[i] == 0));
    }

    /// One experiment. Mirrors the body of the experiment loop in `sub_search.c` (or, with
    /// `params.aspiration`, `asub_search.c`) statement by statement, so that random numbers are
    /// drawn in exactly the same order. Each move chooses a swap, makes it, and then finds the
    /// best swaps for the next move: listing the candidate columns, bounding the rows, and
    /// collecting the swaps of least conflict.
    fn run<G: SearchGraph, R: SearchRng>(
        &mut self,
        graph: &G,
        params: &SearchParams,
        rng: &mut R,
    ) -> Experiment {
        let mut trace = params
            .record_trace
            .then(|| Vec::with_capacity(params.max_moves));
        // signed difference between the subgraph's edge count (or weight) and the target
        let mut new_min_val = self.start_experiment(graph, params, rng);
        let mut g_opt_val = self.e;
        let mut old_min_val = self.e;
        let (mut opt_mov, mut asp_mov) = (0usize, 0usize);
        let mut success = false;
        let mut j = 0;
        while j < params.max_moves {
            let (sub_vert, rest_vert) =
                self.choose_move(graph, params, j, new_min_val, opt_mov, asp_mov, rng);
            let rest_col = self.vert_inds[rest_vert];
            new_min_val += self.apply_swap(graph, sub_vert, rest_vert, j);

            let mut obj_val = new_min_val.abs();
            if let Some(c) = self.components.as_mut() {
                let members = params.fixed.iter().chain(&self.sub_verts).copied();
                obj_val += c.extra(members, &self.sub_ch, |u| graph.neighbors(u));
            }
            if let Some(t) = trace.as_mut() {
                t.push(obj_val.min(u32::MAX as i64) as u32);
            }
            if obj_val < g_opt_val {
                g_opt_val = obj_val;
                self.opt_ch.copy_from_slice(&self.sub_ch);
            }
            if obj_val == 0 {
                success = true;
                break;
            }

            let cols = self.list_candidates(params, j, sub_vert, rest_col);
            // if the edge count changed, every row gets a full scan
            if new_min_val != old_min_val {
                for k in 0..self.v_sub {
                    self.u_ch[self.sub_verts[k]] = 1;
                }
            }
            self.bound_rows(graph, new_min_val, j, rest_vert, &cols);
            opt_mov = self.collect_best_moves(graph, j, &cols);
            // with tenure 1 the full criterion allows the same swaps as the original's, whose code
            // (and order of moves) it then keeps
            if params.full_aspiration && self.tenure > 1 {
                asp_mov = self.full_aspiration_moves(graph, new_min_val, g_opt_val, j);
            } else if params.aspiration || params.full_aspiration {
                asp_mov = self.aspiration_moves(graph, new_min_val, g_opt_val, sub_vert, rest_vert);
            }
            for k in 0..cols.active {
                self.u_ch[self.u_list[k]] = 0;
            }
            old_min_val = new_min_val;
            j += 1;
        }

        let vertices = (0..graph.num_vertices())
            .filter(|&k| self.opt_ch[k] != 0)
            .collect();
        // return the free subgraph vertices to the outside for the next experiment
        for k in 0..self.v_sub {
            self.sub_ch[self.sub_verts[k]] = 0;
        }
        Experiment {
            best_conflict: g_opt_val,
            moves: j,
            success,
            vertices,
            trace,
        }
    }

    /// Starts an experiment: a random subgraph, every vertex's degree into it, the outside
    /// vertices listed, and the kick, tabu and bucket state cleared. Returns the signed difference
    /// between the subgraph's edge count (or weight) and the target.
    #[inline]
    fn start_experiment<G: SearchGraph, R: SearchRng>(
        &mut self,
        graph: &G,
        params: &SearchParams,
        rng: &mut R,
    ) -> i64 {
        let v = graph.num_vertices();
        self.sub_degs.fill(0);
        self.u_ch.fill(0);

        // generate a new random subgraph
        let mut j = self.rand_perm.len();
        for k in 0..self.v_sub {
            let index = rng.below(j as u32) as usize;
            let vert = self.rand_perm[index];
            j -= 1;
            self.rand_perm[index] = self.rand_perm[j];
            self.rand_perm[j] = vert;
            self.sub_ch[vert] = 1;
            self.sub_verts[k] = vert;
            self.vert_inds[vert] = k;
        }

        // vertex degrees within the subgraph; list the outside vertices
        let mut l = 0;
        for j in 0..v {
            if self.sub_ch[j] != 0 {
                for &x in graph.neighbors(j) {
                    self.sub_degs[x as usize] += graph.neighbor_value(j, x as usize) as i32;
                }
            } else {
                self.vert_inds[j] = l;
                self.rest_verts[l] = j;
                l += 1;
            }
        }

        if params.kick_size > 0 {
            self.banned_until.fill(0);
            self.kick_queue.clear();
        }
        if self.tenure > 1 {
            self.tabu_in.fill(0);
            self.tabu_out.fill(0);
        }
        self.buckets.reset();

        (0..v)
            .filter(|&j| self.sub_ch[j] != 0)
            .map(|j| i64::from(self.sub_degs[j]))
            .sum::<i64>()
            / 2
            - params.edges
    }

    /// Chooses the swap for move `j`: a region kick's next move, a random (diversifying) move, an
    /// aspiration move, or one of the `opt_mov` best moves.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn choose_move<G: SearchGraph, R: SearchRng>(
        &mut self,
        graph: &G,
        params: &SearchParams,
        j: usize,
        new_min_val: i64,
        opt_mov: usize,
        asp_mov: usize,
        rng: &mut R,
    ) -> (usize, usize) {
        let kick = params.kick_size > 0;
        // with kicks on, a region kick starts each diversification cycle after the first, whose
        // random moves are kept (so a search shorter than div_freq moves is the same either way)
        if kick && j > 0 && params.div_dur > 0 && j.is_multiple_of(params.div_freq) {
            self.kick_queue = self.region(graph, params.kick_size, rng);
        }
        while let Some(s) = self.kick_queue.pop() {
            if self.sub_ch[s] != 0 {
                if let Some(r) = self.best_swap_in(graph, new_min_val, s, j, rng) {
                    self.banned_until[s] = j + params.kick_ban;
                    return (s, r);
                }
                break;
            }
        }
        if ((!kick || j < params.div_freq) && j % params.div_freq < params.div_dur) || opt_mov == 0
        {
            let s = self.sub_verts[rng.below(self.v_sub as u32) as usize];
            let r = self.rest_verts[rng.below(self.v_rest as u32) as usize];
            (s, r)
        } else if asp_mov > 0 {
            let index = rng.below(asp_mov as u32) as usize;
            (self.sub_asp[index], self.rest_asp[index])
        } else {
            let index = rng.below(opt_mov as u32) as usize;
            (self.sub_opt[index], self.rest_opt[index])
        }
    }

    /// Swaps `sub_vert` out of the subgraph and `rest_vert` in at move `j`: degrees, the marks of
    /// vertices adjacent to exactly one of the two, membership, tabu stamps and the vertex lists.
    /// Returns the change in the edge count (or weight).
    #[inline]
    fn apply_swap<G: SearchGraph>(
        &mut self,
        graph: &G,
        sub_vert: usize,
        rest_vert: usize,
        j: usize,
    ) -> i64 {
        let sub_row = self.vert_inds[sub_vert];
        let rest_col = self.vert_inds[rest_vert];
        let change = i64::from(self.sub_degs[rest_vert])
            - i64::from(self.sub_degs[sub_vert])
            - graph.edge_value(sub_vert, rest_vert);

        // update subgraph degrees; mark vertices adjacent to exactly one of the two swapped
        for &x in graph.neighbors(sub_vert) {
            let x = x as usize;
            self.sub_degs[x] -= graph.neighbor_value(sub_vert, x) as i32;
            self.u_ch[x] = 1;
        }
        for &x in graph.neighbors(rest_vert) {
            let x = x as usize;
            self.sub_degs[x] += graph.neighbor_value(rest_vert, x) as i32;
            self.u_ch[x] = 1 - self.u_ch[x];
        }

        self.sub_ch[sub_vert] = 0;
        self.sub_ch[rest_vert] = 1;
        if self.tenure > 1 {
            self.tabu_in[sub_vert] = j + self.tenure;
            self.tabu_out[rest_vert] = j + self.tenure;
        }

        self.sub_verts[sub_row] = rest_vert;
        self.vert_inds[rest_vert] = sub_row;
        self.rest_verts[rest_col] = sub_vert;
        self.vert_inds[sub_vert] = rest_col;
        change
    }

    /// Lists the candidate columns for the next move in `u_list`: the active ones (adjacent to
    /// exactly one of the two vertices just swapped) first, then the others, leaving out the vertex
    /// just swapped out (at `rest_col`) and any a region kick or the tabu tenure bans, and notes
    /// the range of their subgraph degrees. The inactive ones fill the buffer from its end, then
    /// move up to follow the active ones, in the same order as if they had been counted first.
    #[inline]
    fn list_candidates(
        &mut self,
        params: &SearchParams,
        j: usize,
        sub_vert: usize,
        rest_col: usize,
    ) -> Columns {
        let (kick, tenure) = (params.kick_size > 0, self.tenure);
        let v_rest = self.v_rest;
        let mut cols = Columns {
            active: 0,
            all: 0,
            active_range: (i64::MAX, i64::MIN),
            all_range: (i64::MAX, i64::MIN),
        };
        self.u_ch[sub_vert] = 0;
        let mut temp = v_rest;
        for k in 0..v_rest {
            let vert = self.rest_verts[k];
            if k == rest_col
                || (kick && self.banned_until[vert] > j)
                || (tenure > 1 && self.tabu_in[vert] > j)
            {
                if tenure > 1 {
                    self.u_ch[vert] = 0;
                }
                continue;
            }
            let d = i64::from(self.sub_degs[vert]);
            cols.all_range = (cols.all_range.0.min(d), cols.all_range.1.max(d));
            if self.u_ch[vert] != 0 {
                self.u_list[cols.active] = vert;
                cols.active += 1;
                self.u_ch[vert] = 0;
                cols.active_range = (cols.active_range.0.min(d), cols.active_range.1.max(d));
            } else {
                temp -= 1;
                self.u_list[temp] = vert;
            }
        }
        cols.all = cols.active + (v_rest - temp);
        self.u_list.copy_within(temp..v_rest, cols.active);
        cols
    }

    /// Lists in `rows` the subgraph vertices whose swaps the next move may make, other than the
    /// one just swapped in and any the tabu tenure holds: each with its candidate columns (all of
    /// them for rows marked for a full scan, else the active ones), the constant of its swaps'
    /// values, and a lower bound on its best value. Swapping `vert` for `t` has value
    /// |degree - w + d|, with d = sub_degs[t] in the columns' range and w, the edge's value, from 0
    /// to the row's largest.
    #[inline]
    fn bound_rows<G: SearchGraph>(
        &mut self,
        graph: &G,
        new_min_val: i64,
        j: usize,
        rest_vert: usize,
        cols: &Columns,
    ) {
        self.u_ch[rest_vert] = 0;
        self.rows.clear();
        for k in 0..self.v_sub {
            let vert = self.sub_verts[k];
            if vert == rest_vert {
                continue;
            }
            if self.tenure > 1 && self.tabu_out[vert] > j {
                self.u_ch[vert] = 0;
                continue;
            }
            let w_cols = if self.u_ch[vert] != 0 {
                self.u_ch[vert] = 0;
                cols.all
            } else {
                cols.active
            };
            if w_cols == 0 {
                continue;
            }
            let (lo, hi) = cols.range(w_cols);
            let degree = new_min_val - i64::from(self.sub_degs[vert]);
            let (low, high) = (degree + lo - graph.max_row_value(vert), degree + hi);
            let bound = if high < 0 {
                -high
            } else if low > 0 {
                low
            } else {
                0
            };
            #[cfg(test)]
            let bound = if NO_ROW_SKIPPING.load(std::sync::atomic::Ordering::Relaxed) {
                0
            } else {
                bound
            };
            self.rows.push((vert, w_cols, degree, bound));
        }
    }

    /// Collects the optimal moves (at most v of them) into `sub_opt`/`rest_opt` and returns how
    /// many: exactly the ones a scan of every row in order would. It first finds the best value,
    /// scanning the most promising row first and skipping rows that cannot beat it, then lists the
    /// moves that reach it, in row order. When the columns' degrees span a narrow enough range,
    /// they are bucketed by degree, and each row visits only the buckets that can hold its best
    /// moves.
    #[inline]
    fn collect_best_moves<G: SearchGraph>(&mut self, graph: &G, j: usize, cols: &Columns) -> usize {
        let m = cols.all;
        let bucketed = m > 0
            && j >= self.buckets.paused_until
            && self.buckets.build(
                &self.u_list[..m],
                &self.sub_degs,
                cols.all_range.0,
                cols.all_range.1,
            );
        #[cfg(test)]
        let bucketed = bucketed && !NO_ROW_SKIPPING.load(std::sync::atomic::Ordering::Relaxed);

        let mut opt_val = self.e;
        if let Some(first) = (0..self.rows.len()).min_by_key(|&i| self.rows[i].3) {
            for i in std::iter::once(first).chain((0..self.rows.len()).filter(|&i| i != first)) {
                let (vert, w_cols, degree, bound) = self.rows[i];
                if bound < opt_val {
                    let row = graph.adjacency_row(vert);
                    opt_val = if bucketed {
                        self.buckets.row_min(
                            row,
                            &self.u_list,
                            w_cols,
                            cols.range(w_cols),
                            degree,
                            graph.max_row_value(vert),
                            bound,
                            opt_val,
                        )
                    } else {
                        row_min(
                            row,
                            &self.u_list[..w_cols],
                            &self.sub_degs,
                            degree,
                            bound,
                            opt_val,
                        )
                    };
                }
            }
        }

        let mut opt_mov = 0;
        for i in 0..self.rows.len() {
            let (vert, w_cols, degree, bound) = self.rows[i];
            if bound > opt_val {
                continue;
            }
            let row = graph.adjacency_row(vert);
            let collected = bucketed
                && self.buckets.collect_row(
                    row,
                    &self.u_list,
                    w_cols,
                    cols.range(w_cols),
                    degree,
                    graph.max_row_value(vert),
                    opt_val,
                    vert,
                    &mut opt_mov,
                    &mut self.sub_opt,
                    &mut self.rest_opt,
                );
            if !collected {
                scan_row(
                    row,
                    &self.u_list[..w_cols],
                    &self.sub_degs,
                    degree,
                    vert,
                    opt_val,
                    &mut opt_mov,
                    &mut self.sub_opt,
                    &mut self.rest_opt,
                );
            }
        }
        if bucketed {
            self.buckets.review(j);
        }
        opt_mov
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cycle(n: usize) -> Graph {
        Graph::from_edges(n, (0..n).map(|i| (i, (i + 1) % n))).unwrap()
    }

    #[test]
    fn finds_induced_paths_in_a_cycle() {
        let g = cycle(30);
        let mut p = SearchParams::new(5, 4);
        p.experiments = 8;
        p.max_moves = 500;
        p.div_freq = 50;
        for rng in [RngKind::Xoshiro, RngKind::Legacy] {
            p.rng = rng;
            let r = search(&g, &p).unwrap();
            assert_eq!(r.successes(), 8, "{rng:?}");
            for e in &r.experiments {
                assert_eq!(e.vertices.len(), 5);
                assert_eq!(g.induced_edge_count(&e.vertices), 4);
            }
        }
    }

    #[test]
    fn keeps_fixed_vertices() {
        let g = cycle(20);
        let mut p = SearchParams::new(4, 0);
        p.fixed = vec![3, 11];
        let r = search(&g, &p).unwrap();
        for e in r.experiments.iter().filter(|e| e.success) {
            assert!(e.vertices.contains(&3) && e.vertices.contains(&11));
            assert_eq!(g.induced_edge_count(&e.vertices), 0);
        }
        assert!(r.successes() > 0);
    }

    #[test]
    fn settings_resolve_by_kind_of_search() {
        assert_eq!(
            search_settings(None, None, false, false).unwrap(),
            (DEFAULT_KICK_SIZE, AUTO_TABU_TENURE)
        );
        assert_eq!(search_settings(None, None, true, false).unwrap(), (0, 1));
        assert_eq!(search_settings(None, None, false, true).unwrap(), (0, 1));
        assert_eq!(
            search_settings(Some(16), Some(7), false, true).unwrap(),
            (16, 7)
        );
        assert_eq!(
            search_settings(Some(0), Some(1), true, false).unwrap(),
            (0, 1)
        );
        assert!(search_settings(Some(4), None, true, false).is_err());
        assert!(search_settings(None, Some(AUTO_TABU_TENURE), true, false).is_err());
        assert!(search_settings(None, Some(3), true, false).is_err());
    }

    #[test]
    fn tabu_tenure_defaults_and_resolution() {
        assert_eq!(SearchParams::new(30, 0).tabu_tenure, AUTO_TABU_TENURE);
        // a tenth of the size or an eightieth of the rest, whichever is smaller, from 1 to 20
        assert_eq!(SearchParams::new(200, 0).resolved_tabu_tenure(2000), 20);
        assert_eq!(
            SearchParams::new(1080, 0).resolved_tabu_tenure(85320),
            MAX_AUTO_TABU_TENURE
        );
        assert_eq!(SearchParams::new(200, 0).resolved_tabu_tenure(500), 3);
        assert_eq!(SearchParams::new(72, 0).resolved_tabu_tenure(2048), 7);
        assert_eq!(SearchParams::new(15, 0).resolved_tabu_tenure(2000), 1);
        assert_eq!(SearchParams::new(30, 0).resolved_tabu_tenure(100), 1);
        let mut p = SearchParams::new(45, 0);
        p.tabu_tenure = 7;
        assert_eq!(p.resolved_tabu_tenure(2000), 7);
        // results saved before the tenure existed used 1
        assert_eq!(default_tabu_tenure(), 1);
    }

    #[test]
    fn automatic_tenure_is_one_for_small_subgraphs() {
        let g = cycle(60);
        let mut p = SearchParams::new(12, 4);
        p.experiments = 8;
        p.max_moves = 600;
        let auto = search(&g, &p).unwrap().experiments;
        p.tabu_tenure = 1;
        assert_eq!(auto, search(&g, &p).unwrap().experiments);
    }

    #[test]
    fn long_tenures_still_find_subgraphs() {
        let g = cycle(120);
        for tenure in [AUTO_TABU_TENURE, 2, 8] {
            let mut p = SearchParams::new(40, 20);
            p.experiments = 8;
            p.max_moves = 3000;
            p.tabu_tenure = tenure;
            let r = search(&g, &p).unwrap();
            assert!(r.successes() > 0, "tenure {tenure}");
            for e in r.experiments.iter().filter(|e| e.success) {
                assert_eq!(g.induced_edge_count(&e.vertices), 20);
            }
        }
    }

    #[test]
    fn parallel_and_sequential_xoshiro_agree() {
        let g = cycle(40);
        let mut p = SearchParams::new(7, 3);
        p.experiments = 16;
        p.record_trace = true;
        let parallel = search(&g, &p).unwrap().experiments;
        let mut sequential = Vec::new();
        search_sequential(&g, &p, |e| sequential.push(e.clone())).unwrap();
        assert_eq!(parallel, sequential);
        assert!(
            parallel
                .iter()
                .all(|e| e.trace.as_ref().unwrap().len() == e.moves + usize::from(e.success))
        );
    }

    #[test]
    fn skipping_rows_changes_no_result() {
        // every search makes the same moves whether or not it skips the rows the bounds rule out
        // (other tests running meanwhile may also scan every row, which changes nothing either)
        let n = 150;
        let pairs = (0..n).flat_map(|i| (i + 1..n).map(move |j| (i, j)));
        let g = Graph::from_edges(n, pairs.clone().filter(|&(i, j)| (i * 31 + j * 17) % 5 < 2))
            .unwrap();
        let w = WeightedGraph::from_weighted_edges(
            n,
            pairs
                .filter(|&(i, j)| (i * 13 + j * 7) % 3 != 0)
                .map(|(i, j)| (i, j, ((i * j) % 9 + 1) as u32)),
        )
        .unwrap();
        for case in [
            (
                20,
                1_000_000,
                DEFAULT_KICK_SIZE,
                AUTO_TABU_TENURE,
                RngKind::Xoshiro,
            ), // densest
            (20, 0, DEFAULT_KICK_SIZE, AUTO_TABU_TENURE, RngKind::Xoshiro), // sparsest
            (30, 180, 0, 1, RngKind::Xoshiro),
            (30, 180, 16, 5, RngKind::Xoshiro),
            (40, 560, DEFAULT_KICK_SIZE, 8, RngKind::Xoshiro),
            (40, 600, 0, 1, RngKind::Legacy),
        ]
        .into_iter()
        .flat_map(|case| [(case, false), (case, true)])
        {
            let ((size, edges, kick, tenure, rng), full_aspiration) = case;
            let mut p = SearchParams::new(size, edges);
            p.full_aspiration = full_aspiration;
            p.experiments = 8;
            p.max_moves = 1500;
            p.div_freq = 300;
            p.div_dur = 8;
            p.kick_size = kick;
            p.tabu_tenure = tenure;
            p.rng = rng;
            p.record_trace = true;
            let run = || {
                (
                    search(&g, &p).unwrap().experiments,
                    search(&w, &p).unwrap().experiments,
                )
            };
            let skipping = run();
            NO_ROW_SKIPPING.store(true, std::sync::atomic::Ordering::Relaxed);
            let full = run();
            NO_ROW_SKIPPING.store(false, std::sync::atomic::Ordering::Relaxed);
            assert_eq!(
                skipping, full,
                "size {size}, edges {edges}, full aspiration {full_aspiration}"
            );
        }
    }

    #[test]
    fn densest_ties_go_to_the_best_exact_score() {
        // with resolution 1 every positive score becomes weight 1, so every edge ties; the
        // exact scores tell {2, 3} apart as the heaviest pair and {0, 1} as the lightest
        let scores = [(0, 1, 1.0), (2, 3, 2.0), (0, 2, 1.5), (1, 3, 1.25)];
        let (w, _) = WeightedGraph::from_scores(4, scores, 1).unwrap();
        let mut p = SearchParams::new(2, 1);
        p.experiments = 20;
        let result = search(&w, &p).unwrap();
        assert!(result.experiments.iter().all(|e| e.success));
        let pick = |highest| {
            result.experiments[result.best_by_score(&w, highest).unwrap()]
                .vertices
                .clone()
        };
        assert_eq!((pick(true), pick(false)), (vec![2, 3], vec![0, 1]));
        // without scores, the earliest of the tied experiments
        let plain =
            WeightedGraph::from_weighted_edges(4, scores.map(|(u, v, _)| (u, v, 1))).unwrap();
        assert_eq!(result.best_by_score(&plain, true), Some(0));
    }

    #[test]
    fn full_aspiration_is_the_original_with_tenure_one_and_differs_with_longer() {
        let n = 120;
        let g = Graph::from_edges(
            n,
            (0..n)
                .flat_map(|i| (i + 1..n).map(move |j| (i, j)))
                .filter(|&(i, j)| (i * 29 + j * 13) % 7 < 3),
        )
        .unwrap();
        let mut p = SearchParams::new(30, 190);
        p.experiments = 8;
        p.max_moves = 2000;
        p.div_freq = 300;
        p.div_dur = 8;
        p.tabu_tenure = 1;
        let run = |p: &SearchParams| search(&g, p).unwrap().experiments;
        let (mut limited, mut full) = (p.clone(), p.clone());
        limited.aspiration = true;
        full.full_aspiration = true;
        // with tenure 1 the only tabu swaps involve the two vertices just swapped
        assert_eq!(run(&limited), run(&full));
        // with a longer tenure it considers more swaps, and every result is still what it reports
        limited.tabu_tenure = 6;
        full.tabu_tenure = 6;
        let result = run(&full);
        assert_ne!(run(&limited), result);
        for e in &result {
            let edges = g.induced_edge_count(&e.vertices) as i64;
            assert_eq!((edges - 190).abs(), e.best_conflict);
        }
        // the degree search has no aspiration
        let mut regular = full.clone();
        regular.regular = Some(Regular {
            degree: 3,
            two_phase: false,
        });
        assert!(search(&g, &regular).is_err());
    }

    #[test]
    fn connected_searches_succeed_only_on_connected_subgraphs() {
        // two triangles and a 6-cycle: 6 vertices spanning 6 edges, all of degree 2, are either
        // both triangles or the 6-cycle, and only the 6-cycle is connected
        let g = Graph::from_edges(
            12,
            [(0, 1), (1, 2), (2, 0), (3, 4), (4, 5), (5, 3)]
                .into_iter()
                .chain((6..12).map(|i| (i, 6 + (i - 5) % 6))),
        )
        .unwrap();
        let hexagon: Vec<usize> = (6..12).collect();
        let regular = |two_phase| {
            Some(Regular {
                degree: 2,
                two_phase,
            })
        };
        for target in [None, regular(false), regular(true)] {
            for rng in [RngKind::Xoshiro, RngKind::Legacy] {
                let mut p = SearchParams::new(6, 6);
                p.regular = target;
                p.rng = rng;
                p.experiments = 30;
                p.max_moves = 500;
                p.div_freq = 50;
                p.record_trace = true;
                let plain = search(&g, &p).unwrap();
                p.connected = true;
                let connected = search(&g, &p).unwrap();
                let label = format!("{target:?}, {rng:?}");
                assert!(connected.successes() > 0, "{label}");
                for e in &connected.experiments {
                    assert!(
                        !e.success || e.vertices == hexagon,
                        "{label}: {:?}",
                        e.vertices
                    );
                    // a two-phase search's trace counts edges, not regularity, in its first phase
                    if !target.is_some_and(|r| r.two_phase) {
                        let lowest = *e.trace.as_ref().unwrap().iter().min().unwrap();
                        assert_eq!(i64::from(lowest), e.best_conflict, "{label}");
                    }
                }
                assert!(
                    plain
                        .experiments
                        .iter()
                        .any(|e| e.success && e.vertices != hexagon),
                    "{label}: the plain search also finds the two triangles"
                );
            }
        }
    }

    #[test]
    fn kicks_favor_no_vertex_among_equally_near_ones() {
        // in a clique every member is at distance 1 from every other, so a kick of 5 should draw
        // members from all over it, not the lowest-numbered ones
        let n = 64;
        let complete =
            Graph::from_edges(n, (0..n).flat_map(|u| (u + 1..n).map(move |v| (u, v)))).unwrap();
        let mut p = SearchParams::new(32, 0);
        p.kick_size = 5;
        let mut ws = Workspace::new(&complete, &p);
        for (k, v) in (0..32).map(|v| v * 2).enumerate() {
            ws.sub_ch[v] = 1;
            ws.sub_verts[k] = v;
        }
        let mut rng = Xoshiro::new(1);
        let mut drawn = vec![0; n];
        for _ in 0..400 {
            let region = ws.region(&complete, p.kick_size, &mut rng);
            assert_eq!(region.len(), 5);
            for v in region {
                assert_eq!(v % 2, 0, "only members of the subgraph");
                drawn[v] += 1;
            }
        }
        // 2000 draws over 32 members: each about 62 times; all of them drawn many times
        assert!((0..32).all(|k| drawn[2 * k] > 20), "{drawn:?}");
    }

    #[test]
    fn region_kicks() {
        // the 6-cube: independent sets of 32 (the even-weight half) and 4-cycles, with kicks on
        let n = 6;
        let cube = Graph::from_edges(
            1 << n,
            (0..1usize << n)
                .flat_map(|u| (0..n).map(move |b| (u, u ^ (1 << b))))
                .filter(|&(u, w)| u < w),
        )
        .unwrap();
        for (size, edges) in [(32, 0), (16, 32), (4, 4)] {
            let mut p = SearchParams::new(size, edges);
            p.experiments = 20;
            p.max_moves = 2000;
            p.div_freq = 100;
            p.div_dur = 4;
            p.kick_size = 6;
            let r = search(&cube, &p).unwrap();
            assert!(r.successes() > 0, "{size} {edges}");
            for e in r.experiments.iter().filter(|e| e.success) {
                assert_eq!(cube.induced_edge_count(&e.vertices), edges as usize);
            }
            // sequential and parallel runs still agree, and fixed vertices stay
            let mut out = Vec::new();
            search_sequential(&cube, &p, |e| out.push(e.clone())).unwrap();
            assert_eq!(out, r.experiments);
            p.fixed = vec![0];
            assert!(
                search(&cube, &p)
                    .unwrap()
                    .experiments
                    .iter()
                    .all(|e| e.vertices.contains(&0))
            );
        }
        // kicks change the search: the same seed gives different experiments
        let mut p = SearchParams::new(32, 0);
        p.experiments = 4;
        p.max_moves = 1000;
        p.div_freq = 10; // kicks from move 10 on, before these searches succeed
        p.kick_size = 0;
        let plain = search(&cube, &p).unwrap();
        p.kick_size = 6;
        assert_ne!(plain.experiments, search(&cube, &p).unwrap().experiments);
        // regular and path searches ignore the kick size
        p.path = true;
        let kicked = search(&cube, &p).unwrap();
        p.kick_size = 0;
        assert_eq!(kicked.experiments, search(&cube, &p).unwrap().experiments);
    }

    #[test]
    fn validates_parameters() {
        let g = cycle(10);
        let mut p = SearchParams::new(10, 3);
        assert!(matches!(
            search(&g, &p),
            Err(Error::InvalidSubgraphSize(10))
        ));
        p.size = 4;
        p.div_freq = 0;
        assert!(matches!(
            search(&g, &p),
            Err(Error::InvalidDiversificationFrequency(0))
        ));
        p.div_freq = 5;
        p.fixed = vec![2, 2];
        assert!(matches!(search(&g, &p), Err(Error::DuplicateVertex(2))));
    }

    #[test]
    fn targets_above_the_total_still_find_the_closest_subgraphs() {
        // a 30-cycle has 30 edges, so 40 among 10 vertices is out of reach: the closest is an
        // induced path with 9, at conflict 31
        let g = cycle(30);
        let mut p = SearchParams::new(10, 40);
        p.experiments = 8;
        let r = search(&g, &p).unwrap();
        let best = r
            .experiments
            .iter()
            .min_by_key(|e| e.best_conflict)
            .unwrap();
        assert_eq!(best.best_conflict, 31);
        assert_eq!(best.vertices.len(), 10);
        assert_eq!(g.induced_edge_count(&best.vertices), 9);
        // the same in a weighted path whose total weight is 25: 3 vertices weigh at most 10
        let w = WeightedGraph::from_weighted_edges(6, (0..5).map(|i| (i, i + 1, 5))).unwrap();
        let mut p = SearchParams::new(3, 100);
        p.experiments = 8;
        let r = search(&w, &p).unwrap();
        let best = r
            .experiments
            .iter()
            .min_by_key(|e| e.best_conflict)
            .unwrap();
        assert_eq!(best.best_conflict, 90);
        assert_eq!(best.vertices.len(), 3);
    }

    #[test]
    fn kicks_start_after_the_first_cycle() {
        assert_eq!(SearchParams::new(4, 4).kick_size, DEFAULT_KICK_SIZE);
        // within the first div_freq moves a search with kicks is the same as one without
        let n = 6;
        let cube = Graph::from_edges(
            1 << n,
            (0..1usize << n)
                .flat_map(|u| (0..n).map(move |b| (u, u ^ (1 << b))))
                .filter(|&(u, w)| u < w),
        )
        .unwrap();
        let mut p = SearchParams::new(16, 32);
        p.experiments = 20;
        p.max_moves = 100;
        p.div_freq = 100;
        p.kick_size = 0;
        let plain = search(&cube, &p).unwrap();
        p.kick_size = 6;
        assert_eq!(search(&cube, &p).unwrap().experiments, plain.experiments);
        // and regular and path searches ignore the kick size
        let mut p = SearchParams::new(10, 0);
        p.path = true;
        p.kick_size = 4;
        assert!(search(&cycle(30), &p).unwrap().successes() > 0);
    }
}
