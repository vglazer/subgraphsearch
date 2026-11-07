//! Force-directed graph layout (Fruchterman–Reingold), for visualisation.
//!
//! Repulsion is computed exactly between all pairs (in parallel), which is fast enough for a few
//! thousand vertices; that is also about as many as a drawing can usefully show.

use rayon::prelude::*;

use crate::graph::Graph;
use crate::rng::Xoshiro;

/// Positions in the unit square, one `(x, y)` per vertex.
pub fn spring(graph: &Graph, iterations: usize, seed: u64) -> Vec<(f64, f64)> {
    let n = graph.num_vertices();
    let mut rng = Xoshiro::new(seed);
    let mut pos: Vec<(f64, f64)> = (0..n).map(|_| (rng.next_f64(), rng.next_f64())).collect();
    if n < 2 {
        return pos.into_iter().map(|_| (0.5, 0.5)).collect();
    }
    // ideal edge length for n vertices spread over the unit square
    let k = (1.0 / n as f64).sqrt();
    let mut temperature = 0.1;
    let cooling = temperature / (iterations as f64 + 1.0);
    for _ in 0..iterations {
        let displacement: Vec<(f64, f64)> = (0..n)
            .into_par_iter()
            .map(|i| {
                let (xi, yi) = pos[i];
                let (mut dx, mut dy) = (0.0, 0.0);
                for (j, &(xj, yj)) in pos.iter().enumerate() {
                    if i == j {
                        continue;
                    }
                    let (ex, ey) = (xi - xj, yi - yj);
                    let d2 = (ex * ex + ey * ey).max(1e-9);
                    // repulsion k^2 / d, along the unit vector: k^2 * e / d^2
                    let f = k * k / d2;
                    dx += ex * f;
                    dy += ey * f;
                }
                for &j in graph.neighbors(i) {
                    let (xj, yj) = pos[j as usize];
                    let (ex, ey) = (xi - xj, yi - yj);
                    let d = (ex * ex + ey * ey).sqrt().max(1e-9);
                    // attraction d^2 / k, along the unit vector: d * e / k
                    let f = d / k;
                    dx -= ex * f;
                    dy -= ey * f;
                }
                (dx, dy)
            })
            .collect();
        for (p, (dx, dy)) in pos.iter_mut().zip(displacement) {
            let len = (dx * dx + dy * dy).sqrt().max(1e-9);
            let step = len.min(temperature);
            p.0 += dx / len * step;
            p.1 += dy / len * step;
        }
        temperature -= cooling;
    }
    normalise(&mut pos);
    pos
}

/// Scales positions into the unit square, preserving the aspect ratio.
fn normalise(pos: &mut [(f64, f64)]) {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(x, y) in pos.iter() {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let span = (x1 - x0).max(y1 - y0).max(1e-12);
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    for p in pos.iter_mut() {
        p.0 = 0.5 + (p.0 - cx) / span;
        p.1 = 0.5 + (p.1 - cy) / span;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbours_end_up_closer_than_non_neighbours() {
        // two triangles joined by nothing: within-triangle distances should be small
        let g = Graph::from_edges(6, [(0, 1), (1, 2), (0, 2), (3, 4), (4, 5), (3, 5)]).unwrap();
        let p = spring(&g, 200, 1);
        let d = |a: usize, b: usize| ((p[a].0 - p[b].0).powi(2) + (p[a].1 - p[b].1).powi(2)).sqrt();
        assert!(d(0, 1) < d(0, 4) && d(3, 5) < d(2, 3));
        assert!(
            p.iter()
                .all(|&(x, y)| (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y))
        );
    }
}
