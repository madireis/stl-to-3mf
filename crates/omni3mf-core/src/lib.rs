#![forbid(unsafe_code)]
//! Omni3MF Core: High-performance, memory-safe STL ingestion, mesh repair, and 3MF packaging engine.

pub mod math;
pub mod mesh;
pub mod package;
pub mod pipeline;
pub mod repair;
pub mod sanitizer;
pub mod scale;
pub mod shell;
pub mod stl;
pub mod thumbnail;

pub use math::{
    BoundingBox3d, Point3d, Vector3d, AREA_EPSILON_SQ, FLOAT_EPSILON, VERTEX_MERGE_EPSILON,
    VERTEX_MERGE_EPSILON_SQ,
};
pub use mesh::{DirectedEdge, EdgeTopology, IndexedMesh, SpatialHashGrid, UndirectedEdge};
pub use package::{
    build_3mf_package, build_3mf_package_from_mesh, serialize_mesh_to_model_xml,
    serialize_shells_to_model_xml, PackagingOptions, ThreeMfError, ThreeMfPackage,
    COLOR_PALETTE_HEX, CONTENT_TYPES_XML, MODEL_RELS_XML, ROOT_RELS_NO_THUMB_XML,
    ROOT_RELS_WITH_THUMB_XML,
};
pub use pipeline::{
    convert_stl_to_3mf, convert_stl_to_3mf_bytes, PipelineError, PipelineOptions, PipelineResult,
};
pub use repair::{
    extract_boundary_loops, filter_degenerate_triangles, patch_boundary_holes,
    patch_hole_iterative_fan, unify_winding_and_orient_outward,
};
pub use sanitizer::{
    is_blocked_slicer_file, sanitize_3mf_archive, sanitize_model_xml, SanitizerOptions,
};
pub use scale::{
    check_scale_sanity, check_scale_sanity_bbox, check_scale_sanity_mesh, check_scale_sanity_points,
    ScaleAnomaly, ScaleSanityReport,
};
pub use shell::{separate_connected_shells, DisjointSetUnion, FaceAdjacencyGraph, MeshShell};
pub use stl::{
    detect_stl_format, parse_ascii_stl, parse_binary_stl, read_stl, write_ascii_stl,
    write_binary_stl, RawFacet, StlError, StlFormat,
};
pub use thumbnail::{
    render_thumbnail, render_thumbnail_mesh, RasterizerOptions, ThumbnailRasterizer,
    DEFAULT_SHELL_COLORS,
};

/// Ingests an STL buffer, heals defects, unifies normals, and separates into discrete connected shells.
pub fn ingest_repair_and_separate_shells(
    stl_bytes: &[u8],
    tolerance: f64,
    patch_holes: bool,
    max_hole_edges: usize,
) -> Result<Vec<MeshShell>, StlError> {
    let mesh = ingest_and_repair_stl(stl_bytes, tolerance, patch_holes, max_hole_edges)?;
    Ok(separate_connected_shells(&mesh))
}

/// Ingests an STL byte buffer (binary or ASCII), welds coincident vertices within `tolerance`,
/// prunes degenerate facets, optionally patches boundary holes, and unifies normal orientation outward.
pub fn ingest_and_repair_stl(
    stl_bytes: &[u8],
    tolerance: f64,
    patch_holes: bool,
    max_hole_edges: usize,
) -> Result<IndexedMesh, StlError> {
    let (_format, facets) = read_stl(stl_bytes)?;
    let (vertices, raw_faces) = SpatialHashGrid::weld_raw_facets(&facets, tolerance);
    let valid_faces = filter_degenerate_triangles(&raw_faces, &vertices);
    let mut mesh = IndexedMesh::from_vertices_and_faces(vertices, valid_faces);
    mesh.prune_unreferenced_vertices();

    if patch_holes {
        unify_winding_and_orient_outward(&mut mesh);
        patch_boundary_holes(&mut mesh, max_hole_edges);
    }

    unify_winding_and_orient_outward(&mut mesh);
    mesh.prune_unreferenced_vertices();
    Ok(mesh)
}
