//! Disconnected shell separation and topological face adjacency graph.

pub mod graph;
pub mod separator;

pub use graph::{DisjointSetUnion, FaceAdjacencyGraph};
pub use separator::{separate_connected_shells, MeshShell};
