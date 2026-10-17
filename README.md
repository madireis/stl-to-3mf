# StlTo3mf ⚡

<p align="center">
  <a href="https://madireis.github.io/stl-to-3mf/">
    <img src="https://img.shields.io/badge/Live%20Web%20Converter-Online%20(Free)-0ea5e9?style=for-the-badge&logo=googlechrome&logoColor=white" alt="Live Web Converter" />
  </a>
  <a href="https://madireis.github.io/stl-to-3mf/docs/">
    <img src="https://img.shields.io/badge/Documentation-Interactive%20Portal-6366f1?style=for-the-badge&logo=bookstack&logoColor=white" alt="Docs Portal" />
  </a>
  <a href="https://github.com/madireis/stl-to-3mf/releases">
    <img src="https://img.shields.io/badge/CLI%20Tool-Native%20Release-10b981?style=for-the-badge&logo=rust&logoColor=white" alt="CLI Releases" />
  </a>
</p>

<p align="center">
  <a href="https://github.com/madireis/stl-to-3mf"><img src="https://img.shields.io/badge/rust-1.75%2B-orange.svg" alt="Rust Version" /></a>
  <a href="https://github.com/madireis/stl-to-3mf"><img src="https://img.shields.io/badge/unsafe-0%25%20(forbid)-brightgreen.svg" alt="100% Safe Rust" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT%20%7C%20Apache--2.0-blue.svg" alt="License: MIT / Apache-2.0" /></a>
  <a href="crates/omni3mf-wasm"><img src="https://img.shields.io/badge/wasm-ready-blueviolet.svg" alt="WebAssembly Ready" /></a>
  <a href="https://github.com/madireis/stl-to-3mf/actions"><img src="https://img.shields.io/badge/tests-85%2F85%20passing-success.svg" alt="Tests: 85/85 Passing" /></a>
  <a href="https://github.com/madireis/stl-to-3mf"><img src="https://img.shields.io/badge/platform-windows%20%7C%20linux%20%7C%20macos%20%7C%20web-lightgrey.svg" alt="Platform" /></a>
</p>

<p align="center">
  <strong>Transform messy, single-color STL files into clean, multi-part 3MF models ready for multi-color 3D printing.</strong><br>
  <em>Runs 100% locally in your web browser or at native speed via the multi-core CLI. Zero cloud uploads. Zero data tracking.</em>
</p>

<p align="center">
  <a href="https://madireis.github.io/stl-to-3mf/"><strong>🚀 Open Live Web Converter</strong></a> •
  <a href="https://madireis.github.io/stl-to-3mf/docs/"><strong>📖 Read Documentation</strong></a> •
  <a href="#quickstart"><strong>⚡ Quickstart</strong></a> •
  <a href="#key-features"><strong>✨ Key Features</strong></a> •
  <a href="#supported-slicers"><strong>🖨️ Supported Slicers</strong></a> •
  <a href="#technical-documentation"><strong>📚 Technical Documentation</strong></a>
</p>

---

<a id="overview"></a>
## ⏱️ 30-Second Overview: What is StlTo3mf?

Exporting a 3D model as an **STL (`.stl`)** destroys its assembly structure. STL files are an unindexed "triangle soup": all distinct parts are fused into a single coordinate blob, and files routinely have holes, cracks, non-manifold seams, and reversed triangle normals. When loaded into modern multi-color slicers (Bambu Studio, PrusaSlicer, OrcaSlicer), you cannot paint or select parts separately without tedious manual vertex painting.

**StlTo3mf** instantly analyzes your STL geometry, detects disconnected parts using topological graph clustering, heals mesh defects, and outputs a modern **3MF (`.3mf`)** package with discrete, selectable parts for instant multi-filament 3D printing!

### Why 3MF vs. STL?

| Feature | Legacy STL (`.stl`) ❌ | Modern 3MF with StlTo3mf ⚡ |
|:---|:---:|:---:|
| **Multi-Part Printing** | Glued into 1 uneditable block | **Split into discrete selectable parts** |
| **Multi-Color Slicing** | Painful manual vertex painting | **1-click filament/color assignment per part** |
| **File Size** | Bloated uncompressed coordinates | **60%–80% smaller** (ZIP compressed) |
| **Mesh Watertightness** | Frequent cracks, holes & reversed facets | **Auto-healed watertight geometry** |
| **Printer Touchscreen** | Blank generic cube icon | **Embedded 256×256 thumbnail preview** |
| **Slicer Safety** | No config filtering | **Sanitizes dangerous vendor profiles** |
| **Privacy & Security** | Slow cloud upload converters | **100% client-side (WASM & local CLI)** |

---

<a id="quickstart"></a>
## ⚡ Quickstart

### Option 1: In Your Browser (Zero Installation)

1. **Launch App:** Open the **[Live Web Converter](https://madireis.github.io/stl-to-3mf/)** in any modern web browser.
2. **Drop File:** Drag & drop your `.stl` file (binary or ASCII).
3. **Inspect & Download:** Rotate and inspect in 3D, toggle multi-shell separation, and click **Download .3mf**!

> 💡 *Everything runs directly in your browser using WebAssembly & WebGL. Your 3D models never leave your computer.*

### Option 2: High-Speed CLI (For Power Users & Automation)

Convert files or entire directories across all CPU cores in milliseconds:

```bash
# 1. Clone & Build optimized release binary
git clone https://github.com/madireis/stl-to-3mf.git
cd stl-to-3mf
cargo build --release

# 2. Convert a single file with automated repair and multi-shell splitting
./target/release/stl-to-3mf convert model.stl -o model.3mf

# 3. Batch convert an entire folder using all CPU cores in parallel
./target/release/stl-to-3mf convert ./my_stls/ -o ./converted_3mf/
```

#### Handy CLI Commands:
- **Inspect Geometry:** `./target/release/stl-to-3mf inspect model.stl` (or `--json` for automated CI/CD pipelines)
- **Sanitize Community 3MF:** `./target/release/stl-to-3mf sanitize downloaded.3mf -o clean.3mf`
- **Scale Conversion:** `./target/release/stl-to-3mf convert model_inches.stl -o model_mm.3mf --scale 25.4`

---

<a id="key-features"></a>
## ✨ Key Features

- 🧩 **Smart Multi-Shell Separation:** Uses Disjoint Set Union (DSU) graph clustering to extract disconnected components into separate `<object>` nodes for instant multi-color printing.
- 🩹 **Automated Mesh Repair:** Welds duplicate vertices ($10^{-6}\text{ mm}$ tolerance), eliminates degenerate zero-area sliver triangles, and patches boundary loop holes.
- 🧭 **Divergence Theorem Outward Normals:** Applies Gauss-Ostrogradsky calculus to guarantee all face normals point outward with mathematical certainty.
- 🛡️ **Slicer Profile Sanitizer:** Strips vendor `.config`, `.gcode`, and print speed overrides from downloaded community 3MFs while preserving custom colors and supports.
- 🖼️ **Touchscreen Preview Thumbnailer:** Pure-Rust software rasterizer creates an embedded 256×256 thumbnail visible on printer screens (Bambu X1C, P1S, Prusa MK4, Creality K1).
- 🦀 **100% Memory-Safe Rust:** `#![forbid(unsafe_code)]` across all crates. Zero C/C++ dependencies. Zero segfaults.

---

<a id="supported-slicers"></a>
## 🖨️ Supported Slicers & Printers

StlTo3mf archives are tested and verified for seamless import:

| Slicer Application | Multi-Part Separation | Color Assignment | Touchscreen Preview | Profile Sanitization |
|:---|:---:|:---:|:---:|:---:|
| **Bambu Studio** (v1.8+) | ✅ Separate Parts | ✅ Supported (`<m:colorgroup>`) | ✅ X1C / P1S / A1 Series | ✅ Verified |
| **OrcaSlicer** (v2.0+) | ✅ Separate Parts | ✅ Supported | ✅ K1 / Neptune / QIDI | ✅ Verified |
| **PrusaSlicer** (v2.7+) | ✅ Discrete Objects | ✅ Supported | ✅ MK4 / XL / MINI | ✅ Verified |
| **UltiMaker Cura** (v5.6+) | ✅ Grouped Models | ✅ Supported | ✅ S-Series | ✅ Verified |
| **SuperSlicer** (v2.5+) | ✅ Discrete Objects | ✅ Supported | ✅ Supported | ✅ Verified |

---

<a id="technical-documentation"></a>
## 📚 Technical Documentation

Looking for deeper technical specifications, CLI flags, REST API, or mathematical proofs? Visit our interactive portal or markdown references:

- 📖 **CLI Reference & Flags:** [Interactive Guide](https://madireis.github.io/stl-to-3mf/docs/#cli-guide) • [Markdown Source (`docs/CLI_GUIDE.md`)](docs/CLI_GUIDE.md)
  - Full flag reference, exit codes, JSON schema, and multi-core batch processing recipes.
- 🌐 **Web Studio Guide:** [Interactive Guide](https://madireis.github.io/stl-to-3mf/docs/#web-guide) • [Markdown Source (`docs/WEB_GUIDE.md`)](docs/WEB_GUIDE.md)
  - Architecture, client-side geometry pipeline, Node.js development server, and Docker deployment.
- 📐 **Architecture & Math Proofs:** [Interactive Guide](https://madireis.github.io/stl-to-3mf/docs/#architecture) • [Markdown Source (`docs/ARCHITECTURE.md`)](docs/ARCHITECTURE.md)
  - 11-stage pipeline, Gauss Divergence Theorem volume calculations, Newell normals, and OPC ZIP archive specifications.
- 🤝 **Contributing Guidelines:** [Markdown Source (`docs/CONTRIBUTING.md`)](docs/CONTRIBUTING.md)
  - Code standards, `#![forbid(unsafe_code)]` safety guarantees, and running the 85-test synthetic verification suite.

---

<a id="license"></a>
## 📜 License

Dual-licensed under either:
- **MIT License** ([LICENSE](LICENSE))
- **Apache License, Version 2.0** ([LICENSE](LICENSE))

at your option.
