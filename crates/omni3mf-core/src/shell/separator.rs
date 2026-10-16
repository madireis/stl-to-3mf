//! Mesh shell separation, compact vertex re-indexing, and per-shell geometric metrics.

use serde::{Deserialize, Serialize};

use super::graph::FaceAdjacencyGraph;
use crate::math::{BoundingBox3d, Point3d};
use crate::mesh::{EdgeTopology, IndexedMesh};

/// A discrete, connected topological shell extracted from a mesh.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshShell {
    /// 1-based unique identifier for this shell (e.g. 1, 2, ...).
    pub id: u32,
    /// Human-readable name for the shell (e.g. "Shell_1").
    pub name: String,
    /// Compact 0-indexed triangle mesh containing only vertices referenced by this shell.
    pub mesh: IndexedMesh,
    /// 3D axis-aligned bounding box.
    pub bbox: BoundingBox3d,
    /// Centroid / center of mass of the shell's vertices.
    pub center_of_mass: Point3d,
    /// Signed volume computed via the divergence theorem (positive for outward-oriented shells).
    pub signed_volume: f64,
    /// Total surface area of all triangular faces in the shell.
    pub surface_area: f64,
    /// Whether the shell is a closed, watertight 2-manifold without boundary edges.
    pub is_watertight: bool,
}

/// Separates an `IndexedMesh` into discrete connected topological shells.
///
/// 1. Uses a dual face adjacency graph and DSU (Disjoint Set Union) to find connected components.
/// 2. For each shell, performs compact 0-based re-indexing of vertices, pruning unreferenced orphan vertices.
/// 3. Computes per-shell bounding box, center of mass, signed volume, surface area, and watertightness.
pub fn separate_connected_shells(mesh: &IndexedMesh) -> Vec<MeshShell> {
    if mesh.faces.is_empty() || mesh.vertices.is_empty() {
        return Vec::new();
    }

    let graph = FaceAdjacencyGraph::build(&mesh.faces);
    let components = graph.extract_connected_components();

    let mut shells = Vec::with_capacity(components.len());

    for (shell_idx, comp_faces) in components.into_iter().enumerate() {
        let shell_id = (shell_idx + 1) as u32;

        // Step 1: Identify all vertices referenced by this component's faces
        let mut referenced = vec![false; mesh.vertices.len()];
        for &f_idx in &comp_faces {
            let [v0, v1, v2] = mesh.faces[f_idx];
            if (v0 as usize) < mesh.vertices.len() {
                referenced[v0 as usize] = true;
            }
            if (v1 as usize) < mesh.vertices.len() {
                referenced[v1 as usize] = true;
            }
            if (v2 as usize) < mesh.vertices.len() {
                referenced[v2 as usize] = true;
            }
        }

        // Step 2: Build compact 0-indexed vertex buffer and mapping
        let mut old_to_new = vec![u32::MAX; mesh.vertices.len()];
        let mut new_vertices = Vec::new();

        for (old_idx, &is_ref) in referenced.iter().enumerate() {
            if is_ref {
                old_to_new[old_idx] = new_vertices.len() as u32;
                new_vertices.push(mesh.vertices[old_idx]);
            }
        }

        // Step 3: Re-index faces compactly using the shell-local indices
        let new_faces: Vec<[u32; 3]> = comp_faces
            .iter()
            .map(|&f_idx| {
                let [v0, v1, v2] = mesh.faces[f_idx];
                let n0 = if (v0 as usize) < old_to_new.len() {
                    old_to_new[v0 as usize]
                } else {
                    0
                };
                let n1 = if (v1 as usize) < old_to_new.len() {
                    old_to_new[v1 as usize]
                } else {
                    0
                };
                let n2 = if (v2 as usize) < old_to_new.len() {
                    old_to_new[v2 as usize]
                } else {
                    0
                };
                [n0, n1, n2]
            })
            .collect();

        // Step 4: Construct the shell IndexedMesh (recalculates normals)
        let shell_mesh = IndexedMesh::from_vertices_and_faces(new_vertices, new_faces);

        // Step 5: Compute geometric and topological properties
        let bbox = shell_mesh.bounding_box();
        let center_of_mass = shell_mesh.center_of_mass();
        let signed_volume = shell_mesh.signed_volume();
        let surface_area = shell_mesh.surface_area();
        let is_watertight =
            !shell_mesh.faces.is_empty() && EdgeTopology::build(&shell_mesh.faces).is_watertight();

        shells.push(MeshShell {
            id: shell_id,
            name: format!("Shell_{}", shell_id),
            mesh: shell_mesh,
            bbox,
            center_of_mass,
            signed_volume,
            surface_area,
            is_watertight,
        });
    }

    shells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_separate_two_disjoint_triangles() {
        let vertices = vec![
            Point3d::new(0.0, 0.0, 0.0),
            Point3d::new(1.0, 0.0, 0.0),
            Point3d::new(0.0, 1.0, 0.0),
            Point3d::new(99.0, 99.0, 99.0), // orphan vertex
            Point3d::new(10.0, 10.0, 10.0),
            Point3d::new(11.0, 10.0, 10.0),
            Point3d::new(10.0, 11.0, 10.0),
        ];
        let faces = vec![[0, 1, 2], [4, 5, 6]];
        let mesh = IndexedMesh::from_vertices_and_faces(vertices, faces);

        let shells = separate_connected_shells(&mesh);
        assert_eq!(shells.len(), 2);

        // First shell: vertices 0, 1, 2
        assert_eq!(shells[0].id, 1);
        assert_eq!(shells[0].name, "Shell_1");
        assert_eq!(shells[0].mesh.vertex_count(), 3);
        assert_eq!(shells[0].mesh.faces, vec![[0, 1, 2]]);
        assert!(!shells[0].is_watertight); // single triangle is not closed

        // Second shell: vertices 4, 5, 6 compactly mapped to 0, 1, 2
        assert_eq!(shells[1].id, 2);
        assert_eq!(shells[1].name, "Shell_2");
        assert_eq!(shells[1].mesh.vertex_count(), 3);
        assert_eq!(shells[1].mesh.faces, vec![[0, 1, 2]]);
        assert_eq!(
            shells[1].mesh.vertices[0],
            Point3d::new(10.0, 10.0, 10.0)
        );

        // Centers of mass are distinct
        assert!((shells[0].center_of_mass.x - 1.0 / 3.0).abs() < 1e-6);
        assert!((shells[1].center_of_mass.x - (31.0 / 3.0)).abs() < 1e-6);
    }
}
