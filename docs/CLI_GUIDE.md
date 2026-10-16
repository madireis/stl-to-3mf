# StlTo3mf CLI Guide

Welcome to the command-line interface guide for **StlTo3mf** (powered by the **Omni3MF** engine).

StlTo3mf provides a high-throughput, memory-safe command-line utility for converting legacy ASCII and binary STL files into compliant 3MF packages, repairing non-manifold defects, separating multi-body shells, and sanitizing untrusted slicer configurations.

---

## Table of Contents

- [Binary Names & Aliases](#binary-names--aliases)
- [Installation & Setup](#installation--setup)
- [Command Synopsis](#command-synopsis)
- [Global Options & Exit Codes](#global-options--exit-codes)
- [Subcommands](#subcommands)
  - [`convert`](#convert)
  - [`inspect`](#inspect)
  - [`sanitize`](#sanitize)
- [Complete Flag & Parameter Reference](#complete-flag--parameter-reference)
- [Real-World Recipes & Flag Patterns](#real-world-recipes--flag-patterns)
  - [Recipe 1: Multi-Shell Separation for Multi-Color 3D Printing](#recipe-1-multi-shell-separation-for-multi-color-3d-printing)
  - [Recipe 2: Aggressive Hole Healing for Scanned & Topology-Defective Meshes](#recipe-2-aggressive-hole-healing-for-scanned--topology-defective-meshes)
  - [Recipe 3: Scrubbing Slicer Overrides from Community 3MF Downloads](#recipe-3-scrubbing-slicer-overrides-from-community-3mf-downloads)
  - [Recipe 4: Imperial to Metric Conversion](#recipe-4-imperial-to-metric-conversion)
  - [Recipe 5: Tuning Rayon Multi-Threading for Print Farms](#recipe-5-tuning-rayon-multi-threading-for-print-farms)
- [Batch Processing & Automation Scripts](#batch-processing--automation-scripts)
  - [Bash Directory Processing](#bash-directory-processing)
  - [PowerShell Directory Processing](#powershell-directory-processing)
  - [CI/CD Validation Pipeline with `jq`](#cicd-validation-pipeline-with-jq)
- [Troubleshooting & Diagnostics](#troubleshooting--diagnostics)

---

## Binary Names & Aliases

The CLI crate compiles three identical binary aliases for developer convenience:

| Binary Alias | Description |
|---|---|
| `stl-to-3mf` | Canonical modern name for the conversion tool. |
| `stls-to-3mf` | Plural alias for multi-file / batch conversion scripts. |
| `omni3mf` | Core engine binary alias reflecting the underlying geometry pipeline. |

All three binaries share identical arguments, flags, and functionality. In this guide, examples use `stl-to-3mf`.

---

## Installation & Setup

### Building from Source

Ensure you have Rust 1.75+ installed:

```bash
# Check Rust version
rustc --version

# Clone and build release binary
git clone https://github.com/madireis/stl-to-3mf.git
cd stl-to-3mf
cargo build --release --bin stl-to-3mf

# Symlink or copy to your PATH (e.g. Linux / macOS)
sudo cp target/release/stl-to-3mf /usr/local/bin/

# On Windows (PowerShell)
Copy-Item target\release\stl-to-3mf.exe -Destination "$env:USERPROFILE\.cargo\bin\"
```

Verify the installation:
```bash
stl-to-3mf --help
```

---

## Command Synopsis

```
stl-to-3mf <COMMAND> [OPTIONS]
```

### Subcommands:
- `convert`: Ingests an STL file or an entire directory of STL files, heals defects, separates shells, and serializes into 3MF.
- `inspect`: Diagnoses geometric metrics, topology, watertightness, and scale sanity of STL or 3MF files.
- `sanitize`: Cleans an existing `.3mf` archive by stripping vendor slicer configurations while preserving materials and supports.

---

## Global Options & Exit Codes

### Global Options:
- `-h, --help`: Prints help information.
- `-V, --version`: Prints version information.

### Exit Codes:
| Exit Code | Meaning |
|:---:|---|
| `0` | Success. The operation completed without errors. |
| `1` | General error (invalid arguments, missing input file, or fatal I/O failure). |
| `2` | Geometry parsing or corruption failure (malformed STL syntax, unreadable binary header). |

---

## Subcommands

### `convert`

Converts a single STL file or a folder of STL files into 3MF archives.

```bash
stl-to-3mf convert <INPUT> [OPTIONS]
```

#### Arguments:
- `<INPUT>` *(required)*: Path to a single `.stl` file (binary or ASCII) or a directory containing `.stl` files.

#### Key Options:
- `-o, --output <OUTPUT>`: Target `.3mf` file path (or output directory if input is a directory). If omitted, defaults to the input path with the `.3mf` extension.
- `--merge-tolerance <f64>`: Vertex spatial welding distance in millimeters. Defaults to `1e-6` ($0.000001\text{ mm}$).
- `--no-repair`: Disables hole loop detection and ear-clipping hole patching.
- `--hole-max-edges <usize>`: Maximum perimeter edges in a boundary loop to patch. Defaults to `64`.
- `--no-split`: Preserves all triangles in a single `<object>` instead of partitioning disconnected shells into separate objects.
- `--strip-slicer-config`: Strips vendor configuration files and metadata from the generated package.
- `--preserve-supports <bool>`: Preserves custom support geometry in the package. Defaults to `true`.
- `--scale <f64>`: Manual uniform scale multiplier (e.g. `25.4` converts inches to mm).
- `--threads <usize>`: Number of Rayon worker threads. `0` automatically uses all available CPU logical cores. Defaults to `0`.
- `-q, --quiet`: Suppresses progress bars and interactive terminal spinners.
- `-v, --verbose`: Enables detailed debugging logs.

---

### `inspect`

Analyzes the topological invariants, dimensions, volume, surface area, and scale sanity of an STL or 3MF model without modifying the file.

```bash
stl-to-3mf inspect <INPUT> [OPTIONS]
```

#### Arguments:
- `<INPUT>` *(required)*: Path to an STL or 3MF file.

#### Options:
- `--json`: Formats and outputs the diagnostic data as structured JSON (ideal for CI/CD pipelines, pre-commit hooks, and automation).

---

### `sanitize`

Purges an existing `.3mf` archive of vendor-specific configuration files (such as `Metadata/project_settings.config`, Bambu/OrcaSlicer/Prusa acceleration tables, temperature overrides, and custom G-code).

```bash
stl-to-3mf sanitize <INPUT> [OPTIONS]
```

#### Arguments:
- `<INPUT>` *(required)*: Path to an existing `.3mf` file.

#### Options:
- `-o, --output <OUTPUT>`: Target sanitized `.3mf` path. Defaults to `<stem>_sanitized.3mf`.
- `--strip-config <bool>`: Strips `.config`, `.cfg`, `.ini`, `.gcode`, and vendor XML namespaces. Defaults to `true`.
- `--preserve-supports <bool>`: Preserves custom support structures. Defaults to `true`.
- `--preserve-colors <bool>`: Preserves material color palettes (`<m:colorgroup>`). Defaults to `true`.

---

## Complete Flag & Parameter Reference

| Subcommand | Flag / Parameter | Type | Default | Description |
|---|---|:---:|:---:|---|
| `convert` | `<INPUT>` | Path | *(required)* | Path to an STL file or directory of STL files. |
| `convert` | `-o, --output <OUTPUT>` | Path | `None` (auto) | Target `.3mf` file or directory path. |
| `convert` | `--merge-tolerance <f64>` | Float | `1e-6` | Spatial welding tolerance ($\text{mm}$). Merges coincident vertices. |
| `convert` | `--no-repair` | Flag | `false` | Skips open boundary cycle detection and hole patching. |
| `convert` | `--hole-max-edges <usize>` | Integer | `64` | Maximum perimeter edges in a boundary loop to patch. |
| `convert` | `--no-split` | Flag | `false` | Keeps mesh as a single unified `<object>`. |
| `convert` | `--strip-slicer-config` | Flag | `false` | Scrubs vendor configuration files and metadata tags. |
| `convert` | `--preserve-supports <bool>` | Boolean | `true` | Retains custom support volume structures. |
| `convert` | `--scale <f64>` | Float | `None` | Multiplies all vertex coordinates uniformly. |
| `convert` | `--threads <usize>` | Integer | `0` (all cores) | Number of Rayon worker threads in thread pool. |
| `convert` | `-q, --quiet` | Flag | `false` | Suppresses progress spinners and banners. |
| `convert` | `-v, --verbose` | Flag | `false` | Enables verbose step-by-step debug logging. |
| `inspect` | `<INPUT>` | Path | *(required)* | File to diagnose (`.stl` or `.3mf`). |
| `inspect` | `--json` | Flag | `false` | Emits structured JSON diagnostic report to stdout. |
| `sanitize` | `<INPUT>` | Path | *(required)* | Existing 3MF package to scrub. |
| `sanitize` | `-o, --output <OUTPUT>` | Path | `<stem>_sanitized.3mf` | Destination path for sanitized archive. |
| `sanitize` | `--strip-config <bool>` | Boolean | `true` | Drops configuration and G-code files. |
| `sanitize` | `--preserve-supports <bool>`| Boolean | `true` | Preserves custom support geometry. |
| `sanitize` | `--preserve-colors <bool>` | Boolean | `true` | Preserves `<m:colorgroup>` and vertex colors. |

---

## Real-World Recipes & Flag Patterns

### Recipe 1: Multi-Shell Separation for Multi-Color 3D Printing

**Problem:** You have a multi-colored CAD model (e.g., figurine, sign with raised letters) exported as a single STL. In Bambu Studio or PrusaSlicer, it imports as an indivisible gray lump.

**Solution:** StlTo3mf's Disjoint Set Union (DSU) engine detects every disconnected topological component and writes each as an independent `<object>` node:

```bash
stl-to-3mf convert multi_part_figure.stl -o figure_multicolor.3mf
```

*Result:* When loaded into Bambu Studio or OrcaSlicer, each shell appears in the object tree under "Objects", allowing you to assign independent filament colors (e.g. Filament 1 for body, Filament 2 for eyes, Filament 3 for base) in a single click.

---

### Recipe 2: Aggressive Hole Healing for Scanned & Topology-Defective Meshes

**Problem:** 3D scan data or Boolean CAD exports frequently feature missing triangles and open seams, which trigger "Non-manifold edges" slicer warnings.

**Solution:** Enable ear-clipping hole patching with an expanded boundary edge loop threshold:

```bash
stl-to-3mf convert raw_scan.stl -o healed_scan.3mf \
    --merge-tolerance 1e-4 \
    --hole-max-edges 256 \
    -v
```

*How it works:*
1. Merges close-proximity vertices within $0.0001\text{ mm}$ using the 27-neighborhood spatial hash grid.
2. Identifies boundary edges lacking twin half-edges.
3. Projects 3D boundary loops up to 256 edges onto their Newell normal planes and fills them with ear-clipping triangulation.
4. Normalizes all face windings outward using the Gauss Divergence Theorem.

---

### Recipe 3: Scrubbing Slicer Overrides from Community 3MF Downloads

**Problem:** You downloaded a 3MF file from Printables or MakerWorld. When opened, it attempts to overwrite your printer's bed temperature to 70°C, alters retraction speeds, and resets acceleration limits to unsafe values.

**Solution:** Sanitize the file before opening in your slicer:

```bash
stl-to-3mf sanitize downloaded_project.3mf -o safe_to_slice.3mf
```

*What gets removed:*
- `Metadata/project_settings.config`
- `Metadata/slice_info.config`
- Any embedded `.gcode` or `.ini` presets
- Vendor custom metadata namespaces (`slic3rpe:*`, `bambu:*`, `orcaslicer:*`, `cura:*`)

*What is preserved:*
- All 3D geometry and discrete shells
- Multi-color material palette groups (`<m:colorgroup>`)
- Slicer support structures (`<object type="support">`)

---

### Recipe 4: Imperial to Metric Conversion

**Problem:** A model designed in AutoCAD or SolidWorks was exported in inches. When imported into a metric slicer, it appears tiny ($25.4\times$ too small).

**Solution:** Apply a uniform scale factor of `25.4`:

```bash
stl-to-3mf convert imperial_bracket.stl -o metric_bracket.3mf --scale 25.4
```

Alternatively, `stl-to-3mf inspect imperial_bracket.stl` will automatically diagnose this condition and recommend `--scale 25.4`:
```json
{
  "scale_anomaly": "TooSmall (< 2mm, suggest 25.4x)",
  "suggested_scale": 25.4,
  "scale_warning": "Warning: Model bounding box is under 2.0 mm. Did you export in inches? Try --scale 25.4."
}
```

---

### Recipe 5: Tuning Rayon Multi-Threading for Print Farms

On a high-core production server (e.g. 32-core AMD EPYC), you may want to dedicate specific thread pools to batch conversion jobs to prevent CPU starvation for other services:

```bash
# Limit conversion to 8 threads
stl-to-3mf convert ./incoming_orders/ -o ./farm_queue/ --threads 8
```

To run quietly inside Docker or headless background jobs without terminal ANSI sequences:
```bash
stl-to-3mf convert order_1082.stl -o order_1082.3mf -q
```

---

## Batch Processing & Automation Scripts

### Bash Directory Processing

Convert every `.stl` file in a directory and report conversions:

```bash
#!/usr/bin/env bash
set -euo pipefail

INPUT_DIR="./models_in"
OUTPUT_DIR="./models_3mf"

mkdir -p "$OUTPUT_DIR"

echo "Starting StlTo3mf batch conversion..."
stl-to-3mf convert "$INPUT_DIR" -o "$OUTPUT_DIR" --threads 0 --strip-slicer-config

echo "Batch processing finished. Summary:"
ls -lh "$OUTPUT_DIR"/*.3mf
```

---

### PowerShell Directory Processing

Windows PowerShell script to iterate recursively through a 3D assets library:

```powershell
$InputDir = "C:\Assets\StlArchive"
$OutputDir = "C:\Assets\3mfArchive"

if (!(Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

Write-Host "Converting all STL files in $InputDir..." -ForegroundColor Cyan
stl-to-3mf convert $InputDir -o $OutputDir --strip-slicer-config --threads 0

$Count = (Get-ChildItem -Path $OutputDir -Filter *.3mf).Count
Write-Host "Successfully converted $Count models to 3MF!" -ForegroundColor Green
```

---

### CI/CD Validation Pipeline with `jq`

Use `stl-to-3mf inspect --json` as an automated gate in GitHub Actions or GitLab CI to ensure submitted pull requests only contain watertight, manifold geometry within printable dimensions:

```bash
#!/usr/bin/env bash
set -euo pipefail

MODEL="hardware/bracket.stl"

# Run inspection and store JSON
REPORT=$(stl-to-3mf inspect "$MODEL" --json)

# Check watertightness
IS_WATERTIGHT=$(echo "$REPORT" | jq -r '.is_watertight')
if [ "$IS_WATERTIGHT" != "true" ]; then
    echo "ERROR: Model $MODEL is not watertight!" >&2
    exit 1
fi

# Check volume > 0
VOLUME=$(echo "$REPORT" | jq -r '.volume')
echo "Model volume: $VOLUME mm³"

# Check shell count
SHELLS=$(echo "$REPORT" | jq -r '.shell_count')
echo "Detected $SHELLS distinct topological shell(s)."

echo "Geometry check PASSED!"
```

---

## Troubleshooting & Diagnostics

### Error: "Invalid STL file format"
- **Cause:** The input file is neither a valid binary STL (80-byte header + 4-byte count + $50 \times N$ facet bytes) nor a recognized ASCII STL (`solid <name>` ... `endsolid`).
- **Resolution:** Verify file integrity. Ensure the file wasn't truncated during download or saved as HTML.

### Warning: "Model bounding box is under 2.0 mm"
- **Cause:** The model is likely modeled in inches.
- **Resolution:** Pass `--scale 25.4` to `convert`.

### Warning: "Model bounding box exceeds 5000.0 mm"
- **Cause:** The model is likely modeled in micrometers ($\mu\text{m}$) or millimeters with an extreme scale.
- **Resolution:** Pass `--scale 0.001` or inspect with `stl-to-3mf inspect <INPUT>`.

### Error: "Output file cannot be identical to input file"
- **Cause:** You specified the same file path for both input and output.
- **Resolution:** Choose a distinct output path with a `.3mf` extension.
