//! Iterative fan and ear-clipping hole patching for open boundary loops.

use super::boundary::extract_boundary_loops;
use crate::math::{Point3d, Vector3d};
use crate::mesh::{EdgeTopology, IndexedMesh};

/// Patches open boundary holes in the mesh up to `max_hole_edges` per loop.
/// Returns the number of holes patched.
pub fn patch_boundary_holes(mesh: &mut IndexedMesh, max_hole_edges: usize) -> usize {
    let mut total_patched = 0;

    // Iteratively patch until no more boundary loops or no progress
    loop {
        let topology = EdgeTopology::build(&mesh.faces);
        let boundary_edges = topology.find_boundary_directed_edges();
        if boundary_edges.is_empty() {
            break;
        }

        let loops = extract_boundary_loops(&boundary_edges);
        if loops.is_empty() {
            break;
        }

        let mut new_triangles = Vec::new();
        let mut patched_this_pass = 0;

        for loop_verts in &loops {
            if max_hole_edges > 0 && loop_verts.len() > max_hole_edges {
                continue;
            }

            let patch = patch_hole_iterative_fan(loop_verts, &mesh.vertices);
            if !patch.is_empty() {
                new_triangles.extend(patch);
                patched_this_pass += 1;
            }
        }

        if patched_this_pass == 0 {
            break;
        }

        mesh.faces.extend(new_triangles);
        total_patched += patched_this_pass;
    }

    if total_patched > 0 {
        mesh.recalculate_face_normals();
    }

    total_patched
}

/// Triangulates a closed polygonal loop using iterative ear-clipping with Newell normal projection
/// and centroid/pivot fan fallback.
pub fn patch_hole_iterative_fan(loop_verts: &[u32], vertices: &[Point3d]) -> Vec<[u32; 3]> {
    let n = loop_verts.len();
    if n < 3 {
        return Vec::new();
    }
    if n == 3 {
        return vec![[loop_verts[0], loop_verts[1], loop_verts[2]]];
    }

    // Step 1: Compute Newell's normal of the 3D polygon loop
    let mut normal = Vector3d::ZERO;
    for i in 0..n {
        let p_curr = vertices[loop_verts[i] as usize];
        let p_next = vertices[loop_verts[(i + 1) % n] as usize];
        normal.x += (p_curr.y - p_next.y) * (p_curr.z + p_next.z);
        normal.y += (p_curr.z - p_next.z) * (p_curr.x + p_next.x);
        normal.z += (p_curr.x - p_next.x) * (p_curr.y + p_next.y);
    }

    // Step 2: Select projection plane (drop coordinate with largest normal component)
    let nx_abs = normal.x.abs();
    let ny_abs = normal.y.abs();
    let nz_abs = normal.z.abs();

    let project = |p: Point3d| -> (f64, f64) {
        if nz_abs >= nx_abs && nz_abs >= ny_abs {
            (p.x, p.y)
        } else if ny_abs >= nx_abs && ny_abs >= nz_abs {
            (p.x, p.z)
        } else {
            (p.y, p.z)
        }
    };

    // Step 3: Compute signed 2D area to determine polygon winding in 2D
    let mut area_2d = 0.0;
    for i in 0..n {
        let a = project(vertices[loop_verts[i] as usize]);
        let b = project(vertices[loop_verts[(i + 1) % n] as usize]);
        area_2d += a.0 * b.1 - b.0 * a.1;
    }
    let is_ccw = area_2d >= 0.0;

    let mut remaining: Vec<u32> = loop_verts.to_vec();
    let mut patch_triangles = Vec::new();

    // Ear clipping loop
    let mut stuck_count = 0;
    while remaining.len() > 3 && stuck_count < remaining.len() {
        let mut ear_found = false;
        let m = remaining.len();

        for i in 0..m {
            let prev_idx = remaining[(i + m - 1) % m];
            let curr_idx = remaining[i];
            let next_idx = remaining[(i + 1) % m];

            let a = project(vertices[prev_idx as usize]);
            let b = project(vertices[curr_idx as usize]);
            let c = project(vertices[next_idx as usize]);

            // Convexity check in 2D: cross product of (b - a) and (c - b)
            let cross_2d = (b.0 - a.0) * (c.1 - b.1) - (b.1 - a.1) * (c.0 - b.0);
            let is_convex = if is_ccw {
                cross_2d > 1e-12
            } else {
                cross_2d < -1e-12
            };

            if !is_convex {
                continue;
            }

            // Containment check: No other remaining vertex inside triangle (a, b, c)
            let mut contains_other = false;
            for j in 0..m {
                if j == (i + m - 1) % m || j == i || j == (i + 1) % m {
                    continue;
                }
                let p = project(vertices[remaining[j] as usize]);
                if point_in_triangle_2d(p, a, b, c) {
                    contains_other = true;
                    break;
                }
            }

            if !contains_other {
                patch_triangles.push([prev_idx, curr_idx, next_idx]);
                remaining.remove(i);
                ear_found = true;
                stuck_count = 0;
                break;
            }
        }

        if !ear_found {
            stuck_count += 1;
        }
    }

    // Final triangle or fallback pivot fan if remaining > 3
    if remaining.len() == 3 {
        patch_triangles.push([remaining[0], remaining[1], remaining[2]]);
    } else {
        // Fallback: pivot fan from remaining[0]
        for i in 1..remaining.len() - 1 {
            patch_triangles.push([remaining[0], remaining[i], remaining[i + 1]]);
        }
    }

    patch_triangles
}

/// Tests if point `p` lies strictly inside triangle `(a, b, c)` in 2D.
fn point_in_triangle_2d(p: (f64, f64), a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> bool {
    let sign = |p1: (f64, f64), p2: (f64, f64), p3: (f64, f64)| -> f64 {
        (p1.0 - p3.0) * (p2.1 - p3.1) - (p2.0 - p3.0) * (p1.1 - p3.1)
    };

    let d1 = sign(p, a, b);
    let d2 = sign(p, b, c);
    let d3 = sign(p, c, a);

    let has_neg = (d1 < -1e-12) || (d2 < -1e-12) || (d3 < -1e-12);
    let has_pos = (d1 > 1e-12) || (d2 > 1e-12) || (d3 > 1e-12);

    !(has_neg && has_pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_patch_quad_hole() {
        let vertices = vec![
            Point3d::new(0.0, 0.0, 0.0), // 0
            Point3d::new(1.0, 0.0, 0.0), // 1
            Point3d::new(1.0, 1.0, 0.0), // 2
            Point3d::new(0.0, 1.0, 0.0), // 3
        ];

        let loop_verts = vec![0, 1, 2, 3];
        let triangles = patch_hole_iterative_fan(&loop_verts, &vertices);
        assert_eq!(triangles.len(), 2);
    }
}
