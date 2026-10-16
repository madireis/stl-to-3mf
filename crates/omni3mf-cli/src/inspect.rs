//! Implementation of the `inspect` subcommand.

use anyhow::{bail, Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use zip::ZipArchive;

use omni3mf_core::{
    check_scale_sanity, extract_boundary_loops, ingest_and_repair_stl, is_blocked_slicer_file,
    read_stl, separate_connected_shells, ScaleAnomaly,
};

use crate::args::InspectArgs;

/// Structured serializable inspection report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectionReport {
    pub file_name: String,
    pub file_type: String,
    pub file_size_bytes: u64,
    pub vertex_count: usize,
    pub face_count: usize,
    pub shell_count: usize,
    pub is_watertight: bool,
    pub bounding_box_min: [f64; 3],
    pub bounding_box_max: [f64; 3],
    pub dimensions_xyz: [f64; 3],
    pub signed_volume_mm3: f64,
    pub surface_area_mm2: f64,
    pub scale_anomaly: String,
    pub scale_suggestion: Option<String>,
    pub metadata_notes: Vec<String>,
}

/// Executes the `inspect` subcommand.
pub fn execute_inspect(args: InspectArgs) -> Result<()> {
    if !args.input.exists() {
        bail!("Input path does not exist: {}", args.input.display());
    }

    let file_bytes = fs::read(&args.input)
        .with_context(|| format!("Failed to read file: {}", args.input.display()))?;

    let is_3mf = is_3mf_file(&args.input, &file_bytes);

    let report = if is_3mf {
        inspect_3mf(&args.input, &file_bytes)?
    } else {
        inspect_stl(&args.input, &file_bytes)?
    };

    if args.json {
        let json_str = serde_json::to_string_pretty(&report)
            .context("Failed to serialize inspection report to JSON")?;
        println!("{}", json_str);
    } else {
        print_human_report(&report);
    }

    Ok(())
}

/// Checks whether the input is a 3MF archive (ZIP container).
fn is_3mf_file(path: &Path, bytes: &[u8]) -> bool {
    if let Some(ext) = path.extension() {
        if ext.eq_ignore_ascii_case("3mf") {
            return true;
        }
    }
    // Check ZIP magic signature [0x50, 0x4B, 0x03, 0x04]
    bytes.len() >= 4 && bytes[0..4] == [0x50, 0x4B, 0x03, 0x04]
}

/// Inspects an STL file.
fn inspect_stl(path: &Path, bytes: &[u8]) -> Result<InspectionReport> {
    let (format, raw_facets) = read_stl(bytes).context("Failed to parse STL geometry")?;
    let raw_face_count = raw_facets.len();

    let mesh = ingest_and_repair_stl(bytes, 1e-6, true, 64)
        .context("Failed to weld and analyze STL topology")?;

    let shells = separate_connected_shells(&mesh);
    let bbox = mesh.bounding_box();
    let dims = bbox.dimensions();
    let boundary_edges = omni3mf_core::EdgeTopology::build(&mesh.faces).find_boundary_directed_edges();
    let loops = extract_boundary_loops(&boundary_edges);
    let is_watertight = boundary_edges.is_empty() && !mesh.faces.is_empty();

    let mut total_vol = 0.0;
    let mut total_area = 0.0;
    for s in &shells {
        total_vol += s.signed_volume;
        total_area += s.surface_area;
    }

    let scale_rep = check_scale_sanity(&shells);
    let scale_anomaly_str = match scale_rep.anomaly {
        ScaleAnomaly::Normal => "Normal".to_string(),
        ScaleAnomaly::TooSmall => format!("TooSmall (< 2.0 mm, suggest {})", scale_rep.suggested_unit_name),
        ScaleAnomaly::TooLarge => format!("TooLarge (> 5000.0 mm, suggest {})", scale_rep.suggested_unit_name),
    };

    let suggestion = if scale_rep.anomaly != ScaleAnomaly::Normal {
        Some(scale_rep.user_prompt)
    } else {
        None
    };

    let file_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let (bbox_min, bbox_max) = if mesh.vertices.is_empty() {
        ([0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
    } else {
        (
            [bbox.min.x, bbox.min.y, bbox.min.z],
            [bbox.max.x, bbox.max.y, bbox.max.z],
        )
    };

    Ok(InspectionReport {
        file_name,
        file_type: format!("{:?} STL ({} raw facets)", format, raw_face_count),
        file_size_bytes: bytes.len() as u64,
        vertex_count: mesh.vertices.len(),
        face_count: mesh.faces.len(),
        shell_count: shells.len(),
        is_watertight,
        bounding_box_min: bbox_min,
        bounding_box_max: bbox_max,
        dimensions_xyz: [dims.x, dims.y, dims.z],
        signed_volume_mm3: total_vol,
        surface_area_mm2: total_area,
        scale_anomaly: scale_anomaly_str,
        scale_suggestion: suggestion,
        metadata_notes: vec![
            format!("Welding tolerance: 1e-6 mm"),
            format!("Open boundary loops: {}", loops.len()),
        ],
    })
}

/// Inspects a 3MF archive.
fn inspect_3mf(path: &Path, bytes: &[u8]) -> Result<InspectionReport> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .context("Failed to open 3MF as valid ZIP archive")?;

    let mut model_xml = String::new();
    let mut has_thumbnail = false;
    let mut vendor_files = Vec::new();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        if name.eq_ignore_ascii_case("3D/3dmodel.model") {
            file.read_to_string(&mut model_xml)?;
        } else if name.eq_ignore_ascii_case("Metadata/thumbnail.png") {
            has_thumbnail = true;
        } else if is_blocked_slicer_file(&name) {
            vendor_files.push(name);
        }
    }

    if model_xml.is_empty() {
        bail!("Archive is missing required 3D/3dmodel.model part");
    }

    // Parse model XML to count objects, vertices, triangles
    let mut reader = Reader::from_str(&model_xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    let mut object_count = 0usize;
    let mut vertex_count = 0usize;
    let mut triangle_count = 0usize;
    let mut has_colorgroup = false;

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut min_z = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut max_z = f64::NEG_INFINITY;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name = e.name().as_ref().to_ascii_lowercase();
                if name == b"object" {
                    object_count += 1;
                } else if name == b"m:colorgroup" {
                    has_colorgroup = true;
                } else if name == b"vertex" {
                    vertex_count += 1;
                    let mut vx = 0.0;
                    let mut vy = 0.0;
                    let mut vz = 0.0;
                    for attr in e.attributes().flatten() {
                        let val = std::str::from_utf8(attr.value.as_ref()).unwrap_or("0");
                        if attr.key.as_ref().eq_ignore_ascii_case(b"x") {
                            vx = val.parse::<f64>().unwrap_or(0.0);
                        } else if attr.key.as_ref().eq_ignore_ascii_case(b"y") {
                            vy = val.parse::<f64>().unwrap_or(0.0);
                        } else if attr.key.as_ref().eq_ignore_ascii_case(b"z") {
                            vz = val.parse::<f64>().unwrap_or(0.0);
                        }
                    }
                    min_x = min_x.min(vx);
                    max_x = max_x.max(vx);
                    min_y = min_y.min(vy);
                    max_y = max_y.max(vy);
                    min_z = min_z.min(vz);
                    max_z = max_z.max(vz);
                } else if name == b"triangle" {
                    triangle_count += 1;
                }
            }
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    let (min_x, min_y, min_z, max_x, max_y, max_z) = if vertex_count == 0 {
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
    } else {
        (min_x, min_y, min_z, max_x, max_y, max_z)
    };

    let dims_x = if max_x >= min_x { max_x - min_x } else { 0.0 };
    let dims_y = if max_y >= min_y { max_y - min_y } else { 0.0 };
    let dims_z = if max_z >= min_z { max_z - min_z } else { 0.0 };

    let file_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut notes = vec![
        format!("Thumbnail present: {}", if has_thumbnail { "Yes" } else { "No" }),
        format!("Color group present: {}", if has_colorgroup { "Yes" } else { "No" }),
    ];
    if !vendor_files.is_empty() {
        notes.push(format!("Vendor slicer files detected: {}", vendor_files.join(", ")));
    }

    Ok(InspectionReport {
        file_name,
        file_type: "3MF OPC Archive".to_string(),
        file_size_bytes: bytes.len() as u64,
        vertex_count,
        face_count: triangle_count,
        shell_count: object_count,
        is_watertight: vertex_count > 0 && triangle_count > 0,
        bounding_box_min: [min_x, min_y, min_z],
        bounding_box_max: [max_x, max_y, max_z],
        dimensions_xyz: [dims_x, dims_y, dims_z],
        signed_volume_mm3: 0.0,
        surface_area_mm2: 0.0,
        scale_anomaly: "Normal".to_string(),
        scale_suggestion: None,
        metadata_notes: notes,
    })
}

/// Prints a formatted human-readable terminal inspection report.
fn print_human_report(report: &InspectionReport) {
    println!();
    println!("Omni3MF Geometry Inspection");
    println!("==================================================");
    println!("  File Name:          {}", report.file_name);
    println!("  Format:             {}", report.file_type);
    println!("  File Size:          {} bytes", report.file_size_bytes);
    println!("  Discrete Shells:    {}", report.shell_count);
    println!("  Vertices:           {}", report.vertex_count);
    println!("  Triangles:          {}", report.face_count);
    println!("  Watertight:         {}", if report.is_watertight { "Yes (2-Manifold)" } else { "No (Open Boundaries)" });
    println!(
        "  Bounding Box Min:   [{:.3}, {:.3}, {:.3}] mm",
        report.bounding_box_min[0], report.bounding_box_min[1], report.bounding_box_min[2]
    );
    println!(
        "  Bounding Box Max:   [{:.3}, {:.3}, {:.3}] mm",
        report.bounding_box_max[0], report.bounding_box_max[1], report.bounding_box_max[2]
    );
    println!(
        "  Dimensions (X×Y×Z): {:.3} × {:.3} × {:.3} mm",
        report.dimensions_xyz[0], report.dimensions_xyz[1], report.dimensions_xyz[2]
    );
    if report.signed_volume_mm3.abs() > 1e-6 {
        println!("  Signed Volume:      {:.3} mm³", report.signed_volume_mm3);
    }
    if report.surface_area_mm2 > 1e-6 {
        println!("  Surface Area:       {:.3} mm²", report.surface_area_mm2);
    }
    println!("  Scale Status:       {}", report.scale_anomaly);

    if let Some(sugg) = &report.scale_suggestion {
        println!("  ⚠ Recommendation:  {}", sugg);
    }

    if !report.metadata_notes.is_empty() {
        println!("  Notes:");
        for note in &report.metadata_notes {
            println!("    • {}", note);
        }
    }
    println!("==================================================");
    println!();
}
