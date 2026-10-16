//! High-performance zero-copy binary STL parser and serializer.

use byteorder::{ByteOrder, LittleEndian};

use super::error::StlError;
use crate::math::{Point3d, Vector3d};

/// Raw facet record parsed from an STL file.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawFacet {
    pub normal: Vector3d,
    pub v0: Point3d,
    pub v1: Point3d,
    pub v2: Point3d,
    pub attribute_byte_count: u16,
}

impl RawFacet {
    /// Creates a raw facet with explicit vertices and normal.
    pub const fn new(normal: Vector3d, v0: Point3d, v1: Point3d, v2: Point3d) -> Self {
        Self {
            normal,
            v0,
            v1,
            v2,
            attribute_byte_count: 0,
        }
    }
}

/// Parses a binary STL buffer into a vector of `RawFacet` records.
pub fn parse_binary_stl(buffer: &[u8], facet_count: u32) -> Result<Vec<RawFacet>, StlError> {
    let count = facet_count as usize;
    let expected_len = 84usize
        .checked_add(count.checked_mul(50).ok_or(StlError::BufferOverflow)?)
        .ok_or(StlError::BufferOverflow)?;

    if buffer.len() < expected_len {
        return Err(StlError::UnexpectedEof(buffer.len()));
    }

    let mut facets = Vec::with_capacity(count);
    let mut offset = 84;

    for _ in 0..count {
        let chunk = &buffer[offset..offset + 50];

        let nx = LittleEndian::read_f32(&chunk[0..4]) as f64;
        let ny = LittleEndian::read_f32(&chunk[4..8]) as f64;
        let nz = LittleEndian::read_f32(&chunk[8..12]) as f64;

        let v0x = LittleEndian::read_f32(&chunk[12..16]) as f64;
        let v0y = LittleEndian::read_f32(&chunk[16..20]) as f64;
        let v0z = LittleEndian::read_f32(&chunk[20..24]) as f64;

        let v1x = LittleEndian::read_f32(&chunk[24..28]) as f64;
        let v1y = LittleEndian::read_f32(&chunk[28..32]) as f64;
        let v1z = LittleEndian::read_f32(&chunk[32..36]) as f64;

        let v2x = LittleEndian::read_f32(&chunk[36..40]) as f64;
        let v2y = LittleEndian::read_f32(&chunk[40..44]) as f64;
        let v2z = LittleEndian::read_f32(&chunk[44..48]) as f64;

        let attr = LittleEndian::read_u16(&chunk[48..50]);

        facets.push(RawFacet {
            normal: Vector3d::new(nx, ny, nz),
            v0: Point3d::new(v0x, v0y, v0z),
            v1: Point3d::new(v1x, v1y, v1z),
            v2: Point3d::new(v2x, v2y, v2z),
            attribute_byte_count: attr,
        });

        offset += 50;
    }

    Ok(facets)
}

/// Serializes raw facets to a binary STL byte vector.
pub fn write_binary_stl(header_str: &str, facets: &[RawFacet]) -> Result<Vec<u8>, StlError> {
    let count = facets.len();
    let total_len = 84usize
        .checked_add(count.checked_mul(50).ok_or(StlError::BufferOverflow)?)
        .ok_or(StlError::BufferOverflow)?;

    let mut buf = vec![0u8; total_len];

    // Write header (up to 80 bytes)
    let header_bytes = header_str.as_bytes();
    let copy_len = header_bytes.len().min(80);
    buf[..copy_len].copy_from_slice(&header_bytes[..copy_len]);

    // Write facet count at 80..84
    LittleEndian::write_u32(&mut buf[80..84], count as u32);

    let mut offset = 84;
    for facet in facets {
        LittleEndian::write_f32(&mut buf[offset..offset + 4], facet.normal.x as f32);
        LittleEndian::write_f32(&mut buf[offset + 4..offset + 8], facet.normal.y as f32);
        LittleEndian::write_f32(&mut buf[offset + 8..offset + 12], facet.normal.z as f32);

        LittleEndian::write_f32(&mut buf[offset + 12..offset + 16], facet.v0.x as f32);
        LittleEndian::write_f32(&mut buf[offset + 16..offset + 20], facet.v0.y as f32);
        LittleEndian::write_f32(&mut buf[offset + 20..offset + 24], facet.v0.z as f32);

        LittleEndian::write_f32(&mut buf[offset + 24..offset + 28], facet.v1.x as f32);
        LittleEndian::write_f32(&mut buf[offset + 28..offset + 32], facet.v1.y as f32);
        LittleEndian::write_f32(&mut buf[offset + 32..offset + 36], facet.v1.z as f32);

        LittleEndian::write_f32(&mut buf[offset + 36..offset + 40], facet.v2.x as f32);
        LittleEndian::write_f32(&mut buf[offset + 40..offset + 44], facet.v2.y as f32);
        LittleEndian::write_f32(&mut buf[offset + 44..offset + 48], facet.v2.z as f32);

        LittleEndian::write_u16(&mut buf[offset + 48..offset + 50], facet.attribute_byte_count);

        offset += 50;
    }

    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_roundtrip() {
        let f1 = RawFacet {
            normal: Vector3d::new(0.0, 0.0, 1.0),
            v0: Point3d::new(0.0, 0.0, 0.0),
            v1: Point3d::new(1.0, 0.0, 0.0),
            v2: Point3d::new(0.0, 1.0, 0.0),
            attribute_byte_count: 0,
        };

        let encoded = write_binary_stl("Omni3MF Binary Test", &[f1]).unwrap();
        assert_eq!(encoded.len(), 84 + 50);

        let decoded = parse_binary_stl(&encoded, 1).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].normal, f1.normal);
        assert_eq!(decoded[0].v0, f1.v0);
        assert_eq!(decoded[0].v1, f1.v1);
        assert_eq!(decoded[0].v2, f1.v2);
    }
}
