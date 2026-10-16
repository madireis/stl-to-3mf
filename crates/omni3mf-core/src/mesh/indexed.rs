//! Indexed triangle mesh representation.

use serde::{Deserialize, Serialize};

use crate::math::{BoundingBox3d, Point3d, Vector3d, AREA_EPSILON_SQ};

/// An indexed triangle mesh storing unique vertices and indexed triangular faces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexedMesh {
    pub vertices: Vec<Point3d>,
    pub faces: Vec<[u32; 3]>,
    pub normals: Vec<Vector3d>,
}

impl IndexedMesh {
    /// Creates a new empty indexed mesh.
    pub const fn new() -> Self {
        Self {
            vertices: Vec::new(),
            faces: Vec::new(),
            normals: Vec::new(),
        }
    }

    /// Creates an indexed mesh from vertices and faces, recalculating face normals.
    pub fn from_vertices_and_faces(vertices: Vec<Point3d>, faces: Vec<[u32; 3]>) -> Self {
        let mut mesh = Self {
            vertices,
            faces,
            normals: Vec::new(),
        };
        mesh.recalculate_face_normals();
        mesh
    }

    /// Number of vertices in the mesh.
    #[inline(always)]
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Number of triangular faces in the mesh.
    #[inline(always)]
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    /// Checks if the mesh has no vertices or faces.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// Computes the 3D axis-aligned bounding box enclosing all vertices.
    pub fn bounding_box(&self) -> BoundingBox3d {
        BoundingBox3d::from_points(&self.vertices)
    }

    /// Recalculates unit normal vectors for all triangular faces.
    pub fn recalculate_face_normals(&mut self) {
        self.normals.clear();
        self.normals.reserve(self.faces.len());

        for &[v0, v1, v2] in &self.faces {
            let p0 = self.vertices[v0 as usize];
            let p1 = self.vertices[v1 as usize];
            let p2 = self.vertices[v2 as usize];

            let e1 = p1 - p0;
            let e2 = p2 - p0;
            let cross = e1.cross(e2);
            let n_sq = cross.norm_squared();

            if n_sq > AREA_EPSILON_SQ && n_sq.is_finite() {
                self.normals.push(cross * (1.0 / n_sq.sqrt()));
            } else {
                self.normals.push(Vector3d::UNIT_Z);
            }
        }
    }

    /// Computes the total surface area of all triangles.
    pub fn surface_area(&self) -> f64 {
        let mut total_area = 0.0;
        for &[v0, v1, v2] in &self.faces {
            let p0 = self.vertices[v0 as usize];
            let p1 = self.vertices[v1 as usize];
            let p2 = self.vertices[v2 as usize];
            let cross = (p1 - p0).cross(p2 - p0);
            total_area += 0.5 * cross.norm();
        }
        total_area
    }

    /// Computes the signed volume using the divergence theorem:
    /// V = 1/6 * sum_f (v0 . (v1 x v2))
    pub fn signed_volume(&self) -> f64 {
        let mut vol_6x = 0.0;
        for &[v0, v1, v2] in &self.faces {
            let p0 = self.vertices[v0 as usize];
            let p1 = self.vertices[v1 as usize];
            let p2 = self.vertices[v2 as usize];
            vol_6x += p0.dot(p1.cross(p2));
        }
        vol_6x / 6.0
    }

    /// Computes the center of mass (centroid) of the mesh vertices, skipping non-finite coordinates.
    pub fn center_of_mass(&self) -> Point3d {
        if self.vertices.is_empty() {
            return Point3d::ZERO;
        }

        let mut sum = Point3d::ZERO;
        let mut count = 0usize;
        for &v in &self.vertices {
            if v.is_finite() {
                sum += v;
                count += 1;
            }
        }
        if count == 0 {
            return Point3d::ZERO;
        }
        sum / (count as f64)
    }

    /// Returns a new `IndexedMesh` with unreferenced orphan vertices dropped
    /// and faces re-indexed compactly starting from 0.
    pub fn compact(&self) -> Self {
        let mut referenced = vec![false; self.vertices.len()];
        for &[v0, v1, v2] in &self.faces {
            if (v0 as usize) < self.vertices.len() {
                referenced[v0 as usize] = true;
            }
            if (v1 as usize) < self.vertices.len() {
                referenced[v1 as usize] = true;
            }
            if (v2 as usize) < self.vertices.len() {
                referenced[v2 as usize] = true;
            }
        }

        let mut old_to_new = vec![u32::MAX; self.vertices.len()];
        let mut new_vertices = Vec::new();
        for (old_idx, &is_ref) in referenced.iter().enumerate() {
            if is_ref {
                old_to_new[old_idx] = new_vertices.len() as u32;
                new_vertices.push(self.vertices[old_idx]);
            }
        }

        let new_faces: Vec<[u32; 3]> = self
            .faces
            .iter()
            .map(|&[v0, v1, v2]| {
                [
                    old_to_new[v0 as usize],
                    old_to_new[v1 as usize],
                    old_to_new[v2 as usize],
                ]
            })
            .collect();

        Self::from_vertices_and_faces(new_vertices, new_faces)
    }

    /// Prunes unreferenced orphan vertices in-place, updating face indices and recalculating normals.
    pub fn prune_unreferenced_vertices(&mut self) {
        *self = self.compact();
    }
}

impl Default for IndexedMesh {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_center_of_mass_filters_nan() {
        let vertices = vec![
            Point3d::new(0.0, 0.0, 0.0),
            Point3d::new(2.0, 0.0, 0.0),
            Point3d::new(f64::NAN, f64::NAN, f64::NAN),
        ];
        let faces = vec![[0, 1, 0]];
        let mesh = IndexedMesh::from_vertices_and_faces(vertices, faces);
        let com = mesh.center_of_mass();
        assert_eq!(com, Point3d::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn test_compact_prunes_orphans() {
        let vertices = vec![
            Point3d::new(0.0, 0.0, 0.0), // 0: used
            Point3d::new(99.0, 99.0, 99.0), // 1: orphan
            Point3d::new(1.0, 0.0, 0.0), // 2: used
            Point3d::new(0.0, 1.0, 0.0), // 3: used
        ];
        let faces = vec![[0, 2, 3]];
        let mesh = IndexedMesh::from_vertices_and_faces(vertices, faces);
        assert_eq!(mesh.vertex_count(), 4);

        let compacted = mesh.compact();
        assert_eq!(compacted.vertex_count(), 3);
        assert_eq!(compacted.faces, vec![[0, 1, 2]]);
        assert_eq!(compacted.vertices[0], Point3d::new(0.0, 0.0, 0.0));
        assert_eq!(compacted.vertices[1], Point3d::new(1.0, 0.0, 0.0));
        assert_eq!(compacted.vertices[2], Point3d::new(0.0, 1.0, 0.0));
    }
}
