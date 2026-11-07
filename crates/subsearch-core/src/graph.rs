//! Simple undirected graphs on vertices `0..n`.
//!
//! [`Graph`] stores both a bitset adjacency matrix (O(1) edge queries, `n * n / 8` bytes) and
//! sorted neighbour lists (fast traversal), mirroring the original C programs, which kept a
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

    /// Finishes the graph, computing sorted neighbour lists.
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

    /// The neighbours of `u`, in ascending order.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_sorted_symmetric_neighbour_lists() {
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
    fn counts_components() {
        let g = Graph::from_edges(6, [(0, 1), (1, 2), (4, 5)]).unwrap();
        assert_eq!(g.components().1, 3);
        assert_eq!(g.induced_edge_count(&[0, 1, 2, 4]), 2);
    }
}
