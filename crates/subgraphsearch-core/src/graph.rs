//! Simple undirected graphs on vertices `0..n`.
//!
//! [`Graph`] stores both a bitset adjacency matrix (O(1) edge queries, `n * n / 8` bytes) and
//! sorted neighbor lists (fast traversal), mirroring the original C programs, which kept a
//! `char` adjacency matrix alongside adjacency lists for the same reasons.

use crate::Error;

/// An undirected graph without self-loops or multiple edges.
#[derive(Clone, PartialEq, Eq)]
pub struct Graph {
    n: usize,
    words_per_row: usize,
    bits: Vec<u64>,
    neighbors: Vec<Vec<u32>>,
    num_edges: usize,
}

impl std::fmt::Debug for Graph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Graph")
            .field("vertices", &self.n)
            .field("edges", &self.num_edges)
            .finish()
    }
}

/// Whether bit `v` of an adjacency-matrix row (see [`Graph::row`]) is set, as 0 or 1.
#[inline(always)]
pub fn bit(row: &[u64], v: usize) -> u64 {
    row[v >> 6] >> (v & 63) & 1
}

/// Incrementally builds a [`Graph`]. Repeated edges are ignored.
#[derive(Clone, Debug)]
pub struct GraphBuilder {
    n: usize,
    words_per_row: usize,
    bits: Vec<u64>,
}

impl GraphBuilder {
    /// Starts an empty graph on `n` vertices.
    pub fn new(n: usize) -> Self {
        let words_per_row = n.div_ceil(64);
        GraphBuilder {
            n,
            words_per_row,
            bits: vec![0; words_per_row * n],
        }
    }

    /// Number of vertices.
    pub fn num_vertices(&self) -> usize {
        self.n
    }

    /// Adds the edge `{u, v}`. Fails if either endpoint is out of range or `u == v`.
    pub fn add_edge(&mut self, u: usize, v: usize) -> Result<(), Error> {
        self.check(u)?;
        self.check(v)?;
        if u == v {
            return Err(Error::SelfLoop(u));
        }
        self.set(u, v, true);
        Ok(())
    }

    /// Sets or clears the edge `{u, v}`; both endpoints must be valid and distinct.
    pub(crate) fn set(&mut self, u: usize, v: usize, present: bool) {
        let (wu, bu) = (u * self.words_per_row + v / 64, 1u64 << (v % 64));
        let (wv, bv) = (v * self.words_per_row + u / 64, 1u64 << (u % 64));
        if present {
            self.bits[wu] |= bu;
            self.bits[wv] |= bv;
        } else {
            self.bits[wu] &= !bu;
            self.bits[wv] &= !bv;
        }
    }

    pub(crate) fn get(&self, u: usize, v: usize) -> bool {
        self.bits[u * self.words_per_row + v / 64] >> (v % 64) & 1 == 1
    }

    fn check(&self, v: usize) -> Result<(), Error> {
        if v >= self.n {
            return Err(Error::VertexOutOfRange {
                vertex: v as i64,
                n: self.n,
            });
        }
        Ok(())
    }

    /// Finishes the graph, computing sorted neighbor lists.
    pub fn build(self) -> Graph {
        let GraphBuilder {
            n,
            words_per_row,
            bits,
        } = self;
        let mut neighbors = Vec::with_capacity(n);
        let mut twice_edges = 0;
        for u in 0..n {
            let row = &bits[u * words_per_row..(u + 1) * words_per_row];
            let mut list = Vec::new();
            for (w, &word) in row.iter().enumerate() {
                let mut word = word;
                while word != 0 {
                    let b = word.trailing_zeros() as usize;
                    list.push((w * 64 + b) as u32);
                    word &= word - 1;
                }
            }
            twice_edges += list.len();
            neighbors.push(list);
        }
        Graph {
            n,
            words_per_row,
            bits,
            neighbors,
            num_edges: twice_edges / 2,
        }
    }
}

impl Graph {
    /// The graph on integer vectors of length `dim`, held one after another in `vectors`, with two
    /// joined when their inner product is one of `allowed`.
    ///
    /// Sets of vectors with prescribed inner products are then cliques: rows of a Hadamard matrix
    /// are pairwise orthogonal (`allowed = [0]`), equiangular lines have inner products `±c`. The
    /// inner products are computed in parallel straight into the adjacency matrix, so no edge list
    /// is built, which matters when there are hundreds of millions of edges. Entries must be small
    /// enough that inner products fit an `i64`.
    pub fn from_vectors(vectors: &[i64], dim: usize, allowed: &[i64]) -> Result<Graph, Error> {
        use rayon::prelude::*;

        if dim == 0 || !vectors.len().is_multiple_of(dim) {
            return Err(Error::Invalid(format!(
                "{} entries are not a whole number of vectors of length {dim}",
                vectors.len()
            )));
        }
        let n = vectors.len() / dim;
        let mut allowed = allowed.to_vec();
        allowed.sort_unstable();
        allowed.dedup();
        let mut b = GraphBuilder::new(n);
        let words_per_row = b.words_per_row;
        if words_per_row > 0 {
            b.bits
                .par_chunks_mut(words_per_row)
                .enumerate()
                .for_each(|(u, row)| {
                    let x = &vectors[u * dim..(u + 1) * dim];
                    for (v, y) in vectors.chunks_exact(dim).enumerate() {
                        if v == u {
                            continue;
                        }
                        let dot: i64 = x.iter().zip(y).map(|(a, b)| a * b).sum();
                        if allowed.binary_search(&dot).is_ok() {
                            row[v / 64] |= 1 << (v % 64);
                        }
                    }
                });
        }
        Ok(b.build())
    }

    /// Builds a graph on `n` vertices from an edge list.
    pub fn from_edges<I>(n: usize, edges: I) -> Result<Graph, Error>
    where
        I: IntoIterator<Item = (usize, usize)>,
    {
        let mut b = GraphBuilder::new(n);
        for (u, v) in edges {
            b.add_edge(u, v)?;
        }
        Ok(b.build())
    }

    /// The empty graph on `n` vertices.
    pub fn empty(n: usize) -> Graph {
        GraphBuilder::new(n).build()
    }

    /// Number of vertices.
    #[inline]
    pub fn num_vertices(&self) -> usize {
        self.n
    }

    /// Number of edges.
    #[inline]
    pub fn num_edges(&self) -> usize {
        self.num_edges
    }

    /// Whether `{u, v}` is an edge. Panics if either vertex is out of range.
    #[inline]
    pub fn has_edge(&self, u: usize, v: usize) -> bool {
        self.bits[u * self.words_per_row + v / 64] >> (v % 64) & 1 == 1
    }

    /// Row `u` of the adjacency matrix as a bitset: bit `v % 64` of word `v / 64` is set iff
    /// `{u, v}` is an edge. Test bits with [`bit`].
    #[inline]
    pub fn row(&self, u: usize) -> &[u64] {
        &self.bits[u * self.words_per_row..(u + 1) * self.words_per_row]
    }

    /// The neighbors of `u`, in ascending order.
    #[inline]
    pub fn neighbors(&self, u: usize) -> &[u32] {
        &self.neighbors[u]
    }

    /// The degree of `u`.
    #[inline]
    pub fn degree(&self, u: usize) -> usize {
        self.neighbors[u].len()
    }

    /// All degrees, indexed by vertex.
    pub fn degrees(&self) -> Vec<usize> {
        self.neighbors.iter().map(Vec::len).collect()
    }

    /// Edges `(u, v)` with `u < v`, in lexicographic order.
    pub fn edges(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.neighbors.iter().enumerate().flat_map(|(u, list)| {
            list.iter()
                .map(|&v| v as usize)
                .filter(move |&v| v > u)
                .map(move |v| (u, v))
        })
    }

    /// Edge density `m / (n choose 2)`, or 0 for graphs with fewer than two vertices.
    pub fn density(&self) -> f64 {
        if self.n < 2 {
            return 0.0;
        }
        self.num_edges as f64 / (self.n as f64 * (self.n as f64 - 1.0) / 2.0)
    }

    /// Converts back into a builder, e.g. to modify the graph.
    pub fn into_builder(self) -> GraphBuilder {
        GraphBuilder {
            n: self.n,
            words_per_row: self.words_per_row,
            bits: self.bits,
        }
    }

    /// Number of edges among `vertices`, which must be distinct.
    pub fn induced_edge_count(&self, vertices: &[usize]) -> usize {
        let mut count = 0;
        for (i, &u) in vertices.iter().enumerate() {
            for &v in &vertices[i + 1..] {
                if self.has_edge(u, v) {
                    count += 1;
                }
            }
        }
        count
    }

    /// Connected components, as a component index per vertex plus the number of components.
    pub fn components(&self) -> (Vec<usize>, usize) {
        let mut comp = vec![usize::MAX; self.n];
        let mut count = 0;
        let mut stack = Vec::new();
        for s in 0..self.n {
            if comp[s] != usize::MAX {
                continue;
            }
            comp[s] = count;
            stack.push(s);
            while let Some(u) = stack.pop() {
                for &v in self.neighbors(u) {
                    let v = v as usize;
                    if comp[v] == usize::MAX {
                        comp[v] = count;
                        stack.push(v);
                    }
                }
            }
            count += 1;
        }
        (comp, count)
    }
}

/// An undirected graph whose edges carry small positive integer weights (1 to 255), as used by
/// the original `wsub_search` and `wggen`. Weights live in an `n x n` byte matrix (O(1) lookup, `n * n`
/// bytes) alongside sorted neighbor lists.
#[derive(Clone, PartialEq, Eq)]
pub struct WeightedGraph {
    n: usize,
    weights: Vec<u8>,
    neighbors: Vec<Vec<u32>>,
    num_edges: usize,
    total_weight: u64,
}

impl std::fmt::Debug for WeightedGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WeightedGraph")
            .field("vertices", &self.n)
            .field("edges", &self.num_edges)
            .field("total_weight", &self.total_weight)
            .finish()
    }
}

impl WeightedGraph {
    /// Builds a weighted graph from `(u, v, weight)` triples. Weights must be between 1 and
    /// 255; if an edge is listed more than once, the last weight given wins.
    pub fn from_weighted_edges<I>(n: usize, edges: I) -> Result<WeightedGraph, Error>
    where
        I: IntoIterator<Item = (usize, usize, u32)>,
    {
        let mut weights = vec![0u8; n * n];
        for (u, v, w) in edges {
            for x in [u, v] {
                if x >= n {
                    return Err(Error::VertexOutOfRange {
                        vertex: x as i64,
                        n,
                    });
                }
            }
            if u == v {
                return Err(Error::SelfLoop(u));
            }
            if !(1..=255).contains(&w) {
                return Err(Error::Invalid(format!(
                    "edge {{{u}, {v}}} has weight {w}; weights must be between 1 and 255"
                )));
            }
            weights[u * n + v] = w as u8;
            weights[v * n + u] = w as u8;
        }
        Ok(Self::from_matrix(n, weights))
    }

    /// Every edge of `g` with weight 1.
    pub fn from_graph(g: &Graph) -> WeightedGraph {
        let n = g.num_vertices();
        Self::from_weighted_edges(n, g.edges().map(|(u, v)| (u, v, 1))).unwrap()
    }

    pub(crate) fn from_matrix(n: usize, weights: Vec<u8>) -> WeightedGraph {
        let mut neighbors = Vec::with_capacity(n);
        let (mut twice_edges, mut twice_weight) = (0usize, 0u64);
        for u in 0..n {
            let row = &weights[u * n..(u + 1) * n];
            let list: Vec<u32> = (0..n).filter(|&v| row[v] != 0).map(|v| v as u32).collect();
            twice_edges += list.len();
            twice_weight += row.iter().map(|&w| u64::from(w)).sum::<u64>();
            neighbors.push(list);
        }
        WeightedGraph {
            n,
            weights,
            neighbors,
            num_edges: twice_edges / 2,
            total_weight: twice_weight / 2,
        }
    }

    /// Number of vertices.
    #[inline]
    pub fn num_vertices(&self) -> usize {
        self.n
    }

    /// Number of edges.
    #[inline]
    pub fn num_edges(&self) -> usize {
        self.num_edges
    }

    /// Sum of all edge weights.
    #[inline]
    pub fn total_weight(&self) -> u64 {
        self.total_weight
    }

    /// The weight of `{u, v}`, or 0 if it is not an edge.
    #[inline]
    pub fn weight(&self, u: usize, v: usize) -> u8 {
        self.weights[u * self.n + v]
    }

    /// Row `u` of the weight matrix.
    #[inline]
    pub fn row(&self, u: usize) -> &[u8] {
        &self.weights[u * self.n..(u + 1) * self.n]
    }

    /// The neighbors of `u`, in ascending order.
    #[inline]
    pub fn neighbors(&self, u: usize) -> &[u32] {
        &self.neighbors[u]
    }

    /// Edges `(u, v, weight)` with `u < v`, in lexicographic order.
    pub fn edges(&self) -> impl Iterator<Item = (usize, usize, u8)> + '_ {
        self.neighbors
            .iter()
            .enumerate()
            .flat_map(move |(u, list)| {
                list.iter()
                    .map(|&v| v as usize)
                    .filter(move |&v| v > u)
                    .map(move |v| (u, v, self.weight(u, v)))
            })
    }

    /// Total weight of the edges among `vertices`, which must be distinct.
    pub fn induced_weight(&self, vertices: &[usize]) -> u64 {
        let mut total = 0;
        for (i, &u) in vertices.iter().enumerate() {
            for &v in &vertices[i + 1..] {
                total += u64::from(self.weight(u, v));
            }
        }
        total
    }

    /// The underlying unweighted graph.
    pub fn structure(&self) -> Graph {
        Graph::from_edges(self.n, self.edges().map(|(u, v, _)| (u, v))).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphs_of_vectors_join_allowed_inner_products() {
        // the 8 vectors of {+-1}^3: orthogonal pairs never occur (inner products are odd), and
        // inner product -3 joins each vector to its negation
        let cube: Vec<i64> = (0..8)
            .flat_map(|i| (0..3).map(move |b| if i >> b & 1 == 1 { -1 } else { 1 }))
            .collect();
        assert_eq!(Graph::from_vectors(&cube, 3, &[0]).unwrap().num_edges(), 0);
        let opposite = Graph::from_vectors(&cube, 3, &[-3]).unwrap();
        assert_eq!(opposite.num_edges(), 4);
        assert!(opposite.has_edge(0, 7) && opposite.has_edge(2, 5));
        // inner products 1 and -1 join vectors differing in one or two places: 8 * (3 + 3) / 2 pairs
        let near = Graph::from_vectors(&cube, 3, &[1, -1, 1]).unwrap();
        assert_eq!(near.num_edges(), 24);
        // the same as computing every pair by hand, on vectors with more than 64 rows
        let many: Vec<i64> = (0..100i64)
            .flat_map(|i| [i % 3 - 1, i % 5 - 2, i % 7 - 3])
            .collect();
        let g = Graph::from_vectors(&many, 3, &[0, 2]).unwrap();
        let dot = |u: usize, v: usize| {
            (0..3)
                .map(|k| many[3 * u + k] * many[3 * v + k])
                .sum::<i64>()
        };
        for u in 0..100 {
            for v in 0..100 {
                assert_eq!(
                    g.has_edge(u, v),
                    u != v && [0, 2].contains(&dot(u, v)),
                    "{u} {v}"
                );
            }
        }
        assert!(Graph::from_vectors(&[1, 2, 3], 2, &[0]).is_err());
        assert!(Graph::from_vectors(&[1, 2], 0, &[0]).is_err());
        assert_eq!(Graph::from_vectors(&[], 4, &[0]).unwrap().num_vertices(), 0);
    }

    #[test]
    fn builds_sorted_symmetric_neighbor_lists() {
        let g = Graph::from_edges(70, [(3, 1), (1, 65), (1, 3), (0, 69)]).unwrap();
        assert_eq!(g.num_edges(), 3);
        assert_eq!(g.neighbors(1), &[3, 65]);
        assert_eq!(g.neighbors(65), &[1]);
        assert!(g.has_edge(69, 0) && !g.has_edge(2, 3));
        assert_eq!(
            g.edges().collect::<Vec<_>>(),
            vec![(0, 69), (1, 3), (1, 65)]
        );
    }

    #[test]
    fn rejects_bad_edges() {
        assert!(matches!(
            Graph::from_edges(3, [(0, 3)]),
            Err(Error::VertexOutOfRange { .. })
        ));
        assert!(matches!(
            Graph::from_edges(3, [(1, 1)]),
            Err(Error::SelfLoop(1))
        ));
    }

    #[test]
    fn weighted_graphs() {
        let g = WeightedGraph::from_weighted_edges(4, [(0, 1, 3), (2, 1, 5), (0, 1, 2)]).unwrap();
        assert_eq!((g.num_edges(), g.total_weight()), (2, 7));
        assert_eq!((g.weight(1, 0), g.weight(1, 2), g.weight(0, 3)), (2, 5, 0));
        assert_eq!(g.edges().collect::<Vec<_>>(), vec![(0, 1, 2), (1, 2, 5)]);
        assert_eq!(g.induced_weight(&[0, 1, 2]), 7);
        assert_eq!(g.structure().num_edges(), 2);
        assert_eq!(WeightedGraph::from_graph(&g.structure()).total_weight(), 2);
        assert!(WeightedGraph::from_weighted_edges(3, [(0, 1, 0)]).is_err());
        assert!(WeightedGraph::from_weighted_edges(3, [(0, 1, 256)]).is_err());
    }

    #[test]
    fn counts_components() {
        let g = Graph::from_edges(6, [(0, 1), (1, 2), (4, 5)]).unwrap();
        assert_eq!(g.components().1, 3);
        assert_eq!(g.induced_edge_count(&[0, 1, 2, 4]), 2);
    }
}
