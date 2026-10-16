//! Multi-shell 3MF model XML generator using quick-xml.
//!
//! Produces compliant 3MF Core v1.x XML models with:
//! - Root `<model unit="millimeter" ...>`
//! - Metadata records
//! - `<resources>` with `<m:colorgroup>` and independent `<object>` nodes for each shell
//! - Local 0-indexed vertex and triangle buffers
//! - `<build>` with `<item objectid="..."/>` for each shell

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, Event};
use quick_xml::Writer;
use std::io::Cursor;

use super::zip_builder::ThreeMfError;
use crate::mesh::IndexedMesh;
use crate::shell::MeshShell;

/// Hexadecimal color definitions for default color group.
pub const COLOR_PALETTE_HEX: &[&str] = &[
    "#00A3E0FF", // Cyan
    "#FF7A00FF", // Orange
    "#2ECC71FF", // Green
    "#9B59B6FF", // Purple
    "#E74C3CFF", // Red
    "#F1C40FFF", // Yellow
    "#1ABC9CFF", // Turquoise
    "#E67E22FF", // Carrot
];

/// Serializes multiple `MeshShell`s into a valid 3MF `3D/3dmodel.model` XML byte buffer.
pub fn serialize_shells_to_model_xml(shells: &[MeshShell]) -> Result<Vec<u8>, ThreeMfError> {
    let mut buffer = Vec::with_capacity(
        shells
            .iter()
            .map(|s| s.mesh.vertices.len() * 45 + s.mesh.faces.len() * 35 + 512)
            .sum::<usize>()
            .max(1024),
    );

    let mut writer = Writer::new_with_indent(Cursor::new(&mut buffer), b' ', 2);

    // 1. XML Declaration
    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        .map_err(|e| ThreeMfError::Xml(e.to_string()))?;

    // 2. <model>
    let mut model_elem = BytesStart::new("model");
    model_elem.push_attribute(("unit", "millimeter"));
    model_elem.push_attribute(("xml:lang", "en-US"));
    model_elem.push_attribute((
        "xmlns",
        "http://schemas.microsoft.com/3dmanufacturing/core/2015/02",
    ));
    model_elem.push_attribute((
        "xmlns:m",
        "http://schemas.microsoft.com/3dmanufacturing/material/2015/02",
    ));
    writer
        .write_event(Event::Start(model_elem))
        .map_err(|e| ThreeMfError::Xml(e.to_string()))?;

    // 3. <metadata name="Application">StlTo3mf / Omni3MF Engine</metadata>
    let mut meta_app = BytesStart::new("metadata");
    meta_app.push_attribute(("name", "Application"));
    writer
        .write_event(Event::Start(meta_app))
        .map_err(|e| ThreeMfError::Xml(e.to_string()))?;
    writer
        .write_event(Event::Text(quick_xml::events::BytesText::new(
            "StlTo3mf / Omni3MF Engine",
        )))
        .map_err(|e| ThreeMfError::Xml(e.to_string()))?;
    writer
        .write_event(Event::End(BytesEnd::new("metadata")))
        .map_err(|e| ThreeMfError::Xml(e.to_string()))?;

    // 4. <metadata name="Title">StlTo3mf Multi-Shell Model</metadata>
    let mut meta_title = BytesStart::new("metadata");
    meta_title.push_attribute(("name", "Title"));
    writer
        .write_event(Event::Start(meta_title))
        .map_err(|e| ThreeMfError::Xml(e.to_string()))?;
    writer
        .write_event(Event::Text(quick_xml::events::BytesText::new(
            "StlTo3mf Multi-Shell Model",
        )))
        .map_err(|e| ThreeMfError::Xml(e.to_string()))?;
    writer
        .write_event(Event::End(BytesEnd::new("metadata")))
        .map_err(|e| ThreeMfError::Xml(e.to_string()))?;

    // 5. <resources>
    writer
        .write_event(Event::Start(BytesStart::new("resources")))
        .map_err(|e| ThreeMfError::Xml(e.to_string()))?;

    // Color group palette
    let mut colorgroup = BytesStart::new("m:colorgroup");
    colorgroup.push_attribute(("id", "1"));
    writer
        .write_event(Event::Start(colorgroup))
        .map_err(|e| ThreeMfError::Xml(e.to_string()))?;

    for &hex in COLOR_PALETTE_HEX {
        let mut color_elem = BytesStart::new("m:color");
        color_elem.push_attribute(("color", hex));
        writer
            .write_event(Event::Empty(color_elem))
            .map_err(|e| ThreeMfError::Xml(e.to_string()))?;
    }

    writer
        .write_event(Event::End(BytesEnd::new("m:colorgroup")))
        .map_err(|e| ThreeMfError::Xml(e.to_string()))?;

    // Objects (one per shell)
    for (i, shell) in shells.iter().enumerate() {
        let object_id = shell.id + 1;
        let object_id_str = object_id.to_string();
        let pindex_str = (i % COLOR_PALETTE_HEX.len()).to_string();

        let mut obj = BytesStart::new("object");
        obj.push_attribute(("id", object_id_str.as_str()));
        obj.push_attribute(("type", "model"));
        obj.push_attribute(("name", shell.name.as_str()));
        obj.push_attribute(("pid", "1"));
        obj.push_attribute(("pindex", pindex_str.as_str()));

        writer
            .write_event(Event::Start(obj))
            .map_err(|e| ThreeMfError::Xml(e.to_string()))?;

        // <mesh>
        writer
            .write_event(Event::Start(BytesStart::new("mesh")))
            .map_err(|e| ThreeMfError::Xml(e.to_string()))?;

        // <vertices>
        writer
            .write_event(Event::Start(BytesStart::new("vertices")))
            .map_err(|e| ThreeMfError::Xml(e.to_string()))?;

        for v in &shell.mesh.vertices {
            let mut vert = BytesStart::new("vertex");
            let x_str = format!("{:.6}", v.x);
            let y_str = format!("{:.6}", v.y);
            let z_str = format!("{:.6}", v.z);
            vert.push_attribute(("x", x_str.as_str()));
            vert.push_attribute(("y", y_str.as_str()));
            vert.push_attribute(("z", z_str.as_str()));
            writer
                .write_event(Event::Empty(vert))
                .map_err(|e| ThreeMfError::Xml(e.to_string()))?;
        }

        writer
            .write_event(Event::End(BytesEnd::new("vertices")))
            .map_err(|e| ThreeMfError::Xml(e.to_string()))?;

        // <triangles>
        writer
            .write_event(Event::Start(BytesStart::new("triangles")))
            .map_err(|e| ThreeMfError::Xml(e.to_string()))?;

        for tri in &shell.mesh.faces {
            let mut t = BytesStart::new("triangle");
            let v1_str = tri[0].to_string();
            let v2_str = tri[1].to_string();
            let v3_str = tri[2].to_string();
            t.push_attribute(("v1", v1_str.as_str()));
            t.push_attribute(("v2", v2_str.as_str()));
            t.push_attribute(("v3", v3_str.as_str()));
            writer
                .write_event(Event::Empty(t))
                .map_err(|e| ThreeMfError::Xml(e.to_string()))?;
        }

        writer
            .write_event(Event::End(BytesEnd::new("triangles")))
            .map_err(|e| ThreeMfError::Xml(e.to_string()))?;

        // </mesh>
        writer
            .write_event(Event::End(BytesEnd::new("mesh")))
            .map_err(|e| ThreeMfError::Xml(e.to_string()))?;

        // </object>
        writer
            .write_event(Event::End(BytesEnd::new("object")))
            .map_err(|e| ThreeMfError::Xml(e.to_string()))?;
    }

    // </resources>
    writer
        .write_event(Event::End(BytesEnd::new("resources")))
        .map_err(|e| ThreeMfError::Xml(e.to_string()))?;

    // 6. <build>
    writer
        .write_event(Event::Start(BytesStart::new("build")))
        .map_err(|e| ThreeMfError::Xml(e.to_string()))?;

    for shell in shells {
        let object_id = shell.id + 1;
        let object_id_str = object_id.to_string();
        let mut item = BytesStart::new("item");
        item.push_attribute(("objectid", object_id_str.as_str()));
        writer
            .write_event(Event::Empty(item))
            .map_err(|e| ThreeMfError::Xml(e.to_string()))?;
    }

    // </build>
    writer
        .write_event(Event::End(BytesEnd::new("build")))
        .map_err(|e| ThreeMfError::Xml(e.to_string()))?;

    // </model>
    writer
        .write_event(Event::End(BytesEnd::new("model")))
        .map_err(|e| ThreeMfError::Xml(e.to_string()))?;

    drop(writer);
    Ok(buffer)
}

/// Serializes a single `IndexedMesh` into a 3MF `3D/3dmodel.model` XML byte buffer.
pub fn serialize_mesh_to_model_xml(mesh: &IndexedMesh) -> Result<Vec<u8>, ThreeMfError> {
    let bbox = mesh.bounding_box();
    let center = mesh.center_of_mass();
    let vol = mesh.signed_volume();
    let area = mesh.surface_area();
    let watertight = !mesh.faces.is_empty();

    let shell = MeshShell {
        id: 1,
        name: "Shell_1".to_string(),
        mesh: mesh.clone(),
        bbox,
        center_of_mass: center,
        signed_volume: vol,
        surface_area: area,
        is_watertight: watertight,
    };

    serialize_shells_to_model_xml(&[shell])
}
