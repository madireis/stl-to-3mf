//! 3MF Open Packaging Conventions (OPC) package generator and XML model serializer.

pub mod model_xml;
pub mod zip_builder;

pub use model_xml::{
    serialize_mesh_to_model_xml, serialize_shells_to_model_xml, COLOR_PALETTE_HEX,
};
pub use zip_builder::{
    ThreeMfError, ThreeMfPackage, CONTENT_TYPES_XML, MODEL_RELS_XML, ROOT_RELS_NO_THUMB_XML,
    ROOT_RELS_WITH_THUMB_XML,
};

use crate::mesh::IndexedMesh;
use crate::shell::MeshShell;
use crate::thumbnail::{render_thumbnail, render_thumbnail_mesh, RasterizerOptions};

/// Packaging configuration options for 3MF archive generation.
#[derive(Debug, Clone)]
pub struct PackagingOptions {
    /// Whether to generate and embed a 256x256 PNG thumbnail at `Metadata/thumbnail.png`.
    pub include_thumbnail: bool,
    /// Software rasterizer configuration options.
    pub thumbnail_options: RasterizerOptions,
    /// Extra arbitrary files to embed in the OPC ZIP container.
    pub extra_files: Vec<(String, Vec<u8>)>,
}

impl Default for PackagingOptions {
    fn default() -> Self {
        Self {
            include_thumbnail: true,
            thumbnail_options: RasterizerOptions::default(),
            extra_files: Vec::new(),
        }
    }
}

/// Builds a complete, compliant 3MF OPC ZIP archive from disconnected `MeshShell`s.
pub fn build_3mf_package(
    shells: &[MeshShell],
    options: &PackagingOptions,
) -> Result<Vec<u8>, ThreeMfError> {
    let model_xml = serialize_shells_to_model_xml(shells)?;

    let thumbnail_png = if options.include_thumbnail && !shells.is_empty() {
        Some(render_thumbnail(shells, Some(&options.thumbnail_options))?)
    } else {
        None
    };

    let mut package = ThreeMfPackage::new(model_xml, thumbnail_png);
    for (path, data) in &options.extra_files {
        package.add_extra_file(path.clone(), data.clone());
    }

    package.to_bytes()
}

/// Builds a 3MF OPC ZIP archive from a single `IndexedMesh`.
pub fn build_3mf_package_from_mesh(
    mesh: &IndexedMesh,
    options: &PackagingOptions,
) -> Result<Vec<u8>, ThreeMfError> {
    let model_xml = serialize_mesh_to_model_xml(mesh)?;

    let thumbnail_png = if options.include_thumbnail && !mesh.faces.is_empty() {
        Some(render_thumbnail_mesh(mesh, Some(&options.thumbnail_options))?)
    } else {
        None
    };

    let mut package = ThreeMfPackage::new(model_xml, thumbnail_png);
    for (path, data) in &options.extra_files {
        package.add_extra_file(path.clone(), data.clone());
    }

    package.to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Point3d;
    use std::io::Read;
    use zip::ZipArchive;

    #[test]
    fn test_package_multi_shell_zip_structure() {
        // Create 2 disjoint shells
        let shell1 = MeshShell {
            id: 1,
            name: "Shell_1".to_string(),
            mesh: IndexedMesh::from_vertices_and_faces(
                vec![
                    Point3d::new(0.0, 0.0, 0.0),
                    Point3d::new(10.0, 0.0, 0.0),
                    Point3d::new(0.0, 10.0, 0.0),
                ],
                vec![[0, 1, 2]],
            ),
            bbox: crate::math::BoundingBox3d::new(
                Point3d::new(0.0, 0.0, 0.0),
                Point3d::new(10.0, 10.0, 0.0),
            ),
            center_of_mass: Point3d::new(3.33, 3.33, 0.0),
            signed_volume: 0.0,
            surface_area: 50.0,
            is_watertight: false,
        };

        let shell2 = MeshShell {
            id: 2,
            name: "Shell_2".to_string(),
            mesh: IndexedMesh::from_vertices_and_faces(
                vec![
                    Point3d::new(20.0, 20.0, 0.0),
                    Point3d::new(30.0, 20.0, 0.0),
                    Point3d::new(20.0, 30.0, 0.0),
                ],
                vec![[0, 1, 2]],
            ),
            bbox: crate::math::BoundingBox3d::new(
                Point3d::new(20.0, 20.0, 0.0),
                Point3d::new(30.0, 30.0, 0.0),
            ),
            center_of_mass: Point3d::new(23.33, 23.33, 0.0),
            signed_volume: 0.0,
            surface_area: 50.0,
            is_watertight: false,
        };

        let shells = vec![shell1, shell2];
        let options = PackagingOptions::default();

        let zip_bytes = build_3mf_package(&shells, &options).expect("Packaging failed");
        assert!(zip_bytes.len() > 100);

        // Verify ZIP contents
        let mut archive = ZipArchive::new(std::io::Cursor::new(zip_bytes)).expect("Invalid zip");

        let expected_files = [
            "[Content_Types].xml",
            "_rels/.rels",
            "3D/3dmodel.model",
            "Metadata/thumbnail.png",
            "3D/_rels/3dmodel.model.rels",
        ];

        for &fname in &expected_files {
            assert!(
                archive.by_name(fname).is_ok(),
                "Missing expected archive entry: {fname}"
            );
        }

        // Validate 3D/3dmodel.model XML content
        let mut model_xml = String::new();
        {
            let mut model_file = archive.by_name("3D/3dmodel.model").unwrap();
            model_file.read_to_string(&mut model_xml).unwrap();
        }

        // Check namespace, unit, objects, items
        assert!(model_xml.contains(r#"unit="millimeter""#));
        assert!(model_xml.contains(r#"xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02""#));
        assert!(model_xml.contains(r#"<m:colorgroup id="1">"#));
        assert!(model_xml.contains(r#"<object id="2" type="model" name="Shell_1""#));
        assert!(model_xml.contains(r#"<object id="3" type="model" name="Shell_2""#));
        assert!(model_xml.contains(r#"<item objectid="2"/>"#));
        assert!(model_xml.contains(r#"<item objectid="3"/>"#));

        // Validate thumbnail PNG
        let mut thumb_bytes = Vec::new();
        {
            let mut thumb_file = archive.by_name("Metadata/thumbnail.png").unwrap();
            thumb_file.read_to_end(&mut thumb_bytes).unwrap();
        }
        assert_eq!(&thumb_bytes[0..4], &[0x89, b'P', b'N', b'G']);
    }
}
