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
//! degree 2. Each swap is valued by how far the deviation after it would be from 2, so the search
//! is drawn to paths rather than cycles. Its conflict is `|deviation - 2| + |edges - (size - 1)|`
//! plus the components beyond the first, which is 0 exactly for an induced path: a connected graph
//! with `size - 1` edges is a tree, and a tree whose deviation from degree 2 is 2 has two leaves.

use rayon::prelude::*;

use crate::Error;
use crate::graph::Graph;
use crate::rng::{Randu, RngKind, SearchRng, Xoshiro};
use crate::search::{Components, Experiment, SearchParams, SearchResult};

/// The most vertices a regular search supports: like the C programs, it stores neighbor
/// positions in 16-bit integers.
pub const MAX_VERTICES: usize = i16::MAX as usize;

/// Runs a regular search; see [`crate::search::search`] for how experiments are scheduled.
pub(crate) fn search(graph: &Graph, params: &SearchParams) -> Result<SearchResult, Error> {
    validate(graph, params)?;
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
    /// Position of `j` in `adj_list[i]` at `i * v + j`, or -1 if `{i, j}` is not an edge.
    adj: Vec<i16>,
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
            adj: vec![-1; v * v],
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
        ws.reset(graph, params);
        ws
    }

    /// The state the C programs start from: plain sorted neighbor lists, edges marked 0.
    fn reset(&mut self, graph: &Graph, params: &SearchParams) {
        let v = self.v;
        self.adj.fill(-1);
        for i in 0..v {
            self.adj_list[i].clear();
            self.adj_list[i].extend_from_slice(graph.neighbors(i));
            for &j in graph.neighbors(i) {
                self.adj[i * v + j as usize] = 0;
            }
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

    #[inline]
    fn adjacent(&self, a: usize, b: usize) -> bool {
        self.adj[a * self.v + b] >= 0
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
    /// stores positions in `adj`; returns nothing, but leaves `sub_degs` set when `set_degrees`.
    fn partition(&mut self, set_degrees: bool) {
        let v = self.v;
        let mut others = Vec::new();
        for j in 0..v {
            let list = &mut self.adj_list[j];
            list.clear();
            others.clear();
            for k in 0..v {
                if self.adj[j * v + k] >= 0 {
                    if self.sub_ch[k] != 0 {
                        list.push(k as u32);
                    } else {
                        others.push(k as u32);
                    }
                }
            }
            if set_degrees {
                self.sub_degs[j] = list.len() as i64;
                self.u_ch[j] = 0;
            }
            list.extend_from_slice(&others);
        }
        for j in 0..v {
            for (k, &x) in self.adj_list[j].iter().enumerate() {
                self.adj[j * v + x as usize] = k as i16;
            }
        }
    }

    /// Moves `sub_vert` (just left) and `rest_vert` (just joined) to the right side of every
    /// neighbor's partitioned list. Mirrors "update partitioning of adjacency list".
    fn update_partition(&mut self, sub_vert: usize, rest_vert: usize) {
        let v = self.v;
        for k in 0..self.degs[sub_vert] {
            let vert = self.adj_list[sub_vert][k] as usize;
            let index = self.adj[vert * v + sub_vert];
            let temp = self.adj[vert * v + rest_vert];
            if temp >= 0 {
                self.adj_list[vert][index as usize] = rest_vert as u32;
                self.adj_list[vert][temp as usize] = sub_vert as u32;
                self.adj[vert * v + sub_vert] = temp;
                self.adj[vert * v + rest_vert] = index;
            } else {
                let degree = self.sub_degs[vert] as usize;
                if index as usize != degree {
                    let moved = self.adj_list[vert][degree] as usize;
                    self.adj_list[vert][index as usize] = moved as u32;
                    self.adj_list[vert][degree] = sub_vert as u32;
                    self.adj[vert * v + moved] = index;
                    self.adj[vert * v + sub_vert] = degree as i16;
                }
            }
        }
        for k in 0..self.degs[rest_vert] {
            let vert = self.adj_list[rest_vert][k] as usize;
            let index = self.adj[vert * v + sub_vert];
            let temp = self.adj[vert * v + rest_vert];
            if index >= 0 {
                continue;
            }
            let degree = (self.sub_degs[vert] - 1) as usize;
            if temp as usize != degree {
                let moved = self.adj_list[vert][degree] as usize;
                self.adj_list[vert][temp as usize] = moved as u32;
                self.adj_list[vert][degree] = rest_vert as u32;
                self.adj[vert * v + moved] = temp;
                self.adj[vert * v + rest_vert] = degree as i16;
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
                abs(obj_val - 2) + abs(twice_edges / 2 - (params.size as i64 - 1)) + extra
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
            if new_min_val != old_min_val {
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
                    // a path search aims at a deviation of 2, for the path's two endpoints
                    let val = if params.path { abs(val - 2) } else { val };
                    self.store(val, &mut opt_val, &mut opt_mov, sv, rv);
                }
                self.restore_diffs(sv);
            }

            for k in 0..u_cols {
                self.u_ch[self.u_list[k]] = 0;
            }
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
            let swapped_adjacent = self.adjacent(sub_vert, rest_vert);

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
                        let val = abs(
                            degree + self.sub_degs[temp] - i64::from(self.adjacent(vert, temp))
                        );
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
                        let adjacent = self.adjacent(sv, vert);
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
