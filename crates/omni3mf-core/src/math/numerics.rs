//! Safe numeric limits and epsilon constants for geometry calculations.

/// Vertex merge spatial tolerance in millimeters: 10^-6 mm (1 nanometer).
pub const VERTEX_MERGE_EPSILON: f64 = 1e-6;

/// Squared vertex merge tolerance: (10^-6)^2 = 10^-12 mm^2.
pub const VERTEX_MERGE_EPSILON_SQ: f64 = 1e-12;

/// Squared cross-product norm threshold for degenerate triangle detection: 10^-24.
/// Cross-product magnitude ||(v1 - v0) x (v2 - v0)||^2 < 1e-24 corresponds to
/// an area < 0.5e-12 mm^2, safely guarding against division-by-zero or NaNs.
pub const AREA_EPSILON_SQ: f64 = 1e-24;

/// General floating-point tolerance for collinearity and coplanarity tests.
pub const FLOAT_EPSILON: f64 = 1e-12;
