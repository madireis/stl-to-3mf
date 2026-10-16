//! Spatial hash grid for O(N) coincident vertex welding within 10^-6 mm.

use std::collections::HashMap;

use crate::math::{Point3d, VERTEX_MERGE_EPSILON};
use crate::stl::RawFacet;

/// A uniform 3D spatial hash grid for deduplicating coincident vertices in expected O(N) time.
pub struct SpatialHashGrid {
    cell_size: f64,
    epsilon_sq: f64,
    table: HashMap<(i64, i64, i64), Vec<u32>>,
    pub vertices: Vec<Point3d>,
}

impl SpatialHashGrid {
    /// Creates a new spatial hash grid with the specified merge tolerance.
    pub fn new(tolerance: f64, estimated_vertices: usize) -> Self {
        let cell_size = if tolerance > 0.0 {
            tolerance
        } else {
            VERTEX_MERGE_EPSILON
        };
        let epsilon_sq = cell_size * cell_size;

        Self {
            cell_size,
            epsilon_sq,
            table: HashMap::with_capacity(estimated_vertices),
            vertices: Vec::with_capacity(estimated_vertices),
        }
    }

    #[inline(always)]
    fn cell_coord(&self, val: f64) -> i64 {
        if !val.is_finite() {
            return 0;
        }
        (val / self.cell_size).floor() as i64
    }

    /// Inserts a vertex or retrieves the index of an existing coincident vertex within tolerance.
    pub fn insert_or_get(&mut self, p: Point3d) -> u32 {
        let cx = self.cell_coord(p.x);
        let cy = self.cell_coord(p.y);
        let cz = self.cell_coord(p.z);

        // Query 3x3x3 = 27 neighbor cells
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let key = (cx + dx, cy + dy, cz + dz);
                    if let Some(candidates) = self.table.get(&key) {
                        for &idx in candidates {
                            let existing = self.vertices[idx as usize];
                            let dist_sq = (p - existing).norm_squared();
                            if dist_sq <= self.epsilon_sq {
                                return idx;
                            }
                        }
                    }
                }
            }
        }

        // No existing vertex within tolerance: add new canonical vertex
        let new_idx = self.vertices.len() as u32;
        self.vertices.push(p);
        self.table.entry((cx, cy, cz)).or_default().push(new_idx);
        new_idx
    }

    /// Welds an unindexed slice of raw STL facets into indexed vertices and faces.
    pub fn weld_raw_facets(
        facets: &[RawFacet],
        tolerance: f64,
    ) -> (Vec<Point3d>, Vec<[u32; 3]>) {
        let est_verts = facets.len().saturating_mul(3) / 2;
        let mut grid = Self::new(tolerance, est_verts);
        let mut faces = Vec::with_capacity(facets.len());

        for f in facets {
            let v0 = grid.insert_or_get(f.v0);
            let v1 = grid.insert_or_get(f.v1);
            let v2 = grid.insert_or_get(f.v2);
            faces.push([v0, v1, v2]);
        }

        (grid.vertices, faces)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_welding_coincident_vertices() {
        let mut grid = SpatialHashGrid::new(1e-6, 16);

        let p0 = Point3d::new(10.0, 20.0, 30.0);
        // Perturb by 1e-7 mm (< 1e-6 mm tolerance)
        let p1 = Point3d::new(10.0 + 1e-7, 20.0 - 1e-7, 30.0 + 1e-7);
        // Perturb by 2e-6 mm (> 1e-6 mm tolerance)
        let p2 = Point3d::new(10.0 + 2e-6, 20.0, 30.0);

        let idx0 = grid.insert_or_get(p0);
        let idx1 = grid.insert_or_get(p1);
        let idx2 = grid.insert_or_get(p2);

        assert_eq!(idx0, idx1, "Points within 1e-6 mm must be welded to the same index");
        assert_ne!(idx0, idx2, "Points separated by > 1e-6 mm must remain distinct");
        assert_eq!(grid.vertices.len(), 2);
    }
}
