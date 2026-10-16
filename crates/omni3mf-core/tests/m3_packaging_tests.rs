//! Omni3MF Milestone 3 Integration Tests:
//! Packaging, Multi-Shell XML, Pure-Rust Thumbnail Rasterizer, Profile Sanitization, and Scale Heuristics.

use std::io::Read;
use zip::ZipArchive;

use omni3mf_core::math::Point3d;
use omni3mf_core::mesh::IndexedMesh;
use omni3mf_core::package::{
    build_3mf_package, build_3mf_package_from_mesh, PackagingOptions, ThreeMfPackage,
};
use omni3mf_core::pipeline::{convert_stl_to_3mf, PipelineOptions};
use omni3mf_core::sanitizer::{sanitize_3mf_archive, SanitizerOptions};
use omni3mf_core::scale::{check_scale_sanity_points, ScaleAnomaly};
use omni3mf_core::shell::{separate_connected_shells, MeshShell};
use omni3mf_core::stl::{write_binary_stl, RawFacet};
use omni3mf_core::thumbnail::{RasterizerOptions, ThumbnailRasterizer};

/// Helper: constructs raw facets for a cube of given side length at offset (ox, oy, oz).
fn create_cube_facets(ox: f64, oy: f64, oz: f64, size: f64) -> Vec<RawFacet> {
    use omni3mf_core::math::Vector3d;
    let p = |x: f64, y: f64, z: f64| Point3d::new(ox + x * size, oy + y * size, oz + z * size);
    let n = Vector3d::new;
    let facet = |normal: Vector3d, v0: Point3d, v1: Point3d, v2: Point3d| RawFacet {
        normal,
        v0,
        v1,
        v2,
        attribute_byte_count: 0,
    };

    vec![
        // Bottom (z = 0)
        facet(n(0.0, 0.0, -1.0), p(0.0, 0.0, 0.0), p(1.0, 1.0, 0.0), p(1.0, 0.0, 0.0)),
        facet(n(0.0, 0.0, -1.0), p(0.0, 0.0, 0.0), p(0.0, 1.0, 0.0), p(1.0, 1.0, 0.0)),
        // Top (z = 1)
        facet(n(0.0, 0.0, 1.0), p(0.0, 0.0, 1.0), p(1.0, 0.0, 1.0), p(1.0, 1.0, 1.0)),
        facet(n(0.0, 0.0, 1.0), p(0.0, 0.0, 1.0), p(1.0, 1.0, 1.0), p(0.0, 1.0, 1.0)),
        // Front (y = 0)
        facet(n(0.0, -1.0, 0.0), p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(1.0, 0.0, 1.0)),
        facet(n(0.0, -1.0, 0.0), p(0.0, 0.0, 0.0), p(1.0, 0.0, 1.0), p(0.0, 0.0, 1.0)),
        // Back (y = 1)
        facet(n(0.0, 1.0, 0.0), p(0.0, 1.0, 0.0), p(1.0, 1.0, 1.0), p(1.0, 1.0, 0.0)),
        facet(n(0.0, 1.0, 0.0), p(0.0, 1.0, 0.0), p(0.0, 1.0, 1.0), p(1.0, 1.0, 1.0)),
        // Left (x = 0)
        facet(n(-1.0, 0.0, 0.0), p(0.0, 0.0, 0.0), p(0.0, 0.0, 1.0), p(0.0, 1.0, 1.0)),
        facet(n(-1.0, 0.0, 0.0), p(0.0, 0.0, 0.0), p(0.0, 1.0, 1.0), p(0.0, 1.0, 0.0)),
        // Right (x = 1)
        facet(n(1.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0), p(1.0, 1.0, 1.0)),
        facet(n(1.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(1.0, 1.0, 1.0), p(1.0, 0.0, 1.0)),
    ]
}

/// Helper: creates an IndexedMesh cube at (ox, oy, oz) with size.
fn create_cube_mesh(ox: f64, oy: f64, oz: f64, size: f64) -> IndexedMesh {
    let vertices = vec![
        Point3d::new(ox, oy, oz),
        Point3d::new(ox + size, oy, oz),
        Point3d::new(ox + size, oy + size, oz),
        Point3d::new(ox, oy + size, oz),
        Point3d::new(ox, oy, oz + size),
        Point3d::new(ox + size, oy, oz + size),
        Point3d::new(ox + size, oy + size, oz + size),
        Point3d::new(ox, oy + size, oz + size),
    ];
    let faces = vec![
        [0, 2, 1], [0, 3, 2], // bottom
        [4, 5, 6], [4, 6, 7], // top
        [0, 1, 5], [0, 5, 4], // front
        [2, 3, 7], [2, 7, 6], // back
        [0, 4, 7], [0, 7, 3], // left
        [1, 2, 6], [1, 6, 5], // right
    ];
    IndexedMesh::from_vertices_and_faces(vertices, faces)
}

// ============================================================================
// 1. OPC ZIP Packaging & Manifest Tests
// ============================================================================

#[test]
fn test_opc_zip_archive_structure_and_manifests() {
    let mesh1 = create_cube_mesh(0.0, 0.0, 0.0, 10.0);
    let mesh2 = create_cube_mesh(25.0, 0.0, 0.0, 10.0);

    let shells = vec![
        MeshShell {
            id: 1,
            name: "Cube_A".to_string(),
            mesh: mesh1.clone(),
            bbox: mesh1.bounding_box(),
            center_of_mass: mesh1.center_of_mass(),
            signed_volume: mesh1.signed_volume(),
            surface_area: mesh1.surface_area(),
            is_watertight: true,
        },
        MeshShell {
            id: 2,
            name: "Cube_B".to_string(),
            mesh: mesh2.clone(),
            bbox: mesh2.bounding_box(),
            center_of_mass: mesh2.center_of_mass(),
            signed_volume: mesh2.signed_volume(),
            surface_area: mesh2.surface_area(),
            is_watertight: true,
        },
    ];

    let options = PackagingOptions::default();
    let zip_bytes = build_3mf_package(&shells, &options).expect("build_3mf_package failed");

    // Unpack ZIP
    let mut archive =
        ZipArchive::new(std::io::Cursor::new(&zip_bytes)).expect("Failed reading 3MF ZIP");

    // 1. Verify mandatory physical layout
    let required_entries = [
        "[Content_Types].xml",
        "_rels/.rels",
        "3D/3dmodel.model",
        "Metadata/thumbnail.png",
        "3D/_rels/3dmodel.model.rels",
    ];

    for &entry_name in &required_entries {
        assert!(
            archive.by_name(entry_name).is_ok(),
            "Archive must contain OPC entry: {entry_name}"
        );
    }

    // 2. Validate [Content_Types].xml
    let mut ct_xml = String::new();
    {
        let mut ct_file = archive.by_name("[Content_Types].xml").unwrap();
        ct_file.read_to_string(&mut ct_xml).unwrap();
    }
    assert!(ct_xml.contains(r#"Extension="rels""#));
    assert!(ct_xml.contains(r#"Extension="model""#));
    assert!(ct_xml.contains(r#"Extension="png""#));
    assert!(ct_xml.contains(r#"ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml""#));

    // 3. Validate _rels/.rels
    let mut rels_xml = String::new();
    {
        let mut rels_file = archive.by_name("_rels/.rels").unwrap();
        rels_file.read_to_string(&mut rels_xml).unwrap();
    }
    assert!(rels_xml.contains(r#"Target="/3D/3dmodel.model""#));
    assert!(rels_xml.contains(r#"Target="/Metadata/thumbnail.png""#));

    // 4. Validate 3D/_rels/3dmodel.model.rels
    let mut model_rels_xml = String::new();
    {
        let mut mr_file = archive.by_name("3D/_rels/3dmodel.model.rels").unwrap();
        mr_file.read_to_string(&mut model_rels_xml).unwrap();
    }
    assert!(model_rels_xml.contains(r#"Target="/Metadata/thumbnail.png""#));

    // 5. Validate deterministic timestamps (1980 DOS epoch)
    let first_file = archive.by_index(0).unwrap();
    let mtime = first_file.last_modified().expect("Missing mtime");
    assert_eq!(mtime.year(), 1980);
    assert_eq!(mtime.month(), 1);
    assert_eq!(mtime.day(), 1);
}

// ============================================================================
// 2. Multi-Shell 3MF Model XML Tests
// ============================================================================

#[test]
fn test_multi_shell_model_n_objects_and_items() {
    // Generate 4 disjoint shells
    let n_shells = 4;
    let mut shells = Vec::new();
    for i in 1..=n_shells {
        let m = create_cube_mesh(i as f64 * 30.0, 0.0, 0.0, 10.0);
        shells.push(MeshShell {
            id: i as u32,
            name: format!("Part_{i}"),
            mesh: m.clone(),
            bbox: m.bounding_box(),
            center_of_mass: m.center_of_mass(),
            signed_volume: m.signed_volume(),
            surface_area: m.surface_area(),
            is_watertight: true,
        });
    }

    let zip_bytes =
        build_3mf_package(&shells, &PackagingOptions::default()).expect("Packaging failed");
    let mut archive = ZipArchive::new(std::io::Cursor::new(&zip_bytes)).unwrap();

    let mut model_xml = String::new();
    {
        let mut model_file = archive.by_name("3D/3dmodel.model").unwrap();
        model_file.read_to_string(&mut model_xml).unwrap();
    }

    // Must declare millimeters unit and 3MF core & material namespaces
    assert!(model_xml.contains(r#"unit="millimeter""#));
    assert!(model_xml.contains(r#"xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02""#));
    assert!(model_xml.contains(r#"xmlns:m="http://schemas.microsoft.com/3dmanufacturing/material/2015/02""#));

    // Must contain color group
    assert!(model_xml.contains(r#"<m:colorgroup id="1">"#));

    // Must contain exactly N object nodes
    for i in 1..=n_shells {
        let obj_id = i + 1;
        let obj_needle = format!(r#"<object id="{obj_id}" type="model" name="Part_{i}""#);
        assert!(model_xml.contains(&obj_needle), "Must contain object node for part {i}");

        let item_needle = format!(r#"<item objectid="{obj_id}"/>"#);
        assert!(model_xml.contains(&item_needle), "Must contain item node for part {i}");
    }

    // Verify object count via string occurrences
    let object_count = model_xml.matches("<object ").count();
    assert_eq!(object_count, n_shells, "Must have exactly {n_shells} object nodes");

    let item_count = model_xml.matches("<item ").count();
    assert_eq!(item_count, n_shells, "Must have exactly {n_shells} item nodes");
}

// ============================================================================
// 3. Pure-Rust Software Thumbnail Rasterizer Tests
// ============================================================================

#[test]
fn test_thumbnail_png_specifications_and_shading() {
    let mesh = create_cube_mesh(0.0, 0.0, 0.0, 20.0);
    let shell = MeshShell {
        id: 1,
        name: "Cube".to_string(),
        mesh: mesh.clone(),
        bbox: mesh.bounding_box(),
        center_of_mass: mesh.center_of_mass(),
        signed_volume: mesh.signed_volume(),
        surface_area: mesh.surface_area(),
        is_watertight: true,
    };

    let opts = RasterizerOptions {
        width: 256,
        height: 256,
        margin: 0.15,
        backface_culling: true,
        lod_triangle_threshold: 50_000,
    };

    let mut rasterizer = ThumbnailRasterizer::new(opts.width, opts.height);
    rasterizer.render_shells(&[shell], &opts);

    // 1. Verify color buffer alpha values
    let cb = rasterizer.color_buffer();
    assert_eq!(cb.len(), 256 * 256 * 4);

    // Check corners: top-left, top-right, bottom-left, bottom-right must be transparent (alpha = 0)
    let corner_top_left = 3;
    let corner_top_right = (255) * 4 + 3;
    let corner_bottom_left = (255 * 256) * 4 + 3;
    let corner_bottom_right = (255 * 256 + 255) * 4 + 3;

    assert_eq!(cb[corner_top_left], 0, "Top-left corner must be transparent");
    assert_eq!(cb[corner_top_right], 0, "Top-right corner must be transparent");
    assert_eq!(cb[corner_bottom_left], 0, "Bottom-left corner must be transparent");
    assert_eq!(cb[corner_bottom_right], 0, "Bottom-right corner must be transparent");

    // Center pixel (128, 128) must be opaque
    let center_alpha = (128 * 256 + 128) * 4 + 3;
    assert_eq!(cb[center_alpha], 255, "Center pixel must be opaque rendered mesh");

    // Center pixel RGB must not be black (shading applied)
    let center_r = cb[(128 * 256 + 128) * 4];
    let center_g = cb[(128 * 256 + 128) * 4 + 1];
    let center_b = cb[(128 * 256 + 128) * 4 + 2];
    assert!(center_r > 0 || center_g > 0 || center_b > 0, "Rendered center must have color");

    // 2. Encode to PNG and verify standard PNG format
    let png_bytes = rasterizer.encode_png().expect("PNG encoding failed");

    // Magic bytes: 0x89 'P' 'N' 'G' '\r' '\n' 0x1A '\n'
    assert_eq!(
        &png_bytes[0..8],
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        "PNG signature must be valid"
    );

    // Read with PNG decoder to verify metadata
    let decoder = png::Decoder::new(std::io::Cursor::new(&png_bytes));
    let mut reader = decoder.read_info().expect("Failed reading PNG header");
    let info = reader.info();
    assert_eq!(info.width, 256);
    assert_eq!(info.height, 256);
    assert_eq!(info.color_type, png::ColorType::Rgba);
    assert_eq!(info.bit_depth, png::BitDepth::Eight);

    // Read image frame
    let mut decoded_buf = vec![0u8; reader.output_buffer_size()];
    let output_info = reader.next_frame(&mut decoded_buf).expect("Failed reading PNG frame");
    assert_eq!(output_info.width, 256);
    assert_eq!(output_info.height, 256);
}

// ============================================================================
// 4. Slicer Profile Sanitizer Tests
// ============================================================================

#[test]
fn test_slicer_profile_sanitizer_drops_configs_and_preserves_materials() {
    // Construct a mock 3MF archive containing vendor config files and XML metadata
    let dirty_model_xml = br##"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02" xmlns:m="http://schemas.microsoft.com/3dmanufacturing/material/2015/02">
  <metadata name="Title">My Clean Model</metadata>
  <metadata name="slic3rpe:print_settings_id">0.20mm SPEED @MK4</metadata>
  <metadata name="bambu:filament_settings_id">Bambu PLA Basic</metadata>
  <metadata name="OrcaSlicer:process_name">Standard 0.20mm</metadata>
  <config>
    <setting name="nozzle_temperature" value="215"/>
    <setting name="first_layer_speed" value="30"/>
  </config>
  <resources>
    <m:colorgroup id="1">
      <m:color color="#00A3E0FF"/>
      <m:color color="#FF7A00FF"/>
    </m:colorgroup>
    <object id="1" type="model" pid="1" pindex="0">
      <mesh>
        <vertices><vertex x="0.0" y="0.0" z="0.0"/></vertices>
        <triangles><triangle v1="0" v2="0" v3="0"/></triangles>
      </mesh>
    </object>
    <object id="2" type="support">
      <mesh>
        <vertices><vertex x="10.0" y="10.0" z="10.0"/></vertices>
        <triangles><triangle v1="0" v2="0" v3="0"/></triangles>
      </mesh>
    </object>
  </resources>
  <build>
    <item objectid="1"/>
    <item objectid="2"/>
  </build>
</model>"##;

    let mut dirty_pkg = ThreeMfPackage::new(dirty_model_xml.to_vec(), Some(vec![0x89, b'P', b'N', b'G']));
    // Inject vendor configs into archive
    dirty_pkg.add_extra_file("Metadata/project_settings.config", b"bambu_project=1".to_vec());
    dirty_pkg.add_extra_file("Metadata/slice_info.config", b"<plate>1</plate>".to_vec());
    dirty_pkg.add_extra_file("Metadata/Slic3r_PE.config", b"prusa_filament=PLA".to_vec());
    dirty_pkg.add_extra_file("Metadata/plate_1.json", b"{\"plate_id\":1}".to_vec());
    dirty_pkg.add_extra_file("Cura/cura.cfg", b"speed=60".to_vec());
    dirty_pkg.add_extra_file("toolpaths.gcode", b"G1 X10 Y10".to_vec());
    // Safe custom asset to preserve
    dirty_pkg.add_extra_file("Metadata/custom_notes.txt", b"Safe user license".to_vec());

    let dirty_bytes = dirty_pkg.to_bytes().expect("Failed creating dirty package");

    // Run sanitizer
    let options = SanitizerOptions::default();
    let clean_bytes = sanitize_3mf_archive(&dirty_bytes, &options).expect("Sanitization failed");

    // Inspect sanitized archive
    let mut clean_archive = ZipArchive::new(std::io::Cursor::new(&clean_bytes)).expect("Failed reading clean zip");

    // 1. Verify blocked files were dropped
    assert!(clean_archive.by_name("Metadata/project_settings.config").is_err(), "Must drop project_settings.config");
    assert!(clean_archive.by_name("Metadata/slice_info.config").is_err(), "Must drop slice_info.config");
    assert!(clean_archive.by_name("Metadata/Slic3r_PE.config").is_err(), "Must drop Slic3r_PE.config");
    assert!(clean_archive.by_name("Metadata/plate_1.json").is_err(), "Must drop plate_1.json");
    assert!(clean_archive.by_name("Cura/cura.cfg").is_err(), "Must drop Cura/cura.cfg");
    assert!(clean_archive.by_name("toolpaths.gcode").is_err(), "Must drop toolpaths.gcode");

    // 2. Verify safe files and thumbnail preserved
    assert!(clean_archive.by_name("Metadata/thumbnail.png").is_ok(), "Must preserve thumbnail");
    assert!(clean_archive.by_name("Metadata/custom_notes.txt").is_ok(), "Must preserve custom safe files");

    // 3. Inspect sanitized 3D/3dmodel.model
    let mut model_xml = String::new();
    {
        let mut model_file = clean_archive.by_name("3D/3dmodel.model").unwrap();
        model_file.read_to_string(&mut model_xml).unwrap();
    }

    // Stripped XML configs & vendor metadata
    assert!(!model_xml.contains("slic3rpe:print_settings_id"), "Must strip slic3rpe metadata");
    assert!(!model_xml.contains("bambu:filament_settings_id"), "Must strip bambu metadata");
    assert!(!model_xml.contains("OrcaSlicer:process_name"), "Must strip OrcaSlicer metadata");
    assert!(!model_xml.contains("<config>"), "Must strip <config> section");
    assert!(!model_xml.contains("nozzle_temperature"), "Must strip nozzle_temperature");

    // Preserved visual materials & support
    assert!(model_xml.contains(r#"<metadata name="Title">My Clean Model</metadata>"#));
    assert!(model_xml.contains(r#"<m:colorgroup id="1">"#), "Must preserve m:colorgroup");
    assert!(model_xml.contains(r##"<m:color color="#00A3E0FF"/>"##), "Must preserve color entries");
    assert!(model_xml.contains(r#"pid="1""#), "Must preserve pid bindings");
    assert!(model_xml.contains(r#"pindex="0""#), "Must preserve pindex bindings");
    assert!(model_xml.contains(r#"<object id="2" type="support">"#), "Must preserve support geometry");
}

// ============================================================================
// 5. Dynamic Scale Sanity Checker Tests
// ============================================================================

#[test]
fn test_scale_sanity_checker_all_heuristics() {
    // Case 1: Inches scale (< 2.0 mm, e.g. 1.5 x 1.0 x 0.5 inches treated as mm)
    let inches_pts = vec![Point3d::new(0.0, 0.0, 0.0), Point3d::new(1.5, 1.0, 0.5)];
    let rep_inches = check_scale_sanity_points(&inches_pts);
    assert_eq!(rep_inches.anomaly, ScaleAnomaly::TooSmall);
    assert_eq!(rep_inches.suggested_scale, 25.4);
    assert_eq!(rep_inches.suggested_unit_name, "inches");
    assert!(rep_inches.user_prompt.contains("under 2mm"));
    assert!(rep_inches.user_prompt.contains("inches"));

    // Case 2: Meters scale (< 2.0 mm, e.g. 0.25 x 0.20 x 0.15 m treated as mm)
    let meters_pts = vec![Point3d::new(0.0, 0.0, 0.0), Point3d::new(0.25, 0.20, 0.15)];
    let rep_meters = check_scale_sanity_points(&meters_pts);
    assert_eq!(rep_meters.anomaly, ScaleAnomaly::TooSmall);
    assert_eq!(rep_meters.suggested_scale, 1000.0);
    assert_eq!(rep_meters.suggested_unit_name, "meters");

    // Case 3: Microns scale (> 5000 mm, e.g. 50,000 microns treated as mm)
    let microns_pts = vec![Point3d::new(0.0, 0.0, 0.0), Point3d::new(50_000.0, 20_000.0, 10_000.0)];
    let rep_microns = check_scale_sanity_points(&microns_pts);
    assert_eq!(rep_microns.anomaly, ScaleAnomaly::TooLarge);
    assert_eq!(rep_microns.suggested_scale, 0.001);
    assert_eq!(rep_microns.suggested_unit_name, "microns");
    assert!(rep_microns.user_prompt.contains("exceeds 5000mm"));

    // Case 4: Centimeters scale (> 5000 mm, e.g. 5500 mm)
    let cm_pts = vec![Point3d::new(0.0, 0.0, 0.0), Point3d::new(5500.0, 10.0, 10.0)];
    let rep_cm = check_scale_sanity_points(&cm_pts);
    assert_eq!(rep_cm.anomaly, ScaleAnomaly::TooLarge);
    assert_eq!(rep_cm.suggested_scale, 0.1);
    assert_eq!(rep_cm.suggested_unit_name, "centimeters");

    // Case 5: Safe normal scale (100 x 50 x 25 mm)
    let normal_pts = vec![Point3d::new(0.0, 0.0, 0.0), Point3d::new(100.0, 50.0, 25.0)];
    let rep_normal = check_scale_sanity_points(&normal_pts);
    assert_eq!(rep_normal.anomaly, ScaleAnomaly::Normal);
    assert_eq!(rep_normal.suggested_scale, 1.0);
    assert_eq!(rep_normal.suggested_unit_name, "millimeters");
    assert!(rep_normal.user_prompt.is_empty());
}

// ============================================================================
// 6. End-to-End Pipeline Multi-Shell STL to 3MF Test
// ============================================================================

#[test]
fn test_end_to_end_stl_to_3mf_pipeline() {
    // Create an STL file with 3 disjoint cubes
    let mut facets = Vec::new();
    facets.extend(create_cube_facets(0.0, 0.0, 0.0, 10.0));
    facets.extend(create_cube_facets(25.0, 0.0, 0.0, 10.0));
    facets.extend(create_cube_facets(50.0, 0.0, 0.0, 10.0));

    let stl_bytes = write_binary_stl("TripleCube", &facets).expect("Binary STL write failed");

    let options = PipelineOptions {
        tolerance: 1e-6,
        patch_holes: true,
        max_hole_edges: 128,
        include_thumbnail: true,
        thumbnail_options: RasterizerOptions::default(),
        sanitize_output: true,
        sanitizer_options: SanitizerOptions::default(),
        check_scale: true,
        ..Default::default()
    };

    let result = convert_stl_to_3mf(&stl_bytes, &options).expect("Conversion pipeline failed");

    // Assert 3 shells
    assert_eq!(result.shells.len(), 3, "Pipeline must segment 3 discrete shells");
    assert_eq!(result.shells[0].id, 1);
    assert_eq!(result.shells[1].id, 2);
    assert_eq!(result.shells[2].id, 3);

    // Assert thumbnail generated and valid PNG
    assert!(result.thumbnail_png.is_some());
    let thumb = result.thumbnail_png.unwrap();
    assert_eq!(&thumb[0..8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);

    // Assert scale check was evaluated
    assert!(result.scale_report.is_some());
    assert_eq!(result.scale_report.unwrap().anomaly, ScaleAnomaly::Normal);

    // Inspect the resulting 3MF archive
    let mut archive = ZipArchive::new(std::io::Cursor::new(&result.package_bytes)).expect("Valid zip");
    assert!(archive.by_name("[Content_Types].xml").is_ok());
    assert!(archive.by_name("_rels/.rels").is_ok());
    assert!(archive.by_name("3D/3dmodel.model").is_ok());
    assert!(archive.by_name("Metadata/thumbnail.png").is_ok());

    let mut model_xml = String::new();
    {
        let mut model_file = archive.by_name("3D/3dmodel.model").unwrap();
        model_file.read_to_string(&mut model_xml).unwrap();
    }

    assert_eq!(model_xml.matches("<object ").count(), 3);
    assert_eq!(model_xml.matches("<item ").count(), 3);
}

// ============================================================================
// 7. Empty Mesh Defense & Robustness
// ============================================================================

#[test]
fn test_empty_mesh_defense_and_robustness() {
    // 1. Empty mesh shell separation returns empty Vec without panicking
    let empty_mesh = IndexedMesh::from_vertices_and_faces(Vec::new(), Vec::new());
    let shells = separate_connected_shells(&empty_mesh);
    assert!(shells.is_empty(), "Empty mesh must yield zero shells");

    // 2. Mesh with vertices but 0 faces
    let mesh_verts_only = IndexedMesh::from_vertices_and_faces(
        vec![Point3d::new(1.0, 1.0, 1.0)],
        Vec::new(),
    );
    let shells_v = separate_connected_shells(&mesh_verts_only);
    assert!(shells_v.is_empty(), "Faces-empty mesh must yield zero shells");

    // 3. Packaging empty shells produces valid 3MF archive
    let empty_pkg = build_3mf_package(&[], &PackagingOptions::default()).expect("Packaging empty shells should succeed");
    let mut archive = ZipArchive::new(std::io::Cursor::new(empty_pkg)).expect("Valid zip");
    assert!(archive.by_name("3D/3dmodel.model").is_ok());

    // 4. Packaging empty mesh directly
    let empty_mesh_pkg = build_3mf_package_from_mesh(&empty_mesh, &PackagingOptions::default())
        .expect("Packaging empty mesh should succeed");
    let mut archive_mesh = ZipArchive::new(std::io::Cursor::new(empty_mesh_pkg)).expect("Valid zip");
    assert!(archive_mesh.by_name("3D/3dmodel.model").is_ok());
}
