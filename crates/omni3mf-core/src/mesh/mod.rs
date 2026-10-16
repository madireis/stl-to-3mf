//! Mesh representation, spatial hash welding, and topological queries.

pub mod indexed;
pub mod spatial_hash;
pub mod topology;

pub use indexed::IndexedMesh;
pub use spatial_hash::SpatialHashGrid;
pub use topology::{DirectedEdge, EdgeTopology, UndirectedEdge};
