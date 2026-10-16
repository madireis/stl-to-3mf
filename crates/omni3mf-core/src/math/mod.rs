//! Mathematical primitives, safe vector arithmetic, and numeric limits for 3D geometry.

pub mod bbox;
pub mod numerics;
pub mod vec3;

pub use bbox::BoundingBox3d;
pub use numerics::{AREA_EPSILON_SQ, FLOAT_EPSILON, VERTEX_MERGE_EPSILON, VERTEX_MERGE_EPSILON_SQ};
pub use vec3::{Point3d, Vector3d};
