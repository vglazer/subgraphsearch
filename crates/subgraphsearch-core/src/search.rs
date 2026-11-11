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
//! - The two vertices just swapped are *tabu* for one move (a tabu tenure of 1): the vertex just
//!   added cannot be removed and the vertex just removed cannot be added back.
//! - For the first `div_dur` moves of every `div_freq` (and whenever no optimal move is known),
//!   a uniformly random swap is made instead ("diversification").
//!
//! - With [`SearchParams::aspiration`] (the original `asub_search`), an *aspiration criterion*
//!   lifts the tabu: the swaps involving the two vertices just swapped are also examined after
//!   each move, and one is taken whenever it would beat the best value so far.
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
}

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
    /// keeps its random moves. [`SearchParams::new`] sets [`DEFAULT_KICK_SIZE`], which helped or did
    /// no harm on every problem tried; larger sizes can help much more on some problems and hurt on
    /// others, so compare a few, as with `div_dur`. 0 keeps the random moves throughout, and is what
    /// results saved before kicks existed used. Edge-count and weight searches only: regular and
    /// path searches ignore it. To reproduce the original programs' runs with
    /// [`RngKind::Legacy`], set it to 0.
    #[serde(default)]
    pub kick_size: usize,
    /// How many moves a vertex removed by a region kick stays out (see `kick_size`).
    #[serde(default = "default_kick_ban")]
    pub kick_ban: usize,
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

/// The region kick size [`SearchParams::new`] sets: small kicks helped or did no harm on every
/// problem tried.
pub const DEFAULT_KICK_SIZE: usize = 4;

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
            seed: 1,
            rng: RngKind::Xoshiro,
            record_trace: false,
            aspiration: false,
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
        return crate::regular::search(unweighted(graph)?, params);
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
    Ok(SearchResult {
        params: params.clone(),
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

/// Evaluates swapping `vert` out for each of `cols` in, appending the best swaps to
/// `sub_opt`/`rest_opt` (whose length caps how many are kept, as in `sub_search.c`). This is the
/// search's hot loop; taking separate slices lets the compiler keep everything in registers.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn scan_row<R: AdjacencyRow>(
    row: R,
    cols: &[usize],
    sub_degs: &[i32],
    degree: i64,
    vert: usize,
    opt_val: &mut i64,
    opt_mov: &mut usize,
    sub_opt: &mut [usize],
    rest_opt: &mut [usize],
) {
    let cap = sub_opt.len().min(rest_opt.len());
    let (mut best, mut count) = (*opt_val, *opt_mov);
    for &t in cols {
        let val = (degree - row.value(t) + i64::from(sub_degs[t])).abs();
        if val <= best {
            if val < best {
                best = val;
                count = 0;
            }
            if count < cap {
                sub_opt[count] = vert;
                rest_opt[count] = t;
                count += 1;
            }
        }
    }
    *opt_val = best;
    *opt_mov = count;
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
            sub_asp: if params.aspiration {
                vec![0; v]
            } else {
                Vec::new()
            },
            rest_asp: if params.aspiration {
                vec![0; v]
            } else {
                Vec::new()
            },
            rand_perm: Vec::with_capacity(v - v_fix),
            components: params.connected.then(|| Components::new(v)),
            banned_until: Vec::new(),
            fixed: Vec::new(),
            kick_queue: Vec::new(),
        };
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
    /// drawn in exactly the same order.
    fn run<G: SearchGraph, R: SearchRng>(
        &mut self,
        graph: &G,
        params: &SearchParams,
        rng: &mut R,
    ) -> Experiment {
        let v = graph.num_vertices();
        let (v_sub, v_rest, e) = (self.v_sub, self.v_rest, self.e);
        let mut trace = params
            .record_trace
            .then(|| Vec::with_capacity(params.max_moves));

        self.sub_degs.fill(0);
        self.u_ch.fill(0);

        // generate a new random subgraph
        let mut j = self.rand_perm.len();
        for k in 0..v_sub {
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

        // signed difference between the subgraph's edge count and the target
        let mut new_min_val: i64 = (0..v)
            .filter(|&j| self.sub_ch[j] != 0)
            .map(|j| i64::from(self.sub_degs[j]))
            .sum::<i64>()
            / 2
            - params.edges;

        let mut g_opt_val = e;
        let mut old_min_val = e;
        let mut opt_mov = 0usize;
        let mut asp_mov = 0usize;
        let mut success = false;
        let kick = params.kick_size > 0;
        if kick {
            self.banned_until.fill(0);
            self.kick_queue.clear();
        }
        let mut j = 0;
        while j < params.max_moves {
            // with kicks on, a region kick starts each diversification cycle after the first, whose
            // random moves are kept (so a search shorter than div_freq moves is the same either way)
            if kick && j > 0 && params.div_dur > 0 && j % params.div_freq == 0 {
                self.kick_queue = self.region(graph, params.kick_size, rng);
            }
            let mut kicked = None;
            while let Some(s) = self.kick_queue.pop() {
                if self.sub_ch[s] != 0 {
                    if let Some(r) = self.best_swap_in(graph, new_min_val, s, j, rng) {
                        self.banned_until[s] = j + params.kick_ban;
                        kicked = Some((s, r));
                    }
                    break;
                }
            }
            // choose a kick move, a random (diversifying) move, an aspiration move, or a locally
            // optimal one
            let (sub_vert, rest_vert) = if let Some(swap) = kicked {
                swap
            } else if ((!kick || j < params.div_freq) && j % params.div_freq < params.div_dur)
                || opt_mov == 0
            {
                let s = self.sub_verts[rng.below(v_sub as u32) as usize];
                let r = self.rest_verts[rng.below(v_rest as u32) as usize];
                (s, r)
            } else if asp_mov > 0 {
                let index = rng.below(asp_mov as u32) as usize;
                (self.sub_asp[index], self.rest_asp[index])
            } else {
                let index = rng.below(opt_mov as u32) as usize;
                (self.sub_opt[index], self.rest_opt[index])
            };
            let sub_row = self.vert_inds[sub_vert];
            let rest_col = self.vert_inds[rest_vert];

            new_min_val += i64::from(self.sub_degs[rest_vert])
                - i64::from(self.sub_degs[sub_vert])
                - graph.edge_value(sub_vert, rest_vert);
            let mut obj_val = new_min_val.abs();

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

            self.sub_verts[sub_row] = rest_vert;
            self.vert_inds[rest_vert] = sub_row;
            self.rest_verts[rest_col] = sub_vert;
            self.vert_inds[sub_vert] = rest_col;

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

            // list candidate columns: active ones from the front, inactive ones from the back (and,
            // with a region kick, none that are banned)
            let mut u_cols = 0;
            self.u_ch[sub_vert] = 0;
            let banned = |w: usize| kick && self.banned_until[w] > j;
            let m = if kick {
                (0..v_rest)
                    .filter(|&k| k != rest_col && !banned(self.rest_verts[k]))
                    .count()
            } else {
                v_rest - 1
            };
            let mut temp = m;
            for k in 0..v_rest {
                if k == rest_col || banned(self.rest_verts[k]) {
                    continue;
                }
                let vert = self.rest_verts[k];
                if self.u_ch[vert] != 0 {
                    self.u_list[u_cols] = vert;
                    u_cols += 1;
                    self.u_ch[vert] = 0;
                } else {
                    temp -= 1;
                    self.u_list[temp] = vert;
                }
            }

            // if the edge count changed, every row gets a full scan
            if new_min_val != old_min_val {
                for k in 0..v_sub {
                    self.u_ch[self.sub_verts[k]] = 1;
                }
            }

            // collect the optimal moves (at most v of them)
            let mut opt_val = e;
            opt_mov = 0;
            self.u_ch[rest_vert] = 0;
            for k in 0..v_sub {
                let vert = self.sub_verts[k];
                if vert == rest_vert {
                    continue;
                }
                let w_cols = if self.u_ch[vert] != 0 {
                    self.u_ch[vert] = 0;
                    m
                } else {
                    u_cols
                };
                let degree = new_min_val - i64::from(self.sub_degs[vert]);
                scan_row(
                    graph.adjacency_row(vert),
                    &self.u_list[..w_cols],
                    &self.sub_degs,
                    degree,
                    vert,
                    &mut opt_val,
                    &mut opt_mov,
                    &mut self.sub_opt,
                    &mut self.rest_opt,
                );
            }

            if params.aspiration {
                asp_mov = self.aspiration_moves(graph, new_min_val, g_opt_val, sub_vert, rest_vert);
            }

            for k in 0..u_cols {
                self.u_ch[self.u_list[k]] = 0;
            }
            old_min_val = new_min_val;
            j += 1;
        }

        let vertices = (0..v).filter(|&k| self.opt_ch[k] != 0).collect();

        // return the free subgraph vertices to the outside for the next experiment
        for k in 0..v_sub {
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
