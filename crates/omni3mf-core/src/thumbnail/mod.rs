//! Software thumbnail generation for 3MF packages.
//!
//! Generates 256x256 RGBA preview images with transparent background, orthographic projection,
//! Lambertian diffuse shading, and pure-Rust PNG encoding stored at `Metadata/thumbnail.png`.

pub mod rasterizer;

pub use rasterizer::{
    RasterizerOptions, ThumbnailRasterizer, DEFAULT_SHELL_COLORS,
};

use crate::mesh::IndexedMesh;
use crate::shell::MeshShell;

/// Generates a 256x256 pure-Rust PNG thumbnail from a slice of `MeshShell`s.
pub fn render_thumbnail(
    shells: &[MeshShell],
    options: Option<&RasterizerOptions>,
) -> Result<Vec<u8>, png::EncodingError> {
    let default_opts = RasterizerOptions::default();
    let opts = options.unwrap_or(&default_opts);
    let mut rasterizer = ThumbnailRasterizer::new(opts.width, opts.height);
    rasterizer.render_shells(shells, opts);
    rasterizer.encode_png()
}

/// Generates a 256x256 pure-Rust PNG thumbnail from an `IndexedMesh`.
pub fn render_thumbnail_mesh(
    mesh: &IndexedMesh,
    options: Option<&RasterizerOptions>,
) -> Result<Vec<u8>, png::EncodingError> {
    let default_opts = RasterizerOptions::default();
    let opts = options.unwrap_or(&default_opts);
    let mut rasterizer = ThumbnailRasterizer::new(opts.width, opts.height);
    let default_color = DEFAULT_SHELL_COLORS[0];
    rasterizer.render_mesh(mesh, default_color, opts);
    rasterizer.encode_png()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Point3d;

    #[test]
    fn test_thumbnail_png_generation_cube() {
        // Construct a simple unit cube mesh
        let vertices = vec![
            Point3d::new(0.0, 0.0, 0.0),
            Point3d::new(10.0, 0.0, 0.0),
            Point3d::new(10.0, 10.0, 0.0),
            Point3d::new(0.0, 10.0, 0.0),
            Point3d::new(0.0, 0.0, 10.0),
            Point3d::new(10.0, 0.0, 10.0),
            Point3d::new(10.0, 10.0, 10.0),
            Point3d::new(0.0, 10.0, 10.0),
        ];
        let faces = vec![
            [0, 2, 1], [0, 3, 2], // bottom
            [4, 5, 6], [4, 6, 7], // top
            [0, 1, 5], [0, 5, 4], // front
            [2, 3, 7], [2, 7, 6], // back
            [0, 4, 7], [0, 7, 3], // left
            [1, 2, 6], [1, 6, 5], // right
        ];
        let mesh = IndexedMesh::from_vertices_and_faces(vertices, faces);

        let opts = RasterizerOptions::default();
        let mut rasterizer = ThumbnailRasterizer::new(opts.width, opts.height);
        rasterizer.render_mesh(&mesh, [0, 163, 224], &opts);

        // Check buffer: corners are transparent (alpha 0)
        let cb = rasterizer.color_buffer();
        assert_eq!(cb[3], 0, "Top-left corner must be transparent");
        let tr_idx = (opts.width - 1) * 4;
        assert_eq!(cb[tr_idx + 3], 0, "Top-right corner must be transparent");

        // Center pixel (128, 128) must be opaque (alpha 255)
        let center_idx = (128 * opts.width + 128) * 4;
        assert_eq!(cb[center_idx + 3], 255, "Center pixel must be opaque");

        // Encode PNG
        let png_bytes = rasterizer.encode_png().expect("PNG encoding failed");
        assert!(png_bytes.len() > 8);
        // Standard PNG Magic: 0x89 'P' 'N' 'G' '\r' '\n' 0x1A '\n'
        assert_eq!(
            &png_bytes[0..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
            "Must start with PNG signature"
        );

        // Verify PNG dimensions using png::Decoder
        let cursor = std::io::Cursor::new(&png_bytes);
        let decoder = png::Decoder::new(cursor);
        let reader = decoder.read_info().expect("Failed reading PNG header");
        let info = reader.info();
        assert_eq!(info.width, 256);
        assert_eq!(info.height, 256);
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!(info.bit_depth, png::BitDepth::Eight);
    }
}
