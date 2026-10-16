# Product Requirement Document (PRD)

## Project Title: Next-Gen Smart STL-to-3MF Ecosystem (Omni3MF)
**Document Version:** 1.1  
**Author:** AI Collaborator  
**Status:** Approved for Core Architecture  

---

## 1. Executive Summary & Vision

### 1.1 Objective
To build **Omni3MF**, the definitive, production-grade converter that bridges the legacy STL format with the modern 3MF standard. Unlike current utilities that merely repackage raw triangle streams into standard ZIP archives, Omni3MF solves structural mesh defects, strips disruptive machine configurations, isolates multi-material color zones, and optimizes files for modern high-speed desktop slicers.

### 1.2 Core Philosophy
* **Never Pass Garbage:** Fix geometry dynamically on ingestion.
* **Respect the Slicer:** Give the user discrete, multi-part objects and strip out hardcoded machine profiles that overwrite custom printer presets.
* **Zero Friction:** Accessible over a web app or an offline CLI tool with automated unit conversion.

---

## 2. Target Audience & Personas

### 2.1 The Multi-Color Enthusiast (Bambu Lab / Prusa / Klipper Users)
* **Needs:** Wants to paint models easily or assign multiple extruders.
* **Pain Point:** Importing a multi-shell STL brings in a single unified blob that cannot be cleanly painted or split.

### 2.2 The Functional Engineering Designer
* **Needs:** Perfect dimensional accuracy across conversions.
* **Pain Point:** Mismatched unit scales (inches vs. mm) causing models to import 25.4x too large or 1000x too small.

### 2.3 The Digital Content Creator (Thingiverse/Printables Designers)
* **Needs:** Generating beautiful 3MF uploads that look professional on print platforms and display thumbnails natively on 3D printer touchscreens.
* **Pain Point:** Designing an optimized model, but losing all texture or shell designations when users convert it back and forth.

---

## 3. Product Architecture & Technical Specifications

### 3.1 Technology Stack Choice: Rust & WebAssembly
To guarantee memory safety, high-performance geometry computations, and an exceptional user experience, the core conversion engine of Omni3MF is explicitly mandated to be written in **Rust**.
* **Performance without Garbage Collection:** Rust enables zero-cost abstractions to parse heavy binary/ASCII datasets with low latency.
* **WebAssembly (Wasm) Targeting:** The entire core parsing pipeline must compile directly to WebAssembly (`wasm32-unknown-unknown`). This enables local-first, native-speed client-side execution within the user's browser, bypassing the need for cloud server overhead or uploading private 3D designs to external servers.
* **Concurrency:** Rust’s data-race protection must be utilized (via crates like `rayon` for native CLI or specialized Wasm workers) to distribute mesh repair, indexing, and vertex deduplication operations across multiple threads safely.

---

## 4. Detailed Functional Requirements

### 4.1 Automated Geometry & Repair Engine (The "Fix-On-Fly" Phase)
* **FR-1.1: Manifold Repair:** The system must scan incoming files for non-manifold edges, disconnected vertices, and open holes. It must automatically patch small holes using an iterative fan-triangulation algorithm.
* **FR-1.2: Normal Vector Recalculation:** The engine must analyze vertex winding orders. If inverted facets are discovered, it must unify normal directions outward from the center of mass.
* **FR-1.3: Coplanar Degeneracy Stripping:** Remove zero-area triangles and merge overlapping double vertices within a threshold of 10⁻⁶ mm.

### 4.2 Structural Segmentation (The "Shell Splitter")
* **FR-2.1: Multi-Body Graph Separation:** The parser must not treat the incoming STL as one flat array. It must map the adjacency matrix of the mesh triangles. If separate, non-touching closed shells are discovered within a single file, it must write them into the 3MF XML structure as distinct, selectable `<object>` IDs.
* **FR-2.2: Intersecting Component Isolation:** For overlapping but distinct geometry configurations, the engine must offer a toggle to split intersections into individual boolean-driven component parts.

### 4.3 Intelligent Profile Cleansing & Metadata Ingestion
* **FR-3.1: Profile Scrubber Mode:** When a user uploads a 3MF file to fix it, or wraps an STL into a project, the engine must isolate the `<config>` tags. It must expose an optional user toggle to drop all manufacturer-specific configurations (e.g., custom acceleration adjustments, absolute temperature settings, flow multipliers) while strictly preserving visual coloring records and custom support vectors.
* **FR-3.2: Automated Thumbnail Rendering:** The system must generate a lightweight, standardized PNG preview screen (256 × 256 pixels, transparent background) of the 3D model, saving it into the `/Metadata/thumbnail.png` path inside the 3MF container. This ensures immediate rendering capability on active printer screens (e.g., Bambu X1C, Prusa MK4, Creality K1).
* **FR-3.3: Dynamic Scale Detection:** The tool will run a sanity check on bounding boxes. If a model measures under 2 mm or over 5000 mm in total length, an interactive modal will prompt the user: *"This model looks unusual in size. Convert from Inches to MM or adjust scale factor?"*

---

## 5. Non-Functional Requirements

### 5.1 Performance & Processing Thresholds
* **NFR-1.1: File Size Handling:** The web client must accept single files up to 250 MB without crashing the active browser context.
* **NFR-1.2: Latency Targets:** Basic geometrical packaging transformations for files smaller than 50 MB must execute under 1.5 seconds. Complex mesh analysis and non-manifold healing must complete within 8 seconds maximum.

### 5.2 Security & Data Lifecycles
* **NFR-2.1: Local-First Processing:** Thanks to the WebAssembly build target, no geometry configurations are permanently stored on cloud environments unless explicitly requested for collaborative public link tracking.

---

## 6. User Interface & User Experience (UI/UX) Guidelines

### 6.1 Core Layout Matrix
* **Drop Zone Element:** A simple drag-and-drop workspace supporting `.stl` and custom `.3mf` files.
* **Transformation Preview Windows:** A lightweight WebGL engine displaying a real-world preview of the model. 
  * Open holes should show up in **Bright Orange**.
  * Separated component objects should be given **distinct colors** automatically to visually signify successful part splitting.

### 6.2 The Granular Profile Filtering Toggle Panel
A clean slider dashboard allowing users to select precisely what information remains in their files:

```
[ ON ]  Strip Machine Accel/Speed Settings (Recommended for universal safety)
[ ON ]  Strip Filament Temperature Overrides
[OFF]  Keep Multi-Color Paint Matrix Maps
[ON ]  Inject Touchscreen Thumbnail Preview Files
```

---

## 7. Metrics & Analytics Tracking
* **Conversion Success Factors:** Tracking the frequency of successfully parsed files versus unhandled engine crashes.
* **Post-Processing Slicer Integrity:** Monitoring the drop rate of users returning to re-adjust scale elements or re-run repair scripts inside their native printing programs.
* **Interface Conversions:** Counting how many users actively opt to strip profile data versus keeping default settings intact.
