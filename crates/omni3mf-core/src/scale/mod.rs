//! Dynamic scale sanity checks and unit conversion heuristics.
//!
//! Evaluates model bounding boxes against 3D printing build volume thresholds:
//! - `< 2.0 mm`: Flagged as abnormally small (likely modelled in inches or meters).
//! - `> 5000.0 mm`: Flagged as abnormally large (likely modelled in microns or centimeters).
//! - `2.0 mm ..= 5000.0 mm`: Safe normal printing dimensions.

use serde::{Deserialize, Serialize};

use crate::math::{BoundingBox3d, Point3d};
use crate::mesh::IndexedMesh;
use crate::shell::MeshShell;

/// Classification of model dimensional scale sanity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScaleAnomaly {
    /// Dimension is within the normal 3D printing range [2.0 mm, 5000.0 mm].
    Normal,
    /// Maximum dimension is strictly below 2.0 mm.
    TooSmall,
    /// Maximum dimension strictly exceeds 5000.0 mm (5 meters).
    TooLarge,
}

/// Detailed scale sanity report including bounding dimensions and suggested conversion factors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScaleSanityReport {
    /// Minimum coordinates [min_x, min_y, min_z] in millimeters.
    pub min_bounds: [f64; 3],
    /// Maximum coordinates [max_x, max_y, max_z] in millimeters.
    pub max_bounds: [f64; 3],
    /// Bounding box dimensional extents [dx, dy, dz] in millimeters.
    pub dimensions: [f64; 3],
    /// Maximum dimension max(dx, dy, dz) in millimeters.
    pub max_dimension: f64,
    /// Classification anomaly type.
    pub anomaly: ScaleAnomaly,
    /// Suggested linear scale multiplier to normalize to millimeters.
    pub suggested_scale: f64,
    /// Name of the detected origin unit (e.g. "inches", "meters", "microns", "millimeters").
    pub suggested_unit_name: &'static str,
    /// Human-readable recommendation prompt for UI or CLI display.
    pub user_prompt: String,
}

/// Evaluates the scale sanity of a collection of `MeshShell`s.
pub fn check_scale_sanity(shells: &[MeshShell]) -> ScaleSanityReport {
    if shells.is_empty() {
        return empty_report();
    }
    let mut total_bbox = BoundingBox3d::empty();
    for shell in shells {
        total_bbox = total_bbox.union(&shell.bbox);
    }
    check_scale_sanity_bbox(&total_bbox)
}

/// Evaluates the scale sanity of an `IndexedMesh`.
pub fn check_scale_sanity_mesh(mesh: &IndexedMesh) -> ScaleSanityReport {
    if mesh.vertices.is_empty() {
        return empty_report();
    }
    let bbox = mesh.bounding_box();
    check_scale_sanity_bbox(&bbox)
}

/// Evaluates the scale sanity of a slice of 3D points.
pub fn check_scale_sanity_points(points: &[Point3d]) -> ScaleSanityReport {
    if points.is_empty() {
        return empty_report();
    }
    let bbox = BoundingBox3d::from_points(points);
    check_scale_sanity_bbox(&bbox)
}

/// Evaluates the scale sanity from an existing `BoundingBox3d`.
pub fn check_scale_sanity_bbox(bbox: &BoundingBox3d) -> ScaleSanityReport {
    if bbox.is_empty() {
        return empty_report();
    }

    let min = bbox.min;
    let max = bbox.max;
    let dims = bbox.dimensions();
    let max_dim = dims.x.max(dims.y).max(dims.z);

    if !max_dim.is_finite() {
        if max_dim.is_infinite() && max_dim.is_sign_positive() {
            // Allow positive infinity to fall through to TooLarge
        } else {
            return empty_report();
        }
    }

    let min_bounds = [min.x, min.y, min.z];
    let max_bounds = [max.x, max.y, max.z];
    let dimensions = [dims.x, dims.y, dims.z];

    if max_dim < 2.0 {
        let (factor, unit) = if max_dim * 25.4 >= 10.0 && max_dim * 25.4 <= 600.0 {
            (25.4, "inches")
        } else if max_dim * 1000.0 >= 20.0 && max_dim * 1000.0 <= 500.0 {
            (1000.0, "meters")
        } else {
            (25.4, "inches")
        };

        ScaleSanityReport {
            min_bounds,
            max_bounds,
            dimensions,
            max_dimension: max_dim,
            anomaly: ScaleAnomaly::TooSmall,
            suggested_scale: factor,
            suggested_unit_name: unit,
            user_prompt: format!(
                "Model size ({:.2} x {:.2} x {:.2} mm) is under 2mm. Convert from {} to mm ({:.1}x)?",
                dims.x, dims.y, dims.z, unit, factor
            ),
        }
    } else if max_dim > 5000.0 {
        let (factor, unit) = if max_dim * 0.001 >= 10.0 && max_dim * 0.001 <= 500.0 {
            (0.001, "microns")
        } else if max_dim * 0.1 >= 10.0 && max_dim * 0.1 <= 600.0 {
            (0.1, "centimeters")
        } else {
            (0.001, "microns")
        };

        ScaleSanityReport {
            min_bounds,
            max_bounds,
            dimensions,
            max_dimension: max_dim,
            anomaly: ScaleAnomaly::TooLarge,
            suggested_scale: factor,
            suggested_unit_name: unit,
            user_prompt: format!(
                "Model size ({:.2} x {:.2} x {:.2} mm) exceeds 5000mm. Convert from {} to mm ({:.3}x)?",
                dims.x, dims.y, dims.z, unit, factor
            ),
        }
    } else {
        ScaleSanityReport {
            min_bounds,
            max_bounds,
            dimensions,
            max_dimension: max_dim,
            anomaly: ScaleAnomaly::Normal,
            suggested_scale: 1.0,
            suggested_unit_name: "millimeters",
            user_prompt: String::new(),
        }
    }
}

fn empty_report() -> ScaleSanityReport {
    ScaleSanityReport {
        min_bounds: [0.0, 0.0, 0.0],
        max_bounds: [0.0, 0.0, 0.0],
        dimensions: [0.0, 0.0, 0.0],
        max_dimension: 0.0,
        anomaly: ScaleAnomaly::Normal,
        suggested_scale: 1.0,
        suggested_unit_name: "millimeters",
        user_prompt: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_sanity_normal() {
        let points = vec![
            Point3d::new(0.0, 0.0, 0.0),
            Point3d::new(100.0, 50.0, 25.0),
        ];
        let report = check_scale_sanity_points(&points);
        assert_eq!(report.anomaly, ScaleAnomaly::Normal);
        assert_eq!(report.suggested_scale, 1.0);
        assert_eq!(report.suggested_unit_name, "millimeters");
        assert!(report.user_prompt.is_empty());
    }

    #[test]
    fn test_scale_sanity_inches() {
        // 1.5 inches = 38.1 mm, if treated as mm, max_dim = 1.5 mm
        let points = vec![Point3d::new(0.0, 0.0, 0.0), Point3d::new(1.5, 1.0, 0.5)];
        let report = check_scale_sanity_points(&points);
        assert_eq!(report.anomaly, ScaleAnomaly::TooSmall);
        assert_eq!(report.suggested_scale, 25.4);
        assert_eq!(report.suggested_unit_name, "inches");
        assert!(report.user_prompt.contains("under 2mm"));
        assert!(report.user_prompt.contains("inches"));
    }

    #[test]
    fn test_scale_sanity_meters() {
        // 0.25 meters = 250 mm. If treated as mm: 0.25 mm.
        // 0.25 * 25.4 = 6.35 (< 10), 0.25 * 1000 = 250 (in [20, 500]) -> meters
        let points = vec![Point3d::new(0.0, 0.0, 0.0), Point3d::new(0.25, 0.20, 0.15)];
        let report = check_scale_sanity_points(&points);
        assert_eq!(report.anomaly, ScaleAnomaly::TooSmall);
        assert_eq!(report.suggested_scale, 1000.0);
        assert_eq!(report.suggested_unit_name, "meters");
    }

    #[test]
    fn test_scale_sanity_microns() {
        // 50,000 microns = 50 mm. If treated as mm: 50,000 mm.
        let points = vec![
            Point3d::new(0.0, 0.0, 0.0),
            Point3d::new(50_000.0, 20_000.0, 10_000.0),
        ];
        let report = check_scale_sanity_points(&points);
        assert_eq!(report.anomaly, ScaleAnomaly::TooLarge);
        assert_eq!(report.suggested_scale, 0.001);
        assert_eq!(report.suggested_unit_name, "microns");
        assert!(report.user_prompt.contains("exceeds 5000mm"));
    }

    #[test]
    fn test_scale_sanity_centimeters() {
        // 2000 cm = 20,000 mm.
        // 20,000 * 0.001 = 20 (in [10, 500]) -> microns is picked first if in range.
        // To test centimeters: 7000 mm -> 7000 * 0.001 = 7.0 (< 10.0), 7000 * 0.1 = 700 (> 600).
        // If max_dim = 5500: 5500 * 0.001 = 5.5 (< 10.0), 5500 * 0.1 = 550.0 (in [10, 600]) -> centimeters!
        let points = vec![Point3d::new(0.0, 0.0, 0.0), Point3d::new(5500.0, 10.0, 10.0)];
        let report = check_scale_sanity_points(&points);
        assert_eq!(report.anomaly, ScaleAnomaly::TooLarge);
        assert_eq!(report.suggested_scale, 0.1);
        assert_eq!(report.suggested_unit_name, "centimeters");
    }

    #[test]
    fn test_scale_sanity_empty() {
        let points: Vec<Point3d> = Vec::new();
        let report = check_scale_sanity_points(&points);
        assert_eq!(report.anomaly, ScaleAnomaly::Normal);
        assert_eq!(report.suggested_scale, 1.0);
    }

    #[test]
    fn test_scale_sanity_nan() {
        let bbox = BoundingBox3d {
            min: Point3d::new(f64::NAN, 0.0, 0.0),
            max: Point3d::new(f64::NAN, 10.0, 10.0),
        };
        let report = check_scale_sanity_bbox(&bbox);
        assert_eq!(report.anomaly, ScaleAnomaly::Normal);
        assert_eq!(report.suggested_scale, 1.0);
    }
}
