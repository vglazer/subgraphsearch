//! Regular induced subgraphs: ports of the original `reg_search` and `ereg_search`.
//!
//! Instead of an edge count, the search minimizes the total deviation from regularity,
//! `sum over subgraph vertices of |degree within the subgraph - degree|`, which is zero exactly
//! when the subgraph is regular of the requested degree. Moves and diversification work as in
//! [`crate::search`](mod@crate::search). To evaluate swaps quickly, every vertex's neighbor list is kept
//! partitioned, neighbors inside the subgraph first, and an `n x n` matrix holds each
//! neighbor's position in that list (or -1 for non-neighbors); both are updated every move.
//!
//! The two-phase variant (`ereg_search`) first searches for the right number of edges with the
//! `sub_search` objective and only then switches to regularity, making only moves that keep
//! the edge count. Two quirks of the original are kept so that results match exactly: the
//! phase-one edge target is `(size - fixed) * degree / 2` rather than `size * degree / 2`, and
//! the partitioned lists carry over from one experiment to the next, which affects only the
//! order in which [`crate::legacy`] prints a solution's adjacency lists.
//!
//! A third quirk is kept only in [`crate::legacy`]'s output: `ereg_search` reports, for an
//! experiment that never reaches the second phase, how far its edge count was from the target, and
//! for one whose second phase stalls at once, `size * size`. Neither is the conflict of the subgraph
//! the experiment returns, and the first is not even a deviation from regularity, so everywhere
//! else a failed two-phase experiment's best conflict is recomputed from its subgraph.
//!
//! A path search ([`SearchParams::path`]) is a one-phase search for degree 2 that aims at a
//! deviation of 2 instead of 0: a path's two endpoints have degree 1 and every other vertex
//! degree 2. Each swap is valued by how far the deviation after it would be from 2 plus how far
//! the edge count would be from `size - 1`, so the search is drawn to paths rather than cycles or
//! paths that loop back into themselves, which have the right deviation but an edge too many. Its
//! conflict adds the components beyond the first, and is 0 exactly for an induced path: a connected
//! graph with `size - 1` edges is a tree, and a tree whose deviation from degree 2 is 2 has two
//! leaves. Components are not part of a swap's value, since counting them for every swap would
//! cost too much.
//!
//! A path search between two given vertices ([`SearchParams::endpoints`]) searches a graph with a
//! new vertex attached to each of them for a path two vertices longer, with both new vertices and
//! both endpoints fixed. A vertex with one neighbor can only be an end of an induced path, so an
//! exact solution is a path from one new vertex to the other through the two endpoints, and
//! leaving the new vertices out gives a path between the endpoints. Conflicts are those of the
//! longer path, so a near miss is as far from a path between the endpoints as from a path.

use rayon::prelude::*;

use crate::Error;
use crate::graph::Graph;
use crate::rng::{Randu, RngKind, SearchRng, Xoshiro};
use crate::search::{Components, Experiment, SearchParams, SearchResult};

/// The most vertices a regular search supports: like the C programs, it stores neighbor
/// positions in 16-bit integers.
pub const MAX_VERTICES: usize = i16::MAX as usize;

/// The most vertices for which a search keeps neighbor positions in a `v * v` table, as the C
/// programs do: 8 MB per thread at most. Larger graphs keep one per edge end, which is slower to
/// look up but takes memory in proportion to the number of edges.
const DENSE_POSITIONS: usize = 2048;

/// A `v * v` position table's entry for a pair of vertices that is not an edge.
const NOT_AN_EDGE: u16 = u16::MAX;

#[cfg(test)]
thread_local! {
    /// Makes searches on this thread keep positions per edge end whatever the graph's size, so
    /// that tests can check both ways give the same experiments.
    static PER_EDGE_POSITIONS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether a search on `v` vertices keeps neighbor positions in a `v * v` table.
fn dense_positions(v: usize) -> bool {
    #[cfg(test)]
    if PER_EDGE_POSITIONS.with(|f| f.get()) {
        return false;
    }
    v <= DENSE_POSITIONS
}

/// For a path search with [`SearchParams::endpoints`] `[s, t]`: the graph with two new vertices,
/// adjacent to `s` and to `t` alone, and parameters for a path two vertices longer that fix both
/// new vertices as well as `s` and `t`. A vertex with one neighbor can only be an end of an
/// induced path, so the paths found run from one new vertex through `s`, ..., `t` to the other;
/// [`without_pendants`] then leaves the new vertices out. `None` without endpoints.
fn with_pendants(
    graph: &Graph,
    params: &SearchParams,
) -> Result<Option<(Graph, SearchParams)>, Error> {
    let Some([s, t]) = params.endpoints else {
        return Ok(None);
    };
    let n = graph.num_vertices();
    let edges = graph.edges().chain([(n, s), (n + 1, t)]);
    let pendants = Graph::from_edges(n + 2, edges)?;
    let mut p = params.clone();
    p.size += 2;
    p.endpoints = None;
    for v in [s, t, n, n + 1] {
        if !p.fixed.contains(&v) {
            p.fixed.push(v);
        }
    }
    Ok(Some((pendants, p)))
}

/// An experiment of a search made by [`with_pendants`], without the two new vertices.
fn without_pendants(e: &mut Experiment, n: usize) {
    e.vertices.retain(|&v| v < n);
}

/// Runs a regular search; see [`crate::search::search`] for how experiments are scheduled.
pub(crate) fn search(graph: &Graph, params: &SearchParams) -> Result<SearchResult, Error> {
    validate(graph, params)?;
    if let Some((pendants, p)) = with_pendants(graph, params)? {
        let mut result = search(&pendants, &p)?;
        for e in &mut result.experiments {
            without_pendants(e, graph.num_vertices());
        }
        result.params = params.clone();
        return Ok(result);
    }
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
                    ws.reset(graph, params);
                    let mut rng = Xoshiro::for_stream(params.seed as u64, i as u64);
                    let mut e = ws.run(graph, params, &mut rng);
                    recompute_conflict(graph, params, &mut e);
                    e
                },
            )
            .collect(),
    };
    Ok(SearchResult {
        params: params.clone(),
        experiments,
    })
}

/// Runs the experiments one at a time, calling `on_experiment` with each result.
pub(crate) fn search_sequential<F>(
    graph: &Graph,
    params: &SearchParams,
    mut on_experiment: F,
) -> Result<(), Error>
where
    F: FnMut(&Experiment),
{
    validate(graph, params)?;
    if let Some((pendants, p)) = with_pendants(graph, params)? {
        let n = graph.num_vertices();
        // a path search's conflicts need no recomputing
        return search_sequential_as_c(&pendants, &p, |e, _| {
            let mut e = e.clone();
            without_pendants(&mut e, n);
            on_experiment(&e)
        });
    }
    search_sequential_as_c(graph, params, |e, _| {
        let mut e = e.clone();
        recompute_conflict(graph, params, &mut e);
        on_experiment(&e)
    })
}

/// A failed two-phase experiment's best conflict, recomputed from the subgraph it returns: its
/// deviation from regularity, plus its components beyond the first with `params.connected`. See
/// the module documentation. Other experiments already report that.
fn recompute_conflict(graph: &Graph, params: &SearchParams, e: &mut Experiment) {
    let Some(regular) = params.regular else {
        return;
    };
    // with no moves at all, an experiment returns no subgraph
    if !regular.two_phase || e.success || e.vertices.is_empty() {
        return;
    }
    let mut in_sub = vec![0u8; graph.num_vertices()];
    for &v in &e.vertices {
        in_sub[v] = 1;
    }
    let mut conflict: i64 = e
        .vertices
        .iter()
        .map(|&v| {
            let within = graph
                .neighbors(v)
                .iter()
                .filter(|&&x| in_sub[x as usize] != 0)
                .count() as i64;
            (within - regular.degree).abs()
        })
        .sum();
    if params.connected {
        let mut components = Components::new(graph.num_vertices());
        conflict += components.extra(e.vertices.iter().copied(), &in_sub, |u| graph.neighbors(u));
    }
    e.best_conflict = conflict;
}

/// Runs the experiments one at a time, calling `on_experiment` with each result, its best conflict
/// as `ereg_search` reports it, and with the current (partitioned) neighbor lists, which the legacy
/// output prints from.
pub(crate) fn search_sequential_as_c<F>(
    graph: &Graph,
    params: &SearchParams,
    mut on_experiment: F,
) -> Result<(), Error>
where
    F: FnMut(&Experiment, &[Vec<u32>]),
{
    validate(graph, params)?;
    if params.endpoints.is_some() {
        return Err(Error::Invalid(
            "endpoints are not available with the original programs' output".into(),
        ));
    }
    let mut ws = Workspace::new(graph, params);
    match params.rng {
        RngKind::Legacy => {
            let mut rng = Randu::new(params.seed);
            for _ in 0..params.experiments {
                let e = ws.run(graph, params, &mut rng);
                on_experiment(&e, &ws.adj_list);
            }
        }
        RngKind::Xoshiro => {
            for i in 0..params.experiments {
                ws.reset(graph, params);
                let mut rng = Xoshiro::for_stream(params.seed as u64, i as u64);
                let e = ws.run(graph, params, &mut rng);
                on_experiment(&e, &ws.adj_list);
            }
        }
    }
    Ok(())
}

fn validate(graph: &Graph, params: &SearchParams) -> Result<(), Error> {
    params.validate(graph.num_vertices())?;
    if graph.num_vertices() > MAX_VERTICES {
        return Err(Error::Invalid(format!(
            "regular searches support at most {MAX_VERTICES} vertices, got {}",
            graph.num_vertices()
        )));
    }
    if params.aspiration {
        return Err(Error::Invalid(
            "aspiration is not available for regular or path searches".into(),
        ));
    }
    if params.kick_radius > 0 {
        return Err(Error::Invalid(
            "region kicks are available for edge-count searches only, not regular or path searches"
                .into(),
        ));
    }
    if params.path && params.regular.is_some() {
        return Err(Error::Invalid(
            "a path search has no degree: search for a path or for a regular subgraph".into(),
        ));
    }
    Ok(())
}

#[inline]
fn abs(x: i64) -> i64 {
    x.abs()
}

/// Search state. Field names follow `reg_search.c` and `ereg_search.c`.
struct Workspace {
    v: usize,
    v_sub: usize,
    v_rest: usize,
    /// `v * v`, the regularity objective's starting bound.
    v_sq: i64,
    /// Sum of all degrees, the edge-count objective's starting bound (`ereg_search`).
    e: i64,
    degs: Vec<usize>,
    /// The graph's neighbor lists, ascending and concatenated: vertex `i`'s are
    /// `nbrs[offsets[i]..offsets[i + 1]]`.
    offsets: Vec<usize>,
    nbrs: Vec<u32>,
    /// Positions of neighbors in `adj_list`: see [`Workspace::slot`]. With up to
    /// [`DENSE_POSITIONS`] vertices, a `v * v` table as in the C programs, `NOT_AN_EDGE` for
    /// non-edges; with more, one entry per edge end, in the order of `nbrs`. Every thread has its
    /// own, so a `v * v` table for a large graph would take more memory than the graph itself.
    pos: Vec<u16>,
    dense: bool,
    /// Neighbor lists, partitioned: subgraph neighbors first.
    adj_list: Vec<Vec<u32>>,
    sub_degs: Vec<i64>,
    /// `sub_degs - degree` for subgraph vertices; also scratch space, as in the C programs.
    diffs: Vec<i64>,
    sub_verts: Vec<usize>,
    rest_verts: Vec<usize>,
    vert_inds: Vec<usize>,
    sub_ch: Vec<u8>,
    opt_ch: Vec<u8>,
    u_ch: Vec<u8>,
    u_list: Vec<usize>,
    sub_opt: Vec<usize>,
    rest_opt: Vec<usize>,
    rand_perm: Vec<usize>,
    /// With `params.connected`.
    components: Option<Components>,
}

impl Workspace {
    fn new(graph: &Graph, params: &SearchParams) -> Self {
        let v = graph.num_vertices();
        let v_fix = params.fixed.len();
        let mut ws = Workspace {
            v,
            v_sub: params.size - v_fix,
            v_rest: v - params.size,
            v_sq: (v * v) as i64,
            e: 2 * graph.num_edges() as i64,
            degs: graph.degrees(),
            offsets: std::iter::once(0)
                .chain((0..v).scan(0, |end, i| {
                    *end += graph.neighbors(i).len();
                    Some(*end)
                }))
                .collect(),
            nbrs: (0..v)
                .flat_map(|i| graph.neighbors(i).iter().copied())
                .collect(),
            pos: Vec::new(),
            dense: dense_positions(v),
            adj_list: vec![Vec::new(); v],
            sub_degs: vec![0; v],
            diffs: vec![0; v],
            sub_verts: vec![0; params.size - v_fix],
            rest_verts: vec![0; v - params.size],
            vert_inds: vec![0; v],
            sub_ch: vec![0; v],
            opt_ch: vec![0; v],
            u_ch: vec![0; v],
            u_list: vec![0; v - params.size],
            sub_opt: vec![0; v],
            rest_opt: vec![0; v],
            rand_perm: Vec::with_capacity(v - v_fix),
            components: (params.connected || params.path).then(|| Components::new(v)),
        };
        ws.pos = if ws.dense {
            let mut table = vec![NOT_AN_EDGE; v * v];
            for (i, j) in graph.edges() {
                table[i * v + j] = 0;
                table[j * v + i] = 0;
            }
            table
        } else {
            vec![0; ws.nbrs.len()]
        };
        ws.reset(graph, params);
        ws
    }

    /// The state the C programs start from: plain sorted neighbor lists. (Positions are set by
    /// [`Workspace::partition`], which every experiment starts with.)
    fn reset(&mut self, graph: &Graph, params: &SearchParams) {
        let v = self.v;
        for i in 0..v {
            self.adj_list[i].clear();
            self.adj_list[i].extend_from_slice(graph.neighbors(i));
        }
        self.sub_ch.fill(0);
        self.opt_ch.fill(0);
        for &f in &params.fixed {
            self.sub_ch[f] = 1;
        }
        self.rand_perm.clear();
        self.rand_perm
            .extend((0..v).filter(|&i| self.sub_ch[i] == 0));
    }

    /// Where `pos` keeps the position of `j` in `adj_list[i]`, if `{i, j}` is an edge.
    #[inline]
    fn slot(&self, i: usize, j: usize) -> Option<usize> {
        if self.dense {
            let s = i * self.v + j;
            return (self.pos[s] != NOT_AN_EDGE).then_some(s);
        }
        let (start, end) = (self.offsets[i], self.offsets[i + 1]);
        self.nbrs[start..end]
            .binary_search(&(j as u32))
            .ok()
            .map(|k| start + k)
    }

    /// The position of `j` in `adj_list[i]`, or -1 if `{i, j}` is not an edge.
    #[inline]
    fn position(&self, i: usize, j: usize) -> i64 {
        self.slot(i, j).map_or(-1, |s| i64::from(self.pos[s]))
    }

    /// Records that `j`, a neighbor of `i`, is now at `position` in `adj_list[i]`.
    #[inline]
    fn set_position(&mut self, i: usize, j: usize, position: usize) {
        let s = self.slot(i, j).expect("a neighbor");
        self.pos[s] = position as u16;
    }

    fn run<R: SearchRng>(
        &mut self,
        graph: &Graph,
        params: &SearchParams,
        rng: &mut R,
    ) -> Experiment {
        let (g_opt_val, moves, success, trace) = match params.regular {
            Some(r) if r.two_phase => self.run_two_phase(graph, params, r.degree, rng),
            Some(r) => self.run_regular(graph, params, r.degree, rng),
            None => self.run_regular(graph, params, 2, rng), // a path search
        };
        let vertices = (0..self.v).filter(|&k| self.opt_ch[k] != 0).collect();
        for k in 0..self.v_sub {
            self.sub_ch[self.sub_verts[k]] = 0;
        }
        Experiment {
            best_conflict: g_opt_val,
            moves,
            success,
            vertices,
            trace,
        }
    }

    /// Picks a new random starting subgraph, as all the C programs do.
    fn random_subgraph<R: SearchRng>(&mut self, rng: &mut R) {
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
    }

    /// Rebuilds every neighbor list with subgraph neighbors first (each part ascending) and
    /// stores positions in `pos`; returns nothing, but leaves `sub_degs` set when `set_degrees`.
    fn partition(&mut self, set_degrees: bool) {
        let mut others = Vec::new();
        for j in 0..self.v {
            let (start, end) = (self.offsets[j], self.offsets[j + 1]);
            let list = &mut self.adj_list[j];
            list.clear();
            others.clear();
            for &k in &self.nbrs[start..end] {
                if self.sub_ch[k as usize] != 0 {
                    list.push(k);
                } else {
                    others.push(k);
                }
            }
            if set_degrees {
                self.sub_degs[j] = list.len() as i64;
                self.u_ch[j] = 0;
            }
            list.extend_from_slice(&others);
            for k in 0..end - start {
                let x = self.adj_list[j][k] as usize;
                self.set_position(j, x, k);
            }
        }
    }

    /// Moves `sub_vert` (just left) and `rest_vert` (just joined) to the right side of every
    /// neighbor's partitioned list. Mirrors "update partitioning of adjacency list".
    fn update_partition(&mut self, sub_vert: usize, rest_vert: usize) {
        for k in 0..self.degs[sub_vert] {
            let vert = self.adj_list[sub_vert][k] as usize;
            let index = self.position(vert, sub_vert) as usize;
            let temp = self.position(vert, rest_vert);
            if temp >= 0 {
                let temp = temp as usize;
                self.adj_list[vert][index] = rest_vert as u32;
                self.adj_list[vert][temp] = sub_vert as u32;
                self.set_position(vert, sub_vert, temp);
                self.set_position(vert, rest_vert, index);
            } else {
                let degree = self.sub_degs[vert] as usize;
                if index != degree {
                    let moved = self.adj_list[vert][degree] as usize;
                    self.adj_list[vert][index] = moved as u32;
                    self.adj_list[vert][degree] = sub_vert as u32;
                    self.set_position(vert, moved, index);
                    self.set_position(vert, sub_vert, degree);
                }
            }
        }
        for k in 0..self.degs[rest_vert] {
            let vert = self.adj_list[rest_vert][k] as usize;
            if self.position(vert, sub_vert) >= 0 {
                continue;
            }
            let temp = self.position(vert, rest_vert) as usize;
            let degree = (self.sub_degs[vert] - 1) as usize;
            if temp != degree {
                let moved = self.adj_list[vert][degree] as usize;
                self.adj_list[vert][temp] = moved as u32;
                self.adj_list[vert][degree] = rest_vert as u32;
                self.set_position(vert, moved, temp);
                self.set_position(vert, rest_vert, degree);
            }
        }
    }

    /// Lists candidate columns, active first: as in `sub_search`. Returns `(u_cols, m)`.
    fn active_columns(&mut self, sub_vert: usize, rest_col: usize) -> (usize, usize) {
        let mut u_cols = 0;
        self.u_ch[sub_vert] = 0;
        let m = self.v_rest - 1;
        let mut temp = m;
        for k in 0..self.v_rest {
            if k == rest_col {
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
        (u_cols, m)
    }

    fn store(
        &mut self,
        val: i64,
        opt_val: &mut i64,
        opt_mov: &mut usize,
        sub_vert: usize,
        rest_vert: usize,
    ) {
        if val <= *opt_val {
            if val < *opt_val {
                *opt_val = val;
                *opt_mov = 0;
            }
            if *opt_mov < self.v {
                self.sub_opt[*opt_mov] = sub_vert;
                self.rest_opt[*opt_mov] = rest_vert;
                *opt_mov += 1;
            }
        }
    }

    /// The regularity objective after swapping `sub_vert` out for `rest_vert` in, given
    /// `degree`: the objective with `sub_vert` removed (and `diffs` adjusted for that).
    fn swap_value(&self, degree: i64, d_sub: i64, sub_vert: usize, rest_vert: usize) -> i64 {
        let mut curr_val = degree;
        let mut temp = self.sub_degs[rest_vert] - d_sub;
        for m in 0..self.sub_degs[rest_vert] as usize {
            let vert = self.adj_list[rest_vert][m] as usize;
            if vert == sub_vert {
                temp -= 1;
            } else if self.diffs[vert] >= 0 {
                curr_val += 1;
            } else {
                curr_val -= 1;
            }
        }
        curr_val + abs(temp)
    }

    /// The objective with `sub_vert` removed; adjusts `diffs` of its subgraph neighbors, which
    /// [`Workspace::restore_diffs`] undoes.
    fn without(&mut self, obj_val: i64, sub_vert: usize) -> i64 {
        let mut degree = obj_val - abs(self.diffs[sub_vert]);
        for l in 0..self.sub_degs[sub_vert] as usize {
            let x = self.adj_list[sub_vert][l] as usize;
            if self.diffs[x] > 0 {
                degree -= 1;
            } else {
                degree += 1;
            }
            self.diffs[x] -= 1;
        }
        degree
    }

    fn restore_diffs(&mut self, sub_vert: usize) {
        for l in 0..self.sub_degs[sub_vert] as usize {
            let x = self.adj_list[sub_vert][l] as usize;
            self.diffs[x] += 1;
        }
    }

    /// Selects a move: random while diversifying (or with no optimal move), else optimal.
    fn select<R: SearchRng>(&self, diversify: bool, opt_mov: usize, rng: &mut R) -> (usize, usize) {
        if diversify {
            let s = self.sub_verts[rng.below(self.v_sub as u32) as usize];
            let r = self.rest_verts[rng.below(self.v_rest as u32) as usize];
            (s, r)
        } else {
            let index = rng.below(opt_mov as u32) as usize;
            (self.sub_opt[index], self.rest_opt[index])
        }
    }

    fn swap_lists(&mut self, sub_vert: usize, rest_vert: usize, sub_row: usize, rest_col: usize) {
        self.sub_verts[sub_row] = rest_vert;
        self.vert_inds[rest_vert] = sub_row;
        self.rest_verts[rest_col] = sub_vert;
        self.vert_inds[sub_vert] = rest_col;
    }

    /// The components of the current subgraph beyond the first, with `params.connected`; else 0.
    fn extra_components(&mut self, graph: &Graph, fixed: &[usize]) -> i64 {
        match self.components.as_mut() {
            Some(c) => {
                let members = fixed.iter().chain(&self.sub_verts).copied();
                c.extra(members, &self.sub_ch, |u| graph.neighbors(u))
            }
            None => 0,
        }
    }

    /// One `reg_search` experiment: `(best value, moves, success, trace)`.
    fn run_regular<R: SearchRng>(
        &mut self,
        graph: &Graph,
        params: &SearchParams,
        d_sub: i64,
        rng: &mut R,
    ) -> (i64, usize, bool, Option<Vec<u32>>) {
        let v = self.v;
        let mut trace = params
            .record_trace
            .then(|| Vec::with_capacity(params.max_moves));
        self.random_subgraph(rng);
        self.partition(true);

        let mut obj_val = 0i64;
        let mut l = 0;
        for j in 0..v {
            if self.sub_ch[j] != 0 {
                self.diffs[j] = self.sub_degs[j] - d_sub;
                obj_val += abs(self.diffs[j]);
            } else {
                self.vert_inds[j] = l;
                self.rest_verts[l] = j;
                l += 1;
            }
        }

        let v_sq = self.v_sq;
        let (mut g_opt_val, mut old_min_val) = (v_sq, v_sq);
        // a path search also tracks the edge count, which it values swaps by
        let target_edges = params.size as i64 - 1;
        let (mut edges, mut old_edges) = (0i64, -1i64);
        let mut opt_mov = 0usize;
        let mut j = 0;
        while j < params.max_moves {
            let diversify = j % params.div_freq < params.div_dur || opt_mov == 0;
            let (sub_vert, rest_vert) = self.select(diversify, opt_mov, rng);
            let sub_row = self.vert_inds[sub_vert];
            let rest_col = self.vert_inds[rest_vert];

            // objective and differences
            obj_val -= abs(self.diffs[sub_vert]);
            for k in 0..self.sub_degs[sub_vert] as usize {
                let vert = self.adj_list[sub_vert][k] as usize;
                if self.diffs[vert] > 0 {
                    obj_val -= 1;
                } else {
                    obj_val += 1;
                }
                self.diffs[vert] -= 1;
            }
            for k in 0..self.sub_degs[rest_vert] as usize {
                let vert = self.adj_list[rest_vert][k] as usize;
                if vert == sub_vert {
                    continue;
                }
                if self.diffs[vert] >= 0 {
                    obj_val += 1;
                } else {
                    obj_val -= 1;
                }
                self.diffs[vert] += 1;
            }

            // subgraph degrees; mark vertices adjacent to exactly one of the two swapped
            for k in 0..self.degs[sub_vert] {
                let vert = self.adj_list[sub_vert][k] as usize;
                self.sub_degs[vert] -= 1;
                self.u_ch[vert] = 1;
            }
            for k in 0..self.degs[rest_vert] {
                let vert = self.adj_list[rest_vert][k] as usize;
                self.sub_degs[vert] += 1;
                self.u_ch[vert] = 1 - self.u_ch[vert];
            }

            self.diffs[rest_vert] = self.sub_degs[rest_vert] - d_sub;
            obj_val += abs(self.diffs[rest_vert]);

            self.sub_ch[sub_vert] = 0;
            self.sub_ch[rest_vert] = 1;
            self.swap_lists(sub_vert, rest_vert, sub_row, rest_col);

            let extra = self.extra_components(graph, &params.fixed);
            let conflict = if params.path {
                let twice_edges: i64 = params
                    .fixed
                    .iter()
                    .chain(&self.sub_verts)
                    .map(|&u| self.sub_degs[u])
                    .sum();
                edges = twice_edges / 2;
                abs(obj_val - 2) + abs(edges - target_edges) + extra
            } else {
                obj_val + extra
            };
            if let Some(t) = trace.as_mut() {
                t.push(conflict.clamp(0, u32::MAX as i64) as u32);
            }

            let new_min_val = obj_val;
            if conflict < g_opt_val {
                g_opt_val = conflict;
                self.opt_ch.copy_from_slice(&self.sub_ch);
            }

            self.update_partition(sub_vert, rest_vert);

            if conflict == 0 {
                return (g_opt_val, j, true, trace);
            }

            let (u_cols, m) = self.active_columns(sub_vert, rest_col);
            if new_min_val != old_min_val || (params.path && edges != old_edges) {
                for k in 0..self.v_sub {
                    self.u_ch[self.sub_verts[k]] = 1;
                }
            }

            // optimal moves
            let mut opt_val = v_sq;
            let curr_rest = rest_vert;
            opt_mov = 0;
            self.u_ch[rest_vert] = 0;
            for k in 0..self.v_sub {
                let sv = self.sub_verts[k];
                if sv == curr_rest {
                    continue;
                }
                let w_cols = if self.u_ch[sv] != 0 {
                    self.u_ch[sv] = 0;
                    m
                } else {
                    u_cols
                };
                let degree = self.without(obj_val, sv);
                for l in 0..w_cols {
                    let rv = self.u_list[l];
                    let val = self.swap_value(degree, d_sub, sv, rv);
                    // a path search aims at a deviation of 2, for the path's two endpoints, and
                    // at one edge fewer than vertices
                    let val = if params.path {
                        let after = edges - self.sub_degs[sv] + self.sub_degs[rv]
                            - i64::from(graph.has_edge(sv, rv));
                        abs(val - 2) + abs(after - target_edges)
                    } else {
                        val
                    };
                    self.store(val, &mut opt_val, &mut opt_mov, sv, rv);
                }
                self.restore_diffs(sv);
            }

            for k in 0..u_cols {
                self.u_ch[self.u_list[k]] = 0;
            }
            old_edges = edges;
            old_min_val = new_min_val;
            j += 1;
        }
        (g_opt_val, j, false, trace)
    }

    /// One `ereg_search` experiment: `(best value, moves, success, trace)`.
    fn run_two_phase<R: SearchRng>(
        &mut self,
        graph: &Graph,
        params: &SearchParams,
        d_sub: i64,
        rng: &mut R,
    ) -> (i64, usize, bool, Option<Vec<u32>>) {
        let v = self.v;
        let mut trace = params
            .record_trace
            .then(|| Vec::with_capacity(params.max_moves));
        // as in ereg_search.c, computed from the number of free (non-fixed) subgraph vertices
        let e_sub = self.v_sub as i64 * d_sub / 2;
        self.sub_degs.fill(0);
        self.u_ch.fill(0);
        self.random_subgraph(rng);

        let mut l = 0;
        for j in 0..v {
            if self.sub_ch[j] != 0 {
                for k in 0..self.degs[j] {
                    let x = self.adj_list[j][k] as usize;
                    self.sub_degs[x] += 1;
                }
            } else {
                self.vert_inds[j] = l;
                self.rest_verts[l] = j;
                l += 1;
            }
        }
        let mut new_min_val: i64 = (0..v)
            .filter(|&j| self.sub_ch[j] != 0)
            .map(|j| self.sub_degs[j])
            .sum::<i64>()
            / 2
            - e_sub;

        let (e, v_sq) = (self.e, self.v_sq);
        let (mut g_opt_val, mut old_min_val) = (e, e);
        let mut opt_mov = 0usize;
        let mut obj_val = 0i64;
        let mut sol_found = false;
        let mut j = 0;
        while j < params.max_moves {
            let diversify = !sol_found && (j % params.div_freq < params.div_dur || opt_mov == 0);
            let (sub_vert, rest_vert) = self.select(diversify, opt_mov, rng);
            let sub_row = self.vert_inds[sub_vert];
            let rest_col = self.vert_inds[rest_vert];
            let swapped_adjacent = graph.has_edge(sub_vert, rest_vert);

            if !sol_found {
                new_min_val += self.sub_degs[rest_vert]
                    - self.sub_degs[sub_vert]
                    - i64::from(swapped_adjacent);
                obj_val = abs(new_min_val);
            } else {
                obj_val -= abs(self.diffs[sub_vert]);
                self.diffs[rest_vert] =
                    self.sub_degs[rest_vert] - d_sub - i64::from(swapped_adjacent);
                obj_val += abs(self.diffs[rest_vert]);
                for k in 0..self.sub_degs[sub_vert] as usize {
                    let vert = self.adj_list[sub_vert][k] as usize;
                    if self.diffs[vert] > 0 {
                        obj_val -= 1;
                    } else {
                        obj_val += 1;
                    }
                    self.diffs[vert] -= 1;
                }
                for k in 0..self.sub_degs[rest_vert] as usize {
                    let vert = self.adj_list[rest_vert][k] as usize;
                    if vert == sub_vert {
                        continue;
                    }
                    if self.diffs[vert] >= 0 {
                        obj_val += 1;
                    } else {
                        obj_val -= 1;
                    }
                    self.diffs[vert] += 1;
                }
            }

            for k in 0..self.degs[sub_vert] {
                let x = self.adj_list[sub_vert][k] as usize;
                self.sub_degs[x] -= 1;
            }
            for k in 0..self.degs[rest_vert] {
                let x = self.adj_list[rest_vert][k] as usize;
                self.sub_degs[x] += 1;
            }

            self.sub_ch[sub_vert] = 0;
            self.sub_ch[rest_vert] = 1;
            self.swap_lists(sub_vert, rest_vert, sub_row, rest_col);

            let extra = self.extra_components(graph, &params.fixed);
            let mut conflict = obj_val + extra;
            if let Some(t) = trace.as_mut() {
                t.push(conflict.clamp(0, u32::MAX as i64) as u32);
            }

            if conflict < g_opt_val {
                g_opt_val = conflict;
                self.opt_ch.copy_from_slice(&self.sub_ch);
            }

            let (mut u_cols, mut m) = (0, 0);
            if sol_found {
                self.update_partition(sub_vert, rest_vert);
            } else {
                for k in 0..self.degs[sub_vert] {
                    let x = self.adj_list[sub_vert][k] as usize;
                    self.u_ch[x] = 1;
                }
                for k in 0..self.degs[rest_vert] {
                    let x = self.adj_list[rest_vert][k] as usize;
                    self.u_ch[x] = 1 - self.u_ch[x];
                }
                (u_cols, m) = self.active_columns(sub_vert, rest_col);
                if new_min_val != old_min_val {
                    for k in 0..self.v_sub {
                        self.u_ch[self.sub_verts[k]] = 1;
                    }
                }
            }

            // the right number of edges: switch to the regularity objective
            if !sol_found && obj_val == 0 {
                sol_found = true;
                self.partition(false);
                obj_val = 0;
                for k in 0..v {
                    if self.sub_ch[k] != 0 {
                        self.diffs[k] = self.sub_degs[k] - d_sub;
                        obj_val += abs(self.diffs[k]);
                    }
                }
                conflict = obj_val + extra;
                if conflict != 0 {
                    g_opt_val = v_sq;
                }
            }

            if conflict == 0 {
                return (g_opt_val, j, true, trace);
            }

            opt_mov = 0;
            if !sol_found {
                let mut opt_val = e;
                self.u_ch[rest_vert] = 0;
                for k in 0..self.v_sub {
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
                    let degree = new_min_val - self.sub_degs[vert];
                    for l in 0..w_cols {
                        let temp = self.u_list[l];
                        let val =
                            abs(degree + self.sub_degs[temp]
                                - i64::from(graph.has_edge(vert, temp)));
                        self.store(val, &mut opt_val, &mut opt_mov, vert, temp);
                    }
                }
                for k in 0..u_cols {
                    self.u_ch[self.u_list[k]] = 0;
                }
                old_min_val = new_min_val;
            } else {
                let mut opt_val = v_sq;
                let curr_rest = rest_vert;
                let mut candidates = Vec::new();
                for k in 0..self.v_sub {
                    let sv = self.sub_verts[k];
                    if sv == curr_rest {
                        continue;
                    }
                    let degree = self.without(obj_val, sv);
                    // only swaps that keep the edge count
                    candidates.clear();
                    for l in 0..self.v_rest {
                        let vert = self.rest_verts[l];
                        let adjacent = graph.has_edge(sv, vert);
                        if (adjacent && self.sub_degs[vert] == self.sub_degs[sv] + 1)
                            || (!adjacent && self.sub_degs[vert] == self.sub_degs[sv])
                        {
                            candidates.push(vert);
                        }
                    }
                    for &rv in &candidates {
                        let val = self.swap_value(degree, d_sub, sv, rv);
                        self.store(val, &mut opt_val, &mut opt_mov, sv, rv);
                    }
                    self.restore_diffs(sv);
                }
                // no move keeps the edge count: the experiment fails
                if opt_mov == 0 {
                    return (g_opt_val, j, false, trace);
                }
            }
            j += 1;
        }
        (g_opt_val, j, false, trace)
    }
}

#[cfg(test)]
mod tests {
    use super::{PER_EDGE_POSITIONS, search_sequential};
    use crate::graph::Graph;
    use crate::rng::RngKind;
    use crate::search::{Regular, SearchParams, search};

    /// The Paley graph on 61 vertices, strongly regular with parameters (61, 30, 14, 15).
    fn paley(q: usize) -> Graph {
        let squares: std::collections::HashSet<usize> = (1..q).map(|x| x * x % q).collect();
        Graph::from_edges(
            q,
            (0..q)
                .flat_map(|i| (i + 1..q).map(move |j| (i, j)))
                .filter(|&(i, j)| squares.contains(&((j - i) % q))),
        )
        .unwrap()
    }

    fn degrees_within(g: &Graph, vs: &[usize]) -> Vec<usize> {
        vs.iter()
            .map(|&u| vs.iter().filter(|&&w| g.has_edge(u, w)).count())
            .collect()
    }

    #[test]
    fn finds_regular_subgraphs() {
        let g = paley(61);
        for two_phase in [false, true] {
            for rng in [RngKind::Xoshiro, RngKind::Legacy] {
                let mut p = SearchParams::new(10, 0);
                p.regular = Some(Regular {
                    degree: 4,
                    two_phase,
                });
                p.rng = rng;
                p.experiments = 8;
                p.max_moves = 2000;
                p.div_freq = 300;
                p.div_dur = 8;
                let r = search(&g, &p).unwrap();
                assert!(r.successes() > 0, "two_phase {two_phase}, {rng:?}");
                for e in r.experiments.iter().filter(|e| e.success) {
                    assert_eq!(e.vertices.len(), 10);
                    assert!(
                        degrees_within(&g, &e.vertices).iter().all(|&d| d == 4),
                        "{:?}",
                        e.vertices
                    );
                }
            }
        }
    }

    #[test]
    fn two_phase_best_conflicts_describe_the_subgraphs_returned() {
        let g = paley(61);
        for connected in [false, true] {
            let mut p = SearchParams::new(20, 0);
            p.regular = Some(Regular {
                degree: 8,
                two_phase: true,
            });
            p.connected = connected;
            p.experiments = 40;
            p.max_moves = 3;
            for rng in [RngKind::Xoshiro, RngKind::Legacy] {
                p.rng = rng;
                let mut as_c = Vec::new();
                super::search_sequential_as_c(&g, &p, |e, _| as_c.push(e.best_conflict)).unwrap();
                let r = search(&g, &p).unwrap();
                for e in &r.experiments {
                    let deviation: i64 = degrees_within(&g, &e.vertices)
                        .iter()
                        .map(|&d| (d as i64 - 8).abs())
                        .sum();
                    assert_eq!(e.best_conflict, deviation, "{rng:?}, {:?}", e.vertices);
                }
                // the C program's values differ: these runs exercise the quirk
                let fixed: Vec<i64> = r.experiments.iter().map(|e| e.best_conflict).collect();
                assert_ne!(as_c, fixed, "{rng:?}");
            }
        }
    }

    /// Whether `vs` induces a path: connected, one edge fewer than vertices, no degree above 2.
    fn is_induced_path(g: &Graph, vs: &[usize]) -> bool {
        let degrees = degrees_within(g, vs);
        let (mut reached, mut stack) = (vec![vs[0]], vec![vs[0]]);
        while let Some(u) = stack.pop() {
            for &w in vs {
                if g.has_edge(u, w) && !reached.contains(&w) {
                    reached.push(w);
                    stack.push(w);
                }
            }
        }
        reached.len() == vs.len()
            && degrees.iter().sum::<usize>() == 2 * (vs.len() - 1)
            && degrees.iter().all(|&d| d <= 2)
    }

    #[test]
    fn path_searches_find_induced_paths() {
        // a 6x6 grid: plenty of induced paths, and 4-cycles that a search for degree 2 ends on
        let grid = Graph::from_edges(
            36,
            (0..36)
                .filter(|i| i % 6 < 5)
                .map(|i| (i, i + 1))
                .chain((0..30).map(|i| (i, i + 6))),
        )
        .unwrap();
        for rng in [RngKind::Xoshiro, RngKind::Legacy] {
            for k in [5, 12] {
                let mut p = SearchParams::new(k, 0);
                p.path = true;
                p.rng = rng;
                p.experiments = 30;
                p.max_moves = 1000;
                p.record_trace = true;
                let r = search(&grid, &p).unwrap();
                assert!(r.successes() > 0, "{rng:?}, {k}");
                for e in &r.experiments {
                    assert!(
                        !e.success || is_induced_path(&grid, &e.vertices),
                        "{:?}",
                        e.vertices
                    );
                    let lowest = *e.trace.as_ref().unwrap().iter().min().unwrap();
                    assert_eq!(i64::from(lowest), e.best_conflict);
                }
            }
        }
        let mut p = SearchParams::new(8, 0);
        p.path = true;
        p.regular = Some(Regular {
            degree: 2,
            two_phase: false,
        });
        assert!(search(&grid, &p).is_err());
        p.regular = None;
        p.aspiration = true;
        assert!(search(&grid, &p).is_err());
        p.aspiration = false;
        assert!(search(&crate::graph::WeightedGraph::from_graph(&grid), &p).is_err());
    }

    #[test]
    fn path_searches_reach_the_6_cube_snake_record() {
        // the longest induced path in the 6-cube has 27 vertices
        let n = 6;
        let cube = Graph::from_edges(
            1 << n,
            (0..1usize << n)
                .flat_map(|u| (0..n).map(move |b| (u, u ^ (1 << b))))
                .filter(|&(u, w)| u < w),
        )
        .unwrap();
        let mut p = SearchParams::new(27, 0);
        p.path = true;
        p.experiments = 50;
        p.max_moves = 2000;
        p.div_freq = 300;
        p.div_dur = 8;
        let r = search(&cube, &p).unwrap();
        assert!(r.successes() > 0);
        for e in r.experiments.iter().filter(|e| e.success) {
            assert!(is_induced_path(&cube, &e.vertices), "{:?}", e.vertices);
        }
    }

    #[test]
    fn path_searches_run_between_given_endpoints() {
        let n = 6;
        let cube = Graph::from_edges(
            1 << n,
            (0..1usize << n)
                .flat_map(|u| (0..n).map(move |b| (u, u ^ (1 << b))))
                .filter(|&(u, w)| u < w),
        )
        .unwrap();
        let (s, t) = (0, 63);
        let ends_at = |vs: &[usize]| {
            let ends: Vec<usize> = vs
                .iter()
                .copied()
                .filter(|&v| vs.iter().filter(|&&w| cube.has_edge(v, w)).count() == 1)
                .collect();
            ends == [s, t]
        };
        for rng in [RngKind::Xoshiro, RngKind::Legacy] {
            let mut p = SearchParams::new(21, 0);
            p.path = true;
            p.endpoints = Some([s, t]);
            p.experiments = 20;
            p.max_moves = 2000;
            p.rng = rng;
            let r = search(&cube, &p).unwrap();
            assert!(r.successes() > 0);
            assert_eq!(r.params.endpoints, Some([s, t]));
            for e in &r.experiments {
                assert_eq!(e.vertices.len(), 21);
                assert!(e.vertices.contains(&s) && e.vertices.contains(&t));
                if e.success {
                    assert!(is_induced_path(&cube, &e.vertices), "{:?}", e.vertices);
                    assert!(ends_at(&e.vertices), "{:?}", e.vertices);
                }
            }
        }
        // 0 and 63 differ in all six bits, an even number, so every path between them has an odd
        // number of vertices
        let mut p = SearchParams::new(20, 0);
        p.path = true;
        p.endpoints = Some([s, t]);
        p.experiments = 20;
        p.max_moves = 2000;
        assert_eq!(search(&cube, &p).unwrap().successes(), 0);

        let bad = |f: &dyn Fn(&mut SearchParams)| {
            let mut p = SearchParams::new(20, 0);
            p.path = true;
            p.endpoints = Some([s, t]);
            f(&mut p);
            search(&cube, &p).is_err()
        };
        assert!(bad(&|p| p.path = false));
        assert!(bad(&|p| p.endpoints = Some([5, 5])));
        assert!(bad(&|p| p.endpoints = Some([0, 64])));
        assert!(bad(&|p| p.size = 2));
    }

    #[test]
    fn positions_per_edge_give_the_same_experiments() {
        // the C programs' v * v position table and the per-edge one must give identical searches
        let n = 7;
        let cube = Graph::from_edges(
            1 << n,
            (0..1usize << n)
                .flat_map(|u| (0..n).map(move |b| (u, u ^ (1 << b))))
                .filter(|&(u, w)| u < w),
        )
        .unwrap();
        let mut two_phase = SearchParams::new(20, 0);
        two_phase.regular = Some(Regular {
            degree: 8,
            two_phase: true,
        });
        let mut regular = SearchParams::new(12, 0);
        regular.regular = Some(Regular {
            degree: 6,
            two_phase: false,
        });
        let mut coil = SearchParams::new(40, 0);
        coil.regular = Some(Regular {
            degree: 2,
            two_phase: false,
        });
        coil.connected = true;
        let mut path = SearchParams::new(41, 0);
        path.path = true;
        path.endpoints = Some([0, 127]);
        path.fixed = vec![5];
        let cases = [
            (paley(61), two_phase),
            (paley(37), regular),
            (cube.clone(), coil),
            (cube, path),
        ];
        for (g, mut p) in cases {
            for rng in [RngKind::Xoshiro, RngKind::Legacy] {
                p.rng = rng;
                p.experiments = 10;
                p.max_moves = 1000;
                let run = |per_edge: bool| {
                    PER_EDGE_POSITIONS.with(|f| f.set(per_edge));
                    let mut out = Vec::new();
                    search_sequential(&g, &p, |e| out.push(e.clone())).unwrap();
                    PER_EDGE_POSITIONS.with(|f| f.set(false));
                    out
                };
                assert_eq!(run(false), run(true));
            }
        }
    }

    #[test]
    fn keeps_fixed_vertices_and_rejects_bad_requests() {
        let g = paley(37);
        let mut p = SearchParams::new(8, 0);
        p.regular = Some(Regular {
            degree: 3,
            two_phase: false,
        });
        p.fixed = vec![0, 5];
        p.experiments = 6;
        let r = search(&g, &p).unwrap();
        for e in r.experiments.iter().filter(|e| e.success) {
            assert!(e.vertices.contains(&0) && e.vertices.contains(&5));
            assert!(degrees_within(&g, &e.vertices).iter().all(|&d| d == 3));
        }
        assert!(r.successes() > 0);
        p.aspiration = true;
        assert!(search(&g, &p).is_err());
        p.aspiration = false;
        let w = crate::graph::WeightedGraph::from_graph(&g);
        assert!(search(&w, &p).is_err());
    }
}
