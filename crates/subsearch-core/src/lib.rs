//! # subsearch
//!
//! Randomised restricted local search for vertex-induced subgraphs with a prescribed number of
//! edges, ported from the USRA `sub_search` program, together with the graph generators and
//! operations of its companion `ggen`, and readers and writers for standard graph formats.
//!
//! ```
//! use subsearch_core::{Graph, SearchParams, search};
//!
//! // find induced 4-vertex paths (3 edges) in a 12-cycle
//! let g = Graph::from_edges(12, (0..12).map(|i| (i, (i + 1) % 12))).unwrap();
//! let result = search(&g, &SearchParams::new(4, 3)).unwrap();
//! let found = result.experiments.iter().find(|e| e.success).unwrap();
//! assert_eq!(g.induced_edge_count(&found.vertices), 3);
//! ```
//!
//! Modules:
//! - [`graph`]: the [`Graph`] type.
//! - [`search`](mod@search): the search itself.
//! - [`generate`] and [`ops`]: random graphs, complements, induced subgraphs, switching,
//!   incidence structures.
//! - [`formats`]: graph6, sparse6, Matrix Market, edge lists, USRA adjacency lists, DOT, GraphML.
//! - [`rng`]: the original programs' generator (for exact reproduction) and xoshiro256++.
//! - [`layout`]: force-directed layout for drawing graphs.
//! - [`legacy`]: byte-for-byte emulation of the original `sub_search` and `ggen` programs.

pub mod formats;
pub mod generate;
pub mod graph;
pub mod layout;
pub mod legacy;
pub mod ops;
pub mod rng;
pub mod search;

pub use formats::Format;
pub use generate::Model;
pub use graph::{Graph, GraphBuilder};
pub use rng::RngKind;
pub use search::{Experiment, SearchParams, SearchResult, search, search_sequential};

/// Errors reported by this crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("vertex {vertex} is out of range [0, {n})")]
    VertexOutOfRange { vertex: i64, n: usize },
    #[error("self-loop at vertex {0}; graphs must be simple")]
    SelfLoop(usize),
    #[error("vertex {0} is listed twice")]
    DuplicateVertex(usize),
    #[error(
        "{0} is not a valid subgraph size: it must be at least 2, larger than the number of fixed vertices, and smaller than the number of graph vertices"
    )]
    InvalidSubgraphSize(i64),
    #[error("{0} is not a valid diversification frequency: it must be at least 1")]
    InvalidDiversificationFrequency(i64),
    #[error(
        "the file holds {0} graphs; choose one (CLI: --index N, Python: index=N) or use them all (CLI: search --all, Python: read_all)"
    )]
    MultipleGraphs(usize),
    #[error("{0}")]
    Invalid(String),
    #[error("{}{msg}", line.map(|l| format!("line {l}: ")).unwrap_or_default())]
    Parse { line: Option<usize>, msg: String },
    /// An error message exactly as the original C program would have printed it.
    #[error("{0}")]
    Legacy(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
