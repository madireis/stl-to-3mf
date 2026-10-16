//! 64-bit precision 3D vector and point representations.

use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

use serde::{Deserialize, Serialize};

use super::numerics::AREA_EPSILON_SQ;

/// A 3D vector with 64-bit IEEE 754 floating point components.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vector3d {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// Point in 3D Euclidean space (alias for `Vector3d`).
pub type Point3d = Vector3d;

impl Vector3d {
    /// Zero vector `(0, 0, 0)`.
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    /// Unit vector along X axis `(1, 0, 0)`.
    pub const UNIT_X: Self = Self::new(1.0, 0.0, 0.0);

    /// Unit vector along Y axis `(0, 1, 0)`.
    pub const UNIT_Y: Self = Self::new(0.0, 1.0, 0.0);

    /// Unit vector along Z axis `(0, 0, 1)`.
    pub const UNIT_Z: Self = Self::new(0.0, 0.0, 1.0);

    /// Creates a new vector from `(x, y, z)` coordinates.
    #[inline(always)]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Computes the dot product with another vector.
    #[inline(always)]
    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// Computes the cross product `self x other`.
    #[inline(always)]
    pub fn cross(self, other: Self) -> Self {
        Self {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }

    /// Computes the squared Euclidean norm (magnitude squared).
    #[inline(always)]
    pub fn norm_squared(self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    /// Computes the Euclidean norm (length / magnitude).
    #[inline(always)]
    pub fn norm(self) -> f64 {
        self.norm_squared().sqrt()
    }

    /// Returns a normalized unit vector, or `None` if the length is too small (< 1e-12) or non-finite.
    pub fn normalize(self) -> Option<Self> {
        let n_sq = self.norm_squared();
        if n_sq > AREA_EPSILON_SQ && n_sq.is_finite() {
            let inv_n = 1.0 / n_sq.sqrt();
            Some(self * inv_n)
        } else {
            None
        }
    }

    /// Returns a normalized vector, falling back to `fallback` if length is sub-epsilon.
    pub fn safe_normalize(self, fallback: Self) -> Self {
        self.normalize().unwrap_or(fallback)
    }

    /// Computes the Euclidean distance squared between two points.
    #[inline(always)]
    pub fn distance_squared(self, other: Self) -> f64 {
        (self - other).norm_squared()
    }

    /// Computes the Euclidean distance between two points.
    #[inline(always)]
    pub fn distance(self, other: Self) -> f64 {
        (self - other).norm()
    }

    /// Checks if all components are finite (not NaN, not infinite).
    #[inline(always)]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

impl Add for Vector3d {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl AddAssign for Vector3d {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
    }
}

impl Sub for Vector3d {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

impl SubAssign for Vector3d {
    #[inline(always)]
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
    }
}

impl Mul<f64> for Vector3d {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: f64) -> Self {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
            z: self.z * rhs,
        }
    }
}

impl Mul<Vector3d> for f64 {
    type Output = Vector3d;

    #[inline(always)]
    fn mul(self, rhs: Vector3d) -> Vector3d {
        rhs * self
    }
}

impl MulAssign<f64> for Vector3d {
    #[inline(always)]
    fn mul_assign(&mut self, rhs: f64) {
        self.x *= rhs;
        self.y *= rhs;
        self.z *= rhs;
    }
}

impl Div<f64> for Vector3d {
    type Output = Self;

    #[inline(always)]
    fn div(self, rhs: f64) -> Self {
        let inv = 1.0 / rhs;
        Self {
            x: self.x * inv,
            y: self.y * inv,
            z: self.z * inv,
        }
    }
}

impl DivAssign<f64> for Vector3d {
    #[inline(always)]
    fn div_assign(&mut self, rhs: f64) {
        let inv = 1.0 / rhs;
        self.x *= inv;
        self.y *= inv;
        self.z *= inv;
    }
}

impl Neg for Vector3d {
    type Output = Self;

    #[inline(always)]
    fn neg(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec3_basic_ops() {
        let a = Vector3d::new(1.0, 2.0, 3.0);
        let b = Vector3d::new(4.0, 5.0, 6.0);
        assert_eq!(a + b, Vector3d::new(5.0, 7.0, 9.0));
        assert_eq!(b - a, Vector3d::new(3.0, 3.0, 3.0));
        assert_eq!(a * 2.0, Vector3d::new(2.0, 4.0, 6.0));
        assert_eq!(2.0 * a, Vector3d::new(2.0, 4.0, 6.0));
        assert_eq!(a / 2.0, Vector3d::new(0.5, 1.0, 1.5));
        assert_eq!(-a, Vector3d::new(-1.0, -2.0, -3.0));
    }

    #[test]
    fn test_vec3_dot_and_cross() {
        let x = Vector3d::UNIT_X;
        let y = Vector3d::UNIT_Y;
        let z = Vector3d::UNIT_Z;

        assert_eq!(x.dot(y), 0.0);
        assert_eq!(x.cross(y), z);
        assert_eq!(y.cross(z), x);
        assert_eq!(z.cross(x), y);
    }

    #[test]
    fn test_vec3_norm_and_normalize() {
        let v = Vector3d::new(3.0, 4.0, 0.0);
        assert_eq!(v.norm_squared(), 25.0);
        assert_eq!(v.norm(), 5.0);
        let n = v.normalize().unwrap();
        assert!((n.norm() - 1.0).abs() < 1e-12);
        assert!((n.x - 0.6).abs() < 1e-12);
        assert!((n.y - 0.8).abs() < 1e-12);

        // Zero vector normalization returns None or fallback
        let zero = Vector3d::ZERO;
        assert!(zero.normalize().is_none());
        assert_eq!(zero.safe_normalize(Vector3d::UNIT_Z), Vector3d::UNIT_Z);
    }
}
