//! Milestone 2 Integration & Invariant Tests:
//! Structural graph adjacency, DSU connected shell extraction, compact vertex re-indexing,
//! per-shell geometric metrics, and Reviewer 2 feedback validation.

use omni3mf_core::{
    ingest_and_repair_stl, ingest_repair_and_separate_shells, separate_connected_shells,
    write_binary_stl, IndexedMesh, Point3d, RawFacet, Vector3d,
};

/// Helper to generate a watertight 12-triangle cube with outward-facing normals.
fn make_cube(origin: Point3d, size: f64) -> (Vec<Point3d>, Vec<[u32; 3]>) {
    let x0 = origin.x;
    let y0 = origin.y;
    let z0 = origin.z;
    let x1 = x0 + size;
    let y1 = y0 + size;
    let z1 = z0 + size;

    let vertices = vec![
        Point3d::new(x0, y0, z0), // 0
        Point3d::new(x1, y0, z0), // 1
        Point3d::new(x1, y1, z0), // 2
        Point3d::new(x0, y1, z0), // 3
        Point3d::new(x0, y0, z1), // 4
        Point3d::new(x1, y0, z1), // 5
        Point3d::new(x1, y1, z1), // 6
        Point3d::new(x0, y1, z1), // 7
    ];

    let faces = vec![
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
    ];

    (vertices, faces)
}

#[test]
fn test_single_cube_produces_one_shell() {
    let (verts, faces) = make_cube(Point3d::new(0.0, 0.0, 0.0), 10.0);
    let mesh = IndexedMesh::from_vertices_and_faces(verts, faces);

    let shells = separate_connected_shells(&mesh);
    assert_eq!(shells.len(), 1, "A single connected cube must yield exactly 1 shell");

    let shell = &shells[0];
    assert_eq!(shell.id, 1);
    assert_eq!(shell.name, "Shell_1");
    assert!(shell.is_watertight, "Closed cube must be watertight");
    assert_eq!(shell.mesh.vertex_count(), 8);
    assert_eq!(shell.mesh.face_count(), 12);

    // Bounding box
    assert_eq!(shell.bbox.min, Point3d::new(0.0, 0.0, 0.0));
    assert_eq!(shell.bbox.max, Point3d::new(10.0, 10.0, 10.0));

    // Center of mass
    let expected_com = Point3d::new(5.0, 5.0, 5.0);
    assert!(
        shell.center_of_mass.distance(expected_com) < 1e-6,
        "Center of mass must be (5, 5, 5)"
    );

    // Volume and surface area
    assert!(
        (shell.signed_volume - 1000.0).abs() < 1e-4,
        "10x10x10 cube must have volume 1000"
    );
    assert!(
        (shell.surface_area - 600.0).abs() < 1e-4,
        "10x10x10 cube must have surface area 600"
    );
}

#[test]
fn test_disjoint_cubes_produce_distinct_shells() {
    let (verts1, faces1) = make_cube(Point3d::new(0.0, 0.0, 0.0), 2.0);
    let (verts2, faces2) = make_cube(Point3d::new(20.0, 30.0, 40.0), 4.0);

    let offset = verts1.len() as u32;
    let mut combined_verts = verts1;
    combined_verts.extend(verts2);

    let mut combined_faces = faces1;
    for &[v0, v1, v2] in &faces2 {
        combined_faces.push([v0 + offset, v1 + offset, v2 + offset]);
    }

    let mesh = IndexedMesh::from_vertices_and_faces(combined_verts, combined_faces);
    let shells = separate_connected_shells(&mesh);

    assert_eq!(
        shells.len(),
        2,
        "Two disjoint non-touching cubes must produce exactly 2 shells"
    );

    // Shell 1: at (0, 0, 0), size 2.0
    let s1 = &shells[0];
    assert_eq!(s1.id, 1);
    assert_eq!(s1.name, "Shell_1");
    assert!(s1.is_watertight);
    assert_eq!(s1.bbox.min, Point3d::new(0.0, 0.0, 0.0));
    assert_eq!(s1.bbox.max, Point3d::new(2.0, 2.0, 2.0));
    assert!(s1.center_of_mass.distance(Point3d::new(1.0, 1.0, 1.0)) < 1e-6);
    assert!((s1.signed_volume - 8.0).abs() < 1e-4);
    assert!((s1.surface_area - 24.0).abs() < 1e-4);

    // Shell 2: at (20, 30, 40), size 4.0
    let s2 = &shells[1];
    assert_eq!(s2.id, 2);
    assert_eq!(s2.name, "Shell_2");
    assert!(s2.is_watertight);
    assert_eq!(s2.bbox.min, Point3d::new(20.0, 30.0, 40.0));
    assert_eq!(s2.bbox.max, Point3d::new(24.0, 34.0, 44.0));
    assert!(s2.center_of_mass.distance(Point3d::new(22.0, 32.0, 42.0)) < 1e-6);
    assert!((s2.signed_volume - 64.0).abs() < 1e-4);
    assert!((s2.surface_area - 96.0).abs() < 1e-4);

    // Verify distinct bounding boxes and centers of mass
    assert_ne!(s1.bbox, s2.bbox);
    assert!(s1.center_of_mass.distance(s2.center_of_mass) > 10.0);
}

#[test]
fn test_compact_reindexing_and_orphan_pruning() {
    let (verts1, faces1) = make_cube(Point3d::new(0.0, 0.0, 0.0), 2.0);
    let (verts2, faces2) = make_cube(Point3d::new(10.0, 10.0, 10.0), 2.0);

    // Inject unreferenced orphan vertices, including non-finite coordinates
    let mut messy_verts = Vec::new();
    messy_verts.push(Point3d::new(999.0, 999.0, 999.0)); // Orphan 0
    messy_verts.push(Point3d::new(f64::NAN, f64::NAN, f64::NAN)); // Orphan 1 (NaN)

    let v1_start = messy_verts.len() as u32;
    messy_verts.extend(verts1);

    messy_verts.push(Point3d::new(-50.0, -50.0, -50.0)); // Orphan 2

    let v2_start = messy_verts.len() as u32;
    messy_verts.extend(verts2);

    messy_verts.push(Point3d::new(123.0, 456.0, 789.0)); // Orphan 3

    let mut messy_faces = Vec::new();
    for &[a, b, c] in &faces1 {
        messy_faces.push([a + v1_start, b + v1_start, c + v1_start]);
    }
    for &[a, b, c] in &faces2 {
        messy_faces.push([a + v2_start, b + v2_start, c + v2_start]);
    }

    let raw_mesh = IndexedMesh::from_vertices_and_faces(messy_verts, messy_faces);
    assert_eq!(raw_mesh.vertex_count(), 8 + 8 + 4); // 20 vertices total

    let shells = separate_connected_shells(&raw_mesh);
    assert_eq!(shells.len(), 2);

    for shell in &shells {
        let v_count = shell.mesh.vertex_count();
        assert_eq!(v_count, 8, "Each cube shell must retain only 8 referenced vertices");
        assert_eq!(shell.mesh.face_count(), 12);

        // Invariant: Indices must start at 0, be dense, and max index == vertex_count - 1
        let mut seen = vec![false; v_count];
        let mut min_idx = u32::MAX;
        let mut max_idx = 0u32;

        for &[a, b, c] in &shell.mesh.faces {
            for &idx in &[a, b, c] {
                assert!(
                    (idx as usize) < v_count,
                    "Face vertex index {} out of shell bounds {}",
                    idx,
                    v_count
                );
                seen[idx as usize] = true;
                min_idx = min_idx.min(idx);
                max_idx = max_idx.max(idx);
            }
        }

        assert_eq!(min_idx, 0, "Internal indices must start at 0");
        assert_eq!(
            max_idx,
            (v_count - 1) as u32,
            "Max face index must be vertex_count - 1"
        );
        assert!(
            seen.iter().all(|&s| s),
            "Every vertex in shell vertex buffer must be referenced by at least one face"
        );

        // Center of mass must be finite and match cube center
        assert!(shell.center_of_mass.is_finite());
    }
}

#[test]
fn test_point_contact_at_single_vertex_splits_shells() {
    // Two tetrahedra touching ONLY at a single vertex (0, 0, 0)
    // Pyramid 1 in positive octant
    let p0 = Point3d::new(0.0, 0.0, 0.0); // shared vertex coordinate
    let p1 = Point3d::new(1.0, 0.0, 0.0);
    let p2 = Point3d::new(0.0, 1.0, 0.0);
    let p3 = Point3d::new(0.0, 0.0, 1.0);

    // Pyramid 2 in negative octant
    let q1 = Point3d::new(-1.0, 0.0, 0.0);
    let q2 = Point3d::new(0.0, -1.0, 0.0);
    let q3 = Point3d::new(0.0, 0.0, -1.0);

    // Global vertex buffer: p0 is index 0, shared by both pyramids
    let vertices = vec![p0, p1, p2, p3, q1, q2, q3];

    // Pyramid 1 faces (touching at vertex 0)
    let faces = vec![
        [0, 2, 1],
        [0, 3, 2],
        [0, 1, 3],
        [1, 2, 3],
        // Pyramid 2 faces (touching at vertex 0)
        [0, 5, 4],
        [0, 6, 5],
        [0, 4, 6],
        [4, 5, 6],
    ];

    let mesh = IndexedMesh::from_vertices_and_faces(vertices, faces);
    let shells = separate_connected_shells(&mesh);

    assert_eq!(
        shells.len(),
        2,
        "Point contact sharing only a single vertex without shared edges must split into 2 shells"
    );

    assert_eq!(shells[0].mesh.vertex_count(), 4);
    assert_eq!(shells[1].mesh.vertex_count(), 4);
    assert!(shells[0].is_watertight);
    assert!(shells[1].is_watertight);

    // Both shells contain their own local copy of (0, 0, 0)
    assert!(shells[0].mesh.vertices.contains(&Point3d::new(0.0, 0.0, 0.0)));
    assert!(shells[1].mesh.vertices.contains(&Point3d::new(0.0, 0.0, 0.0)));
}

#[test]
fn test_open_surface_shell_watertightness_flag() {
    let (verts1, faces1) = make_cube(Point3d::new(0.0, 0.0, 0.0), 2.0); // Watertight closed cube

    // Open sheet: 2 triangles forming a flat quad
    let verts2 = vec![
        Point3d::new(10.0, 10.0, 0.0),
        Point3d::new(12.0, 10.0, 0.0),
        Point3d::new(12.0, 12.0, 0.0),
        Point3d::new(10.0, 12.0, 0.0),
    ];
    let faces2 = vec![[0, 1, 2], [0, 2, 3]];

    let offset = verts1.len() as u32;
    let mut combined_verts = verts1;
    combined_verts.extend(verts2);

    let mut combined_faces = faces1;
    for &[a, b, c] in &faces2 {
        combined_faces.push([a + offset, b + offset, c + offset]);
    }

    let mesh = IndexedMesh::from_vertices_and_faces(combined_verts, combined_faces);
    let shells = separate_connected_shells(&mesh);

    assert_eq!(shells.len(), 2);
    assert!(shells[0].is_watertight, "Cube must be watertight");
    assert!(!shells[1].is_watertight, "Open quad sheet must NOT be watertight");
}

#[test]
fn test_empty_mesh_produces_zero_shells() {
    let empty = IndexedMesh::new();
    let shells = separate_connected_shells(&empty);
    assert_eq!(shells.len(), 0);

    let orphan_only = IndexedMesh::from_vertices_and_faces(
        vec![Point3d::new(1.0, 2.0, 3.0), Point3d::new(4.0, 5.0, 6.0)],
        vec![],
    );
    let shells2 = separate_connected_shells(&orphan_only);
    assert_eq!(shells2.len(), 0);
}

#[test]
fn test_e2e_binary_stl_three_shells_pipeline() {
    // Generate 3 non-touching cubes
    let (v1, f1) = make_cube(Point3d::new(0.0, 0.0, 0.0), 2.0);
    let (v2, f2) = make_cube(Point3d::new(20.0, 0.0, 0.0), 3.0);
    let (v3, f3) = make_cube(Point3d::new(0.0, 50.0, 0.0), 4.0);

    let mut raw_facets = Vec::new();
    for &[a, b, c] in &f1 {
        raw_facets.push(RawFacet::new(Vector3d::UNIT_Z, v1[a as usize], v1[b as usize], v1[c as usize]));
    }
    for &[a, b, c] in &f2 {
        raw_facets.push(RawFacet::new(Vector3d::UNIT_Z, v2[a as usize], v2[b as usize], v2[c as usize]));
    }
    for &[a, b, c] in &f3 {
        raw_facets.push(RawFacet::new(Vector3d::UNIT_Z, v3[a as usize], v3[b as usize], v3[c as usize]));
    }

    let stl_bytes = write_binary_stl("Omni3MF Multi-Shell Test", &raw_facets).unwrap();

    let shells = ingest_repair_and_separate_shells(&stl_bytes, 1e-6, true, 64).unwrap();
    assert_eq!(shells.len(), 3, "Pipeline must separate 3 distinct binary STL shells");

    assert_eq!(shells[0].id, 1);
    assert_eq!(shells[0].name, "Shell_1");
    assert!(shells[0].is_watertight);

    assert_eq!(shells[1].id, 2);
    assert_eq!(shells[1].name, "Shell_2");
    assert!(shells[1].is_watertight);

    assert_eq!(shells[2].id, 3);
    assert_eq!(shells[2].name, "Shell_3");
    assert!(shells[2].is_watertight);

    // Verify volumes are approximately 8.0, 27.0, and 64.0
    let mut volumes = vec![
        shells[0].signed_volume,
        shells[1].signed_volume,
        shells[2].signed_volume,
    ];
    volumes.sort_by(|a, b| a.partial_cmp(b).unwrap());

    assert!((volumes[0] - 8.0).abs() < 1e-3);
    assert!((volumes[1] - 27.0).abs() < 1e-3);
    assert!((volumes[2] - 64.0).abs() < 1e-3);
}

#[test]
fn test_reviewer2_empty_binary_stl_returns_zero_shells() {
    let empty_binary = write_binary_stl("empty 84 byte", &[]).unwrap();
    assert_eq!(empty_binary.len(), 84);

    let mesh = ingest_and_repair_stl(&empty_binary, 1e-6, true, 64).unwrap();
    assert_eq!(mesh.vertex_count(), 0);
    assert_eq!(mesh.face_count(), 0);

    let shells = separate_connected_shells(&mesh);
    assert_eq!(shells.len(), 0);
}
