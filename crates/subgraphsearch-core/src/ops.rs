//! Graph operations from the original `ggen`: complements, induced subgraphs, common neighborhoods
//! and (Seidel) switching, plus graphs built from incidence structures.

use crate::Error;
use crate::graph::{Graph, GraphBuilder};

/// The complement of `g`.
pub fn complement(g: &Graph) -> Graph {
    let n = g.num_vertices();
    let mut b = GraphBuilder::new(n);
    for i in 0..n {
        for j in i + 1..n {
            if !g.has_edge(i, j) {
                b.set(i, j, true);
            }
        }
    }
    b.build()
}

fn check_set(g: &Graph, vertices: &[usize]) -> Result<Vec<bool>, Error> {
    let n = g.num_vertices();
    let mut member = vec![false; n];
    for &v in vertices {
        if v >= n {
            return Err(Error::VertexOutOfRange {
                vertex: v as i64,
                n,
            });
        }
        if std::mem::replace(&mut member[v], true) {
            return Err(Error::DuplicateVertex(v));
        }
    }
    Ok(member)
}

/// The subgraph induced on `vertices`, with vertex `i` of the result being `vertices[i]`.
pub fn induced(g: &Graph, vertices: &[usize]) -> Result<Graph, Error> {
    check_set(g, vertices)?;
    let mut b = GraphBuilder::new(vertices.len());
    for (i, &u) in vertices.iter().enumerate() {
        for (j, &v) in vertices.iter().enumerate().skip(i + 1) {
            if g.has_edge(u, v) {
                b.set(i, j, true);
            }
        }
    }
    Ok(b.build())
}

/// The vertices not in `set`, ascending.
pub fn rest(g: &Graph, set: &[usize]) -> Result<Vec<usize>, Error> {
    let member = check_set(g, set)?;
    Ok((0..g.num_vertices()).filter(|&v| !member[v]).collect())
}

/// Vertices outside `set` adjacent to every vertex of `set` (ascending). `set` must be non-empty.
pub fn common_neighborhood(g: &Graph, set: &[usize]) -> Result<Vec<usize>, Error> {
    common(g, set, true)
}

/// Vertices outside `set` adjacent to no vertex of `set` (ascending). `set` must be non-empty.
pub fn common_non_neighborhood(g: &Graph, set: &[usize]) -> Result<Vec<usize>, Error> {
    common(g, set, false)
}

fn common(g: &Graph, set: &[usize], adjacent: bool) -> Result<Vec<usize>, Error> {
    let member = check_set(g, set)?;
    if set.is_empty() {
        return Err(Error::Invalid("the vertex set must not be empty".into()));
    }
    Ok((0..g.num_vertices())
        .filter(|&v| !member[v] && set.iter().all(|&f| g.has_edge(f, v) == adjacent))
        .collect())
}

/// Switches `g` with respect to `set` (Seidel switching): edges between `set` and the rest become
/// non-edges and vice versa; edges within either part are unchanged.
pub fn switch(g: &Graph, set: &[usize]) -> Result<Graph, Error> {
    let member = check_set(g, set)?;
    let n = g.num_vertices();
    let mut b = g.clone().into_builder();
    for &f in set {
        for v in (0..n).filter(|&v| !member[v]) {
            let present = b.get(f, v);
            b.set(f, v, !present);
        }
    }
    Ok(b.build())
}

/// A family of `sets` over the symbols `0..symbols` (an incidence structure).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Incidence {
    symbols: usize,
    sets: Vec<Vec<bool>>,
}

impl Incidence {
    /// Builds an incidence structure; repeated symbols within a set are ignored.
    pub fn new(symbols: usize, sets: &[Vec<usize>]) -> Result<Self, Error> {
        let mut rows = Vec::with_capacity(sets.len());
        for set in sets {
            let mut row = vec![false; symbols];
            for &s in set {
                if s >= symbols {
                    return Err(Error::VertexOutOfRange {
                        vertex: s as i64,
                        n: symbols,
                    });
                }
                row[s] = true;
            }
            rows.push(row);
        }
        Ok(Incidence {
            symbols,
            sets: rows,
        })
    }

    /// Number of symbols.
    pub fn num_symbols(&self) -> usize {
        self.symbols
    }

    /// Number of sets.
    pub fn num_sets(&self) -> usize {
        self.sets.len()
    }

    /// The graph on symbols where two symbols are adjacent iff exactly `k` sets contain both.
    pub fn symbol_graph(&self, k: usize) -> Graph {
        let mut b = GraphBuilder::new(self.symbols);
        for i in 0..self.symbols {
            let containing: Vec<&Vec<bool>> = self.sets.iter().filter(|s| s[i]).collect();
            for j in i + 1..self.symbols {
                if containing.iter().filter(|s| s[j]).count() == k {
                    b.set(i, j, true);
                }
            }
        }
        b.build()
    }

    /// The graph on sets where two sets are adjacent iff they share exactly `k` symbols.
    pub fn set_graph(&self, k: usize) -> Graph {
        let n = self.sets.len();
        let mut b = GraphBuilder::new(n);
        for i in 0..n {
            let members: Vec<usize> = (0..self.symbols).filter(|&s| self.sets[i][s]).collect();
            for j in i + 1..n {
                if members.iter().filter(|&&s| self.sets[j][s]).count() == k {
                    b.set(i, j, true);
                }
            }
        }
        b.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(n: usize) -> Graph {
        Graph::from_edges(n, (1..n).map(|i| (i - 1, i))).unwrap()
    }

    #[test]
    fn complement_and_switch() {
        let g = path(4);
        assert_eq!(complement(&g).num_edges(), 3);
        assert_eq!(complement(&complement(&g)), g);
        // switching {0}: 0-1 removed, 0-2 and 0-3 added
        let s = switch(&g, &[0]).unwrap();
        assert!(!s.has_edge(0, 1) && s.has_edge(0, 2) && s.has_edge(0, 3) && s.has_edge(1, 2));
    }

    #[test]
    fn neighborhoods_and_induced() {
        let g = path(5);
        assert_eq!(common_neighborhood(&g, &[1, 3]).unwrap(), vec![2]);
        assert_eq!(common_non_neighborhood(&g, &[0]).unwrap(), vec![2, 3, 4]);
        let h = induced(&g, &[3, 2, 0]).unwrap();
        assert_eq!(h.edges().collect::<Vec<_>>(), vec![(0, 1)]);
        assert!(induced(&g, &[1, 1]).is_err());
    }

    #[test]
    fn incidence_graphs() {
        // Fano plane: lines pairwise meet in 1 point, points pairwise lie on 1 line
        let lines = [
            [0, 1, 2],
            [0, 3, 4],
            [0, 5, 6],
            [1, 3, 5],
            [1, 4, 6],
            [2, 3, 6],
            [2, 4, 5],
        ];
        let inc = Incidence::new(7, &lines.map(|l| l.to_vec())).unwrap();
        assert_eq!(inc.set_graph(1).num_edges(), 21);
        assert_eq!(inc.symbol_graph(1).num_edges(), 21);
        assert_eq!(inc.symbol_graph(2).num_edges(), 0);
    }
}
