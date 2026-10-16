//! High-level end-to-end conversion, repair, and 3MF packaging pipeline.

use thiserror::Error;

use crate::package::{build_3mf_package, PackagingOptions, ThreeMfError};
use crate::sanitizer::{sanitize_3mf_archive, SanitizerOptions};
use crate::scale::{check_scale_sanity, ScaleSanityReport};
use crate::shell::MeshShell;
use crate::stl::StlError;
use crate::thumbnail::{render_thumbnail, RasterizerOptions};

/// Unified error enum for the conversion and packaging pipeline.
#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("STL processing error: {0}")]
    Stl(#[from] StlError),

    #[error("3MF packaging error: {0}")]
    Packaging(#[from] ThreeMfError),
}

/// Pipeline configuration options for converting STL to 3MF.
#[derive(Debug, Clone)]
pub struct PipelineOptions {
    /// Spatial hash welding coordinate tolerance (default 1e-6 mm).
    pub tolerance: f64,
    /// Whether to patch open boundary holes.
    pub patch_holes: bool,
    /// Maximum edge loop count to patch via ear-clipping.
    pub max_hole_edges: usize,
    /// Whether to separate connected components into discrete shells.
    pub split_shells: bool,
    /// Manual scale factor override.
    pub scale_override: Option<f64>,
    /// Whether to render and embed a 256x256 PNG thumbnail at `Metadata/thumbnail.png`.
    pub include_thumbnail: bool,
    /// Software rasterizer settings for thumbnail preview.
    pub thumbnail_options: RasterizerOptions,
    /// Whether to sanitize the generated archive (strip vendor configurations).
    pub sanitize_output: bool,
    /// Slicer profile sanitizer settings.
    pub sanitizer_options: SanitizerOptions,
    /// Whether to perform dynamic scale sanity analysis.
    pub check_scale: bool,
}

impl Default for PipelineOptions {
    fn default() -> Self {
        Self {
            tolerance: 1e-6,
            patch_holes: true,
            max_hole_edges: 128,
            split_shells: true,
            scale_override: None,
            include_thumbnail: true,
            thumbnail_options: RasterizerOptions::default(),
            sanitize_output: false,
            sanitizer_options: SanitizerOptions::default(),
            check_scale: true,
        }
    }
}

/// Detailed result of the STL-to-3MF conversion pipeline.
#[derive(Debug, Clone)]
pub struct PipelineResult {
    /// Valid 3MF OPC ZIP archive byte buffer.
    pub package_bytes: Vec<u8>,
    /// Disconnected topological shells extracted from the input geometry.
    pub shells: Vec<MeshShell>,
    /// Scale sanity analysis report (if `check_scale` was enabled).
    pub scale_report: Option<ScaleSanityReport>,
    /// Raw bytes of the generated PNG thumbnail (if `include_thumbnail` was enabled).
    pub thumbnail_png: Option<Vec<u8>>,
}

/// Converts an STL byte buffer (binary or ASCII) into a valid 3MF package with healing,
/// shell separation, thumbnail rasterization, and optional profile sanitization.
pub fn convert_stl_to_3mf(
    stl_bytes: &[u8],
    options: &PipelineOptions,
) -> Result<PipelineResult, PipelineError> {
    // 1. Ingest, weld coincident vertices, prune degenerates, repair holes, unify normals
    let mut mesh = crate::ingest_and_repair_stl(
        stl_bytes,
        options.tolerance,
        options.patch_holes,
        options.max_hole_edges,
    )?;

    // Apply manual scale override if provided
    if let Some(scale) = options.scale_override {
        if scale > 0.0 && scale.is_finite() && (scale - 1.0).abs() > 1e-12 {
            for v in &mut mesh.vertices {
                v.x *= scale;
                v.y *= scale;
                v.z *= scale;
            }
        }
    }

    // Separate into shells or keep as a single model shell
    let shells = if options.split_shells {
        crate::separate_connected_shells(&mesh)
    } else if mesh.faces.is_empty() || mesh.vertices.is_empty() {
        Vec::new()
    } else {
        let bbox = mesh.bounding_box();
        let center = mesh.center_of_mass();
        let vol = mesh.signed_volume();
        let area = mesh.surface_area();
        let watertight = !mesh.faces.is_empty();
        vec![MeshShell {
            id: 1,
            name: "Model".to_string(),
            mesh,
            bbox,
            center_of_mass: center,
            signed_volume: vol,
            surface_area: area,
            is_watertight: watertight,
        }]
    };

    // 2. Dynamic scale sanity check
    let scale_report = if options.check_scale {
        Some(check_scale_sanity(&shells))
    } else {
        None
    };

    // 3. Render thumbnail preview if requested
    let thumbnail_png = if options.include_thumbnail && !shells.is_empty() {
        Some(
            render_thumbnail(&shells, Some(&options.thumbnail_options))
                .map_err(ThreeMfError::Png)?,
        )
    } else {
        None
    };

    // 4. Assemble 3MF OPC ZIP package
    let pkg_options = PackagingOptions {
        include_thumbnail: options.include_thumbnail,
        thumbnail_options: options.thumbnail_options.clone(),
        extra_files: Vec::new(),
    };

    let mut package_bytes = build_3mf_package(&shells, &pkg_options)?;

    // 5. Optional slicer profile sanitization
    if options.sanitize_output {
        package_bytes = sanitize_3mf_archive(&package_bytes, &options.sanitizer_options)?;
    }

    Ok(PipelineResult {
        package_bytes,
        shells,
        scale_report,
        thumbnail_png,
    })
}

/// Convenience function converting STL bytes directly to a 3MF ZIP byte buffer with default settings.
pub fn convert_stl_to_3mf_bytes(stl_bytes: &[u8]) -> Result<Vec<u8>, PipelineError> {
    let result = convert_stl_to_3mf(stl_bytes, &PipelineOptions::default())?;
    Ok(result.package_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stl::write_binary_stl;
    use zip::ZipArchive;

    #[test]
    fn test_pipeline_roundtrip_cube() {
        // Construct two cubes in facet stream
        let mut facets = Vec::new();
        // Cube 1
        facets.extend(create_cube_facets(0.0, 0.0, 0.0, 10.0));
        // Cube 2
        facets.extend(create_cube_facets(30.0, 30.0, 30.0, 10.0));

        let stl_bytes = write_binary_stl("TwoCubes", &facets).expect("Binary write failed");

        let options = PipelineOptions::default();
        let result = convert_stl_to_3mf(&stl_bytes, &options).expect("Pipeline conversion failed");

        // Assert 2 shells extracted
        assert_eq!(result.shells.len(), 2);
        assert_eq!(result.shells[0].id, 1);
        assert_eq!(result.shells[1].id, 2);

        // Assert thumbnail generated
        assert!(result.thumbnail_png.is_some());
        let thumb = result.thumbnail_png.unwrap();
        assert_eq!(&thumb[0..4], &[0x89, b'P', b'N', b'G']);

        // Assert scale report normal
        assert!(result.scale_report.is_some());
        let scale = result.scale_report.unwrap();
        assert_eq!(scale.anomaly, crate::scale::ScaleAnomaly::Normal);

        // Validate resulting 3MF ZIP archive
        let mut archive =
            ZipArchive::new(std::io::Cursor::new(&result.package_bytes)).expect("Invalid zip");
        assert!(archive.by_name("3D/3dmodel.model").is_ok());
        assert!(archive.by_name("Metadata/thumbnail.png").is_ok());
    }

    fn create_cube_facets(
        ox: f64,
        oy: f64,
        oz: f64,
        size: f64,
    ) -> Vec<crate::stl::RawFacet> {
        use crate::math::{Point3d, Vector3d};
        let p = |x: f64, y: f64, z: f64| Point3d::new(ox + x * size, oy + y * size, oz + z * size);
        let n = Vector3d::new;
        let facet = |normal: Vector3d, v0: Point3d, v1: Point3d, v2: Point3d| {
            crate::stl::RawFacet {
                normal,
                v0,
                v1,
                v2,
                attribute_byte_count: 0,
            }
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
}
