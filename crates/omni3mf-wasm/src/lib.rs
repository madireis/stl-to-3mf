//! High-performance client-side WebAssembly bindings for Omni3MF.
//!
//! Exposes in-browser conversion, inspection, repair, software thumbnail rendering,
//! and slicer profile sanitization via `wasm-bindgen`.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use omni3mf_core::{
    check_scale_sanity, ingest_and_repair_stl, sanitize_3mf_archive,
    separate_connected_shells, PipelineOptions, RasterizerOptions, SanitizerOptions, ScaleAnomaly,
};

/// Configuration options passed from JavaScript into `convert_stl_to_3mf`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WasmPipelineOptions {
    /// Spatial hash welding coordinate tolerance in mm (defaults to 1e-6).
    pub merge_tolerance: Option<f64>,
    /// Whether to heal open boundary holes (defaults to true).
    pub repair: Option<bool>,
    /// Maximum perimeter edges in a boundary loop to patch via fan/ear-clipping (defaults to 64).
    pub hole_max_edges: Option<usize>,
    /// Whether to separate connected components into discrete shells (defaults to true).
    pub split_shells: Option<bool>,
    /// Whether to purge vendor slicer profile configuration files and metadata (defaults to false).
    pub strip_slicer_config: Option<bool>,
    /// Manual scaling factor override (e.g. 25.4 for inches to mm).
    pub scale_override: Option<f64>,
    /// Whether to generate and embed a 256x256 software thumbnail PNG preview (defaults to true).
    pub generate_thumbnail: Option<bool>,
}

/// Structured inspection report returned to JavaScript by `inspect_stl`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmMeshInspection {
    /// Total number of unique vertices after spatial welding.
    pub vertex_count: usize,
    /// Total number of triangular faces.
    pub face_count: usize,
    /// Number of discrete connected shells.
    pub shell_count: usize,
    /// Whether the geometry forms a closed, watertight 2-manifold without boundary edges.
    pub is_watertight: bool,
    /// Bounding box dimensions [X, Y, Z] in millimeters.
    pub dimensions: [f64; 3],
    /// Axis-aligned bounding box minimum coordinates [min_x, min_y, min_z].
    pub bounding_box_min: [f64; 3],
    /// Axis-aligned bounding box maximum coordinates [max_x, max_y, max_z].
    pub bounding_box_max: [f64; 3],
    /// Total signed volume in cubic millimeters.
    pub volume: f64,
    /// Total surface area in square millimeters.
    pub surface_area: f64,
    /// Scale anomaly diagnostic ("Normal", "TooSmall", "TooLarge").
    pub scale_anomaly: String,
    /// Suggested scaling multiplier if a unit mismatch was diagnosed.
    pub suggested_scale: f64,
    /// Human-readable scale warning / recommendation prompt.
    pub scale_warning: Option<String>,
}

/// Initializes the panic hook to print detailed panic callstacks to the browser developer console.
#[wasm_bindgen]
pub fn init_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Optional automatic startup hook called upon WebAssembly module instantiation.
#[wasm_bindgen(start)]
pub fn main_js() {
    init_panic_hook();
}

/// Internal implementation of STL to 3MF conversion.
pub fn convert_stl_to_3mf_internal(
    stl_bytes: &[u8],
    opts: &WasmPipelineOptions,
) -> Result<Vec<u8>, String> {
    if stl_bytes.is_empty() {
        return Err("Input STL bytes buffer is empty".to_string());
    }

    let pipeline_opts = PipelineOptions {
        tolerance: opts.merge_tolerance.unwrap_or(1e-6),
        patch_holes: opts.repair.unwrap_or(true),
        max_hole_edges: opts.hole_max_edges.unwrap_or(64),
        split_shells: opts.split_shells.unwrap_or(true),
        scale_override: opts.scale_override,
        include_thumbnail: opts.generate_thumbnail.unwrap_or(true),
        thumbnail_options: RasterizerOptions::default(),
        sanitize_output: opts.strip_slicer_config.unwrap_or(false),
        sanitizer_options: SanitizerOptions::default(),
        check_scale: true,
    };

    let result = omni3mf_core::convert_stl_to_3mf(stl_bytes, &pipeline_opts)
        .map_err(|e| format!("Conversion error: {e}"))?;

    Ok(result.package_bytes)
}

/// Internal implementation of STL inspection.
pub fn inspect_stl_internal(stl_bytes: &[u8]) -> Result<WasmMeshInspection, String> {
    if stl_bytes.is_empty() {
        return Err("Input STL bytes buffer is empty".to_string());
    }

    let mesh = ingest_and_repair_stl(stl_bytes, 1e-6, true, 64)
        .map_err(|e| format!("Failed to parse STL: {e}"))?;

    let shells = separate_connected_shells(&mesh);
    let bbox = mesh.bounding_box();
    let dims = bbox.dimensions();
    let boundary_edges = omni3mf_core::EdgeTopology::build(&mesh.faces).find_boundary_directed_edges();
    let is_watertight = boundary_edges.is_empty() && !mesh.faces.is_empty();

    let mut total_vol = 0.0;
    let mut total_area = 0.0;
    for s in &shells {
        total_vol += s.signed_volume;
        total_area += s.surface_area;
    }

    let scale_rep = check_scale_sanity(&shells);
    let (anomaly_str, warning_str) = match scale_rep.anomaly {
        ScaleAnomaly::Normal => ("Normal".to_string(), None),
        ScaleAnomaly::TooSmall => (
            format!("TooSmall (< 2mm, suggest {}x)", scale_rep.suggested_scale),
            Some(scale_rep.user_prompt),
        ),
        ScaleAnomaly::TooLarge => (
            format!("TooLarge (> 5000mm, suggest {}x)", scale_rep.suggested_scale),
            Some(scale_rep.user_prompt),
        ),
    };

    Ok(WasmMeshInspection {
        vertex_count: mesh.vertices.len(),
        face_count: mesh.faces.len(),
        shell_count: shells.len(),
        is_watertight,
        dimensions: [dims.x, dims.y, dims.z],
        bounding_box_min: [bbox.min.x, bbox.min.y, bbox.min.z],
        bounding_box_max: [bbox.max.x, bbox.max.y, bbox.max.z],
        volume: total_vol,
        surface_area: total_area,
        scale_anomaly: anomaly_str,
        suggested_scale: scale_rep.suggested_scale,
        scale_warning: warning_str,
    })
}

/// Internal implementation of 3MF sanitization.
pub fn sanitize_3mf_internal(
    three_mf_bytes: &[u8],
    strip_config: bool,
    preserve_colors: bool,
) -> Result<Vec<u8>, String> {
    if three_mf_bytes.is_empty() {
        return Err("Input 3MF bytes buffer is empty".to_string());
    }

    let options = SanitizerOptions {
        strip_vendor_profiles: strip_config,
        preserve_colors,
        preserve_supports: true,
        preserve_thumbnails: true,
    };

    sanitize_3mf_archive(three_mf_bytes, &options)
        .map_err(|e| format!("Sanitization error: {e}"))
}

/// Converts raw STL bytes (binary or ASCII) into a valid 3MF OPC ZIP archive.
///
/// Accepts an optional JavaScript options object (`WasmPipelineOptions`).
/// Returns a `Uint8Array` containing the complete 3MF ZIP archive bytes.
#[wasm_bindgen]
pub fn convert_stl_to_3mf(stl_bytes: &[u8], options: JsValue) -> Result<Vec<u8>, JsValue> {
    if stl_bytes.is_empty() {
        return Err(JsValue::from_str("Input STL bytes buffer is empty"));
    }

    let opts: WasmPipelineOptions = if options.is_undefined() || options.is_null() {
        WasmPipelineOptions::default()
    } else {
        serde_wasm_bindgen::from_value(options)
            .map_err(|e| JsValue::from_str(&format!("Invalid pipeline options: {e}")))?
    };

    convert_stl_to_3mf_internal(stl_bytes, &opts).map_err(|e| JsValue::from_str(&e))
}

/// Analyzes raw STL bytes and returns geometric metrics and a scale sanity diagnosis.
///
/// Returns a serializable JavaScript object corresponding to `WasmMeshInspection`.
#[wasm_bindgen]
pub fn inspect_stl(stl_bytes: &[u8]) -> Result<JsValue, JsValue> {
    if stl_bytes.is_empty() {
        return Err(JsValue::from_str("Input STL bytes buffer is empty"));
    }

    let inspection = inspect_stl_internal(stl_bytes).map_err(|e| JsValue::from_str(&e))?;

    serde_wasm_bindgen::to_value(&inspection)
        .map_err(|e| JsValue::from_str(&format!("Serialization error: {e}")))
}

/// Sanitizes an existing 3MF package by stripping vendor slicer configurations and machine profiles.
///
/// Accepts raw 3MF archive bytes, `strip_config` flag, and `preserve_colors` flag.
/// Returns cleaned 3MF archive bytes as a `Uint8Array`.
#[wasm_bindgen]
pub fn sanitize_3mf(
    three_mf_bytes: &[u8],
    strip_config: bool,
    preserve_colors: bool,
) -> Result<Vec<u8>, JsValue> {
    if three_mf_bytes.is_empty() {
        return Err(JsValue::from_str("Input 3MF bytes buffer is empty"));
    }

    sanitize_3mf_internal(three_mf_bytes, strip_config, preserve_colors)
        .map_err(|e| JsValue::from_str(&e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use omni3mf_core::stl::write_binary_stl;
    use omni3mf_core::{Point3d, RawFacet, Vector3d};
    use zip::ZipArchive;

    fn make_test_cube_facets() -> Vec<RawFacet> {
        let p = |x: f64, y: f64, z: f64| Point3d::new(x * 10.0, y * 10.0, z * 10.0);
        let n = Vector3d::new;
        let facet = |normal: Vector3d, v0: Point3d, v1: Point3d, v2: Point3d| RawFacet {
            normal,
            v0,
            v1,
            v2,
            attribute_byte_count: 0,
        };

        vec![
            facet(n(0.0, 0.0, -1.0), p(0.0, 0.0, 0.0), p(1.0, 1.0, 0.0), p(1.0, 0.0, 0.0)),
            facet(n(0.0, 0.0, -1.0), p(0.0, 0.0, 0.0), p(0.0, 1.0, 0.0), p(1.0, 1.0, 0.0)),
            facet(n(0.0, 0.0, 1.0), p(0.0, 0.0, 1.0), p(1.0, 0.0, 1.0), p(1.0, 1.0, 1.0)),
            facet(n(0.0, 0.0, 1.0), p(0.0, 0.0, 1.0), p(1.0, 1.0, 1.0), p(0.0, 1.0, 1.0)),
            facet(n(0.0, -1.0, 0.0), p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(1.0, 0.0, 1.0)),
            facet(n(0.0, -1.0, 0.0), p(0.0, 0.0, 0.0), p(1.0, 0.0, 1.0), p(0.0, 0.0, 1.0)),
            facet(n(0.0, 1.0, 0.0), p(0.0, 1.0, 0.0), p(1.0, 1.0, 1.0), p(1.0, 1.0, 0.0)),
            facet(n(0.0, 1.0, 0.0), p(0.0, 1.0, 0.0), p(0.0, 1.0, 1.0), p(1.0, 1.0, 1.0)),
            facet(n(-1.0, 0.0, 0.0), p(0.0, 0.0, 0.0), p(0.0, 0.0, 1.0), p(0.0, 1.0, 1.0)),
            facet(n(-1.0, 0.0, 0.0), p(0.0, 0.0, 0.0), p(0.0, 1.0, 1.0), p(0.0, 1.0, 0.0)),
            facet(n(1.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0), p(1.0, 1.0, 1.0)),
            facet(n(1.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(1.0, 1.0, 1.0), p(1.0, 0.0, 1.0)),
        ]
    }

    #[test]
    fn test_wasm_convert_stl_to_3mf_logic() {
        let facets = make_test_cube_facets();
        let stl_bytes = write_binary_stl("WasmCube", &facets).expect("Binary write failed");

        let res = convert_stl_to_3mf_internal(&stl_bytes, &WasmPipelineOptions::default())
            .expect("Wasm convert failed");
        assert!(!res.is_empty());

        let mut archive = ZipArchive::new(std::io::Cursor::new(&res)).expect("Valid ZIP");
        assert!(archive.by_name("3D/3dmodel.model").is_ok());
        assert!(archive.by_name("Metadata/thumbnail.png").is_ok());
    }

    #[test]
    fn test_wasm_inspect_stl_logic() {
        let facets = make_test_cube_facets();
        let stl_bytes = write_binary_stl("WasmCube", &facets).expect("Binary write failed");

        let inspection = inspect_stl_internal(&stl_bytes).expect("Inspection failed");
        assert_eq!(inspection.vertex_count, 8);
        assert_eq!(inspection.face_count, 12);
        assert_eq!(inspection.shell_count, 1);
        assert!(inspection.is_watertight);
        assert_eq!(inspection.scale_anomaly, "Normal");
    }

    #[test]
    fn test_wasm_empty_input_errors() {
        let err = convert_stl_to_3mf_internal(&[], &WasmPipelineOptions::default()).unwrap_err();
        assert!(err.contains("empty"));

        let err2 = inspect_stl_internal(&[]).unwrap_err();
        assert!(err2.contains("empty"));

        let err3 = sanitize_3mf_internal(&[], true, true).unwrap_err();
        assert!(err3.contains("empty"));
    }
}
