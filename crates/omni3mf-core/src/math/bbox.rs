//! 3D axis-aligned bounding box representation.

use serde::{Deserialize, Serialize};

use super::vec3::{Point3d, Vector3d};

/// Axis-aligned bounding box in 3D Euclidean space.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox3d {
    pub min: Point3d,
    pub max: Point3d,
}

impl BoundingBox3d {
    /// Creates an empty bounding box with inverted infinite boundaries.
    pub const fn empty() -> Self {
        Self {
            min: Point3d::new(f64::INFINITY, f64::INFINITY, f64::INFINITY),
            max: Point3d::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
        }
    }

    /// Creates a bounding box from explicit min and max points.
    #[inline(always)]
    pub const fn new(min: Point3d, max: Point3d) -> Self {
        Self { min, max }
    }

    /// Creates a bounding box enclosing a single point.
    #[inline(always)]
    pub fn from_point(p: Point3d) -> Self {
        Self { min: p, max: p }
    }

    /// Computes the bounding box of an iterator of points.
    pub fn from_points<'a, I>(points: I) -> Self
    where
        I: IntoIterator<Item = &'a Point3d>,
    {
        let mut bbox = Self::empty();
        for &p in points {
            bbox.expand_by_point(p);
        }
        bbox
    }

    /// Returns `true` if this bounding box has no points (inverted bounds).
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y || self.min.z > self.max.z
    }

    /// Expands the bounding box to include the specified point `p`.
    #[inline]
    pub fn expand_by_point(&mut self, p: Point3d) {
        if p.x < self.min.x {
            self.min.x = p.x;
        }
        if p.y < self.min.y {
            self.min.y = p.y;
        }
        if p.z < self.min.z {
            self.min.z = p.z;
        }
        if p.x > self.max.x {
            self.max.x = p.x;
        }
        if p.y > self.max.y {
            self.max.y = p.y;
        }
        if p.z > self.max.z {
            self.max.z = p.z;
        }
    }

    /// Returns the union of `self` and `other`.
    #[inline]
    pub fn union(&self, other: &Self) -> Self {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        Self {
            min: Point3d::new(
                self.min.x.min(other.min.x),
                self.min.y.min(other.min.y),
                self.min.z.min(other.min.z),
            ),
            max: Point3d::new(
                self.max.x.max(other.max.x),
                self.max.y.max(other.max.y),
                self.max.z.max(other.max.z),
            ),
        }
    }

    /// Checks if a point lies inside or on the boundaries of this bounding box.
    #[inline]
    pub fn contains(&self, p: Point3d) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }

    /// Returns the center point of the bounding box.
    #[inline]
    pub fn center(&self) -> Point3d {
        (self.min + self.max) * 0.5
    }

    /// Returns the dimensional extents `(width, height, depth)`.
    #[inline]
    pub fn dimensions(&self) -> Vector3d {
        if self.is_empty() {
            Vector3d::ZERO
        } else {
            self.max - self.min
        }
    }

    /// Returns the maximum dimension among X, Y, and Z.
    #[inline]
    pub fn max_dimension(&self) -> f64 {
        let dim = self.dimensions();
        dim.x.max(dim.y).max(dim.z)
    }

    /// Computes the volume of the bounding box.
    #[inline]
    pub fn volume(&self) -> f64 {
        if self.is_empty() {
            0.0
        } else {
            let dim = self.dimensions();
            dim.x * dim.y * dim.z
        }
    }
}

impl Default for BoundingBox3d {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bbox_expansion_and_dimensions() {
        let mut bbox = BoundingBox3d::empty();
        assert!(bbox.is_empty());

        bbox.expand_by_point(Point3d::new(10.0, 20.0, 30.0));
        bbox.expand_by_point(Point3d::new(40.0, 50.0, 60.0));

        assert!(!bbox.is_empty());
        assert_eq!(bbox.min, Point3d::new(10.0, 20.0, 30.0));
        assert_eq!(bbox.max, Point3d::new(40.0, 50.0, 60.0));
        assert_eq!(bbox.dimensions(), Vector3d::new(30.0, 30.0, 30.0));
        assert_eq!(bbox.center(), Point3d::new(25.0, 35.0, 45.0));
        assert_eq!(bbox.max_dimension(), 30.0);
        assert_eq!(bbox.volume(), 27000.0);

        assert!(bbox.contains(Point3d::new(20.0, 30.0, 40.0)));
        assert!(!bbox.contains(Point3d::new(5.0, 30.0, 40.0)));
    }
}
