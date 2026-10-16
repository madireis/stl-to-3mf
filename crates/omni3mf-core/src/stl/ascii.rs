//! Streaming, zero-allocation ASCII STL parser and serializer.

use super::binary::RawFacet;
use super::error::StlError;
use crate::math::Vector3d;

/// Fast byte-level scanner for ASCII STL files.
pub struct AsciiScanner<'a> {
    buffer: &'a [u8],
    cursor: usize,
    line: usize,
}

impl<'a> AsciiScanner<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        Self {
            buffer,
            cursor: 0,
            line: 1,
        }
    }

    /// Skips whitespace characters, tracking line numbers.
    pub fn skip_whitespace(&mut self) {
        while self.cursor < self.buffer.len() {
            let b = self.buffer[self.cursor];
            if b == b'\n' {
                self.line += 1;
                self.cursor += 1;
            } else if b == b' ' || b == b'\t' || b == b'\r' {
                self.cursor += 1;
            } else {
                break;
            }
        }
    }

    /// Returns the next non-whitespace token as a byte slice.
    pub fn next_token(&mut self) -> Option<&'a [u8]> {
        self.skip_whitespace();
        if self.cursor >= self.buffer.len() {
            return None;
        }

        let start = self.cursor;
        while self.cursor < self.buffer.len() {
            let b = self.buffer[self.cursor];
            if b == b' ' || b == b'\t' || b == b'\r' || b == b'\n' {
                break;
            }
            self.cursor += 1;
        }
        Some(&self.buffer[start..self.cursor])
    }

    /// Expects a specific ASCII keyword (case-insensitive).
    pub fn expect_keyword(&mut self, expected: &'static str) -> Result<(), StlError> {
        let token = self.next_token().ok_or_else(|| StlError::UnexpectedKeyword {
            expected,
            found: "EOF".to_string(),
            line: self.line,
        })?;

        if token.eq_ignore_ascii_case(expected.as_bytes()) {
            Ok(())
        } else {
            Err(StlError::UnexpectedKeyword {
                expected,
                found: String::from_utf8_lossy(token).to_string(),
                line: self.line,
            })
        }
    }

    /// Parses the next token as an `f64`.
    pub fn next_f64(&mut self) -> Result<f64, StlError> {
        let token = self
            .next_token()
            .ok_or_else(|| StlError::UnexpectedEof(self.cursor))?;

        let token_str = std::str::from_utf8(token).map_err(|e| StlError::InvalidFloat {
            value: String::from_utf8_lossy(token).to_string(),
            line: self.line,
            reason: e.to_string(),
        })?;

        token_str.parse::<f64>().map_err(|e| StlError::InvalidFloat {
            value: token_str.to_string(),
            line: self.line,
            reason: e.to_string(),
        })
    }

    /// Parses three consecutive `f64` values into a `Vector3d`.
    pub fn next_vec3(&mut self) -> Result<Vector3d, StlError> {
        let x = self.next_f64()?;
        let y = self.next_f64()?;
        let z = self.next_f64()?;
        Ok(Vector3d::new(x, y, z))
    }
}

/// Parses an ASCII STL buffer into a vector of `RawFacet` records.
pub fn parse_ascii_stl(buffer: &[u8]) -> Result<Vec<RawFacet>, StlError> {
    let mut scanner = AsciiScanner::new(buffer);
    let mut facets = Vec::new();

    while let Some(token) = scanner.next_token() {
        if token.eq_ignore_ascii_case(b"solid") {
            // Optional name follows: skip until next keyword 'facet' or 'endsolid'
            continue;
        }

        if token.eq_ignore_ascii_case(b"endsolid") {
            // End of solid. Check if more solids exist or end
            continue;
        }

        if token.eq_ignore_ascii_case(b"facet") {
            // Next is "normal <x> <y> <z>"
            scanner.expect_keyword("normal")?;
            let normal = scanner.next_vec3()?;

            scanner.expect_keyword("outer")?;
            scanner.expect_keyword("loop")?;

            scanner.expect_keyword("vertex")?;
            let v0 = scanner.next_vec3()?;

            scanner.expect_keyword("vertex")?;
            let v1 = scanner.next_vec3()?;

            scanner.expect_keyword("vertex")?;
            let v2 = scanner.next_vec3()?;

            scanner.expect_keyword("endloop")?;
            scanner.expect_keyword("endfacet")?;

            facets.push(RawFacet {
                normal,
                v0,
                v1,
                v2,
                attribute_byte_count: 0,
            });
        }
    }

    Ok(facets)
}

/// Serializes raw facets to an ASCII STL byte vector.
pub fn write_ascii_stl(solid_name: &str, facets: &[RawFacet]) -> Vec<u8> {
    let mut out = String::new();
    out.push_str(&format!("solid {}\n", solid_name));

    for f in facets {
        out.push_str(&format!(
            "  facet normal {:e} {:e} {:e}\n    outer loop\n      vertex {:e} {:e} {:e}\n      vertex {:e} {:e} {:e}\n      vertex {:e} {:e} {:e}\n    endloop\n  endfacet\n",
            f.normal.x, f.normal.y, f.normal.z,
            f.v0.x, f.v0.y, f.v0.z,
            f.v1.x, f.v1.y, f.v1.z,
            f.v2.x, f.v2.y, f.v2.z
        ));
    }

    out.push_str(&format!("endsolid {}\n", solid_name));
    out.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Point3d;

    #[test]
    fn test_ascii_roundtrip() {
        let f1 = RawFacet {
            normal: Vector3d::new(0.0, 0.0, 1.0),
            v0: Point3d::new(0.0, 0.0, 0.0),
            v1: Point3d::new(1.0, 0.0, 0.0),
            v2: Point3d::new(0.0, 1.0, 0.0),
            attribute_byte_count: 0,
        };

        let encoded = write_ascii_stl("test_model", &[f1]);
        let decoded = parse_ascii_stl(&encoded).unwrap();

        assert_eq!(decoded.len(), 1);
        assert!((decoded[0].normal.z - 1.0).abs() < 1e-6);
        assert!((decoded[0].v1.x - 1.0).abs() < 1e-6);
        assert!((decoded[0].v2.y - 1.0).abs() < 1e-6);
    }
}
