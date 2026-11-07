//! Random graph generators, ported from USRA's `ggen`.
//!
//! Densities are in *per mille* (0..=1000), as in `ggen`. Each generator is generic over the
//! random number generator: with [`crate::rng::Randu`] seeded as `ggen` is, the output is
//! identical to `ggen`'s.

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::graph::{Graph, GraphBuilder};
use crate::rng::SearchRng;

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
    fn power_graphs_have_the_expected_edge_count() {
        let (n, density) = (100usize, 500u32);
        let l = (density as usize) * (n - 1) / (2000 - density as usize);
        let g = power(n, density, &mut Randu::new(7));
        assert_eq!(g.num_edges(), (n - l) * l);
    }
}
