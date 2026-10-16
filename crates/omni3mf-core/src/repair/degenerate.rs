//! Degenerate and collinear zero-area triangle pruning without NaNs or divide-by-zero.

use crate::math::{Point3d, AREA_EPSILON_SQ};

/// Filters out degenerate, collapsed-edge, or collinear zero-area triangles.
///
/// A triangle is considered degenerate if:
/// 1. Any two vertex indices are identical (collapsed edge or point).
/// 2. Any vertex coordinate is non-finite (NaN or Inf).
/// 3. The squared magnitude of the edge cross-product ||(v1 - v0) x (v2 - v0)||^2 is
///    less than `AREA_EPSILON_SQ` (10^-24), meaning its area is effectively zero (< 0.5e-12 mm^2).
pub fn filter_degenerate_triangles(
    raw_faces: &[[u32; 3]],
    vertices: &[Point3d],
) -> Vec<[u32; 3]> {
    let mut valid_faces = Vec::with_capacity(raw_faces.len());

    for &[v0, v1, v2] in raw_faces {
        // Check 1: Duplicate vertex indices (collapsed edge/point)
        if v0 == v1 || v1 == v2 || v2 == v0 {
            continue;
        }

        // Bounds check
        let v0_idx = v0 as usize;
        let v1_idx = v1 as usize;
        let v2_idx = v2 as usize;
        if v0_idx >= vertices.len() || v1_idx >= vertices.len() || v2_idx >= vertices.len() {
            continue;
        }

        let p0 = vertices[v0_idx];
        let p1 = vertices[v1_idx];
        let p2 = vertices[v2_idx];

        // Check 2: Finite float values
        if !p0.is_finite() || !p1.is_finite() || !p2.is_finite() {
            continue;
        }

        // Check 3: Cross product magnitude squared
        let e1 = p1 - p0;
        let e2 = p2 - p0;
        let cross = e1.cross(e2);
        let norm_sq = cross.norm_squared();

        if norm_sq < AREA_EPSILON_SQ || !norm_sq.is_finite() {
            // Sub-epsilon area or collinear vertices: drop safely without NaN hazard
            continue;
        }

        valid_faces.push([v0, v1, v2]);
    }

    valid_faces
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_degenerate_triangles() {
        let vertices = vec![
            Point3d::new(0.0, 0.0, 0.0), // 0
            Point3d::new(1.0, 0.0, 0.0), // 1
            Point3d::new(2.0, 0.0, 0.0), // 2: collinear with 0 and 1
            Point3d::new(0.0, 1.0, 0.0), // 3
        ];

        let faces = vec![
            [0, 1, 3], // Valid triangle
            [0, 0, 1], // Collapsed edge (duplicate index)
            [0, 1, 2], // Collinear zero-area triangle
            [1, 2, 1], // Collapsed edge
        ];

        let cleaned = filter_degenerate_triangles(&faces, &vertices);
        assert_eq!(cleaned.len(), 1);
        assert_eq!(cleaned[0], [0, 1, 3]);
    }
}
