//! Comprehensive integration tests for Milestone 1: Ingestion, Welding, Degeneracy Pruning,
//! Boundary Loop Detection, Ear-Clipping Hole Patching, and Outward Normal Reorientation.

use omni3mf_core::math::{Point3d, Vector3d};
use omni3mf_core::mesh::{EdgeTopology, IndexedMesh, SpatialHashGrid};
use omni3mf_core::repair::{
    extract_boundary_loops, filter_degenerate_triangles, patch_boundary_holes,
    unify_winding_and_orient_outward,
};
use omni3mf_core::stl::{
    detect_stl_format, read_stl, write_ascii_stl, write_binary_stl, RawFacet, StlFormat,
};

/// Helper to generate a canonical unit cube centered at origin with 8 vertices and 12 triangles.
fn generate_unit_cube_facets() -> Vec<RawFacet> {
    let v = [
        Point3d::new(-1.0, -1.0, -1.0), // 0
        Point3d::new(1.0, -1.0, -1.0),  // 1
        Point3d::new(1.0, 1.0, -1.0),   // 2
        Point3d::new(-1.0, 1.0, -1.0),  // 3
        Point3d::new(-1.0, -1.0, 1.0),  // 4
        Point3d::new(1.0, -1.0, 1.0),   // 5
        Point3d::new(1.0, 1.0, 1.0),    // 6
        Point3d::new(-1.0, 1.0, 1.0),   // 7
    ];

    // Outward wound triangles: 2 per cube face (6 faces * 2 = 12)
    let tri_indices = [
        // Bottom (-Z)
        ([0, 2, 1], Vector3d::new(0.0, 0.0, -1.0)),
        ([0, 3, 2], Vector3d::new(0.0, 0.0, -1.0)),
        // Top (+Z)
        ([4, 5, 6], Vector3d::new(0.0, 0.0, 1.0)),
        ([4, 6, 7], Vector3d::new(0.0, 0.0, 1.0)),
        // Front (-Y)
        ([0, 1, 5], Vector3d::new(0.0, -1.0, 0.0)),
        ([0, 5, 4], Vector3d::new(0.0, -1.0, 0.0)),
        // Back (+Y)
        ([2, 3, 7], Vector3d::new(0.0, 1.0, 0.0)),
        ([2, 7, 6], Vector3d::new(0.0, 1.0, 0.0)),
        // Left (-X)
        ([0, 4, 7], Vector3d::new(-1.0, 0.0, 0.0)),
        ([0, 7, 3], Vector3d::new(-1.0, 0.0, 0.0)),
        // Right (+X)
        ([1, 2, 6], Vector3d::new(1.0, 0.0, 0.0)),
        ([1, 6, 5], Vector3d::new(1.0, 0.0, 0.0)),
    ];

    tri_indices
        .iter()
        .map(|&([i0, i1, i2], n)| RawFacet::new(n, v[i0], v[i1], v[i2]))
        .collect()
}

#[test]
fn test_stl_format_detection_and_solid_prefix_in_binary() {
    let facets = generate_unit_cube_facets();
    // Exporters like SolidWorks write headers starting with "solid" into binary STLs
    let binary_with_solid_header = write_binary_stl("solid model exported by CAD", &facets).unwrap();

    let detected = detect_stl_format(&binary_with_solid_header).unwrap();
    assert_eq!(
        detected,
        StlFormat::Binary { facet_count: 12 },
        "Must correctly identify binary STL even when header starts with 'solid'"
    );

    let (fmt, read_facets) = read_stl(&binary_with_solid_header).unwrap();
    assert_eq!(fmt, StlFormat::Binary { facet_count: 12 });
    assert_eq!(read_facets.len(), 12);
}

#[test]
fn test_ascii_parser_scientific_notation_and_whitespace() {
    let ascii_content = b"
        solid test_scientific
            facet normal 0.000000e+00 -1.000000e+00 0.000000e+00
                outer loop
                    vertex -1.500000e-02 2.500000e+01 3.000000e+00
                    vertex 1.500000e-02 2.500000e+01 3.000000e+00
                    vertex 0.000000e+00 2.500000e+01 5.000000e+00
                endloop
            endfacet
        endsolid test_scientific
    ";

    let (fmt, facets) = read_stl(ascii_content).unwrap();
    assert_eq!(fmt, StlFormat::Ascii);
    assert_eq!(facets.len(), 1);
    assert!((facets[0].v0.x - (-0.015)).abs() < 1e-9);
    assert!((facets[0].v0.y - 25.0).abs() < 1e-9);
    assert!((facets[0].v0.z - 3.0).abs() < 1e-9);
}

#[test]
fn test_spatial_hash_welding_1e6_tolerance() {
    let mut facets = generate_unit_cube_facets();

    // Perturb vertices of face 0 slightly: within 1e-7 mm (< 1e-6 mm)
    facets[0].v0.x += 1e-7;
    facets[0].v0.y -= 1e-7;
    facets[0].v0.z += 1e-7;

    let (vertices, faces) = SpatialHashGrid::weld_raw_facets(&facets, 1e-6);

    // The unit cube has exactly 8 unique vertices when welded
    assert_eq!(
        vertices.len(),
        8,
        "Coincident vertices within 1e-6 mm must be welded into exactly 8 vertices"
    );
    assert_eq!(faces.len(), 12);
}

#[test]
fn test_degenerate_and_collinear_pruning() {
    let mut vertices = vec![
        Point3d::new(-1.0, -1.0, -1.0), // 0
        Point3d::new(1.0, -1.0, -1.0),  // 1
        Point3d::new(1.0, 1.0, -1.0),   // 2
        Point3d::new(0.0, 0.0, 0.0),    // 3
        Point3d::new(1.0, 0.0, 0.0),    // 4
        Point3d::new(2.0, 0.0, 0.0),    // 5 collinear with 3, 4
    ];

    let faces = vec![
        [0, 1, 2], // Valid triangle
        [0, 0, 1], // Collapsed edge (v0 == v1)
        [1, 2, 2], // Collapsed edge (v1 == v2)
        [3, 4, 5], // Collinear vertices (zero area)
    ];

    let valid = filter_degenerate_triangles(&faces, &vertices);
    assert_eq!(valid.len(), 1);
    assert_eq!(valid[0], [0, 1, 2]);

    // Test non-finite coordinates are safely stripped without panics
    vertices.push(Point3d::new(f64::NAN, 1.0, 2.0));
    let nan_face = vec![[0, 1, 6]];
    let valid_nan = filter_degenerate_triangles(&nan_face, &vertices);
    assert_eq!(valid_nan.len(), 0, "NaN vertices must be pruned");
}

#[test]
fn test_watertight_cube_invariants() {
    let facets = generate_unit_cube_facets();
    let (vertices, faces) = SpatialHashGrid::weld_raw_facets(&facets, 1e-6);
    let valid_faces = filter_degenerate_triangles(&faces, &vertices);
    let mut mesh = IndexedMesh::from_vertices_and_faces(vertices, valid_faces);

    unify_winding_and_orient_outward(&mut mesh);

    let topology = EdgeTopology::build(&mesh.faces);

    // 1. V = 8, F = 12, E = 18
    assert_eq!(mesh.vertex_count(), 8);
    assert_eq!(mesh.face_count(), 12);
    assert_eq!(topology.undirected_edges.len(), 18);

    // 2. Euler characteristic: Chi = 8 - 18 + 12 = 2
    assert_eq!(topology.euler_characteristic(mesh.vertex_count()), 2);

    // 3. Boundary edges = 0, is_watertight = true
    assert_eq!(topology.boundary_edge_count(), 0);
    assert!(topology.is_watertight());

    // 4. Signed volume = 8.0 (dimensions 2x2x2)
    assert!((mesh.signed_volume() - 8.0).abs() < 1e-6);

    // 5. Outward normal condition: for each face i, n_i . (c_face - c_mesh) > 0
    let mesh_center = mesh.center_of_mass();
    for (i, &[v0, v1, v2]) in mesh.faces.iter().enumerate() {
        let p0 = mesh.vertices[v0 as usize];
        let p1 = mesh.vertices[v1 as usize];
        let p2 = mesh.vertices[v2 as usize];
        let face_center = (p0 + p1 + p2) / 3.0;
        let normal = mesh.normals[i];
        let outward_ray = face_center - mesh_center;
        assert!(
            normal.dot(outward_ray) > 0.0,
            "Face normal must point outward from mesh center"
        );
    }
}

#[test]
fn test_open_hole_detection_and_fan_healing() {
    let facets = generate_unit_cube_facets();
    let (vertices, mut faces) = SpatialHashGrid::weld_raw_facets(&facets, 1e-6);

    // Remove face [4, 6, 7] (top face triangle 2), leaving a 3-edge hole
    let _removed = faces.remove(3);
    assert_eq!(faces.len(), 11);

    let mut mesh = IndexedMesh::from_vertices_and_faces(vertices, faces);
    let topology_before = EdgeTopology::build(&mesh.faces);
    assert_eq!(topology_before.boundary_edge_count(), 3);
    assert!(!topology_before.is_watertight());

    // Patch holes
    let patched_count = patch_boundary_holes(&mut mesh, 100);
    assert_eq!(patched_count, 1, "Must patch exactly 1 hole");

    let topology_after = EdgeTopology::build(&mesh.faces);
    assert_eq!(
        topology_after.boundary_edge_count(),
        0,
        "Boundary edges must drop to 0 after healing"
    );
    assert_eq!(mesh.face_count(), 12, "Face count restored to 12");
    assert!(topology_after.is_watertight());
    assert_eq!(topology_after.euler_characteristic(mesh.vertex_count()), 2);
}

#[test]
fn test_multi_edge_hole_ear_clipping() {
    let facets = generate_unit_cube_facets();
    let (vertices, mut faces) = SpatialHashGrid::weld_raw_facets(&facets, 1e-6);

    // Remove both top triangles [4, 5, 6] and [4, 6, 7]
    // Leaving a 4-edge quadrilateral hole: vertices 4, 5, 6, 7
    faces.remove(3); // removes [4, 6, 7]
    faces.remove(2); // removes [4, 5, 6]
    assert_eq!(faces.len(), 10);

    let mut mesh = IndexedMesh::from_vertices_and_faces(vertices, faces);
    let topology_before = EdgeTopology::build(&mesh.faces);
    assert_eq!(topology_before.boundary_edge_count(), 4);

    let boundary_edges = topology_before.find_boundary_directed_edges();
    let loops = extract_boundary_loops(&boundary_edges);
    assert_eq!(loops.len(), 1);
    assert_eq!(loops[0].len(), 4, "Loop must have 4 edges");

    // Heal the 4-edge hole via ear clipping
    let patched_count = patch_boundary_holes(&mut mesh, 100);
    assert_eq!(patched_count, 1);

    let topology_after = EdgeTopology::build(&mesh.faces);
    assert_eq!(topology_after.boundary_edge_count(), 0);
    assert_eq!(mesh.face_count(), 12, "Ear clipping must create 2 triangles, restoring 12 faces");
    assert!(topology_after.is_watertight());
    assert_eq!(topology_after.euler_characteristic(mesh.vertex_count()), 2);
}

#[test]
fn test_winding_propagation_and_reorientation_of_inverted_facets() {
    let facets = generate_unit_cube_facets();
    let (vertices, mut faces) = SpatialHashGrid::weld_raw_facets(&facets, 1e-6);

    // Invert faces 2, 4, 7 by swapping their winding
    faces[2] = [faces[2][0], faces[2][2], faces[2][1]];
    faces[4] = [faces[4][0], faces[4][2], faces[4][1]];
    faces[7] = [faces[7][0], faces[7][2], faces[7][1]];

    let mut mesh = IndexedMesh::from_vertices_and_faces(vertices, faces);

    // Before unification, edge windings will have collisions
    unify_winding_and_orient_outward(&mut mesh);

    // Check signed volume is exactly positive 8.0
    assert!(
        mesh.signed_volume() > 0.0,
        "Mesh signed volume must be positive"
    );
    assert!((mesh.signed_volume() - 8.0).abs() < 1e-6);

    // Check all face normals point outwards
    let com = mesh.center_of_mass();
    for (i, &[v0, v1, v2]) in mesh.faces.iter().enumerate() {
        let p0 = mesh.vertices[v0 as usize];
        let p1 = mesh.vertices[v1 as usize];
        let p2 = mesh.vertices[v2 as usize];
        let face_center = (p0 + p1 + p2) / 3.0;
        let normal = mesh.normals[i];
        assert!(
            normal.dot(face_center - com) > 0.0,
            "Face normal must point outward after unification"
        );
    }
}

#[test]
fn test_ascii_stl_write_and_full_ingest() {
    let facets = generate_unit_cube_facets();
    let ascii_bytes = write_ascii_stl("cube_ascii", &facets);

    let mesh = omni3mf_core::ingest_and_repair_stl(&ascii_bytes, 1e-6, true, 100).unwrap();
    assert_eq!(mesh.vertex_count(), 8);
    assert_eq!(mesh.face_count(), 12);
    assert!((mesh.signed_volume() - 8.0).abs() < 1e-5);
}

