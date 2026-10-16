//! Open Packaging Conventions (OPC) ZIP archive builder for 3MF packages.

use std::io::{Seek, Write};
use thiserror::Error;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

#[derive(Error, Debug)]
pub enum ThreeMfError {
    #[error("ZIP archive error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("XML serialization error: {0}")]
    Xml(String),

    #[error("PNG encoding error: {0}")]
    Png(#[from] png::EncodingError),

    #[error("Sanitizer error: {0}")]
    Sanitizer(String),
}

pub const CONTENT_TYPES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/>
  <Default Extension="png" ContentType="image/png"/>
</Types>
"#;

pub const ROOT_RELS_WITH_THUMB_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rel-model" Target="/3D/3dmodel.model" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/>
  <Relationship Id="rel-thumb" Target="/Metadata/thumbnail.png" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/thumbnail"/>
</Relationships>
"#;

pub const ROOT_RELS_NO_THUMB_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rel-model" Target="/3D/3dmodel.model" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/>
</Relationships>
"#;

pub const MODEL_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rel-thumb" Target="/Metadata/thumbnail.png" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/thumbnail"/>
</Relationships>
"#;

/// In-memory representation of a 3MF package container.
#[derive(Debug, Clone)]
pub struct ThreeMfPackage {
    pub model_xml: Vec<u8>,
    pub thumbnail_png: Option<Vec<u8>>,
    pub extra_files: Vec<(String, Vec<u8>)>,
}

impl ThreeMfPackage {
    pub fn new(model_xml: Vec<u8>, thumbnail_png: Option<Vec<u8>>) -> Self {
        Self {
            model_xml,
            thumbnail_png,
            extra_files: Vec::new(),
        }
    }

    /// Adds an extra file to the 3MF package (e.g. metadata).
    pub fn add_extra_file(&mut self, path: impl Into<String>, data: Vec<u8>) {
        self.extra_files.push((path.into(), data));
    }

    /// Writes the OPC ZIP package to a seekable writer with deterministic timestamps.
    pub fn write_to<W: Write + Seek>(&self, writer: W) -> Result<(), ThreeMfError> {
        let mut zip = ZipWriter::new(writer);

        // Deterministic timestamp: 1980-01-01 00:00:00 (standard DOS epoch)
        let dt = DateTime::from_date_and_time(1980, 1, 1, 0, 0, 0).unwrap_or_default();
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .last_modified_time(dt);

        // 1. [Content_Types].xml
        zip.start_file("[Content_Types].xml", options)?;
        zip.write_all(CONTENT_TYPES_XML.as_bytes())?;

        // 2. _rels/.rels
        zip.start_file("_rels/.rels", options)?;
        if self.thumbnail_png.is_some() {
            zip.write_all(ROOT_RELS_WITH_THUMB_XML.as_bytes())?;
        } else {
            zip.write_all(ROOT_RELS_NO_THUMB_XML.as_bytes())?;
        }

        // 3. 3D/3dmodel.model
        zip.start_file("3D/3dmodel.model", options)?;
        zip.write_all(&self.model_xml)?;

        // 4. Metadata/thumbnail.png & 3D/_rels/3dmodel.model.rels
        if let Some(ref thumb) = self.thumbnail_png {
            zip.start_file("Metadata/thumbnail.png", options)?;
            zip.write_all(thumb)?;

            zip.start_file("3D/_rels/3dmodel.model.rels", options)?;
            zip.write_all(MODEL_RELS_XML.as_bytes())?;
        }

        // 5. Extra preserved files
        for (path, content) in &self.extra_files {
            zip.start_file(path, options)?;
            zip.write_all(content)?;
        }

        zip.finish()?;
        Ok(())
    }

    /// Writes the package into an in-memory byte buffer.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ThreeMfError> {
        let mut buffer = std::io::Cursor::new(Vec::with_capacity(self.model_xml.len() + 4096));
        self.write_to(&mut buffer)?;
        Ok(buffer.into_inner())
    }
}
