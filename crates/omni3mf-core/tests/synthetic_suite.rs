//! Omni3MF Programmatic Synthetic Verification Suite (R5 & TEST_INFRA Tiers 1-4)
//!
//! Provides zero-dependency programmatic mesh generators for topological edge cases
//! and asserts mathematical invariants (Euler characteristics, boundary edges,
//! signed divergence volumes, outward ray tests, and disjoint shell counts).

use std::collections::VecDeque;

use omni3mf_core::math::{BoundingBox3d, Point3d, Vector3d};
use omni3mf_core::mesh::{EdgeTopology, IndexedMesh, SpatialHashGrid, UndirectedEdge};
use omni3mf_core::repair::{
    extract_boundary_loops, filter_degenerate_triangles, patch_boundary_holes,
    unify_winding_and_orient_outward,
};
use omni3mf_core::stl::{
    detect_stl_format, read_stl, write_ascii_stl, write_binary_stl, RawFacet, StlFormat,
};

// ============================================================================
// 1. Programmatic Mesh Generators
// ============================================================================

/// Type of hole defect to inject into a cube.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CubeHoleType {
    /// Remove 1 triangular face (3-edge open boundary loop).
    SingleTriangle,
    /// Remove 2 coplanar triangular faces (4-edge quad open boundary loop).
    QuadHole,
    /// Remove opposite top and bottom faces (two 4-edge holes, making an open tube).
    OppositeFaces,
}

/// Mode of winding inversion to inject into a mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvertMode {
    /// Invert a single face's vertex winding order.
    SingleFacet,
    /// Invert multiple faces across different cube sides.
    MultipleFacets,
    /// Invert every face in the mesh (entire mesh inside-out).
    EntireMesh,
}

/// Canonical 8 vertices of a 2x2x2 unit cube centered at the origin: [-1, 1]^3.
pub fn canonical_cube_vertices() -> [Point3d; 8] {
    [
        Point3d::new(-1.0, -1.0, -1.0), // 0: -X, -Y, -Z
        Point3d::new(1.0, -1.0, -1.0),  // 1: +X, -Y, -Z
        Point3d::new(1.0, 1.0, -1.0),   // 2: +X, +Y, -Z
        Point3d::new(-1.0, 1.0, -1.0),  // 3: -X, +Y, -Z
        Point3d::new(-1.0, -1.0, 1.0),  // 4: -X, -Y, +Z
        Point3d::new(1.0, -1.0, 1.0),   // 5: +X, -Y, +Z
        Point3d::new(1.0, 1.0, 1.0),    // 6: +X, +Y, +Z
        Point3d::new(-1.0, 1.0, 1.0),   // 7: -X, +Y, +Z
    ]
}

/// 12 triangular faces of the canonical cube wound CCW when viewed from outside.
pub fn canonical_cube_faces() -> [[u32; 3]; 12] {
    [
        // Bottom (-Z)
        [0, 2, 1],
        [0, 3, 2],
        // Top (+Z)
        [4, 5, 6],
        [4, 6, 7],
        // Front (-Y)
        [0, 1, 5],
        [0, 5, 4],
        // Back (+Y)
        [2, 3, 7],
        [2, 7, 6],
        // Left (-X)
        [0, 4, 7],
        [0, 7, 3],
        // Right (+X)
        [1, 2, 6],
        [1, 6, 5],
    ]
}

/// Generates a canonical watertight 2-manifold cube (8 vertices, 12 faces).
pub fn generate_manifold_cube() -> IndexedMesh {
    let vertices = canonical_cube_vertices().to_vec();
    let faces = canonical_cube_faces().to_vec();
    IndexedMesh::from_vertices_and_faces(vertices, faces)
}

/// Generates unwelded RawFacets for the canonical manifold cube.
pub fn generate_manifold_cube_facets() -> Vec<RawFacet> {
    let verts = canonical_cube_vertices();
    let faces = canonical_cube_faces();
    let normals = [
        Vector3d::new(0.0, 0.0, -1.0),
        Vector3d::new(0.0, 0.0, -1.0),
        Vector3d::new(0.0, 0.0, 1.0),
        Vector3d::new(0.0, 0.0, 1.0),
        Vector3d::new(0.0, -1.0, 0.0),
        Vector3d::new(0.0, -1.0, 0.0),
        Vector3d::new(0.0, 1.0, 0.0),
        Vector3d::new(0.0, 1.0, 0.0),
        Vector3d::new(-1.0, 0.0, 0.0),
        Vector3d::new(-1.0, 0.0, 0.0),
        Vector3d::new(1.0, 0.0, 0.0),
        Vector3d::new(1.0, 0.0, 0.0),
    ];

    faces
        .iter()
        .zip(normals.iter())
        .map(|(&[i0, i1, i2], &n)| RawFacet::new(n, verts[i0 as usize], verts[i1 as usize], verts[i2 as usize]))
        .collect()
}

/// Generates a cube with boundary hole defect(s).
pub fn generate_cube_with_hole(hole_type: CubeHoleType) -> IndexedMesh {
    let vertices = canonical_cube_vertices().to_vec();
    let mut faces = canonical_cube_faces().to_vec();

    match hole_type {
        CubeHoleType::SingleTriangle => {
            // Remove facet 3 ([4, 6, 7]), leaving a 3-edge triangular boundary loop
            faces.remove(3);
        }
        CubeHoleType::QuadHole => {
            // Remove both top faces (indices 3 and 2), leaving a 4-edge quadrilateral boundary loop
            faces.remove(3);
            faces.remove(2);
        }
        CubeHoleType::OppositeFaces => {
            // Remove top face (indices 3 and 2) and bottom face (indices 1 and 0)
            faces.remove(3);
            faces.remove(2);
            faces.remove(1);
            faces.remove(0);
        }
    }

    IndexedMesh::from_vertices_and_faces(vertices, faces)
}

/// Generates a compound mesh containing `shell_count` disjoint cubes separated by `spacing` mm.
pub fn generate_multi_shell_compound(shell_count: usize, spacing: f64) -> IndexedMesh {
    let mut all_vertices = Vec::with_capacity(shell_count * 8);
    let mut all_faces = Vec::with_capacity(shell_count * 12);

    let base_verts = canonical_cube_vertices();
    let base_faces = canonical_cube_faces();

    for i in 0..shell_count {
        let v_offset = (i * 8) as u32;
        let x_shift = (i as f64) * spacing;

        for &p in &base_verts {
            all_vertices.push(Point3d::new(p.x + x_shift, p.y, p.z));
        }

        for &[f0, f1, f2] in &base_faces {
            all_faces.push([f0 + v_offset, f1 + v_offset, f2 + v_offset]);
        }
    }

    IndexedMesh::from_vertices_and_faces(all_vertices, all_faces)
}

/// Generates a cube with inverted normal facets.
pub fn generate_inverted_facets_mesh(mode: InvertMode) -> IndexedMesh {
    let vertices = canonical_cube_vertices().to_vec();
    let mut faces = canonical_cube_faces().to_vec();

    match mode {
        InvertMode::SingleFacet => {
            // Invert face 2: swap vertex 1 and 2
            let f = faces[2];
            faces[2] = [f[0], f[2], f[1]];
        }
        InvertMode::MultipleFacets => {
            // Invert faces 2, 4, 7
            for &idx in &[2, 4, 7] {
                let f = faces[idx];
                faces[idx] = [f[0], f[2], f[1]];
            }
        }
        InvertMode::EntireMesh => {
            // Invert every face
            for f in &mut faces {
                *f = [f[0], f[2], f[1]];
            }
        }
    }

    IndexedMesh::from_vertices_and_faces(vertices, faces)
}

/// Injects collinear points, collapsed edges, sub-epsilon coincident vertices, and NaNs.
pub fn generate_degenerate_mesh() -> (Vec<Point3d>, Vec<[u32; 3]>) {
    let mut vertices = canonical_cube_vertices().to_vec();
    let mut faces = canonical_cube_faces().to_vec();

    // 1. Add collinear points (area = 0 when formed as triangle)
    let p_collinear_0 = Point3d::new(10.0, 0.0, 0.0);
    let p_collinear_1 = Point3d::new(11.0, 0.0, 0.0);
    let p_collinear_2 = Point3d::new(12.0, 0.0, 0.0);
    let c0 = vertices.len() as u32;
    vertices.push(p_collinear_0);
    vertices.push(p_collinear_1);
    vertices.push(p_collinear_2);
    faces.push([c0, c0 + 1, c0 + 2]); // Collinear triangle

    // 2. Add collapsed-edge triangles (indices repeated)
    faces.push([0, 0, 1]); // v0 == v1
    faces.push([1, 2, 1]); // v0 == v2

    // 3. Add sub-epsilon coincident vertex (within 10^-7 mm < 10^-6 mm tolerance)
    let p_sub_epsilon = Point3d::new(-1.0 + 1e-7, -1.0, -1.0);
    let se_idx = vertices.len() as u32;
    vertices.push(p_sub_epsilon);
    // Face using sub-epsilon vertex
    faces.push([se_idx, 1, 5]);

    // 4. Add non-finite (NaN) vertex
    let p_nan = Point3d::new(f64::NAN, 1.0, 2.0);
    let nan_idx = vertices.len() as u32;
    vertices.push(p_nan);
    faces.push([nan_idx, 2, 3]);

    (vertices, faces)
}

/// Programmatically generates a synthetic watertight L-shaped mechanical bracket.
/// Constructed from two intersecting rectangular blocks merged into a clean manifold mesh.
pub fn generate_mechanical_bracket() -> IndexedMesh {
    // 12 unique vertices for an L-bracket:
    // Base block: [-2, 2] x [0, 4] x [0, 1]
    // Upright block: [-2, 2] x [0, 1] x [1, 5]
    let vertices = vec![
        Point3d::new(-2.0, 0.0, 0.0), // 0: front-bottom-left
        Point3d::new(2.0, 0.0, 0.0),  // 1: front-bottom-right
        Point3d::new(2.0, 4.0, 0.0),  // 2: back-bottom-right
        Point3d::new(-2.0, 4.0, 0.0), // 3: back-bottom-left
        Point3d::new(-2.0, 4.0, 1.0), // 4: back-mid-left
        Point3d::new(2.0, 4.0, 1.0),  // 5: back-mid-right
        Point3d::new(2.0, 1.0, 1.0),  // 6: inner-corner-right
        Point3d::new(-2.0, 1.0, 1.0), // 7: inner-corner-left
        Point3d::new(-2.0, 1.0, 5.0), // 8: top-back-left
        Point3d::new(2.0, 1.0, 5.0),  // 9: top-back-right
        Point3d::new(2.0, 0.0, 5.0),  // 10: top-front-right
        Point3d::new(-2.0, 0.0, 5.0), // 11: top-front-left
    ];

    // Outward wound triangular faces for the 8 planar faces of an L-bracket (16 triangles):
    let faces = vec![
        // Bottom face (-Z): quad [0, 1, 2, 3]
        [0, 2, 1],
        [0, 3, 2],
        // Front upright face (-Y): quad [0, 1, 10, 11]
        [0, 1, 10],
        [0, 10, 11],
        // Top upright face (+Z): quad [11, 10, 9, 8]
        [11, 10, 9],
        [11, 9, 8],
        // Upright inner face (+Y): quad [8, 9, 6, 7]
        [8, 9, 6],
        [8, 6, 7],
        // Base shelf top face (+Z): quad [7, 6, 5, 4]
        [7, 6, 5],
        [7, 5, 4],
        // Base back face (+Y): quad [4, 5, 2, 3]
        [3, 2, 5],
        [3, 5, 4],
        // Left side face (-X, L-shaped 6-gon triangulated into 4 triangles): [0, 3, 4, 7, 8, 11]
        [0, 7, 3],
        [3, 7, 4],
        [0, 11, 7],
        [11, 8, 7],
        // Right side face (+X, L-shaped 6-gon triangulated into 4 triangles): [1, 2, 5, 6, 9, 10]
        [1, 2, 6],
        [2, 5, 6],
        [1, 6, 10],
        [10, 6, 9],
    ];

    let mut mesh = IndexedMesh::from_vertices_and_faces(vertices, faces);
    unify_winding_and_orient_outward(&mut mesh);
    mesh
}

/// Helper to count disconnected shells using face-adjacency traversal on undirected edges.
pub fn count_connected_components(mesh: &IndexedMesh) -> usize {
    if mesh.faces.is_empty() {
        return 0;
    }

    let mut edge_to_faces: std::collections::HashMap<UndirectedEdge, Vec<usize>> =
        std::collections::HashMap::with_capacity(mesh.faces.len() * 3 / 2);

    for (f_idx, &[v0, v1, v2]) in mesh.faces.iter().enumerate() {
        edge_to_faces.entry(UndirectedEdge::new(v0, v1)).or_default().push(f_idx);
        edge_to_faces.entry(UndirectedEdge::new(v1, v2)).or_default().push(f_idx);
        edge_to_faces.entry(UndirectedEdge::new(v2, v0)).or_default().push(f_idx);
    }

    let mut visited = vec![false; mesh.faces.len()];
    let mut component_count = 0;
    let mut queue = VecDeque::new();

    for seed in 0..mesh.faces.len() {
        if visited[seed] {
            continue;
        }

        component_count += 1;
        visited[seed] = true;
        queue.push_back(seed);

        while let Some(curr) = queue.pop_front() {
            let [v0, v1, v2] = mesh.faces[curr];
            let edges = [
                UndirectedEdge::new(v0, v1),
                UndirectedEdge::new(v1, v2),
                UndirectedEdge::new(v2, v0),
            ];

            for e in &edges {
                if let Some(neighbors) = edge_to_faces.get(e) {
                    for &adj in neighbors {
                        if !visited[adj] {
                            visited[adj] = true;
                            queue.push_back(adj);
                        }
                    }
                }
            }
        }
    }

    component_count
}

// ============================================================================
// 2. Tier 1: Feature Coverage & Invariant Verification
// ============================================================================

#[test]
fn test_tier1_canonical_manifold_cube_invariants() {
    let mesh = generate_manifold_cube();
    let topology = EdgeTopology::build(&mesh.faces);

    // 1. Invariant: V = 8, F = 12, E = 18
    assert_eq!(mesh.vertex_count(), 8, "Canonical cube must have exactly 8 vertices");
    assert_eq!(mesh.face_count(), 12, "Canonical cube must have exactly 12 faces");
    assert_eq!(topology.undirected_edges.len(), 18, "Cube must have exactly 18 undirected edges");

    // 2. Invariant: Euler Characteristic Chi = V - E + F = 8 - 18 + 12 = 2
    assert_eq!(topology.euler_characteristic(mesh.vertex_count()), 2, "Chi must be 2 for genus-0 sphere/cube");

    // 3. Invariant: Boundary edges = 0, Watertight = true, 2-Manifold = true
    assert_eq!(topology.boundary_edge_count(), 0, "Watertight cube must have 0 boundary edges");
    assert!(topology.is_watertight(), "Cube must be watertight");
    assert!(topology.is_2_manifold(), "Cube must be 2-manifold");

    // 4. Invariant: Signed Volume = 8.0 mm^3 (2x2x2)
    let vol = mesh.signed_volume();
    assert!((vol - 8.0).abs() < 1e-6, "Volume must equal 8.0, got {vol}");

    // 5. Invariant: Surface Area = 6 * 4.0 = 24.0 mm^2
    let area = mesh.surface_area();
    assert!((area - 24.0).abs() < 1e-6, "Surface area must equal 24.0, got {area}");

    // 6. Invariant: Outward Normals: for each face i, n_i . (c_face - c_mesh) > 0
    let c_mesh = mesh.center_of_mass();
    for (i, &[v0, v1, v2]) in mesh.faces.iter().enumerate() {
        let p0 = mesh.vertices[v0 as usize];
        let p1 = mesh.vertices[v1 as usize];
        let p2 = mesh.vertices[v2 as usize];
        let c_face = (p0 + p1 + p2) / 3.0;
        let n = mesh.normals[i];
        let outward_ray = c_face - c_mesh;
        assert!(n.dot(outward_ray) > 0.0, "Face {i} normal must point outward from mesh center");
    }
}

#[test]
fn test_tier1_open_hole_boundary_detection_and_healing() {
    let mut mesh = generate_cube_with_hole(CubeHoleType::SingleTriangle);
    let top_before = EdgeTopology::build(&mesh.faces);

    // Initial defect verification
    assert_eq!(mesh.face_count(), 11);
    assert_eq!(top_before.boundary_edge_count(), 3, "Missing 1 triangle must create 3 boundary edges");
    assert!(!top_before.is_watertight(), "Mesh with hole must not be watertight");
    assert_eq!(top_before.euler_characteristic(mesh.vertex_count()), 1);

    // Heal boundary hole
    let patched = patch_boundary_holes(&mut mesh, 100);
    assert_eq!(patched, 1, "Must patch exactly 1 hole");

    // Post-repair invariant verification
    let top_after = EdgeTopology::build(&mesh.faces);
    assert_eq!(top_after.boundary_edge_count(), 0, "Boundary edges must drop to 0 post-healing");
    assert_eq!(mesh.face_count(), 12, "Face count must be restored to 12");
    assert!(top_after.is_watertight(), "Post-healing mesh must be watertight");
    assert_eq!(top_after.euler_characteristic(mesh.vertex_count()), 2, "Chi must be restored to 2");
    assert!((mesh.signed_volume() - 8.0).abs() < 1e-5, "Signed volume must be restored to 8.0");
}

#[test]
fn test_tier1_quad_hole_ear_clipping_healing() {
    let mut mesh = generate_cube_with_hole(CubeHoleType::QuadHole);
    let top_before = EdgeTopology::build(&mesh.faces);

    assert_eq!(mesh.face_count(), 10);
    assert_eq!(top_before.boundary_edge_count(), 4, "Missing quad face must leave 4 boundary edges");

    let boundary_edges = top_before.find_boundary_directed_edges();
    let loops = extract_boundary_loops(&boundary_edges);
    assert_eq!(loops.len(), 1, "Must extract exactly 1 boundary cycle");
    assert_eq!(loops[0].len(), 4, "Loop must have length 4");

    // Heal quad hole (synthesizes 2 triangles)
    let patched = patch_boundary_holes(&mut mesh, 100);
    assert_eq!(patched, 1);

    let top_after = EdgeTopology::build(&mesh.faces);
    assert_eq!(top_after.boundary_edge_count(), 0);
    assert_eq!(mesh.face_count(), 12, "Quad hole must be triangulated into 2 faces");
    assert!(top_after.is_watertight());
    assert_eq!(top_after.euler_characteristic(mesh.vertex_count()), 2);
}

#[test]
fn test_tier1_multi_hole_opposite_faces_healing() {
    let mut mesh = generate_cube_with_hole(CubeHoleType::OppositeFaces);
    let top_before = EdgeTopology::build(&mesh.faces);

    // Two 4-edge holes on top and bottom: 8 boundary edges total
    assert_eq!(top_before.boundary_edge_count(), 8);
    let boundary_edges = top_before.find_boundary_directed_edges();
    let loops = extract_boundary_loops(&boundary_edges);
    assert_eq!(loops.len(), 2, "Must detect 2 separate boundary loops");

    // Patch both holes
    let patched = patch_boundary_holes(&mut mesh, 100);
    assert_eq!(patched, 2, "Must patch both top and bottom holes");

    let top_after = EdgeTopology::build(&mesh.faces);
    assert_eq!(top_after.boundary_edge_count(), 0);
    assert_eq!(mesh.face_count(), 12);
    assert!(top_after.is_watertight());
    assert_eq!(top_after.euler_characteristic(mesh.vertex_count()), 2);
}

#[test]
fn test_tier1_inverted_facet_unification_and_outward_normals() {
    // Generate cube with multiple inverted facets
    let mut mesh = generate_inverted_facets_mesh(InvertMode::MultipleFacets);

    // Run unification
    unify_winding_and_orient_outward(&mut mesh);

    let top = EdgeTopology::build(&mesh.faces);
    assert_eq!(top.boundary_edge_count(), 0, "No boundary edges after reorientation");
    assert!(mesh.signed_volume() > 0.0, "Signed volume must be positive");
    assert!((mesh.signed_volume() - 8.0).abs() < 1e-6);

    let c_mesh = mesh.center_of_mass();
    for (i, &[v0, v1, v2]) in mesh.faces.iter().enumerate() {
        let p0 = mesh.vertices[v0 as usize];
        let p1 = mesh.vertices[v1 as usize];
        let p2 = mesh.vertices[v2 as usize];
        let c_face = (p0 + p1 + p2) / 3.0;
        let n = mesh.normals[i];
        assert!(
            n.dot(c_face - c_mesh) > 0.0,
            "Face {i} normal must point outward after unification"
        );
    }
}

#[test]
fn test_tier1_entire_mesh_inverted_reorientation() {
    // A completely inside-out cube
    let mut mesh = generate_inverted_facets_mesh(InvertMode::EntireMesh);
    assert!(mesh.signed_volume() < -7.9, "Inside-out cube must have negative signed volume");

    unify_winding_and_orient_outward(&mut mesh);
    assert!(
        mesh.signed_volume() > 7.9,
        "Reoriented inside-out cube must have positive signed volume"
    );
    assert!((mesh.signed_volume() - 8.0).abs() < 1e-6);
}

#[test]
fn test_tier1_multi_shell_compound_discrete_count() {
    // Generate 3 disjoint cubes separated by 10 mm
    let mesh = generate_multi_shell_compound(3, 10.0);
    let topology = EdgeTopology::build(&mesh.faces);

    // Invariants: 3 cubes -> 24 vertices, 36 faces, 54 edges
    assert_eq!(mesh.vertex_count(), 24);
    assert_eq!(mesh.face_count(), 36);
    assert_eq!(topology.undirected_edges.len(), 54);

    // For 3 disjoint genus-0 shells, Chi = 3 * 2 = 6
    assert_eq!(
        topology.euler_characteristic(mesh.vertex_count()),
        6,
        "Euler characteristic must be 2 * N = 6 for 3 disjoint spheres/cubes"
    );

    // Watertight and 0 boundary edges
    assert_eq!(topology.boundary_edge_count(), 0);
    assert!(topology.is_watertight());

    // Discrete component count assertion
    let shells = count_connected_components(&mesh);
    assert_eq!(shells, 3, "Adjacency graph must discover exactly 3 discrete shells");

    // Volume of 3 unit cubes = 3 * 8.0 = 24.0
    assert!((mesh.signed_volume() - 24.0).abs() < 1e-5);
}

#[test]
fn test_tier1_spatial_hash_welding_sub_epsilon_merging() {
    let mut facets = generate_manifold_cube_facets();

    // Perturb vertex 0 of face 0 by 5e-7 mm (< 1e-6 mm threshold)
    facets[0].v0.x += 5e-7;
    facets[0].v0.y -= 5e-7;

    let (vertices, faces) = SpatialHashGrid::weld_raw_facets(&facets, 1e-6);
    assert_eq!(
        vertices.len(),
        8,
        "Sub-epsilon coincident vertex must be welded into canonical 8 vertices"
    );
    assert_eq!(faces.len(), 12);
}

#[test]
fn test_tier1_degeneracy_pruning_safely_drops_collinear_and_collapsed() {
    let (vertices, raw_faces) = generate_degenerate_mesh();
    let initial_faces_count = raw_faces.len();
    assert!(initial_faces_count > 12);

    let valid_faces = filter_degenerate_triangles(&raw_faces, &vertices);

    // Degenerate collinear, collapsed, and NaN triangles must all be filtered out
    for &f in &valid_faces {
        assert_ne!(f[0], f[1], "No collapsed edges");
        assert_ne!(f[1], f[2], "No collapsed edges");
        assert_ne!(f[2], f[0], "No collapsed edges");

        let p0 = vertices[f[0] as usize];
        let p1 = vertices[f[1] as usize];
        let p2 = vertices[f[2] as usize];

        assert!(p0.is_finite() && p1.is_finite() && p2.is_finite());
        let norm_sq = (p1 - p0).cross(p2 - p0).norm_squared();
        assert!(norm_sq > 1e-24, "No sub-epsilon or zero-area triangles");
    }
}

// ============================================================================
// 3. Tier 2: Boundary & Corner Cases
// ============================================================================

#[test]
fn test_tier2_empty_mesh_safety() {
    let mesh = IndexedMesh::new();
    assert_eq!(mesh.vertex_count(), 0);
    assert_eq!(mesh.face_count(), 0);
    assert!(mesh.is_empty());
    assert_eq!(mesh.surface_area(), 0.0);
    assert_eq!(mesh.signed_volume(), 0.0);
    assert_eq!(mesh.center_of_mass(), Point3d::ZERO);

    let top = EdgeTopology::build(&mesh.faces);
    assert_eq!(top.boundary_edge_count(), 0);
    assert!(top.is_watertight());
    assert_eq!(top.euler_characteristic(mesh.vertex_count()), 0);

    // Running repair algorithms on empty mesh must not panic
    let mut mutable_mesh = mesh;
    patch_boundary_holes(&mut mutable_mesh, 100);
    unify_winding_and_orient_outward(&mut mutable_mesh);
    assert_eq!(mutable_mesh.face_count(), 0);
}

#[test]
fn test_tier2_single_isolated_triangle() {
    let vertices = vec![
        Point3d::new(0.0, 0.0, 0.0),
        Point3d::new(1.0, 0.0, 0.0),
        Point3d::new(0.0, 1.0, 0.0),
    ];
    let faces = vec![[0, 1, 2]];

    let mut mesh = IndexedMesh::from_vertices_and_faces(vertices, faces);
    let top = EdgeTopology::build(&mesh.faces);

    assert_eq!(mesh.vertex_count(), 3);
    assert_eq!(mesh.face_count(), 1);
    assert_eq!(top.boundary_edge_count(), 3, "Single triangle must have 3 boundary half-edges");
    assert!(!top.is_watertight());
    assert_eq!(top.euler_characteristic(mesh.vertex_count()), 1); // 3 - 3 + 1 = 1

    // Outward orientation on flat triangle must not panic
    unify_winding_and_orient_outward(&mut mesh);
    assert_eq!(mesh.face_count(), 1);
}

#[test]
fn test_tier2_exact_spatial_tolerance_boundaries() {
    let p_base = Point3d::new(0.0, 0.0, 0.0);

    // Case A: Separation is 0.9e-6 mm (< 1.0e-6 mm tolerance) -> MUST WELD
    let p_inside = Point3d::new(0.9e-6, 0.0, 0.0);
    let facets_weld = vec![
        RawFacet::new(Vector3d::UNIT_Z, p_base, Point3d::new(1.0, 0.0, 0.0), Point3d::new(0.0, 1.0, 0.0)),
        RawFacet::new(Vector3d::UNIT_Z, p_inside, Point3d::new(1.0, 0.0, 0.0), Point3d::new(0.0, 1.0, 0.0)),
    ];
    let (v_weld, _) = SpatialHashGrid::weld_raw_facets(&facets_weld, 1e-6);
    // p_base and p_inside welded -> unique vertices are { (0,0,0), (1,0,0), (0,1,0) } = 3
    assert_eq!(v_weld.len(), 3, "Points separated by 0.9e-6 mm must merge");

    // Case B: Separation is 1.5e-6 mm (> 1.0e-6 mm tolerance) -> MUST REMAIN DISTINCT
    let p_outside = Point3d::new(1.5e-6, 0.0, 0.0);
    let facets_distinct = vec![
        RawFacet::new(Vector3d::UNIT_Z, p_base, Point3d::new(1.0, 0.0, 0.0), Point3d::new(0.0, 1.0, 0.0)),
        RawFacet::new(Vector3d::UNIT_Z, p_outside, Point3d::new(1.0, 0.0, 0.0), Point3d::new(0.0, 1.0, 0.0)),
    ];
    let (v_distinct, _) = SpatialHashGrid::weld_raw_facets(&facets_distinct, 1e-6);
    // p_base and p_outside kept distinct -> unique vertices are { p_base, p_outside, (1,0,0), (0,1,0) } = 4
    assert_eq!(v_distinct.len(), 4, "Points separated by 1.5e-6 mm must not merge");
}

#[test]
fn test_tier2_extreme_bounding_box_scale_boundaries() {
    // Model A: Abnormally small (< 2.0 mm)
    let small_verts = vec![
        Point3d::new(0.0, 0.0, 0.0),
        Point3d::new(0.5, 0.0, 0.0),
        Point3d::new(0.0, 0.5, 0.0),
    ];
    let bbox_small = BoundingBox3d::from_points(&small_verts);
    let dims_small = bbox_small.dimensions();
    let max_small = dims_small.x.max(dims_small.y).max(dims_small.z);
    assert!(max_small < 2.0, "Must be under 2.0 mm threshold");

    // Model B: Abnormally large (> 5000.0 mm)
    let large_verts = vec![
        Point3d::new(0.0, 0.0, 0.0),
        Point3d::new(6000.0, 0.0, 0.0),
        Point3d::new(0.0, 6000.0, 0.0),
    ];
    let bbox_large = BoundingBox3d::from_points(&large_verts);
    let dims_large = bbox_large.dimensions();
    let max_large = dims_large.x.max(dims_large.y).max(dims_large.z);
    assert!(max_large > 5000.0, "Must exceed 5000.0 mm threshold");

    // Model C: Normal print dimensions (e.g., 100 mm)
    let normal_verts = vec![
        Point3d::new(0.0, 0.0, 0.0),
        Point3d::new(100.0, 50.0, 25.0),
    ];
    let bbox_normal = BoundingBox3d::from_points(&normal_verts);
    let max_normal = bbox_normal.dimensions().x;
    assert!(max_normal >= 2.0 && max_normal <= 5000.0);
}

// ============================================================================
// 4. Tier 3: Cross-Feature Combinations (The "Dirty Mesh" Stress Test)
// ============================================================================

#[test]
fn test_tier3_dirty_mesh_comprehensive_repair_pipeline() {
    // Construct a dirty STL facet stream combining:
    // - Base manifold cube
    // - 1 missing face (creating a 3-edge hole)
    // - 2 inverted faces
    // - 2 collinear zero-area triangles
    // - 1 sub-epsilon duplicate vertex
    // - ASCII STL serialization with exponential notation

    let mut facets = generate_manifold_cube_facets();

    // 1. Remove facet 3 (creates an open hole)
    facets.remove(3);

    // 2. Invert winding of facet 1: swap v1 and v2
    let f1 = &mut facets[1];
    std::mem::swap(&mut f1.v1, &mut f1.v2);

    // 3. Invert winding of facet 5: swap v1 and v2
    let f5 = &mut facets[5];
    std::mem::swap(&mut f5.v1, &mut f5.v2);

    // 4. Inject collinear zero-area facet
    facets.push(RawFacet::new(
        Vector3d::UNIT_Z,
        Point3d::new(0.0, 0.0, 0.0),
        Point3d::new(1.0, 0.0, 0.0),
        Point3d::new(2.0, 0.0, 0.0),
    ));

    // 5. Inject sub-epsilon coincident perturbation
    facets[0].v0.x += 1e-7;

    // Serialize to ASCII STL
    let ascii_bytes = write_ascii_stl("dirty_synthetic_mesh", &facets);

    // Ingest through reading, welding, and degeneracy filtering
    let (_fmt, read_facets) = read_stl(&ascii_bytes).expect("Reading dirty ASCII STL must succeed");
    let (vertices, raw_faces) = SpatialHashGrid::weld_raw_facets(&read_facets, 1e-6);
    let valid_faces = filter_degenerate_triangles(&raw_faces, &vertices);
    let mut repaired_mesh = IndexedMesh::from_vertices_and_faces(vertices, valid_faces);

    // Unify winding first so adjacent inverted facets do not create false boundary half-edges
    unify_winding_and_orient_outward(&mut repaired_mesh);
    let holes_patched = patch_boundary_holes(&mut repaired_mesh, 100);
    assert_eq!(holes_patched, 1, "Must patch exactly the 1 real missing face");
    unify_winding_and_orient_outward(&mut repaired_mesh);

    // Assert final watertight 2-manifold invariants
    let topology = EdgeTopology::build(&repaired_mesh.faces);

    // The active manifold mesh references exactly 8 unique vertices from the 12 faces
    let active_vertices: std::collections::HashSet<u32> =
        repaired_mesh.faces.iter().flat_map(|f| f.iter().copied()).collect();
    assert_eq!(
        active_vertices.len(),
        8,
        "Active manifold faces must reference exactly 8 unique welded vertices"
    );
    assert_eq!(repaired_mesh.face_count(), 12, "Must heal and prune to exactly 12 faces");
    assert_eq!(topology.boundary_edge_count(), 0, "All boundary holes must be healed");
    assert!(topology.is_watertight(), "Repaired mesh must be completely watertight");
    assert_eq!(
        topology.euler_characteristic(active_vertices.len()),
        2,
        "Chi must equal 2"
    );

    // Signed volume must be positive and equal 8.0
    assert!(
        repaired_mesh.signed_volume() > 0.0,
        "Signed volume must be positive outward"
    );
    assert!((repaired_mesh.signed_volume() - 8.0).abs() < 1e-5);

    // All face normals must point outward from cube center
    let c_mesh = Point3d::ZERO;
    for (i, &[v0, v1, v2]) in repaired_mesh.faces.iter().enumerate() {
        let p0 = repaired_mesh.vertices[v0 as usize];
        let p1 = repaired_mesh.vertices[v1 as usize];
        let p2 = repaired_mesh.vertices[v2 as usize];
        let c_face = (p0 + p1 + p2) / 3.0;
        let n = repaired_mesh.normals[i];
        assert!(n.dot(c_face - c_mesh) > 0.0, "Face {i} normal must be outward");
    }
}

// ============================================================================
// 5. Tier 4: Real-World Application Scenarios
// ============================================================================

#[test]
fn test_tier4_watertight_mechanical_bracket() {
    let bracket = generate_mechanical_bracket();
    let topology = EdgeTopology::build(&bracket.faces);

    // 1. Manifoldness & Watertightness
    assert!(topology.is_2_manifold(), "Mechanical bracket must be 2-manifold");
    assert_eq!(topology.boundary_edge_count(), 0, "Mechanical bracket must have 0 boundary edges");
    assert!(topology.is_watertight(), "Mechanical bracket must be watertight");

    // 2. Genus 0 topological invariant: Chi = 2
    assert_eq!(
        topology.euler_characteristic(bracket.vertex_count()),
        2,
        "Solid L-bracket must have Euler characteristic Chi = 2"
    );

    // 3. Positive volume:
    // Base block: 4 x 4 x 1 = 16
    // Upright block: 4 x 1 x 4 = 16
    // Total volume = 32.0 mm^3
    let vol = bracket.signed_volume();
    assert!((vol - 32.0).abs() < 1e-5, "Expected volume 32.0, got {vol}");

    // 4. Bounding box matches exact dimensions: [-2, 2] x [0, 4] x [0, 5]
    let bbox = bracket.bounding_box();
    assert!((bbox.dimensions().x - 4.0).abs() < 1e-6);
    assert!((bbox.dimensions().y - 4.0).abs() < 1e-6);
    assert!((bbox.dimensions().z - 5.0).abs() < 1e-6);

    // 5. Normal consistency
    let _com = bracket.center_of_mass();
    for &n in &bracket.normals {
        assert!(n.norm_squared() > 0.99 && n.norm_squared() < 1.01, "Normal must be unit length");
    }
}

#[test]
fn test_tier4_binary_stl_roundtrip_with_format_validation() {
    let original = generate_manifold_cube_facets();

    // Write to binary STL
    let binary_bytes = write_binary_stl("omni3mf_synthetic_roundtrip", &original)
        .expect("Writing binary STL must succeed");

    // Detect format
    let detected = detect_stl_format(&binary_bytes).expect("Format detection must succeed");
    assert_eq!(detected, StlFormat::Binary { facet_count: 12 });

    // Read back
    let (fmt, read_facets) = read_stl(&binary_bytes).expect("Reading binary STL must succeed");
    assert_eq!(fmt, StlFormat::Binary { facet_count: 12 });
    assert_eq!(read_facets.len(), 12);

    // Full conversion through spatial hash welding
    let (v, f) = SpatialHashGrid::weld_raw_facets(&read_facets, 1e-6);
    assert_eq!(v.len(), 8);
    assert_eq!(f.len(), 12);
}
