# Omni3MF Testing Infrastructure & Methodology Specification (TEST_INFRA)

**Document Version:** 1.0  
**Author:** Test Writer (E2E Testing Track)  
**Status:** Approved for Core Verification & CI Gates  

---

## 1. Testing Philosophy & Architecture

Omni3MF is an engineering-grade geometry conversion engine. A single geometric inconsistency—such as an inverted normal, a micro-gap between vertices, or a division-by-zero during cross-product normalization—can ruin a 20-hour 3D print or crash slicer software.

To ensure absolute topological integrity, mathematical correctness, and memory safety, Omni3MF adopts a **Strict Programmatic Synthetic Verification Strategy**:
1. **Zero External File Dependencies**: Tests must not depend on static `.stl` binary assets stored in git. All test meshes are synthesized programmatically in code, ensuring bit-for-bit reproducibility, zero asset drift, and transparent defect parametrization.
2. **Mathematical Invariant Enforcement**: Tests do not merely perform string or byte assertions; they verify fundamental algebraic topology and vector calculus invariants (Euler characteristics, signed divergence volumes, half-edge twin reciprocity, and outward ray dot products).
3. **4-Tier Testing Hierarchy**: Every feature and edge condition is mapped to one of four structured testing tiers.

---

## 2. 4-Tier Testing Hierarchy

```
+-----------------------------------------------------------------------+
| Tier 4: Real-World Synthetic Scenarios                                |
|  - Watertight mechanical bracket with mounting holes & chamfers       |
|  - Multi-body figurine assembly (head, torso, limbs, disjoint shells) |
+-----------------------------------------------------------------------+
                                  ^
+-----------------------------------------------------------------------+
| Tier 3: Cross-Feature Stress Combinations                             |
|  - Dirty multi-defect meshes (holes + inverted facets + degeneracies) |
|  - Disjoint multi-shells with mixed winding & duplicate vertices      |
+-----------------------------------------------------------------------+
                                  ^
+-----------------------------------------------------------------------+
| Tier 2: Boundary & Corner Cases                                       |
|  - Empty mesh (0 vertices, 0 faces), single isolated triangle         |
|  - Exact 10^-6 mm spatial tolerance threshold boundaries              |
|  - Collinear zero-area triangles, non-planar boundary loops           |
|  - Extreme bounding boxes (< 2.0 mm and > 5000.0 mm)                  |
+-----------------------------------------------------------------------+
                                  ^
+-----------------------------------------------------------------------+
| Tier 1: Feature Coverage (>= 5 tests per feature)                     |
|  - STL Ingestion (Binary & ASCII)   - Spatial Hash Welding            |
|  - Degeneracy Pruning               - Boundary Loop & Fan Healing     |
|  - Winding Propagation & Normals    - Structural Shell Separation     |
|  - 3MF OPC ZIP Packaging            - Slicer Profile Sanitizer        |
|  - Headless 256x256 Thumbnail       - Dynamic Scale Sanity Checker    |
+-----------------------------------------------------------------------+
```

---

### Tier 1: Unit & Feature Coverage (>= 5 Tests Per Feature)

Tier 1 exercises every core capability in isolation with at least 5 targeted unit tests:

| Feature Area | Minimum Tests | Key Verification Points |
|---|---|---|
| **1. Binary STL Parsing** | 5 | Valid 80-byte header, 4-byte facet count, 50-byte chunk reading, headers starting with `solid`, truncated file error handling. |
| **2. ASCII STL Parsing** | 5 | Standard ASCII format, exponential/scientific notation (`-1.5e-2`), irregular whitespace/newlines, multi-solid blocks, malformed tokens. |
| **3. Format Disambiguation** | 5 | Deterministic formula $L = 84 + 50N$, binary with `solid` prefix, ASCII without `solid` keyword, 84-byte empty binary, random junk bytes. |
| **4. Spatial Hash Welding** | 5 | Coincident vertices within $10^{-6}$ mm, vertices separated by $> 10^{-6}$ mm, hash grid 27-cell neighbor queries, large coordinate offsets, hash collisions. |
| **5. Degeneracy Pruning** | 5 | Duplicate vertex indices (`[v0, v0, v1]`), collinear points (area $= 0$), sub-epsilon area ($< 10^{-24}$ norm squared), `NaN`/`Inf` coordinate handling, preservation of valid slender triangles. |
| **6. Boundary Healing** | 5 | Single missing triangle (3-edge hole), quad hole (4-edge hole), multi-edge planar loop, iterative multi-hole healing, max-edge limit enforcement. |
| **7. Winding & Normals** | 5 | Single inverted face flip, completely inverted mesh, mixed winding BFS alignment, divergence theorem volume ($V > 0$), outward ray dot products. |
| **8. Shell Separation** | 5 | Single closed shell, two disjoint cubes, $N$-component compound, vertex re-indexing to compact 0-based buffers, shell bounding boxes and COM. |
| **9. 3MF OPC Packaging** | 5 | PKZIP archive validity, `[Content_Types].xml` content, `_rels/.rels` content, `3D/3dmodel.model` XML schema, multi-object build items. |
| **10. Sanitizer & Scale** | 5 | Slicer `<config>` stripping, vendor metadata purging, color preservation (`<m:colorgroup>`), scale check $< 2$ mm, scale check $> 5000$ mm. |

---

### Tier 2: Boundary & Corner Cases

Tier 2 targets pathological inputs, mathematical singularities, and extreme scale factors:

1. **Empty Mesh Ingestion**: 0 facets, empty byte buffer, 84-byte empty binary header. Must return clean empty structures without panicking.
2. **Single Isolated Triangle**: 3 vertices, 1 face. Non-watertight, 3 boundary edges, area $> 0$, signed volume $= 0$.
3. **Exact Tolerance Thresholds**:
   - Two vertices separated by $0.999 \times 10^{-6}$ mm $\to$ **Welded** into 1 vertex.
   - Two vertices separated by $1.001 \times 10^{-6}$ mm $\to$ **Preserved** as 2 distinct vertices.
4. **Collinear Singularities**: Points $(0,0,0), (1,0,0), (2,0,0)$ where $\vec{e}_1 \times \vec{e}_2 = \vec{0}$. Cross-product norm squared is 0; must not cause division-by-zero or `NaN` during normalization.
5. **Non-Planar Boundary Holes**: Saddle-shaped or non-planar boundary loops healed safely via projection and iterative fan fallback.
6. **Inverted Normal Extremes**: A fully inverted sphere or cube whose signed volume is strictly negative ($-8.0$). Engine must detect and invert all facet windings to $+8.0$.
7. **Scale Boundary Limits**:
   - Model dimensions $= 1.99$ mm $\to$ Flagged as abnormally small (`is_abnormal_scale = true`).
   - Model dimensions $= 5001.0$ mm $\to$ Flagged as abnormally large (`is_abnormal_scale = true`).
   - Model dimensions $= 100.0$ mm $\to$ Passed as normal (`is_abnormal_scale = false`).

---

### Tier 3: Cross-Feature Stress Combinations

Tier 3 exercises interactions where multiple defects occur simultaneously in the same mesh buffer:

1. **The "Dirty Mesh" Pipeline**:
   - Ingestion of ASCII STL with scientific notation coordinates.
   - Perturbed duplicate vertices within $10^{-7}$ mm.
   - Multiple collinear zero-area triangles injected.
   - 2 missing faces creating boundary loops.
   - 3 faces inverted in winding.
   - Verification: Ingestion $\to$ Welding $\to$ Degeneracy Pruning $\to$ Boundary Healing $\to$ Outward Orientation produces a clean, watertight 2-manifold mesh with $\chi = 2$, boundary count $= 0$, and $V > 0$.
2. **Multi-Shell Compound with Internal Defects**:
   - Shell A: Canonical unit cube with 1 missing face.
   - Shell B: Inverted unit cube translated by $(+10, 0, 0)$.
   - Verification: Both shells healed and oriented independently; shell separation detects 2 distinct closed objects; Euler characteristic $\chi = 4$.

---

### Tier 4: Real-World Synthetic Application Scenarios

Tier 4 generates complex, multi-triangle synthetic engineering parts:

1. **Watertight Mechanical Bracket (`generate_mechanical_bracket`)**:
   - Programmatically synthesizes an L-shaped mounting bracket with a through-hole.
   - 36+ triangles, exact planar alignments.
   - Verifies Euler characteristic of a torus/through-hole topology ($\chi = V - E + F = 0$ for genus 1).
   - Validates watertightness, surface area, and positive volume.
2. **Multi-Body Figurine Assembly (`generate_figurine_compound`)**:
   - Synthesizes a composite model comprising head (cube/polyhedron), torso, and limbs as disjoint closed components.
   - Verifies shell splitting into discrete selectable parts.
   - Verifies per-shell center of mass and bounding box computations.

---

## 3. Programmatic Mesh Generators

The test suite in `crates/omni3mf-core/tests/synthetic_suite.rs` exposes 7 zero-dependency programmatic mesh generators:

```rust
// 1. Canonical closed 2-manifold cube (8 vertices, 12 faces, Chi = 2, Vol = 8.0)
pub fn generate_manifold_cube() -> IndexedMesh;

// 2. Unit cube with 1 or more facets removed, exposing open boundary loops
pub fn generate_cube_with_hole(hole_type: HoleType) -> IndexedMesh;

// 3. Multi-shell compound containing N disjoint geometries separated by > 10^-6 mm
pub fn generate_multi_shell_compound(shell_count: usize, spacing: f64) -> IndexedMesh;

// 4. Manifold cube with specified facet winding orders reversed
pub fn generate_inverted_facets_mesh(mode: InvertMode) -> IndexedMesh;

// 5. Mesh injected with collinear, collapsed, sub-epsilon duplicate, and NaN vertices
pub fn generate_degenerate_mesh() -> (Vec<Point3d>, Vec<[u32; 3]>);

// 6. Synthetic L-bracket mechanical model with genus 1 through-hole
pub fn generate_mechanical_bracket() -> IndexedMesh;

// 7. Multi-body figurine with head, torso, and disjoint limbs
pub fn generate_figurine_compound() -> IndexedMesh;
```

---

## 4. Mathematical & Topological Invariant Assertions

Every test asserts strict mathematical invariants:

### 4.1 Euler Characteristic Formula
$$\chi = V - E + F$$
- For a genus-0 closed 2-manifold surface (sphere, cube): $\chi = 2$.
- For a genus-1 surface (torus, mechanical part with 1 through-hole): $\chi = 0$.
- For $N$ disjoint genus-0 closed surfaces: $\chi = 2N$.

### 4.2 Boundary Half-Edge Reciprocity
In a closed 2-manifold surface, every directed edge $(u, v)$ must have exactly one opposite twin $(v, u)$:
$$\forall (u, v) \in \mathcal{E}_{directed}, \quad (v, u) \in \mathcal{E}_{directed}$$
$$\text{BoundaryEdgeCount} = |\{ (u, v) \in \mathcal{E}_{directed} \mid (v, u) \notin \mathcal{E}_{directed} \}| = 0$$

### 4.3 Divergence Theorem Signed Volume
For outward-oriented triangle facets:
$$V = \frac{1}{6} \sum_{i=1}^{F} \vec{v}_{i0} \cdot (\vec{v}_{i1} \times \vec{v}_{i2}) > 0$$
A negative signed volume indicates inverted facet winding.

### 4.4 Outward Normal Ray Condition
For any convex surface with center of mass $\vec{c}_{mesh}$, every face normal $\vec{n}_i$ must satisfy:
$$\vec{n}_i \cdot (\vec{c}_{face, i} - \vec{c}_{mesh}) > 0$$

---

## 5. Test Execution Commands & CI Automation

### 5.1 Test Runner Commands
```powershell
# Run the complete test suite across workspace
cargo test --workspace

# Run core library unit tests
cargo test -p omni3mf-core --lib

# Run the synthetic invariant verification suite
cargo test -p omni3mf-core --test synthetic_suite

# Run Milestone 1 integration pipeline tests
cargo test -p omni3mf-core --test m1_pipeline_tests
```

### 5.2 Quality Gate & CI Verification Matrix
```powershell
# Gate 1: Code Formatting Check
cargo fmt --all -- --check

# Gate 2: Clippy Static Analysis (Zero Warnings)
cargo clippy --workspace --all-targets -- -D warnings

# Gate 3: Unit and Integration Test Execution
cargo test --workspace --all-targets

# Gate 4: Release Binary Build
cargo build --release --workspace

# Gate 5: WebAssembly Target Check
cargo check --target wasm32-unknown-unknown -p omni3mf-core -p omni3mf-wasm
```

---

## 6. Pass / Fail Acceptance Criteria

| Invariant / Check | Expected Value | Tolerance |
|---|---|---|
| Watertight Cube Euler Characteristic $\chi$ | $2$ | Exact integer |
| Disjoint $N$-Cube Euler Characteristic $\chi$ | $2N$ | Exact integer |
| Boundary Edges Post-Healing | $0$ | Exact integer |
| Unit Cube Signed Volume | $+8.0\text{ mm}^3$ | $\pm 10^{-6}$ |
| Unit Cube Surface Area | $24.0\text{ mm}^2$ | $\pm 10^{-6}$ |
| Face Normal Dot Product with Outward Ray | $> 0.0$ | Strict inequality |
| Vertex Deduplication within Threshold | Merged to single index | Distance $< 10^{-6}$ mm |
| Vertex Separation across Threshold | Distinct indices | Distance $> 10^{-6}$ mm |
| Zero-Area Triangle Pruning | Removed from mesh | Cross product norm sq $< 10^{-24}$ |
