//! Random graph generators, ported from the original `ggen`.
//!
//! Densities are in *per mille* (0..=1000), as in `ggen`. Each generator is generic over the
//! random number generator: with [`crate::rng::Randu`] seeded as `ggen` is, the output is
//! identical to `ggen`'s.

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::graph::{Graph, GraphBuilder, WeightedGraph};
use crate::rng::{SearchRng, Xoshiro};

/// The resolution of random coordinates and probabilities, as in `ggen`.
pub const SCALE: u32 = 10_000;

/// The random graph models `ggen` provides.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Model {
    /// Erdős–Rényi: each pair is an edge independently with probability `density / 1000`.
    Exponential,
    /// Every vertex `i < n - l` is joined to `l` distinct vertices chosen uniformly from those
    /// numbered after it, with `l = density * (n - 1) / (2000 - density)`. (`ggen` calls these
    /// "power" graphs; they are not preferential-attachment graphs and have no heavy tail.)
    Power,
    /// Random geometric graph in the unit square (no wrap-around): points are joined when the
    /// disc around one with area `density / 1000` contains the other.
    Geometric,
}

impl std::str::FromStr for Model {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "exponential" | "erdos-renyi" | "gnp" => Ok(Model::Exponential),
            "power" => Ok(Model::Power),
            "geometric" => Ok(Model::Geometric),
            _ => Err(format!(
                "unknown model '{s}' (expected exponential, power or geometric)"
            )),
        }
    }
}

/// Generates a graph from `model` on `n` vertices with `density` per mille.
pub fn generate<R: SearchRng>(
    model: Model,
    n: usize,
    density: u32,
    rng: &mut R,
) -> Result<Graph, Error> {
    if density > 1000 {
        return Err(Error::Invalid(format!(
            "density must be between 0 and 1000, got {density}"
        )));
    }
    Ok(match model {
        Model::Exponential => exponential(n, density, rng),
        Model::Power => power(n, density, rng),
        Model::Geometric => geometric(n, density, rng),
    })
}

/// Erdős–Rényi random graph; see [`Model::Exponential`].
pub fn exponential<R: SearchRng>(n: usize, density: u32, rng: &mut R) -> Graph {
    let threshold = density * SCALE / 1000;
    let mut b = GraphBuilder::new(n);
    for i in 0..n {
        for j in i + 1..n {
            if rng.below(SCALE) < threshold {
                b.set(i, j, true);
            }
        }
    }
    b.build()
}

/// `ggen`'s "power" random graph; see [`Model::Power`].
pub fn power<R: SearchRng>(n: usize, density: u32, rng: &mut R) -> Graph {
    // signed arithmetic, as in ggen, so that n = 0 behaves identically
    let v = n as i64;
    let l = i64::from(density) * (v - 1) / (2000 - i64::from(density));
    let mut b = GraphBuilder::new(n);
    let mut rand_perm = vec![0i64; n];
    let mut i = 0;
    while i < v - l {
        for j in 0..(v - i) {
            rand_perm[j as usize] = j;
        }
        let k = v - i - l - 1;
        let mut j = l + k;
        while j > k {
            let temp = rng.below(j as u32) as usize;
            let m = rand_perm[temp] + i + 1;
            rand_perm[temp] = rand_perm[(j - 1) as usize];
            b.set(i as usize, m as usize, true);
            j -= 1;
        }
        i += 1;
    }
    b.build()
}

/// Random geometric graph; see [`Model::Geometric`].
pub fn geometric<R: SearchRng>(n: usize, density: u32, rng: &mut R) -> Graph {
    let mut coords = Vec::with_capacity(n);
    for _ in 0..n {
        let x = i64::from(rng.below(SCALE));
        let y = i64::from(rng.below(SCALE));
        coords.push((x, y));
    }
    // ggen's integer arithmetic: SCALE^2 / 3140 approximates SCALE^2 / (1000 pi)
    let threshold = i64::from(SCALE * SCALE / 3140) * i64::from(density);
    let mut b = GraphBuilder::new(n);
    for i in 0..n {
        for j in i + 1..n {
            let (dx, dy) = (coords[i].0 - coords[j].0, coords[i].1 - coords[j].1);
            if dx * dx + dy * dy < threshold {
                b.set(i, j, true);
            }
        }
    }
    b.build()
}

/// The largest weight [`weighted`] creates: the original `wggen` and `wsub_search` store weights in a
/// signed `char`.
pub const MAX_GENERATED_WEIGHT: u8 = 127;

/// A weighted random graph, as the original `wggen` makes it: `v * (v - 1) * density / 200` times, a
/// uniformly random pair of distinct vertices has its weight increased by 1. So `density` is in
/// percent, and the mean weight per vertex pair is `density / 100`; it may exceed 100. Fails if
/// any weight would exceed [`MAX_GENERATED_WEIGHT`] or the total weight would not fit an `i32`.
pub fn weighted<R: SearchRng>(n: usize, density: u32, rng: &mut R) -> Result<WeightedGraph, Error> {
    if n < 1 {
        return Err(Error::Invalid(
            "a weighted graph needs at least 1 vertex".into(),
        ));
    }
    let total = n as f64 * (n as f64 - 1.0) * f64::from(density) / 200.0;
    if total > f64::from(i32::MAX) {
        return Err(Error::Invalid(format!(
            "total weight v * (v - 1) * density / 200 exceeds {}",
            i32::MAX
        )));
    }
    let mut weights = vec![0u8; n * n];
    for _ in 0..total as i64 {
        let j = rng.below(n as u32) as usize;
        let k = loop {
            let k = rng.below(n as u32) as usize;
            if k != j {
                break k;
            }
        };
        if weights[j * n + k] == MAX_GENERATED_WEIGHT {
            return Err(Error::Invalid(format!(
                "edge weight exceeds {MAX_GENERATED_WEIGHT}; lower the density"
            )));
        }
        weights[j * n + k] += 1;
        weights[k * n + j] += 1;
    }
    Ok(WeightedGraph::from_matrix(n, weights))
}

/// The most dimensions [`hypercube`] builds: the 16-cube has 65,536 vertices, and a graph's
/// adjacency matrix takes a bit per pair of vertices.
pub const MAX_HYPERCUBE_DIMENSION: usize = 16;

/// The `dimension`-dimensional hypercube: vertices `0..2^dimension`, joined when they differ in one
/// bit.
pub fn hypercube(dimension: usize) -> Result<Graph, Error> {
    if dimension > MAX_HYPERCUBE_DIMENSION {
        return Err(Error::Invalid(format!(
            "hypercubes of up to {MAX_HYPERCUBE_DIMENSION} dimensions are supported, got {dimension}"
        )));
    }
    let n = 1usize << dimension;
    let mut b = GraphBuilder::new(n);
    for u in 0..n {
        for bit in 0..dimension {
            b.set(u, u ^ (1 << bit), true);
        }
    }
    Ok(b.build())
}

/// The `rows` by `cols` grid: vertex `r * cols + c` is joined to the vertices beside it in its row
/// and column.
pub fn grid(rows: usize, cols: usize) -> Graph {
    let mut b = GraphBuilder::new(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            let u = r * cols + c;
            if c + 1 < cols {
                b.set(u, u + 1, true);
            }
            if r + 1 < rows {
                b.set(u, u + cols, true);
            }
        }
    }
    b.build()
}

/// An Erdős–Rényi random graph with each pair of vertices an edge independently with probability
/// `mean_degree / (n - 1)`, so that vertices have `mean_degree` neighbors on average. Unlike
/// [`exponential`], which draws a number for every pair, it skips from one edge to the next
/// (Batagelj and Brandes, 2005), so it takes time in proportion to the number of edges and suits
/// large sparse graphs.
pub fn sparse_random(n: usize, mean_degree: f64, rng: &mut Xoshiro) -> Result<Graph, Error> {
    let most = n.saturating_sub(1) as f64;
    if !(0.0..=most).contains(&mean_degree) {
        return Err(Error::Invalid(format!(
            "mean degree must be between 0 and {most} (the number of vertices minus 1), got {mean_degree}"
        )));
    }
    let mut b = GraphBuilder::new(n);
    if n < 2 || mean_degree == 0.0 {
        return Ok(b.build());
    }
    let p = mean_degree / most;
    if p >= 1.0 {
        for u in 0..n {
            for v in u + 1..n {
                b.set(u, v, true);
            }
        }
        return Ok(b.build());
    }
    // walk through the pairs (v, w) with w < v in order, jumping a geometrically distributed
    // number of pairs to the next edge
    let log_q = (1.0 - p).ln();
    let (mut v, mut w) = (1usize, -1i64);
    while v < n {
        // uniform in [0, 1), from the top 53 bits
        let r = (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        w += 1 + ((1.0 - r).ln() / log_q).floor() as i64;
        while w >= v as i64 && v < n {
            w -= v as i64;
            v += 1;
        }
        if v < n {
            b.set(v, w as usize, true);
        }
    }
    Ok(b.build())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::{Randu, Xoshiro};

    #[test]
    fn density_extremes() {
        let mut r = Xoshiro::new(3);
        assert_eq!(exponential(20, 0, &mut r).num_edges(), 0);
        assert_eq!(exponential(20, 1000, &mut r).num_edges(), 190);
        assert_eq!(power(20, 0, &mut r).num_edges(), 0);
        // l = n - 1 at density 1000, so vertex 0 alone is joined to everyone
        assert_eq!(power(20, 1000, &mut r).num_edges(), 19);
        assert!(generate(Model::Geometric, 5, 1001, &mut r).is_err());
    }

    #[test]
    fn weighted_graphs_have_the_requested_total_weight() {
        let g = weighted(50, 150, &mut Xoshiro::new(2)).unwrap();
        assert_eq!(g.total_weight(), 50 * 49 * 150 / 200);
        assert!(g.edges().any(|(_, _, w)| w > 1));
        assert!(weighted(10, 30_000, &mut Randu::new(1)).is_err());
        assert!(weighted(0, 5, &mut Randu::new(1)).is_err());
    }

    #[test]
    fn power_graphs_have_the_expected_edge_count() {
        let (n, density) = (100usize, 500u32);
        let l = (density as usize) * (n - 1) / (2000 - density as usize);
        let g = power(n, density, &mut Randu::new(7));
        assert_eq!(g.num_edges(), (n - l) * l);
    }

    #[test]
    fn hypercubes_and_grids() {
        let q = hypercube(5).unwrap();
        assert_eq!((q.num_vertices(), q.num_edges()), (32, 80));
        assert!(q.has_edge(0b10110, 0b10100) && !q.has_edge(0b10110, 0b10101));
        assert!(q.degrees().iter().all(|&d| d == 5));
        assert_eq!(hypercube(0).unwrap().num_vertices(), 1);
        assert!(hypercube(MAX_HYPERCUBE_DIMENSION + 1).is_err());

        let g = grid(3, 4);
        assert_eq!((g.num_vertices(), g.num_edges()), (12, 3 * 3 + 2 * 4));
        assert!(g.has_edge(5, 6) && g.has_edge(5, 9) && !g.has_edge(3, 4));
    }

    #[test]
    fn sparse_random_graphs_have_the_requested_mean_degree() {
        let n = 20_000;
        let g = sparse_random(n, 3.0, &mut Xoshiro::new(1)).unwrap();
        let mean = 2.0 * g.num_edges() as f64 / n as f64;
        assert!((mean - 3.0).abs() < 0.05, "{mean}");
        // and with the same seed, the same graph
        let again = sparse_random(n, 3.0, &mut Xoshiro::new(1)).unwrap();
        assert!(g.edges().eq(again.edges()));
        assert_eq!(
            sparse_random(30, 29.0, &mut Xoshiro::new(1))
                .unwrap()
                .num_edges(),
            435
        );
        assert_eq!(
            sparse_random(30, 0.0, &mut Xoshiro::new(1))
                .unwrap()
                .num_edges(),
            0
        );
        assert!(sparse_random(30, 30.0, &mut Xoshiro::new(1)).is_err());
        assert!(sparse_random(30, -1.0, &mut Xoshiro::new(1)).is_err());
    }
}
