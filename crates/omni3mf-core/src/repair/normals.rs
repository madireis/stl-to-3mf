//! Topological winding unification and outward normal reorientation.

use std::collections::{HashMap, VecDeque};

use crate::math::Point3d;
use crate::mesh::{IndexedMesh, UndirectedEdge};

/// Unifies triangle winding orders via BFS topological propagation and reorients normals outward.
///
/// Handles multi-component meshes independently:
/// 1. Uses BFS traversal across shared edges to align winding orders within each connected component.
/// 2. For closed watertight components, uses the divergence theorem signed volume (V > 0).
/// 3. For open/flat components, uses an extreme-facet ray test from the component's center of mass.
/// 4. Recomputes all unit face normals.
pub fn unify_winding_and_orient_outward(mesh: &mut IndexedMesh) {
    let num_faces = mesh.faces.len();
    if num_faces == 0 {
        return;
    }

    // Step 1: Build edge to incident face indices map
    let mut edge_to_faces: HashMap<UndirectedEdge, Vec<usize>> =
        HashMap::with_capacity(num_faces * 3 / 2);

    for (f_idx, &[v0, v1, v2]) in mesh.faces.iter().enumerate() {
        edge_to_faces
            .entry(UndirectedEdge::new(v0, v1))
            .or_default()
            .push(f_idx);
        edge_to_faces
            .entry(UndirectedEdge::new(v1, v2))
            .or_default()
            .push(f_idx);
        edge_to_faces
            .entry(UndirectedEdge::new(v2, v0))
            .or_default()
            .push(f_idx);
    }

    let mut visited = vec![false; num_faces];
    let mut queue = VecDeque::new();

    // Step 2: Traverse each connected component
    for seed in 0..num_faces {
        if visited[seed] {
            continue;
        }

        let mut component_faces = Vec::new();
        visited[seed] = true;
        queue.push_back(seed);

        while let Some(curr_idx) = queue.pop_front() {
            component_faces.push(curr_idx);
            let curr_f = mesh.faces[curr_idx];
            let curr_edges = [
                (curr_f[0], curr_f[1]),
                (curr_f[1], curr_f[2]),
                (curr_f[2], curr_f[0]),
            ];

            for &(u, v) in &curr_edges {
                let undir = UndirectedEdge::new(u, v);
                if let Some(neighbors) = edge_to_faces.get(&undir) {
                    for &adj_idx in neighbors {
                        if adj_idx == curr_idx || visited[adj_idx] {
                            continue;
                        }

                        let adj_f = mesh.faces[adj_idx];
                        // In curr_f, the directed edge goes u -> v.
                        // In a consistent neighbor, the shared edge must go v -> u.
                        // If it goes u -> v in adj_f, adj_f has the same winding and must be flipped.
                        let has_same_direction = (adj_f[0] == u && adj_f[1] == v)
                            || (adj_f[1] == u && adj_f[2] == v)
                            || (adj_f[2] == u && adj_f[0] == v);

                        if has_same_direction {
                            // Flip neighbor winding: swap vertex 1 and 2
                            mesh.faces[adj_idx] = [adj_f[0], adj_f[2], adj_f[1]];
                        }

                        visited[adj_idx] = true;
                        queue.push_back(adj_idx);
                    }
                }
            }
        }

        // Step 3: Check outward orientation for this component
        // Verify topological closedness: all undirected edges in component must have exactly 2 incident faces.
        let mut comp_edge_counts: HashMap<UndirectedEdge, usize> =
            HashMap::with_capacity(component_faces.len() * 3 / 2);
        for &f_idx in &component_faces {
            let [v0, v1, v2] = mesh.faces[f_idx];
            *comp_edge_counts.entry(UndirectedEdge::new(v0, v1)).or_default() += 1;
            *comp_edge_counts.entry(UndirectedEdge::new(v1, v2)).or_default() += 1;
            *comp_edge_counts.entry(UndirectedEdge::new(v2, v0)).or_default() += 1;
        }

        let is_closed = !component_faces.is_empty()
            && comp_edge_counts.values().all(|&count| count == 2);

        let must_flip = if is_closed {
            // Closed volume: divergence theorem signed volume is mathematically rigorous
            let mut signed_vol_6x = 0.0;
            for &f_idx in &component_faces {
                let [v0, v1, v2] = mesh.faces[f_idx];
                let p0 = mesh.vertices[v0 as usize];
                let p1 = mesh.vertices[v1 as usize];
                let p2 = mesh.vertices[v2 as usize];
                signed_vol_6x += p0.dot(p1.cross(p2));
            }
            if signed_vol_6x.abs() > 1e-9 {
                signed_vol_6x < 0.0
            } else {
                false
            }
        } else {
            // Open or flat component: center of mass ray test against extreme facet
            let mut com = Point3d::ZERO;
            let mut count = 0.0;
            for &f_idx in &component_faces {
                let [v0, v1, v2] = mesh.faces[f_idx];
                for &vid in &[v0, v1, v2] {
                    let v = mesh.vertices[vid as usize];
                    if v.is_finite() {
                        com += v;
                        count += 1.0;
                    }
                }
            }
            if count > 0.0 {
                com /= count;
            }

            // Find facet with maximum Z centroid
            let max_f_idx = *component_faces
                .iter()
                .max_by(|&&a, &&b| {
                    let fa = mesh.faces[a];
                    let fb = mesh.faces[b];
                    let za = mesh.vertices[fa[0] as usize].z
                        + mesh.vertices[fa[1] as usize].z
                        + mesh.vertices[fa[2] as usize].z;
                    let zb = mesh.vertices[fb[0] as usize].z
                        + mesh.vertices[fb[1] as usize].z
                        + mesh.vertices[fb[2] as usize].z;
                    za.partial_cmp(&zb).unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or(&component_faces[0]);

            let f = mesh.faces[max_f_idx];
            let p0 = mesh.vertices[f[0] as usize];
            let p1 = mesh.vertices[f[1] as usize];
            let p2 = mesh.vertices[f[2] as usize];
            let centroid = (p0 + p1 + p2) / 3.0;
            let ray = centroid - com;
            let normal = (p1 - p0).cross(p2 - p0);

            // If normal points opposite to outward ray, invert
            let dot = ray.dot(normal);
            if dot.abs() > 1e-12 {
                dot < 0.0
            } else {
                normal.z < 0.0
            }
        };

        if must_flip {
            for &f_idx in &component_faces {
                let f = mesh.faces[f_idx];
                mesh.faces[f_idx] = [f[0], f[2], f[1]];
            }
        }
    }

    // Step 4: Prune unreferenced vertices and recalculate face normals
    mesh.prune_unreferenced_vertices();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inverted_cube_reorientation() {
        // Create an inverted unit cube
        let vertices = vec![
            Point3d::new(-1.0, -1.0, -1.0), // 0
            Point3d::new(1.0, -1.0, -1.0),  // 1
            Point3d::new(1.0, 1.0, -1.0),   // 2
            Point3d::new(-1.0, 1.0, -1.0),  // 3
            Point3d::new(-1.0, -1.0, 1.0),  // 4
            Point3d::new(1.0, -1.0, 1.0),   // 5
            Point3d::new(1.0, 1.0, 1.0),    // 6
            Point3d::new(-1.0, 1.0, 1.0),   // 7
        ];

        // Intentionally inward-wound triangles (swapped order)
        let faces = vec![
            [0, 1, 2],
            [0, 2, 3], // Bottom (CW from outside)
            [4, 6, 5],
            [4, 7, 6], // Top (CW)
            [0, 5, 1],
            [0, 4, 5], // Front
            [2, 7, 3],
            [2, 6, 7], // Back
            [0, 7, 4],
            [0, 3, 7], // Left
            [1, 6, 2],
            [1, 5, 6], // Right
        ];

        let mut mesh = IndexedMesh::from_vertices_and_faces(vertices, faces);
        assert!(mesh.signed_volume() < 0.0);

        unify_winding_and_orient_outward(&mut mesh);
        assert!(
            mesh.signed_volume() > 0.0,
            "Reoriented mesh must have positive signed volume"
        );
        assert!((mesh.signed_volume() - 8.0).abs() < 1e-6);
    }

    #[test]
    fn test_open_surface_normal_invariance_to_origin_translation() {
        // An open flat quad at z = 100, far away from origin
        let vertices = vec![
            Point3d::new(100.0, 100.0, 100.0),
            Point3d::new(102.0, 100.0, 100.0),
            Point3d::new(102.0, 102.0, 100.0),
            Point3d::new(100.0, 102.0, 100.0),
        ];
        // Normal pointing up (+Z)
        let faces = vec![[0, 1, 2], [0, 2, 3]];
        let mut mesh = IndexedMesh::from_vertices_and_faces(vertices, faces);

        unify_winding_and_orient_outward(&mut mesh);
        assert!(
            mesh.normals[0].z > 0.99,
            "Open flat surface normal should point up (+Z), not inverted by origin cone"
        );
        assert!(
            mesh.normals[1].z > 0.99,
            "Open flat surface normal should point up (+Z), not inverted by origin cone"
        );
    }
}
