# Contributing to StlTo3mf

Thank you for your interest in contributing to **StlTo3mf** (and the **Omni3MF** engine)!

We are committed to building an exemplary, high-performance, 100% memory-safe STL-to-3MF conversion and mesh healing pipeline in Rust. This document outlines our development process, architectural invariants, code quality standards, and testing conventions.

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [The Zero-Unsafe Invariant](#the-zero-unsafe-invariant)
- [Development Environment Setup](#development-environment-setup)
- [Project Layout](#project-layout)
- [Code Quality & Verification Gates](#code-quality--verification-gates)
- [Synthetic Test Harness Conventions](#synthetic-test-harness-conventions)
- [Coding & Style Guidelines](#coding--style-guidelines)
- [Submitting a Pull Request](#submitting-a-pull-request)

---

## Code of Conduct

All contributors, maintainers, and community members are expected to maintain a welcoming, respectful, and professional environment. Harassment, discrimination, or derogatory behavior will not be tolerated.

---

## The Zero-Unsafe Invariant

Every crate in the StlTo3mf workspace (`omni3mf-core`, `omni3mf-cli`, `omni3mf-wasm`) strictly enforces:

```rust
#![forbid(unsafe_code)]
```

**This is a non-negotiable architectural invariant.** Pull requests containing `unsafe` blocks, `unsafe impl`, or disabling `#![forbid(unsafe_code)]` will be automatically rejected. All geometry operations, coordinate slicing, and memory manipulation must rely entirely on safe Rust idioms.

---

## Development Environment Setup

### Required Tools
- **Rust Toolchain:** Version 1.75.0 or higher.
  ```bash
  rustup update stable
  ```
- **Node.js:** Version 18.0 or higher (for running the Web Studio).
- **wasm-pack:** (Optional, for building WebAssembly bindings).
  ```bash
  cargo install wasm-pack
  ```

### Cloning & Building
```bash
git clone https://github.com/madireis/stl-to-3mf.git
cd stl-to-3mf

# Compile debug workspace
cargo build --workspace

# Compile optimized release binaries
cargo build --release
```

---

## Project Layout

```
StlTo3mf/
├── Cargo.toml                    # Workspace manifest (core, cli, wasm)
├── crates/
│   ├── omni3mf-core/             # 100% Safe Rust core geometry & repair engine
│   │   ├── src/
│   │   │   ├── math/             # Vector3d, Point3d, BoundingBox3d
│   │   │   ├── mesh/             # IndexedMesh, SpatialHashGrid, EdgeTopology
│   │   │   ├── repair/           # Hole patcher, normal reorientation
│   │   │   ├── shell/            # DSU shell segmentation
│   │   │   ├── sanitizer/        # Slicer profile scrubber
│   │   │   ├── scale/            # Dynamic scale sanity detector
│   │   │   ├── thumbnail/        # Pure-Rust software PNG rasterizer
│   │   │   ├── stl/              # Binary & ASCII STL parsers
│   │   │   └── package/          # OPC ZIP archive builder
│   │   └── tests/                # Core synthetic and invariant tests
│   ├── omni3mf-cli/              # Multi-threaded CLI (`stl-to-3mf`, `omni3mf`)
│   │   ├── src/                  # Clap commands, convert, inspect, sanitize
│   │   └── tests/                # CLI integration tests
│   └── omni3mf-wasm/             # WebAssembly browser bindings
│       └── src/lib.rs            # wasm-bindgen export functions
├── docs/                         # In-depth documentation suite
├── web/                          # Zero-dependency Node.js HTTP studio
└── README.md                     # Showcase README
```

---

## Code Quality & Verification Gates

Before submitting any code changes, ensure all verification gates pass cleanly:

### 1. Formatting
```bash
cargo fmt --all -- --check
```

### 2. Clippy Lints
We enforce zero warnings across all targets:
```bash
cargo clippy --workspace --all-targets -- -D warnings
```

### 3. Full Test Suite
All 85 workspace tests must pass without panics:
```bash
cargo test --workspace
```

---

## Synthetic Test Harness Conventions

To keep the repository lightweight and self-contained, **we do not check large binary STL files into git**. All test meshes must be generated programmatically in Rust.

When writing tests in `crates/omni3mf-core/tests/`:
1. **Programmatic Geometry Synthesis:** Construct test shapes (cubes, cylinders, tori, defective non-manifold shells) mathematically using helper functions.
2. **Topological Invariant Verification:** Tests should assert concrete geometric properties:
   - Watertightness: Boundary directed edges count must be `0`.
   - Normal consistency: Signed volume $V(\Omega) > 0$.
   - Shell count: Disjoint components must equal expected count $N$.
   - Scale checks: Dimensions must match expected physical millimeter bounds.
3. **Edge Case Coverage:** Always include tests for:
   - Empty input byte buffers (`0` bytes).
   - Zero-area degenerate triangles (e.g. collinear points).
   - Single isolated triangles (open sheet behavior).
   - Meshes with non-manifold vertex contacts (pinched hourglass points).

Example pattern from `crates/omni3mf-core/tests/synthetic_suite.rs`:
```rust
fn generate_cube_facets(size: f64) -> Vec<RawFacet> {
    // Programmatically generates 12 triangles of a closed manifold cube
    ...
}
```

---

## Coding & Style Guidelines

- **Error Handling:** Avoid `.unwrap()` or `.expect()` in non-test production code. Return structured `Result<T, E>` types using `thiserror` in libraries and `anyhow` in CLI binaries.
- **Floating-Point Equality:** Never compare coordinates using direct `==`. Use epsilon thresholds (e.g. `(a - b).abs() < 1e-9`).
- **Comments and Documentation:** Document public structs, enums, functions, and modules with rustdoc (`///` and `//!`). Include algorithmic complexity and mathematical explanations where applicable.
- **Minimal Dependencies:** Keep crate dependencies lean. Prefer standard library solutions where practical (as demonstrated by our pure-Rust software rasterizer and zero-dependency web server).

---

## Submitting a Pull Request

1. **Fork & Branch:** Create a feature branch from `main`:
   ```bash
   git checkout -b feature/my-enhancement
   ```
2. **Commit Messages:** Write clear, conventional commit messages:
   - `feat: add adaptive hole patching angle tolerance`
   - `fix: handle edge case in binary STL 80-byte header parsing`
   - `docs: update CLI guide with Rayon tuning recipes`
3. **Run Pre-Flight Checks:**
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```
4. **Open PR:** Submit your pull request to the `main` branch with a summary of changes, problem statement, and test confirmation.
