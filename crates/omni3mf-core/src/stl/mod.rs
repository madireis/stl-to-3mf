//! STL format auto-detection, parsing, and serialization.

pub mod ascii;
pub mod binary;
pub mod error;

pub use ascii::{parse_ascii_stl, write_ascii_stl};
pub use binary::{parse_binary_stl, write_binary_stl, RawFacet};
pub use error::StlError;

/// Detected format of an STL file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StlFormat {
    Binary { facet_count: u32 },
    Ascii,
}

/// Fast, deterministic format auto-detection for STL byte buffers.
pub fn detect_stl_format(buffer: &[u8]) -> Result<StlFormat, StlError> {
    let len = buffer.len();

    // Check 1: If length matches the exact binary formula: L == 84 + 50 * facet_count
    if len >= 84 {
        let facet_count = u32::from_le_bytes([buffer[80], buffer[81], buffer[82], buffer[83]]);
        if let Some(expected_len) = 84usize.checked_add((facet_count as usize).saturating_mul(50))
        {
            if expected_len == len {
                return Ok(StlFormat::Binary { facet_count });
            }
        }
    }

    // Check 2: Check for ASCII STL patterns
    if is_ascii_stl(buffer) {
        return Ok(StlFormat::Ascii);
    }

    // Check 3: If binary with trailing padding or comments
    if len >= 84 {
        let facet_count = u32::from_le_bytes([buffer[80], buffer[81], buffer[82], buffer[83]]);
        if let Some(expected_min) = 84usize.checked_add((facet_count as usize).saturating_mul(50))
        {
            // If length is at least the required binary length and facet_count is reasonable
            if len >= expected_min && facet_count > 0 {
                return Ok(StlFormat::Binary { facet_count });
            }
        }
    }

    if len < 84 {
        if is_ascii_stl(buffer) {
            Ok(StlFormat::Ascii)
        } else {
            Err(StlError::FileTooSmall(len))
        }
    } else {
        Err(StlError::InvalidFormat(
            "File header does not match binary STL size or valid ASCII keywords".to_string(),
        ))
    }
}

/// Inspects whether a buffer represents an ASCII STL file.
fn is_ascii_stl(buffer: &[u8]) -> bool {
    let inspect_len = buffer.len().min(1024);
    let sample = &buffer[..inspect_len];

    // Must be valid ASCII characters (reject binary null bytes in sample)
    for &b in sample {
        if b == 0 {
            return false;
        }
    }

    let sample_str = match std::str::from_utf8(sample) {
        Ok(s) => s.to_ascii_lowercase(),
        Err(_) => return false,
    };

    // Must contain 'solid' and either 'facet', 'endsolid', or 'vertex'
    sample_str.contains("solid")
        && (sample_str.contains("facet")
            || sample_str.contains("vertex")
            || sample_str.contains("endsolid")
            || sample_str.trim().starts_with("solid"))
}

/// Ingests any binary or ASCII STL buffer into a vector of raw facets with format detection.
pub fn read_stl(buffer: &[u8]) -> Result<(StlFormat, Vec<RawFacet>), StlError> {
    let format = detect_stl_format(buffer)?;
    let facets = match format {
        StlFormat::Binary { facet_count } => parse_binary_stl(buffer, facet_count)?,
        StlFormat::Ascii => parse_ascii_stl(buffer)?,
    };
    Ok((format, facets))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{Point3d, Vector3d};

    #[test]
    fn test_format_detection_binary() {
        let f = RawFacet::new(
            Vector3d::UNIT_Z,
            Point3d::ZERO,
            Point3d::UNIT_X,
            Point3d::UNIT_Y,
        );
        let binary_bytes = write_binary_stl("solid binary_test", &[f]).unwrap();
        let format = detect_stl_format(&binary_bytes).unwrap();
        assert_eq!(format, StlFormat::Binary { facet_count: 1 });

        let (fmt, facets) = read_stl(&binary_bytes).unwrap();
        assert_eq!(fmt, StlFormat::Binary { facet_count: 1 });
        assert_eq!(facets.len(), 1);
    }

    #[test]
    fn test_format_detection_ascii() {
        let f = RawFacet::new(
            Vector3d::UNIT_Z,
            Point3d::ZERO,
            Point3d::UNIT_X,
            Point3d::UNIT_Y,
        );
        let ascii_bytes = write_ascii_stl("ascii_test", &[f]);
        let format = detect_stl_format(&ascii_bytes).unwrap();
        assert_eq!(format, StlFormat::Ascii);

        let (fmt, facets) = read_stl(&ascii_bytes).unwrap();
        assert_eq!(fmt, StlFormat::Ascii);
        assert_eq!(facets.len(), 1);
    }

    #[test]
    fn test_format_detection_binary_empty() {
        let binary_bytes = write_binary_stl("empty binary", &[]).unwrap();
        assert_eq!(binary_bytes.len(), 84);
        let format = detect_stl_format(&binary_bytes).unwrap();
        assert_eq!(format, StlFormat::Binary { facet_count: 0 });

        let (fmt, facets) = read_stl(&binary_bytes).unwrap();
        assert_eq!(fmt, StlFormat::Binary { facet_count: 0 });
        assert_eq!(facets.len(), 0);
    }
}
