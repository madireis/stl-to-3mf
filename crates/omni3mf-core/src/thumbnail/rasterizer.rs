//! Pure-Rust software rasterizer for generating 3MF thumbnail previews.
//!
//! Features:
//! - 256x256 RGBA image buffer with transparent background.
//! - LookAt camera at 45° azimuth, 30° elevation.
//! - Orthographic projection with 15% margin padding.
//! - Key + fill + ambient directional lighting with Lambertian diffuse shading.
//! - Backface culling and barycentric depth-buffered triangle rasterization.
//! - Pure-Rust PNG encoding via the `png` crate without C/C++ dependencies.

use crate::math::{BoundingBox3d, Point3d, Vector3d};
use crate::mesh::IndexedMesh;
use crate::shell::MeshShell;

/// Default shell color palette (RGBA) matching 3MF PRD color visualization.
pub const DEFAULT_SHELL_COLORS: &[[u8; 3]] = &[
    [0, 163, 224],   // Cyan (#00A3E0)
    [255, 122, 0],   // Orange (#FF7A00)
    [46, 204, 113],  // Green (#2ECC71)
    [155, 89, 182],  // Purple (#9B59B6)
    [231, 76, 60],   // Red (#E74C3C)
    [241, 196, 15],  // Yellow (#F1C40F)
    [26, 188, 156],  // Turquoise (#1ABC9C)
    [230, 126, 34],  // Carrot (#E67E22)
];

/// Software rasterizer configuration options.
#[derive(Debug, Clone)]
pub struct RasterizerOptions {
    /// Image width in pixels (standard 256).
    pub width: usize,
    /// Image height in pixels (standard 256).
    pub height: usize,
    /// Margin padding percentage (e.g. 0.15 for 15% margin).
    pub margin: f64,
    /// Whether to enable backface culling.
    pub backface_culling: bool,
    /// Threshold to trigger LOD triangle subsampling.
    pub lod_triangle_threshold: usize,
}

impl Default for RasterizerOptions {
    fn default() -> Self {
        Self {
            width: 256,
            height: 256,
            margin: 0.15,
            backface_culling: true,
            lod_triangle_threshold: 50_000,
        }
    }
}

/// Headless software rasterizer rendering meshes to an RGBA pixel buffer.
pub struct ThumbnailRasterizer {
    width: usize,
    height: usize,
    z_buffer: Vec<f64>,
    color_buffer: Vec<u8>,
}

impl ThumbnailRasterizer {
    /// Creates a new rasterizer with specified dimensions initialized to transparent RGBA (0,0,0,0).
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            z_buffer: vec![f64::NEG_INFINITY; width * height],
            color_buffer: vec![0u8; width * height * 4],
        }
    }

    /// Returns the raw RGBA color buffer.
    pub fn color_buffer(&self) -> &[u8] {
        &self.color_buffer
    }

    /// Renders an `IndexedMesh` with a specific RGB color using the provided options.
    pub fn render_mesh(
        &mut self,
        mesh: &IndexedMesh,
        color: [u8; 3],
        options: &RasterizerOptions,
    ) {
        if mesh.vertices.is_empty() || mesh.faces.is_empty() {
            return;
        }

        let bbox = mesh.bounding_box();
        self.render_faces_with_bounds(
            &mesh.vertices,
            &mesh.faces,
            &bbox,
            color,
            options,
            mesh.faces.len(),
        );
    }

    /// Renders multiple `MeshShell`s with distinct colors from the default palette.
    pub fn render_shells(&mut self, shells: &[MeshShell], options: &RasterizerOptions) {
        if shells.is_empty() {
            return;
        }

        let mut total_bbox = BoundingBox3d::empty();
        let mut total_triangles = 0;
        for shell in shells {
            total_bbox = total_bbox.union(&shell.bbox);
            total_triangles += shell.mesh.faces.len();
        }

        if total_bbox.is_empty() {
            return;
        }

        for (i, shell) in shells.iter().enumerate() {
            let color = DEFAULT_SHELL_COLORS[i % DEFAULT_SHELL_COLORS.len()];
            self.render_faces_with_bounds(
                &shell.mesh.vertices,
                &shell.mesh.faces,
                &total_bbox,
                color,
                options,
                total_triangles,
            );
        }
    }

    /// Renders faces using shared bounding box and camera transformation.
    fn render_faces_with_bounds(
        &mut self,
        vertices: &[Point3d],
        faces: &[[u32; 3]],
        total_bbox: &BoundingBox3d,
        color: [u8; 3],
        options: &RasterizerOptions,
        total_scene_triangles: usize,
    ) {
        if self.width == 0 || self.height == 0 {
            return;
        }

        let center = total_bbox.center();

        // Camera basis vectors:
        // Azimuth = 45°, Elevation = 30°
        // Looking towards origin/center from front-right-above:
        // Forward vector (from camera towards scene center):
        let f = Vector3d::new(-0.6123724356957945, 0.6123724356957945, -0.5000000000000000)
            .normalize()
            .unwrap_or(Vector3d::new(-0.577, 0.577, -0.577));
        let up_ref = Vector3d::new(0.0, 0.0, 1.0);
        let r = f.cross(up_ref).normalize().unwrap_or(Vector3d::new(1.0, 0.0, 0.0));
        let u = r.cross(f).normalize().unwrap_or(Vector3d::new(0.0, 0.0, 1.0));

        // Compute optimal scale factor to ensure 15% margin padding
        // Project all 8 corners of the bounding box to camera coordinates
        let corners = [
            Point3d::new(total_bbox.min.x, total_bbox.min.y, total_bbox.min.z),
            Point3d::new(total_bbox.max.x, total_bbox.min.y, total_bbox.min.z),
            Point3d::new(total_bbox.min.x, total_bbox.max.y, total_bbox.min.z),
            Point3d::new(total_bbox.max.x, total_bbox.max.y, total_bbox.min.z),
            Point3d::new(total_bbox.min.x, total_bbox.min.y, total_bbox.max.z),
            Point3d::new(total_bbox.max.x, total_bbox.min.y, total_bbox.max.z),
            Point3d::new(total_bbox.min.x, total_bbox.max.y, total_bbox.max.z),
            Point3d::new(total_bbox.max.x, total_bbox.max.y, total_bbox.max.z),
        ];

        let mut min_xc = f64::INFINITY;
        let mut max_xc = f64::NEG_INFINITY;
        let mut min_yc = f64::INFINITY;
        let mut max_yc = f64::NEG_INFINITY;

        for c in &corners {
            let d = *c - center;
            let xc = d.dot(r);
            let yc = d.dot(u);
            if xc < min_xc { min_xc = xc; }
            if xc > max_xc { max_xc = xc; }
            if yc < min_yc { min_yc = yc; }
            if yc > max_yc { max_yc = yc; }
        }

        let span_x = (max_xc - min_xc).abs();
        let span_y = (max_yc - min_yc).abs();
        let max_span = span_x.max(span_y);

        let scale = if max_span > 1e-6 {
            let available_pixels = (self.width.min(self.height) as f64) * (1.0 - options.margin);
            available_pixels / max_span
        } else {
            1.0
        };

        let half_w = (self.width as f64) * 0.5;
        let half_h = (self.height as f64) * 0.5;

        // Directional Key Light & Fill Light
        let key_light = Vector3d::new(0.5, -0.6, 0.8)
            .normalize()
            .unwrap_or(Vector3d::new(0.0, 0.0, 1.0));
        let fill_light = Vector3d::new(-0.5, 0.4, 0.3)
            .normalize()
            .unwrap_or(Vector3d::new(0.0, 0.0, 1.0));

        // LOD subsampling step
        let stride = if total_scene_triangles > options.lod_triangle_threshold {
            (total_scene_triangles / options.lod_triangle_threshold).max(1)
        } else {
            1
        };

        for face in faces.iter().step_by(stride) {
            let i0 = face[0] as usize;
            let i1 = face[1] as usize;
            let i2 = face[2] as usize;

            if i0 >= vertices.len() || i1 >= vertices.len() || i2 >= vertices.len() {
                continue;
            }

            let p0 = vertices[i0];
            let p1 = vertices[i1];
            let p2 = vertices[i2];

            let edge1 = p1 - p0;
            let edge2 = p2 - p0;
            let normal_raw = edge1.cross(edge2);
            let normal_len = normal_raw.norm();
            if normal_len < 1e-12 {
                continue; // degenerate facet
            }
            let normal = normal_raw / normal_len;

            // Backface culling:
            // f is vector from eye towards scene.
            // A front-facing polygon has normal pointing opposite to f (normal.dot(f) < 0).
            if options.backface_culling && normal.dot(f) >= 0.0 {
                continue;
            }

            // Project vertices to camera & screen space
            let d0 = p0 - center;
            let d1 = p1 - center;
            let d2 = p2 - center;

            let u0 = half_w + d0.dot(r) * scale;
            let v0 = half_h - d0.dot(u) * scale;
            let z0 = -d0.dot(f);

            let u1 = half_w + d1.dot(r) * scale;
            let v1 = half_h - d1.dot(u) * scale;
            let z1 = -d1.dot(f);

            let u2 = half_w + d2.dot(r) * scale;
            let v2 = half_h - d2.dot(u) * scale;
            let z2 = -d2.dot(f);

            // Shading: Lambertian diffuse with key, fill, and ambient
            let n_eff = if normal.dot(f) > 0.0 { -normal } else { normal };
            let d_key = n_eff.dot(key_light).max(0.0);
            let d_fill = n_eff.dot(fill_light).max(0.0);
            let intensity = (0.25 + 0.60 * d_key + 0.15 * d_fill).clamp(0.0, 1.0);

            let r_col = (color[0] as f64 * intensity).round().clamp(0.0, 255.0) as u8;
            let g_col = (color[1] as f64 * intensity).round().clamp(0.0, 255.0) as u8;
            let b_col = (color[2] as f64 * intensity).round().clamp(0.0, 255.0) as u8;

            // 2D Screen Bounding Box
            let min_x_f = u0.min(u1).min(u2).floor();
            let max_x_f = u0.max(u1).max(u2).ceil();
            let min_y_f = v0.min(v1).min(v2).floor();
            let max_y_f = v0.max(v1).max(v2).ceil();

            let min_x = (min_x_f as i32).clamp(0, self.width.saturating_sub(1) as i32) as usize;
            let max_x = (max_x_f as i32).clamp(0, self.width.saturating_sub(1) as i32) as usize;
            let min_y = (min_y_f as i32).clamp(0, self.height.saturating_sub(1) as i32) as usize;
            let max_y = (max_y_f as i32).clamp(0, self.height.saturating_sub(1) as i32) as usize;

            // Rasterize pixels via barycentric edge functions
            let denom = (u1 - u0) * (v2 - v0) - (u2 - u0) * (v1 - v0);
            if denom.abs() < 1e-12 {
                continue;
            }
            let inv_denom = 1.0 / denom;

            for y in min_y..=max_y {
                let py = y as f64 + 0.5;
                for x in min_x..=max_x {
                    let px = x as f64 + 0.5;

                    // Barycentric coordinates (w0, w1, w2)
                    let w1 = ((u2 - u0) * (py - v0) - (v2 - v0) * (px - u0)) * -inv_denom;
                    let w2 = ((u1 - u0) * (py - v0) - (v1 - v0) * (px - u0)) * inv_denom;
                    let w0 = 1.0 - w1 - w2;

                    // Sub-pixel inclusion test with small epsilon for edge coverage
                    if w0 >= -1e-4 && w1 >= -1e-4 && w2 >= -1e-4 {
                        let z = w0 * z0 + w1 * z1 + w2 * z2;
                        let pixel_idx = y * self.width + x;

                        if z > self.z_buffer[pixel_idx] {
                            self.z_buffer[pixel_idx] = z;
                            let c_idx = pixel_idx * 4;
                            self.color_buffer[c_idx] = r_col;
                            self.color_buffer[c_idx + 1] = g_col;
                            self.color_buffer[c_idx + 2] = b_col;
                            self.color_buffer[c_idx + 3] = 255;
                        }
                    }
                }
            }
        }
    }

    /// Encodes the current color buffer into a pure-Rust PNG byte stream.
    pub fn encode_png(&self) -> Result<Vec<u8>, png::EncodingError> {
        let mut png_bytes = Vec::with_capacity(32 * 1024);
        let mut encoder = png::Encoder::new(&mut png_bytes, self.width as u32, self.height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);

        let mut writer = encoder.write_header()?;
        writer.write_image_data(&self.color_buffer)?;
        drop(writer);

        Ok(png_bytes)
    }
}
