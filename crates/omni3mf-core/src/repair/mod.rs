//! Mesh repair, degenerate triangle pruning, boundary hole patching, and normal orientation.

pub mod boundary;
pub mod degenerate;
pub mod hole_patcher;
pub mod normals;

pub use boundary::extract_boundary_loops;
pub use degenerate::filter_degenerate_triangles;
pub use hole_patcher::{patch_boundary_holes, patch_hole_iterative_fan};
pub use normals::unify_winding_and_orient_outward;
