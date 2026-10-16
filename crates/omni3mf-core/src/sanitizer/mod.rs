//! Slicer profile sanitizer and metadata scrubber.
//!
//! Strips vendor-specific slicer profiles, printer overrides, and process configurations
//! while strictly preserving color records (`<m:colorgroup>`, `pid`, `p1`), visual appearance,
//! and custom support geometry (`<object type="support">`).

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, Writer};
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read};
use zip::ZipArchive;

use crate::package::zip_builder::{ThreeMfError, ThreeMfPackage};

/// Configuration options for the slicer profile sanitizer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SanitizerOptions {
    /// Whether to strip vendor profile configs, ini files, gcode, and vendor metadata.
    pub strip_vendor_profiles: bool,
    /// Whether to strictly preserve visual color records (`<m:colorgroup>`, `pid`, `p1`).
    pub preserve_colors: bool,
    /// Whether to strictly preserve custom support geometry (`<object type="support">`).
    pub preserve_supports: bool,
    /// Whether to preserve embedded preview thumbnails (`Metadata/thumbnail.png`).
    pub preserve_thumbnails: bool,
}

impl Default for SanitizerOptions {
    fn default() -> Self {
        Self {
            strip_vendor_profiles: true,
            preserve_colors: true,
            preserve_supports: true,
            preserve_thumbnails: true,
        }
    }
}

/// Returns `true` if the given archive file path corresponds to a vendor configuration or slicer preset.
pub fn is_blocked_slicer_file(path: &str) -> bool {
    let normalized = path.to_ascii_lowercase().replace('\\', "/");
    let file_name = normalized.rsplit('/').next().unwrap_or(&normalized);

    // Vendor metadata configurations (Bambu, Prusa, Orca, Creality, etc.)
    normalized.starts_with("metadata/project_settings")
        || normalized.starts_with("metadata/slice_info")
        || normalized.starts_with("metadata/model_settings")
        || normalized.starts_with("metadata/slic3r_pe")
        || normalized.starts_with("metadata/prusaslicer")
        || normalized.starts_with("metadata/plate_")
        || normalized.starts_with("metadata/bambu")
        || normalized.starts_with("metadata/orcaslicer")
        || normalized.starts_with("cura/")
        || normalized.contains("/cura/")
        || normalized.starts_with("metadata/cura")
        || file_name.ends_with(".config")
        || file_name.ends_with(".cfg")
        || file_name.ends_with(".ini")
        || file_name.ends_with(".gcode")
        || file_name.ends_with(".gcode.3mf")
}

/// Sanitizes an entire 3MF OPC ZIP archive byte buffer according to `options`.
pub fn sanitize_3mf_archive(
    input_bytes: &[u8],
    options: &SanitizerOptions,
) -> Result<Vec<u8>, ThreeMfError> {
    let mut reader = ZipArchive::new(Cursor::new(input_bytes))?;

    let mut model_xml_data: Option<Vec<u8>> = None;
    let mut thumbnail_bytes: Option<Vec<u8>> = None;
    let mut extra_files: Vec<(String, Vec<u8>)> = Vec::new();

    for i in 0..reader.len() {
        let mut file = reader.by_index(i)?;
        let name = file.name().to_string();

        if name.eq_ignore_ascii_case("3D/3dmodel.model") {
            let mut content = Vec::new();
            file.read_to_end(&mut content)?;
            model_xml_data = Some(sanitize_model_xml(&content, options)?);
        } else if name.eq_ignore_ascii_case("Metadata/thumbnail.png") {
            if options.preserve_thumbnails {
                let mut content = Vec::new();
                file.read_to_end(&mut content)?;
                thumbnail_bytes = Some(content);
            }
        } else if name.eq_ignore_ascii_case("[Content_Types].xml")
            || name.eq_ignore_ascii_case("_rels/.rels")
            || name.eq_ignore_ascii_case("3D/_rels/3dmodel.model.rels")
        {
            // Manifests will be regenerated cleanly
            continue;
        } else if options.strip_vendor_profiles && is_blocked_slicer_file(&name) {
            // Strip vendor preset file
            continue;
        } else {
            // Preserve other non-blocked files
            let mut content = Vec::new();
            file.read_to_end(&mut content)?;
            extra_files.push((name, content));
        }
    }

    let model_xml = model_xml_data
        .ok_or_else(|| ThreeMfError::Sanitizer("Missing 3D/3dmodel.model in archive".to_string()))?;

    let mut package = ThreeMfPackage::new(model_xml, thumbnail_bytes);
    for (path, data) in extra_files {
        package.add_extra_file(path, data);
    }

    package.to_bytes()
}

/// Checks if an XML attribute name represents a vendor slicer profile metadata element.
fn is_vendor_metadata(attr_val: &str) -> bool {
    let lower = attr_val.to_ascii_lowercase();
    lower.starts_with("slic3rpe:")
        || lower.starts_with("slic3r:")
        || lower.starts_with("bambu:")
        || lower.starts_with("bambustudio:")
        || lower.starts_with("orcaslicer:")
        || lower.starts_with("prusaslicer:")
        || lower.starts_with("cura:")
}

/// Sanitizes a 3MF `3D/3dmodel.model` XML byte slice by stripping vendor configs and tags.
pub fn sanitize_model_xml(
    xml_bytes: &[u8],
    options: &SanitizerOptions,
) -> Result<Vec<u8>, ThreeMfError> {
    let mut reader = Reader::from_reader(Cursor::new(xml_bytes));
    reader.config_mut().trim_text(false);

    let mut writer = Writer::new(Vec::with_capacity(xml_bytes.len()));
    let mut buf = Vec::new();
    let mut skip_depth = 0usize;
    let mut stripped_object_ids = std::collections::HashSet::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(ref e)) => {
                if skip_depth > 0 {
                    skip_depth += 1;
                    continue;
                }

                let tag_name = e.name().as_ref().to_ascii_lowercase();
                if options.strip_vendor_profiles && tag_name == b"config" {
                    skip_depth = 1;
                    continue;
                }

                if options.strip_vendor_profiles && tag_name == b"metadata" {
                    if let Some(attr_name) = get_attr_value(e, b"name") {
                        if is_vendor_metadata(&attr_name) {
                            skip_depth = 1;
                            continue;
                        }
                    }
                }

                if !options.preserve_supports && tag_name == b"object" {
                    if let Some(type_val) = get_attr_value(e, b"type") {
                        if type_val.eq_ignore_ascii_case("support") {
                            if let Some(id_val) = get_attr_value(e, b"id") {
                                stripped_object_ids.insert(id_val);
                            }
                            skip_depth = 1;
                            continue;
                        }
                    }
                }

                if tag_name == b"item" {
                    if let Some(obj_id) = get_attr_value(e, b"objectid") {
                        if stripped_object_ids.contains(&obj_id) {
                            skip_depth = 1;
                            continue;
                        }
                    }
                }

                if !options.preserve_colors && tag_name == b"m:colorgroup" {
                    skip_depth = 1;
                    continue;
                }

                writer
                    .write_event(Event::Start(e.clone()))
                    .map_err(|e| ThreeMfError::Xml(e.to_string()))?;
            }
            Ok(Event::End(ref e)) => {
                if skip_depth > 0 {
                    skip_depth -= 1;
                    continue;
                }
                writer
                    .write_event(Event::End(e.clone()))
                    .map_err(|e| ThreeMfError::Xml(e.to_string()))?;
            }
            Ok(Event::Empty(ref e)) => {
                if skip_depth > 0 {
                    continue;
                }

                let tag_name = e.name().as_ref().to_ascii_lowercase();
                if options.strip_vendor_profiles && tag_name == b"config" {
                    continue;
                }

                if options.strip_vendor_profiles && tag_name == b"metadata" {
                    if let Some(attr_name) = get_attr_value(e, b"name") {
                        if is_vendor_metadata(&attr_name) {
                            continue;
                        }
                    }
                }

                if !options.preserve_supports && tag_name == b"object" {
                    if let Some(type_val) = get_attr_value(e, b"type") {
                        if type_val.eq_ignore_ascii_case("support") {
                            if let Some(id_val) = get_attr_value(e, b"id") {
                                stripped_object_ids.insert(id_val);
                            }
                            continue;
                        }
                    }
                }

                if tag_name == b"item" {
                    if let Some(obj_id) = get_attr_value(e, b"objectid") {
                        if stripped_object_ids.contains(&obj_id) {
                            continue;
                        }
                    }
                }

                if !options.preserve_colors && tag_name == b"m:color" {
                    continue;
                }

                writer
                    .write_event(Event::Empty(e.clone()))
                    .map_err(|e| ThreeMfError::Xml(e.to_string()))?;
            }
            Ok(event) => {
                if skip_depth == 0 {
                    writer
                        .write_event(event)
                        .map_err(|e| ThreeMfError::Xml(e.to_string()))?;
                }
            }
            Err(e) => return Err(ThreeMfError::Xml(e.to_string())),
        }
        buf.clear();
    }

    Ok(writer.into_inner())
}

/// Helper function to retrieve an attribute string value by attribute name.
fn get_attr_value(e: &BytesStart, key: &[u8]) -> Option<String> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref().eq_ignore_ascii_case(key) {
            return String::from_utf8(attr.value.to_vec()).ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_blocked_slicer_file() {
        assert!(is_blocked_slicer_file("Metadata/project_settings.config"));
        assert!(is_blocked_slicer_file("Metadata/slice_info.config"));
        assert!(is_blocked_slicer_file("Metadata/model_settings.config"));
        assert!(is_blocked_slicer_file("Metadata/Slic3r_PE.config"));
        assert!(is_blocked_slicer_file("Metadata/PrusaSlicer.config"));
        assert!(is_blocked_slicer_file("Metadata/plate_1.json"));
        assert!(is_blocked_slicer_file("Cura/cura.cfg"));
        assert!(is_blocked_slicer_file("toolpath.gcode"));

        // Must NOT block valid 3MF parts
        assert!(!is_blocked_slicer_file("3D/3dmodel.model"));
        assert!(!is_blocked_slicer_file("Metadata/thumbnail.png"));
        assert!(!is_blocked_slicer_file("[Content_Types].xml"));
        assert!(!is_blocked_slicer_file("_rels/.rels"));
    }

    #[test]
    fn test_sanitize_model_xml() {
        let raw_xml = br##"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02" xmlns:m="http://schemas.microsoft.com/3dmanufacturing/material/2015/02">
  <metadata name="Title">My Safe Model</metadata>
  <metadata name="slic3rpe:print_settings_id">0.20mm SPEED @MK4</metadata>
  <metadata name="bambu:filament_settings_id">Bambu PLA Basic</metadata>
  <config>
    <setting name="layer_height" value="0.2"/>
  </config>
  <resources>
    <m:colorgroup id="1">
      <m:color color="#00A3E0FF"/>
    </m:colorgroup>
    <object id="1" type="model" pid="1" pindex="0">
      <mesh>
        <vertices>
          <vertex x="0.0" y="0.0" z="0.0"/>
        </vertices>
        <triangles>
          <triangle v1="0" v2="0" v3="0"/>
        </triangles>
      </mesh>
    </object>
    <object id="2" type="support">
      <mesh>
        <vertices>
          <vertex x="1.0" y="1.0" z="1.0"/>
        </vertices>
        <triangles>
          <triangle v1="0" v2="0" v3="0"/>
        </triangles>
      </mesh>
    </object>
  </resources>
  <build>
    <item objectid="1"/>
    <item objectid="2"/>
  </build>
</model>"##;

        let options = SanitizerOptions::default();
        let sanitized = sanitize_model_xml(raw_xml, &options).expect("Sanitization failed");
        let sanitized_str = String::from_utf8(sanitized).expect("Valid utf8");

        // Verified dropped items
        assert!(!sanitized_str.contains("slic3rpe:print_settings_id"));
        assert!(!sanitized_str.contains("bambu:filament_settings_id"));
        assert!(!sanitized_str.contains("<config>"));
        assert!(!sanitized_str.contains("layer_height"));

        // Verified preserved items
        assert!(sanitized_str.contains(r#"<metadata name="Title">My Safe Model</metadata>"#));
        assert!(sanitized_str.contains(r#"<m:colorgroup id="1">"#));
        assert!(sanitized_str.contains(r##"<m:color color="#00A3E0FF"/>"##));
        assert!(sanitized_str.contains(r#"pid="1""#));
        assert!(sanitized_str.contains(r#"pindex="0""#));
        assert!(sanitized_str.contains(r#"<object id="2" type="support">"#));
        assert!(sanitized_str.contains(r#"<item objectid="1"/>"#));
        assert!(sanitized_str.contains(r#"<item objectid="2"/>"#));

        // Test stripping supports and ensuring build items referencing support objects are pruned
        let strip_support_opts = SanitizerOptions {
            preserve_supports: false,
            ..Default::default()
        };
        let stripped = sanitize_model_xml(raw_xml, &strip_support_opts).expect("Sanitization failed");
        let stripped_str = String::from_utf8(stripped).expect("Valid utf8");
        assert!(!stripped_str.contains(r#"type="support""#));
        assert!(!stripped_str.contains(r#"<item objectid="2"/>"#));
        assert!(stripped_str.contains(r#"<item objectid="1"/>"#));
    }
}
