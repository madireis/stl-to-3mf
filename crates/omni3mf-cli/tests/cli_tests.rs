//! CLI integration tests for Omni3MF.
//!
//! Tests the `convert`, `inspect`, and `sanitize` subcommands, including single-file
//! conversion, multi-threaded batch directory processing, JSON inspection, and error handling.

use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::ZipArchive;

use omni3mf_core::stl::write_binary_stl;
use omni3mf_core::{Point3d, RawFacet, ThreeMfPackage, Vector3d};

/// Helper RAII guard that deletes a temporary directory on drop.
struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(name: &str) -> Self {
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("omni3mf_cli_{name}_{unique_id}"));
        fs::create_dir_all(&path).expect("Failed to create temp dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Helper function to locate the compiled `omni3mf` binary.
fn get_cli_bin() -> &'static str {
    env!("CARGO_BIN_EXE_omni3mf")
}

/// Helper: generates a 12-facet canonical cube at origin.
fn make_cube_facets(ox: f64, oy: f64, oz: f64, size: f64) -> Vec<RawFacet> {
    let p = |x: f64, y: f64, z: f64| Point3d::new(ox + x * size, oy + y * size, oz + z * size);
    let n = Vector3d::new;
    let facet = |normal: Vector3d, v0: Point3d, v1: Point3d, v2: Point3d| RawFacet {
        normal,
        v0,
        v1,
        v2,
        attribute_byte_count: 0,
    };

    vec![
        facet(n(0.0, 0.0, -1.0), p(0.0, 0.0, 0.0), p(1.0, 1.0, 0.0), p(1.0, 0.0, 0.0)),
        facet(n(0.0, 0.0, -1.0), p(0.0, 0.0, 0.0), p(0.0, 1.0, 0.0), p(1.0, 1.0, 0.0)),
        facet(n(0.0, 0.0, 1.0), p(0.0, 0.0, 1.0), p(1.0, 0.0, 1.0), p(1.0, 1.0, 1.0)),
        facet(n(0.0, 0.0, 1.0), p(0.0, 0.0, 1.0), p(1.0, 1.0, 1.0), p(0.0, 1.0, 1.0)),
        facet(n(0.0, -1.0, 0.0), p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(1.0, 0.0, 1.0)),
        facet(n(0.0, -1.0, 0.0), p(0.0, 0.0, 0.0), p(1.0, 0.0, 1.0), p(0.0, 0.0, 1.0)),
        facet(n(0.0, 1.0, 0.0), p(0.0, 1.0, 0.0), p(1.0, 1.0, 1.0), p(1.0, 1.0, 0.0)),
        facet(n(0.0, 1.0, 0.0), p(0.0, 1.0, 0.0), p(0.0, 1.0, 1.0), p(1.0, 1.0, 1.0)),
        facet(n(-1.0, 0.0, 0.0), p(0.0, 0.0, 0.0), p(0.0, 0.0, 1.0), p(0.0, 1.0, 1.0)),
        facet(n(-1.0, 0.0, 0.0), p(0.0, 0.0, 0.0), p(0.0, 1.0, 1.0), p(0.0, 1.0, 0.0)),
        facet(n(1.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0), p(1.0, 1.0, 1.0)),
        facet(n(1.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(1.0, 1.0, 1.0), p(1.0, 0.0, 1.0)),
    ]
}

#[test]
fn test_cli_convert_single_stl_to_3mf() {
    let temp = TempDirGuard::new("single_convert");
    let stl_path = temp.path().join("cube.stl");
    let out_3mf_path = temp.path().join("cube.3mf");

    let facets = make_cube_facets(0.0, 0.0, 0.0, 10.0);
    let bytes = write_binary_stl("Cube", &facets).expect("Binary write failed");
    fs::write(&stl_path, &bytes).expect("Failed to write test STL");

    let output = Command::new(get_cli_bin())
        .arg("convert")
        .arg(&stl_path)
        .arg("-o")
        .arg(&out_3mf_path)
        .output()
        .expect("Failed to execute omni3mf CLI");

    assert!(output.status.success(), "CLI convert must succeed. Stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(out_3mf_path.exists(), "Output 3MF must exist");

    // Inspect generated 3MF archive
    let out_bytes = fs::read(&out_3mf_path).expect("Read output 3MF");
    let mut archive = ZipArchive::new(Cursor::new(out_bytes)).expect("Must be valid ZIP");
    assert!(archive.by_name("3D/3dmodel.model").is_ok(), "Must contain model XML");
    assert!(archive.by_name("Metadata/thumbnail.png").is_ok(), "Must contain thumbnail PNG");
    assert!(archive.by_name("[Content_Types].xml").is_ok(), "Must contain content types");
}

#[test]
fn test_cli_convert_batch_directory() {
    let temp_in = TempDirGuard::new("batch_in");
    let temp_out = TempDirGuard::new("batch_out");

    // Write 3 STL files: cube1.stl, cube2.stl, and sub/cube3.stl
    let facets = make_cube_facets(0.0, 0.0, 0.0, 10.0);
    let bytes = write_binary_stl("Cube", &facets).expect("Binary write");

    fs::write(temp_in.path().join("cube1.stl"), &bytes).unwrap();
    fs::write(temp_in.path().join("cube2.STL"), &bytes).unwrap();
    let sub = temp_in.path().join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("cube3.stl"), &bytes).unwrap();

    let output = Command::new(get_cli_bin())
        .arg("convert")
        .arg(temp_in.path())
        .arg("-o")
        .arg(temp_out.path())
        .arg("--threads")
        .arg("2")
        .output()
        .expect("Execute CLI");

    assert!(output.status.success(), "Batch convert failed: {}", String::from_utf8_lossy(&output.stderr));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Omni3MF Batch Conversion Summary"), "Must print batch summary");
    assert!(stdout.contains("cube1.stl"));
    assert!(stdout.contains("cube2.STL"));
    assert!(stdout.contains("cube3.stl"));

    // Verify output files exist
    assert!(temp_out.path().join("cube1.3mf").exists(), "cube1.3mf must exist");
    assert!(temp_out.path().join("cube2.3mf").exists(), "cube2.3mf must exist");
    assert!(temp_out.path().join("sub").join("cube3.3mf").exists(), "sub/cube3.3mf must exist");
}

#[test]
fn test_cli_convert_no_split_and_scale() {
    let temp = TempDirGuard::new("no_split");
    let stl_path = temp.path().join("two_cubes.stl");
    let out_3mf_path = temp.path().join("no_split.3mf");

    // Two disjoint cubes
    let mut facets = make_cube_facets(0.0, 0.0, 0.0, 10.0);
    facets.extend(make_cube_facets(30.0, 30.0, 30.0, 10.0));
    let bytes = write_binary_stl("TwoCubes", &facets).expect("Binary write");
    fs::write(&stl_path, &bytes).unwrap();

    // Convert with --no-split and --scale 2.0
    let output = Command::new(get_cli_bin())
        .arg("convert")
        .arg(&stl_path)
        .arg("-o")
        .arg(&out_3mf_path)
        .arg("--no-split")
        .arg("--scale")
        .arg("2.0")
        .output()
        .expect("Execute CLI");

    assert!(output.status.success(), "Convert with --no-split must succeed");

    let out_bytes = fs::read(&out_3mf_path).unwrap();
    let mut archive = ZipArchive::new(Cursor::new(out_bytes)).unwrap();
    let mut model_xml = String::new();
    archive.by_name("3D/3dmodel.model").unwrap().read_to_string(&mut model_xml).unwrap();

    // Verify exactly 1 object node because --no-split was used
    assert_eq!(model_xml.matches("<object ").count(), 1, "Must have exactly 1 object node under --no-split");
    // Verify scale 2.0: coordinates reach up to (30+10)*2 = 80.0
    assert!(model_xml.contains("80.000000"), "Must contain coordinates scaled by 2.0");
}

#[test]
fn test_cli_inspect_stl_text_and_json() {
    let temp = TempDirGuard::new("inspect");
    let stl_path = temp.path().join("cube.stl");

    let facets = make_cube_facets(0.0, 0.0, 0.0, 15.0);
    let bytes = write_binary_stl("InspectCube", &facets).expect("Binary write");
    fs::write(&stl_path, &bytes).unwrap();

    // 1. Text inspection
    let text_output = Command::new(get_cli_bin())
        .arg("inspect")
        .arg(&stl_path)
        .output()
        .expect("Inspect text");

    assert!(text_output.status.success());
    let text = String::from_utf8_lossy(&text_output.stdout);
    assert!(text.contains("Omni3MF Geometry Inspection"));
    assert!(text.contains("Discrete Shells:    1"));
    assert!(text.contains("Triangles:          12"));
    assert!(text.contains("Watertight:         Yes"));

    // 2. JSON inspection
    let json_output = Command::new(get_cli_bin())
        .arg("inspect")
        .arg(&stl_path)
        .arg("--json")
        .output()
        .expect("Inspect JSON");

    assert!(json_output.status.success());
    let json_str = String::from_utf8_lossy(&json_output.stdout);
    let v: serde_json::Value = serde_json::from_str(&json_str).expect("Valid JSON");
    assert_eq!(v["vertex_count"], 8);
    assert_eq!(v["face_count"], 12);
    assert_eq!(v["shell_count"], 1);
    assert_eq!(v["is_watertight"], true);
}

#[test]
fn test_cli_inspect_3mf() {
    let temp = TempDirGuard::new("inspect_3mf");
    let stl_path = temp.path().join("cube.stl");
    let out_3mf = temp.path().join("cube.3mf");

    let facets = make_cube_facets(0.0, 0.0, 0.0, 10.0);
    let bytes = write_binary_stl("InspectCube3mf", &facets).unwrap();
    fs::write(&stl_path, &bytes).unwrap();

    Command::new(get_cli_bin())
        .arg("convert")
        .arg(&stl_path)
        .arg("-o")
        .arg(&out_3mf)
        .output()
        .unwrap();

    let output = Command::new(get_cli_bin())
        .arg("inspect")
        .arg(&out_3mf)
        .output()
        .unwrap();

    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("3MF OPC Archive"));
    assert!(text.contains("Thumbnail present: Yes"));
}

#[test]
fn test_cli_sanitize_3mf() {
    let temp = TempDirGuard::new("sanitize");
    let dirty_path = temp.path().join("dirty.3mf");
    let clean_path = temp.path().join("clean.3mf");

    // Build a mock 3MF package containing a vendor slicer config
    let model_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <resources>
    <object id="1" type="model">
      <mesh><vertices/><triangles/></mesh>
    </object>
  </resources>
  <build><item objectid="1"/></build>
</model>"#;

    let mut pkg = ThreeMfPackage::new(model_xml.to_vec(), None);
    pkg.add_extra_file("Metadata/slice_info.config".to_string(), b"vendor=bambu".to_vec());
    pkg.add_extra_file("Metadata/user_note.txt".to_string(), b"keep this note".to_vec());
    let dirty_bytes = pkg.to_bytes().expect("Package bytes");
    fs::write(&dirty_path, &dirty_bytes).unwrap();

    let output = Command::new(get_cli_bin())
        .arg("sanitize")
        .arg(&dirty_path)
        .arg("-o")
        .arg(&clean_path)
        .output()
        .expect("Sanitize execution");

    assert!(output.status.success(), "Sanitize must succeed: {}", String::from_utf8_lossy(&output.stderr));
    assert!(clean_path.exists(), "Clean 3MF must exist");

    let clean_bytes = fs::read(&clean_path).unwrap();
    let mut archive = ZipArchive::new(Cursor::new(clean_bytes)).unwrap();
    assert!(archive.by_name("3D/3dmodel.model").is_ok());
    assert!(archive.by_name("Metadata/slice_info.config").is_err(), "Vendor config must be stripped");
    assert!(archive.by_name("Metadata/user_note.txt").is_ok(), "User note must be preserved");
}

#[test]
fn test_cli_invalid_input_fails_gracefully() {
    let output = Command::new(get_cli_bin())
        .arg("convert")
        .arg("non_existent_file_987654321.stl")
        .output()
        .expect("Execute CLI");

    assert!(!output.status.success(), "CLI must return non-zero exit code for missing file");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("does not exist"), "Stderr should report file does not exist: {stderr}");
}

#[test]
fn test_cli_inspect_empty_model_json() {
    let temp = TempDirGuard::new("empty_inspect");
    let empty_stl = temp.path().join("empty.stl");
    let empty_3mf = temp.path().join("empty.3mf");

    // 1. Empty binary STL (0 facets)
    let bytes = write_binary_stl("Empty", &[]).expect("Empty STL write");
    fs::write(&empty_stl, &bytes).unwrap();

    let output_stl = Command::new(get_cli_bin())
        .arg("inspect")
        .arg(&empty_stl)
        .arg("--json")
        .output()
        .expect("Inspect empty STL");

    assert!(output_stl.status.success(), "Inspect empty STL must succeed: {}", String::from_utf8_lossy(&output_stl.stderr));
    let v_stl: serde_json::Value = serde_json::from_slice(&output_stl.stdout).expect("Valid JSON from empty STL");
    assert_eq!(v_stl["vertex_count"], 0);
    assert_eq!(v_stl["face_count"], 0);
    assert_eq!(v_stl["bounding_box_min"], serde_json::json!([0.0, 0.0, 0.0]));
    assert_eq!(v_stl["bounding_box_max"], serde_json::json!([0.0, 0.0, 0.0]));

    // 2. Empty 3MF (model XML with empty mesh)
    let model_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <resources>
    <object id="1" type="model">
      <mesh><vertices/><triangles/></mesh>
    </object>
  </resources>
  <build><item objectid="1"/></build>
</model>"#;
    let pkg = ThreeMfPackage::new(model_xml.to_vec(), None);
    fs::write(&empty_3mf, pkg.to_bytes().unwrap()).unwrap();

    let output_3mf = Command::new(get_cli_bin())
        .arg("inspect")
        .arg(&empty_3mf)
        .arg("--json")
        .output()
        .expect("Inspect empty 3MF");

    assert!(output_3mf.status.success(), "Inspect empty 3MF must succeed: {}", String::from_utf8_lossy(&output_3mf.stderr));
    let v_3mf: serde_json::Value = serde_json::from_slice(&output_3mf.stdout).expect("Valid JSON from empty 3MF");
    assert_eq!(v_3mf["vertex_count"], 0);
    assert_eq!(v_3mf["face_count"], 0);
    assert_eq!(v_3mf["bounding_box_min"], serde_json::json!([0.0, 0.0, 0.0]));
    assert_eq!(v_3mf["bounding_box_max"], serde_json::json!([0.0, 0.0, 0.0]));
}

#[test]
fn test_cli_convert_reject_identical_input_output() {
    let temp = TempDirGuard::new("identical_path");
    let test_file = temp.path().join("model.3mf");
    fs::write(&test_file, b"dummy content").unwrap();

    // Invoking convert without -o on a .3mf file defaults out_path to model.3mf (identical)
    let output = Command::new(get_cli_bin())
        .arg("convert")
        .arg(&test_file)
        .output()
        .expect("Execute CLI");

    assert!(!output.status.success(), "CLI must fail when out_path == input_path");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Output path cannot be identical to input path"),
        "Stderr should contain identical path error message: {stderr}"
    );

    // Explicitly passing identical -o path on an STL
    let stl_file = temp.path().join("model.stl");
    fs::write(&stl_file, b"dummy content").unwrap();

    let output_explicit = Command::new(get_cli_bin())
        .arg("convert")
        .arg(&stl_file)
        .arg("-o")
        .arg(&stl_file)
        .output()
        .expect("Execute CLI");

    assert!(!output_explicit.status.success(), "CLI must fail when explicit -o == input");
    let stderr_explicit = String::from_utf8_lossy(&output_explicit.stderr);
    assert!(
        stderr_explicit.contains("Output path cannot be identical to input path"),
        "Stderr should contain identical path error message: {stderr_explicit}"
    );
}
