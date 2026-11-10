//! The largest induced path, cycle, tree, clique or independent set a search finds, by searching
//! one size after another.
//!
//! [`largest`] first doubles its steps upward from a starting size while searches succeed, then
//! bisects between the last size that succeeded and the first that did not. A size counts as
//! reachable if the search succeeds at it or at one of the next `lookahead` sizes: some sizes are
//! impossible even below the largest, such as paths with an odd number of vertices between two
//! hypercube vertices that differ in an odd number of bits, and a search that stopped at the first
//! failure would stop there. Every search is kept, so the sizes just beyond the largest found give
//! the closest near misses.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::graph::Graph;
use crate::search::{Regular, SearchParams, SearchResult, search};

/// The kinds of induced subgraph [`largest`] can grow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    /// An induced path ([`SearchParams::path`]), between [`SearchParams::endpoints`] if given.
    Path,
    /// An induced cycle: connected and regular of degree 2.
    Cycle,
    /// An induced tree: connected, with one edge fewer than vertices.
    Tree,
    /// A clique: every pair of vertices joined.
    Clique,
    /// An independent set: no pair of vertices joined.
    Independent,
}

impl Shape {
    /// The shape's name: "path", "cycle", "tree", "clique" or "independent".
    pub fn name(self) -> &'static str {
        match self {
            Shape::Path => "path",
            Shape::Cycle => "cycle",
            Shape::Tree => "tree",
            Shape::Clique => "clique",
            Shape::Independent => "independent",
        }
    }

    /// The search for this shape on `size` vertices, with everything else from `base`.
    pub fn params(self, base: &SearchParams, size: usize) -> SearchParams {
        let mut p = base.clone();
        p.size = size;
        p.edges = 0;
        p.path = false;
        p.regular = None;
        if self != Shape::Path {
            p.endpoints = None;
        }
        match self {
            Shape::Path => p.path = true,
            Shape::Cycle => {
                p.connected = true;
                p.regular = Some(Regular {
                    degree: 2,
                    two_phase: false,
                });
            }
            Shape::Tree => {
                p.connected = true;
                p.edges = size as i64 - 1;
            }
            Shape::Clique => p.edges = (size * (size - 1) / 2) as i64,
            Shape::Independent => p.edges = 0,
        }
        p
    }

    /// The fewest vertices the shape has: 3 for a cycle, 2 otherwise.
    fn smallest(self) -> usize {
        if self == Shape::Cycle { 3 } else { 2 }
    }
}

impl std::str::FromStr for Shape {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "path" => Ok(Shape::Path),
            "cycle" => Ok(Shape::Cycle),
            "tree" => Ok(Shape::Tree),
            "clique" => Ok(Shape::Clique),
            "independent" => Ok(Shape::Independent),
            _ => Err(Error::Invalid(format!(
                "unknown shape {s:?}: choose path, cycle, tree, clique or independent"
            ))),
        }
    }
}

/// The outcome of [`largest`]: every search made, by size.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Largest {
    pub shape: Shape,
    /// The largest size at which a search succeeded, if any did.
    pub size: Option<usize>,
    /// Every search made, in increasing order of size.
    pub results: Vec<SearchResult>,
}

/// Searches `graph` for the largest `shape` it can find, starting at `start` vertices (by
/// default the fewest the shape and the fixed vertices allow) and trusting a failure only when the
/// next `lookahead` sizes fail too; see the module documentation. Each search takes its settings
/// from `base`, except the size and target, which the shape sets.
pub fn largest(
    graph: &Graph,
    shape: Shape,
    base: &SearchParams,
    start: Option<usize>,
    lookahead: usize,
) -> Result<Largest, Error> {
    if base.endpoints.is_some() && shape != Shape::Path {
        return Err(Error::Invalid("endpoints need a path search".into()));
    }
    let mut required = base.fixed.clone();
    for v in base.endpoints.into_iter().flatten() {
        if !required.contains(&v) {
            required.push(v);
        }
    }
    let mut floor = shape.smallest().max(required.len() + 1);
    // a path between two vertices has at least as many as the shortest one
    if let Some([s, t]) = base.endpoints {
        if s.max(t) >= graph.num_vertices() {
            return Err(Error::VertexOutOfRange {
                vertex: s.max(t) as i64,
                n: graph.num_vertices(),
            });
        }
        let Some(shortest) = shortest_path_vertices(graph, s, t) else {
            return Err(Error::Invalid(format!("no path joins {s} and {t}")));
        };
        floor = floor.max(shortest);
    }
    let start = start.unwrap_or(floor);
    // a search needs at least one vertex outside the subgraph
    let most = graph.num_vertices().saturating_sub(1);
    if start > most {
        return Err(Error::InvalidSubgraphSize(start as i64));
    }
    // check the settings once, at the starting size, before any search runs
    shape.params(base, start).validate(graph.num_vertices())?;

    let mut results = BTreeMap::new();
    let mut found = |k: usize| -> Result<bool, Error> {
        reachable(graph, shape, base, k, lookahead, most, &mut results)
    };
    if found(start)? {
        let (mut low, mut step) = (start, 1);
        while found(low + step)? {
            low += step;
            step *= 2;
        }
        let mut high = low + step;
        while high - low > 1 {
            let mid = low + (high - low) / 2;
            if found(mid)? {
                low = mid;
            } else {
                high = mid;
            }
        }
    }
    let size = results
        .iter()
        .filter(|(_, r)| r.successes() > 0)
        .map(|(&k, _)| k)
        .max();
    Ok(Largest {
        shape,
        size,
        results: results.into_values().collect(),
    })
}

/// The number of vertices on a shortest path from `s` to `t`, if there is one.
fn shortest_path_vertices(graph: &Graph, s: usize, t: usize) -> Option<usize> {
    let mut dist = vec![usize::MAX; graph.num_vertices()];
    dist[s] = 1;
    let mut queue = std::collections::VecDeque::from([s]);
    while let Some(u) = queue.pop_front() {
        if u == t {
            return Some(dist[u]);
        }
        for &w in graph.neighbors(u) {
            let w = w as usize;
            if dist[w] == usize::MAX {
                dist[w] = dist[u] + 1;
                queue.push_back(w);
            }
        }
    }
    None
}

/// Whether a search succeeds at `k` vertices or at one of the next `lookahead` sizes, searching
/// each size at most once.
fn reachable(
    graph: &Graph,
    shape: Shape,
    base: &SearchParams,
    k: usize,
    lookahead: usize,
    most: usize,
    results: &mut BTreeMap<usize, SearchResult>,
) -> Result<bool, Error> {
    for size in (k..=k + lookahead).take_while(|&size| size <= most) {
        let result = match results.entry(size) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => e.insert(search(graph, &shape.params(base, size))?),
        };
        if result.successes() > 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(n: usize) -> Graph {
        Graph::from_edges(
            1 << n,
            (0..1usize << n)
                .flat_map(|u| (0..n).map(move |b| (u, u ^ (1 << b))))
                .filter(|&(u, w)| u < w),
        )
        .unwrap()
    }

    fn settings() -> SearchParams {
        let mut p = SearchParams::new(2, 0);
        p.experiments = 50;
        p.max_moves = 2000;
        p.div_freq = 300;
        p.div_dur = 8;
        p
    }

    #[test]
    fn finds_the_5_cube_records() {
        // the longest snake and coil in the 5-cube have 14 vertices each (13 and 14 edges), and
        // the cube has no triangles; a path is a tree, so its largest induced trees are no smaller
        let g = cube(5);
        for (shape, at_least, at_most) in [
            (Shape::Path, 14, 14),
            (Shape::Cycle, 14, 14),
            (Shape::Tree, 14, 31),
            (Shape::Clique, 2, 2),
            // the cube is bipartite with two parts of 16, and no independent set is larger
            (Shape::Independent, 16, 16),
        ] {
            let r = largest(&g, shape, &settings(), None, 2).unwrap();
            let size = r.size.unwrap();
            assert!((at_least..=at_most).contains(&size), "{shape:?}: {size}");
            assert!(
                r.results
                    .windows(2)
                    .all(|w| w[0].params.size < w[1].params.size)
            );
            // nothing larger succeeded, and the search looked beyond the largest found
            assert!(
                r.results
                    .iter()
                    .all(|x| x.params.size <= size || x.successes() == 0)
            );
            assert!(r.results.iter().any(|x| x.params.size > size));
            let best = r.results.iter().find(|x| x.params.size == size).unwrap();
            assert!(best.successes() > 0);
        }
    }

    #[test]
    fn looks_past_impossible_sizes() {
        // 0 and 31 differ in five bits, so paths between them have an even number of vertices:
        // without looking ahead, the search would stop at the first odd size
        let g = cube(5);
        let mut p = settings();
        p.endpoints = Some([0, 31]);
        let ahead = largest(&g, Shape::Path, &p, Some(6), 2).unwrap();
        let size = ahead.size.unwrap();
        assert!(size >= 10 && size.is_multiple_of(2), "{size}");
        assert_eq!(
            largest(&g, Shape::Path, &p, Some(6), 0).unwrap().size,
            Some(6)
        );
        // by default it starts at the shortest path between the endpoints, here 6 vertices
        let from_shortest = largest(&g, Shape::Path, &p, None, 2).unwrap();
        assert_eq!(from_shortest.results[0].params.size, 6);
        assert!(from_shortest.size.unwrap() >= 10);
    }

    #[test]
    fn rejects_bad_requests() {
        let g = cube(4);
        let mut p = settings();
        p.endpoints = Some([0, 15]);
        assert!(largest(&g, Shape::Cycle, &p, None, 2).is_err());
        assert!(largest(&g, Shape::Path, &settings(), Some(16), 2).is_err());
        let one_edge = Graph::from_edges(16, [(0, 1)]).unwrap();
        let mut apart = settings();
        apart.endpoints = Some([0, 5]);
        assert!(largest(&one_edge, Shape::Path, &apart, None, 2).is_err());
        assert!("star".parse::<Shape>().is_err());
        assert_eq!("tree".parse::<Shape>().unwrap(), Shape::Tree);
    }
}
