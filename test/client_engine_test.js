/**
 * Comprehensive Automated Verification Suite for Client-Side Geometry Engine & 3MF Packager.
 * Tests StlParser, MeshRepair, ShellSeparator, and ThreeMfPackager.
 */
const assert = require('assert');
const path = require('path');
const fs = require('fs');

const StlParser = require('../web/public/assets/js/stl-parser.js');
const MeshRepair = require('../web/public/assets/js/mesh-repair.js');
const ShellSeparator = require('../web/public/assets/js/shell-separator.js');
const ThreeMfPackager = require('../web/public/assets/js/threemf-packager.js');
const fflate = require('../web/public/assets/vendor/fflate.min.js');

let totalTests = 0;
let passedTests = 0;

function test(name, fn) {
  totalTests++;
  try {
    fn();
    passedTests++;
    console.log(`  ✓ ${name}`);
  } catch (err) {
    console.error(`  ✗ ${name}`);
    console.error(`    ${err.stack || err.message}`);
    process.exitCode = 1;
  }
}

console.log('Running Client-Side Web Converter Geometry & Packaging Test Suite...\n');

// Helper to create synthetic binary STL buffer
function createBinaryStl(facets) {
  const buffer = new ArrayBuffer(84 + facets.length * 50);
  const view = new DataView(buffer);
  view.setUint32(80, facets.length, true);

  let offset = 84;
  for (const f of facets) {
    // Normal (0, 0, 1)
    view.setFloat32(offset, f.normal ? f.normal[0] : 0, true);
    view.setFloat32(offset + 4, f.normal ? f.normal[1] : 0, true);
    view.setFloat32(offset + 8, f.normal ? f.normal[2] : 1, true);

    // v0
    view.setFloat32(offset + 12, f.v0[0], true);
    view.setFloat32(offset + 16, f.v0[1], true);
    view.setFloat32(offset + 20, f.v0[2], true);

    // v1
    view.setFloat32(offset + 24, f.v1[0], true);
    view.setFloat32(offset + 28, f.v1[1], true);
    view.setFloat32(offset + 32, f.v1[2], true);

    // v2
    view.setFloat32(offset + 36, f.v2[0], true);
    view.setFloat32(offset + 40, f.v2[1], true);
    view.setFloat32(offset + 44, f.v2[2], true);

    // Attr
    view.setUint16(offset + 48, 0, true);
    offset += 50;
  }
  return buffer;
}

// Helper to create standard 10x10x10 manifold cube facets (12 triangles)
function createCubeFacets(origin = [0, 0, 0], size = 10) {
  const [ox, oy, oz] = origin;
  const p = [
    [ox, oy, oz],                 // 0
    [ox + size, oy, oz],          // 1
    [ox + size, oy + size, oz],   // 2
    [ox, oy + size, oz],          // 3
    [ox, oy, oz + size],          // 4
    [ox + size, oy, oz + size],   // 5
    [ox + size, oy + size, oz + size], // 6
    [ox, oy + size, oz + size]    // 7
  ];

  // 12 triangles (outward oriented)
  const tris = [
    // Bottom (z = 0)
    [p[0], p[2], p[1]], [p[0], p[3], p[2]],
    // Top (z = size)
    [p[4], p[5], p[6]], [p[4], p[6], p[7]],
    // Front (y = 0)
    [p[0], p[1], p[5]], [p[0], p[5], p[4]],
    // Back (y = size)
    [p[2], p[3], p[7]], [p[2], p[7], p[6]],
    // Left (x = 0)
    [p[0], p[4], p[7]], [p[0], p[7], p[3]],
    // Right (x = size)
    [p[1], p[2], p[6]], [p[1], p[6], p[5]]
  ];

  return tris.map(t => ({ v0: t[0], v1: t[1], v2: t[2] }));
}

// ----------------------------------------------------
// 1. STL Parser Tests
// ----------------------------------------------------
test('StlParser: Binary format auto-detection and parsing', () => {
  const facets = createCubeFacets();
  const binaryBuffer = createBinaryStl(facets);

  const detection = StlParser.detectStlFormat(binaryBuffer);
  assert.strictEqual(detection.format, 'binary');
  assert.strictEqual(detection.facetCount, 12);

  const parsed = StlParser.parseStl(binaryBuffer);
  assert.strictEqual(parsed.format, 'binary');
  assert.strictEqual(parsed.triangleCount, 12);
  assert.strictEqual(parsed.positions.length, 12 * 9); // 108 floats
});

test('StlParser: ASCII format auto-detection and parsing', () => {
  const asciiStl = `solid test_cube
  facet normal 0 0 -1
    outer loop
      vertex 0 0 0
      vertex 10 10 0
      vertex 10 0 0
    endloop
  endfacet
  facet normal 0 0 -1
    outer loop
      vertex 0 0 0
      vertex 0 10 0
      vertex 10 10 0
    endloop
  endfacet
endsolid test_cube`;

  const buffer = Buffer.from(asciiStl, 'utf-8');
  const detection = StlParser.detectStlFormat(buffer);
  assert.strictEqual(detection.format, 'ascii');

  const parsed = StlParser.parseStl(buffer);
  assert.strictEqual(parsed.format, 'ascii');
  assert.strictEqual(parsed.triangleCount, 2);
  assert.strictEqual(parsed.positions.length, 18);
  assert.strictEqual(parsed.header, 'test_cube');
});

// ----------------------------------------------------
// 2. Spatial Hash Vertex Welding Tests
// ----------------------------------------------------
test('MeshRepair: Spatial hash grid vertex deduplication at 10^-6 mm', () => {
  // A cube has 12 triangles = 36 raw vertices, but only 8 unique vertices!
  const facets = createCubeFacets();
  const binaryBuffer = createBinaryStl(facets);
  const parsed = StlParser.parseStl(binaryBuffer);

  const weld = MeshRepair.weldVertices(parsed.positions, 1e-6);
  assert.strictEqual(weld.vertices.length / 3, 8, 'Cube must have exactly 8 deduplicated vertices');
  assert.strictEqual(weld.faces.length / 3, 12, 'Faces count must remain 12');
  assert.strictEqual(weld.deduplicatedCount, 36 - 8, 'Deduplicated 28 redundant vertices');
});

test('MeshRepair: Tolerant welding merges vertices with sub-micron jitter (<1e-6)', () => {
  const positions = new Float32Array([
    0.0, 0.0, 0.0,
    1.0, 0.0, 0.0,
    0.0, 1.0, 0.0,
    // Second triangle with 1e-7 jitter on first vertex
    0.0000001, 0.0, 0.0,
    1.0, 0.0, 0.0,
    0.0, 1.0, 0.0
  ]);

  const weld = MeshRepair.weldVertices(positions, 1e-6);
  assert.strictEqual(weld.vertices.length / 3, 3, 'Jittered coincident vertex must be welded');
});

// ----------------------------------------------------
// 3. Degenerate Facet Pruning Tests
// ----------------------------------------------------
test('MeshRepair: Degenerate facet elimination (collapsed edges and collinear triangles)', () => {
  const vertices = new Float32Array([
    0, 0, 0, // 0
    10, 0, 0, // 1
    20, 0, 0, // 2 (collinear with 0 and 1)
    0, 10, 0  // 3
  ]);

  const faces = new Uint32Array([
    0, 1, 3, // Valid triangle
    0, 0, 1, // Collapsed edge (duplicate index)
    0, 1, 2, // Collinear zero-area triangle
    1, 2, 1  // Collapsed edge
  ]);

  const filtered = MeshRepair.filterDegenerateTriangles(vertices, faces);
  assert.strictEqual(filtered.faces.length / 3, 1, 'Only the valid triangle must survive');
  assert.strictEqual(filtered.prunedCount, 3, '3 degenerate triangles pruned');
  assert.deepStrictEqual(Array.from(filtered.faces), [0, 1, 3]);
});

// ----------------------------------------------------
// 4. Boundary Loop Detection & Hole Healing Tests
// ----------------------------------------------------
test('MeshRepair: Boundary hole healing patches missing cube face', () => {
  // Create a cube missing 1 triangle from top face (making it an open boundary)
  const facets = createCubeFacets();
  facets.pop(); // Remove 1 triangle (leaving a triangular hole)

  const binaryBuffer = createBinaryStl(facets);
  const parsed = StlParser.parseStl(binaryBuffer);

  const weld = MeshRepair.weldVertices(parsed.positions);
  const boundaryBefore = MeshRepair.extractBoundaryDirectedEdges(weld.faces);
  assert.strictEqual(boundaryBefore.length, 3, 'Should have 3 boundary directed edges around triangular hole');

  const repaired = MeshRepair.repairMeshPipeline(parsed.positions, { repair: true });
  assert.strictEqual(repaired.stats.holesPatched >= 1, true, 'At least 1 hole patched');
  assert.strictEqual(repaired.stats.isWatertight, true, 'Mesh must be closed watertight after hole healing');
  assert.strictEqual(repaired.boundaryEdges.length, 0, 'No boundary edges remain');
});

// ----------------------------------------------------
// 5. Winding Unification & Divergence Normals Tests
// ----------------------------------------------------
test('MeshRepair: Divergence Theorem signed volume detects and inverts inverted normals', () => {
  // Create a cube with all inverted windings (inside out)
  const facets = createCubeFacets();
  const invertedFacets = facets.map(f => ({
    v0: f.v0,
    v1: f.v2, // swap to invert
    v2: f.v1
  }));

  const binaryBuffer = createBinaryStl(invertedFacets);
  const parsed = StlParser.parseStl(binaryBuffer);

  const weld = MeshRepair.weldVertices(parsed.positions);
  const orient = MeshRepair.unifyWindingAndOrientOutward(weld.vertices, weld.faces);

  assert.strictEqual(orient.invertedComponents >= 1, true, 'Inverted component must be detected and flipped');
  
  // Re-check signed volume after flip
  const repaired = MeshRepair.repairMeshPipeline(parsed.positions, { repair: true });
  const shells = ShellSeparator.separateShells(repaired.vertices, repaired.faces);
  assert.strictEqual(shells[0].signedVolume > 0, true, 'Final signed volume must be positive (outward oriented)');
  assert.strictEqual(Math.round(shells[0].signedVolume), 1000, 'Cube of size 10 must have volume 1000 mm³');
});

// ----------------------------------------------------
// 6. DSU Multi-Shell Separation Tests
// ----------------------------------------------------
test('ShellSeparator: Disjoint Set Union correctly clusters 2 disconnected cubes', () => {
  const cube1 = createCubeFacets([0, 0, 0], 10);
  const cube2 = createCubeFacets([50, 50, 50], 10);
  const compoundFacets = [...cube1, ...cube2]; // 24 triangles total

  const binaryBuffer = createBinaryStl(compoundFacets);
  const parsed = StlParser.parseStl(binaryBuffer);
  const repaired = MeshRepair.repairMeshPipeline(parsed.positions);

  const shells = ShellSeparator.separateShells(repaired.vertices, repaired.faces, true);
  assert.strictEqual(shells.length, 2, 'Must separate into exactly 2 distinct shells');
  assert.strictEqual(shells[0].faces.length / 3, 12, 'Shell 1 must have 12 triangles');
  assert.strictEqual(shells[1].faces.length / 3, 12, 'Shell 2 must have 12 triangles');
  assert.strictEqual(shells[0].vertices.length / 3, 8, 'Shell 1 compact vertex count 8');
  assert.strictEqual(shells[1].vertices.length / 3, 8, 'Shell 2 compact vertex count 8');
  assert.strictEqual(Math.round(shells[0].signedVolume), 1000);
  assert.strictEqual(Math.round(shells[1].signedVolume), 1000);
});

// ----------------------------------------------------
// 7. 3MF Packager Tests
// ----------------------------------------------------
test('ThreeMfPackager: Generates valid OPC/ZIP 3MF archive with XML models and thumbnail', () => {
  const facets = createCubeFacets();
  const binaryBuffer = createBinaryStl(facets);
  const parsed = StlParser.parseStl(binaryBuffer);
  const repaired = MeshRepair.repairMeshPipeline(parsed.positions);
  const shells = ShellSeparator.separateShells(repaired.vertices, repaired.faces);

  // Fake 256x256 thumbnail PNG bytes (valid PNG header)
  const fakePng = new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 0]);

  const zipBytes = ThreeMfPackager.package3mf(shells, {
    scale: 1.0,
    stripSlicerConfig: true,
    thumbnailPng: fakePng,
    title: 'TestCube'
  });

  assert.strictEqual(zipBytes instanceof Uint8Array, true);
  assert.strictEqual(zipBytes.length > 500, true, 'ZIP archive must be non-empty');

  // Decompress and verify ZIP contents
  const unzipped = fflate.unzipSync(zipBytes);
  const files = Object.keys(unzipped);

  assert.strictEqual(files.includes('[Content_Types].xml'), true, 'Must include [Content_Types].xml');
  assert.strictEqual(files.includes('_rels/.rels'), true, 'Must include _rels/.rels');
  assert.strictEqual(files.includes('3D/3dmodel.model'), true, 'Must include 3D/3dmodel.model');
  assert.strictEqual(files.includes('Metadata/thumbnail.png'), true, 'Must include Metadata/thumbnail.png');
  assert.strictEqual(files.includes('3D/_rels/3dmodel.model.rels'), true, 'Must include 3D/_rels/3dmodel.model.rels');

  // Inspect XML content
  const modelXml = fflate.strFromU8(unzipped['3D/3dmodel.model']);
  assert.strictEqual(modelXml.includes('<model unit="millimeter"'), true);
  assert.strictEqual(modelXml.includes('<m:colorgroup id="1">'), true);
  assert.strictEqual(modelXml.includes('<object id="2" type="model" name="Shell_1"'), true);
  assert.strictEqual(modelXml.includes('<item objectid="2"/>'), true);
  assert.strictEqual(modelXml.includes('xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02"'), true);
});

test('ThreeMfPackager: Multi-shell 3MF contains independent <object> nodes per shell', () => {
  const cube1 = createCubeFacets([0, 0, 0], 10);
  const cube2 = createCubeFacets([30, 0, 0], 10);
  const compound = [...cube1, ...cube2];

  const parsed = StlParser.parseStl(createBinaryStl(compound));
  const repaired = MeshRepair.repairMeshPipeline(parsed.positions);
  const shells = ShellSeparator.separateShells(repaired.vertices, repaired.faces, true);

  const zipBytes = ThreeMfPackager.package3mf(shells, { title: 'TwoCubes' });
  const unzipped = fflate.unzipSync(zipBytes);
  const modelXml = fflate.strFromU8(unzipped['3D/3dmodel.model']);

  // Check 2 objects and 2 build items
  assert.strictEqual(modelXml.includes('<object id="2" type="model" name="Shell_1"'), true);
  assert.strictEqual(modelXml.includes('<object id="3" type="model" name="Shell_2"'), true);
  assert.strictEqual(modelXml.includes('<item objectid="2"/>'), true);
  assert.strictEqual(modelXml.includes('<item objectid="3"/>'), true);
});

test('ThreeMfPackager: Scale multiplier correctly transforms coordinate output', () => {
  const facets = createCubeFacets([0, 0, 0], 10);
  const parsed = StlParser.parseStl(createBinaryStl(facets));
  const repaired = MeshRepair.repairMeshPipeline(parsed.positions);
  const shells = ShellSeparator.separateShells(repaired.vertices, repaired.faces);

  // Scale by 2.0 -> vertex at 10 should become 20.000000
  const zipBytes = ThreeMfPackager.package3mf(shells, { scale: 2.0 });
  const unzipped = fflate.unzipSync(zipBytes);
  const modelXml = fflate.strFromU8(unzipped['3D/3dmodel.model']);

  assert.strictEqual(modelXml.includes('x="20.000000"'), true, 'Scaled coordinates must appear in model XML');
});

console.log(`\n===========================================`);
console.log(`Results: ${passedTests}/${totalTests} tests passed`);
console.log(`===========================================\n`);

if (passedTests !== totalTests) {
  process.exit(1);
}
