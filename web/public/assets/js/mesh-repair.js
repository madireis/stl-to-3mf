/**
 * StlTo3mf — Client-Side Mesh Repair Engine
 * Pure JavaScript implementation with 10^-6 mm spatial hash welding,
 * degenerate facet elimination, boundary loop hole healing,
 * and Gauss-Ostrogradsky Divergence Theorem outward normal reorientation.
 */
(function (root, factory) {
  if (typeof define === 'function' && define.amd) {
    define([], factory);
  } else if (typeof module === 'object' && module.exports) {
    module.exports = factory();
  } else {
    root.MeshRepair = factory();
  }
}(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  const VERTEX_TOLERANCE_DEFAULT = 1e-6; // 10^-6 mm tolerance
  const AREA_EPSILON_SQ = 1e-24; // (10^-12 mm)^2 area threshold

  /**
   * Spatial Hash Grid for O(N) coincident vertex welding.
   * Merges vertices within tolerance (default 10^-6 mm).
   */
  class SpatialHashGrid {
    constructor(tolerance = VERTEX_TOLERANCE_DEFAULT) {
      this.cellSize = tolerance > 0 ? tolerance : VERTEX_TOLERANCE_DEFAULT;
      this.epsilonSq = this.cellSize * this.cellSize;
      // Key: "cx,cy,cz" -> Array of vertex indices
      this.grid = new Map();
      this.vertices = []; // Array of [x, y, z]
    }

    coord(val) {
      if (!Number.isFinite(val)) return 0;
      return Math.floor(val / this.cellSize);
    }

    insertOrGet(x, y, z) {
      const cx = this.coord(x);
      const cy = this.coord(y);
      const cz = this.coord(z);

      // Probe 3x3x3 neighborhood
      for (let dx = -1; dx <= 1; dx++) {
        for (let dy = -1; dy <= 1; dy++) {
          for (let dz = -1; dz <= 1; dz++) {
            const key = `${cx + dx},${cy + dy},${cz + dz}`;
            const bucket = this.grid.get(key);
            if (bucket) {
              for (let i = 0; i < bucket.length; i++) {
                const idx = bucket[i];
                const vx = this.vertices[idx * 3];
                const vy = this.vertices[idx * 3 + 1];
                const vz = this.vertices[idx * 3 + 2];
                const dSq = (x - vx) ** 2 + (y - vy) ** 2 + (z - vz) ** 2;
                if (dSq <= this.epsilonSq) {
                  return idx;
                }
              }
            }
          }
        }
      }

      // Insert new canonical vertex
      const newIdx = this.vertices.length / 3;
      this.vertices.push(x, y, z);
      const centerKey = `${cx},${cy},${cz}`;
      let bucket = this.grid.get(centerKey);
      if (!bucket) {
        bucket = [];
        this.grid.set(centerKey, bucket);
      }
      bucket.push(newIdx);
      return newIdx;
    }
  }

  /**
   * Welds unindexed vertex soup into indexed mesh.
   * @param {Float32Array|number[]} positions Flat array [x0, y0, z0, x1, ...]
   * @param {number} [tolerance=1e-6]
   */
  function weldVertices(positions, tolerance = VERTEX_TOLERANCE_DEFAULT) {
    const rawVertexCount = (positions.length / 3) | 0;
    const triangleCount = (rawVertexCount / 3) | 0;
    const grid = new SpatialHashGrid(tolerance);

    const faces = new Uint32Array(triangleCount * 3);
    let facePtr = 0;

    for (let i = 0; i < rawVertexCount; i++) {
      const x = positions[i * 3];
      const y = positions[i * 3 + 1];
      const z = positions[i * 3 + 2];
      faces[facePtr++] = grid.insertOrGet(x, y, z);
    }

    const uniqueVertices = new Float32Array(grid.vertices);
    const deduplicatedCount = rawVertexCount - (uniqueVertices.length / 3);

    return {
      vertices: uniqueVertices,
      faces,
      deduplicatedCount
    };
  }

  /**
   * Prunes degenerate zero-area and collapsed-edge triangles.
   * @param {Float32Array} vertices
   * @param {Uint32Array} faces
   */
  function filterDegenerateTriangles(vertices, faces) {
    const numTriangles = (faces.length / 3) | 0;
    const validFaces = [];
    let prunedCount = 0;

    const vertLen = (vertices.length / 3) | 0;

    for (let i = 0; i < numTriangles; i++) {
      const v0 = faces[i * 3];
      const v1 = faces[i * 3 + 1];
      const v2 = faces[i * 3 + 2];

      // Check 1: Collapsed edges (duplicate vertex indices)
      if (v0 === v1 || v1 === v2 || v2 === v0) {
        prunedCount++;
        continue;
      }

      if (v0 >= vertLen || v1 >= vertLen || v2 >= vertLen) {
        prunedCount++;
        continue;
      }

      // Check 2: Finite float values
      const p0x = vertices[v0 * 3], p0y = vertices[v0 * 3 + 1], p0z = vertices[v0 * 3 + 2];
      const p1x = vertices[v1 * 3], p1y = vertices[v1 * 3 + 1], p1z = vertices[v1 * 3 + 2];
      const p2x = vertices[v2 * 3], p2y = vertices[v2 * 3 + 1], p2z = vertices[v2 * 3 + 2];

      if (!Number.isFinite(p0x) || !Number.isFinite(p0y) || !Number.isFinite(p0z) ||
          !Number.isFinite(p1x) || !Number.isFinite(p1y) || !Number.isFinite(p1z) ||
          !Number.isFinite(p2x) || !Number.isFinite(p2y) || !Number.isFinite(p2z)) {
        prunedCount++;
        continue;
      }

      // Check 3: Edge vectors and cross product norm squared
      const e1x = p1x - p0x, e1y = p1y - p0y, e1z = p1z - p0z;
      const e2x = p2x - p0x, e2y = p2y - p0y, e2z = p2z - p0z;

      const cx = e1y * e2z - e1z * e2y;
      const cy = e1z * e2x - e1x * e2z;
      const cz = e1x * e2y - e1y * e2x;

      const normSq = cx * cx + cy * cy + cz * cz;
      if (normSq < AREA_EPSILON_SQ || !Number.isFinite(normSq)) {
        prunedCount++;
        continue;
      }

      validFaces.push(v0, v1, v2);
    }

    return {
      faces: new Uint32Array(validFaces),
      prunedCount
    };
  }

  /**
   * Builds directed edge map and extracts boundary edges (edges with no opposing twin).
   * @param {Uint32Array} faces
   */
  function extractBoundaryDirectedEdges(faces) {
    const numTriangles = (faces.length / 3) | 0;
    // Map directed edge "u->v" to count
    const edgeMap = new Map();

    for (let i = 0; i < numTriangles; i++) {
      const v0 = faces[i * 3];
      const v1 = faces[i * 3 + 1];
      const v2 = faces[i * 3 + 2];

      const edges = [[v0, v1], [v1, v2], [v2, v0]];
      for (let e = 0; e < 3; e++) {
        const u = edges[e][0], v = edges[e][1];
        const key = `${u}->${v}`;
        edgeMap.set(key, (edgeMap.get(key) || 0) + 1);
      }
    }

    // Boundary edges: u -> v where opposite v -> u does not exist
    const boundaryEdges = [];
    for (const [key, count] of edgeMap.entries()) {
      const parts = key.split('->');
      const u = parseInt(parts[0], 10);
      const v = parseInt(parts[1], 10);
      const oppKey = `${v}->${u}`;

      if (!edgeMap.has(oppKey)) {
        boundaryEdges.push({ u, v });
      }
    }

    return boundaryEdges;
  }

  /**
   * Chains boundary directed edges into closed oriented cycles.
   * For existing edge u -> v, the step to close the hole is v -> u.
   */
  function extractBoundaryLoops(boundaryEdges) {
    if (boundaryEdges.length === 0) return [];

    // Map: v -> Array of u
    const outgoing = new Map();
    for (const edge of boundaryEdges) {
      let targets = outgoing.get(edge.v);
      if (!targets) {
        targets = [];
        outgoing.set(edge.v, targets);
      }
      targets.push(edge.u);
    }

    const loops = [];

    while (outgoing.size > 0) {
      const startNode = outgoing.keys().next().value;
      const path = [];
      const nodeToPos = new Map();
      let curr = startNode;

      while (outgoing.has(curr)) {
        const targets = outgoing.get(curr);
        if (!targets || targets.length === 0) break;

        const nextNode = targets.pop();
        if (targets.length === 0) {
          outgoing.delete(curr);
        }

        path.push(curr);
        nodeToPos.set(curr, path.length - 1);

        if (nodeToPos.has(nextNode)) {
          const cycleStartIdx = nodeToPos.get(nextNode);
          const cycle = path.slice(cycleStartIdx);
          if (cycle.length >= 3) {
            loops.push(cycle);
          }

          for (let i = cycleStartIdx; i < path.length; i++) {
            nodeToPos.delete(path[i]);
          }
          path.length = cycleStartIdx;

          if (path.length === 0) break;
          curr = path[path.length - 1];
        } else {
          curr = nextNode;
        }
      }

      // Cleanup
      for (const [key, t] of outgoing.entries()) {
        if (t.length === 0) outgoing.delete(key);
      }
    }

    return loops;
  }

  /**
   * Triangulates a 3D closed polygon loop using Newell normal projection & ear clipping
   * with pivot fan fallback.
   */
  function triangulatePolygonLoop(loopVerts, vertices) {
    const n = loopVerts.length;
    if (n < 3) return [];
    if (n === 3) return [loopVerts[0], loopVerts[1], loopVerts[2]];

    // Step 1: Newell's normal
    let nx = 0, ny = 0, nz = 0;
    for (let i = 0; i < n; i++) {
      const cIdx = loopVerts[i] * 3;
      const nIdx = loopVerts[(i + 1) % n] * 3;
      const cx = vertices[cIdx], cy = vertices[cIdx + 1], cz = vertices[cIdx + 2];
      const nx_coord = vertices[nIdx], ny_coord = vertices[nIdx + 1], nz_coord = vertices[nIdx + 2];

      nx += (cy - ny_coord) * (cz + nz_coord);
      ny += (cz - nz_coord) * (cx + nx_coord);
      nz += (cx - nx_coord) * (cy + ny_coord);
    }

    // Step 2: Projection plane (drop maximum normal component)
    const ax = Math.abs(nx), ay = Math.abs(ny), az = Math.abs(nz);
    const project = (vIdx) => {
      const idx = vIdx * 3;
      const x = vertices[idx], y = vertices[idx + 1], z = vertices[idx + 2];
      if (az >= ax && az >= ay) return [x, y];
      if (ay >= ax && ay >= az) return [x, z];
      return [y, z];
    };

    // Step 3: Signed 2D area
    let area2d = 0;
    for (let i = 0; i < n; i++) {
      const p1 = project(loopVerts[i]);
      const p2 = project(loopVerts[(i + 1) % n]);
      area2d += p1[0] * p2[1] - p2[0] * p1[1];
    }
    const isCcw = area2d >= 0;

    // Ear clipping
    const remaining = [...loopVerts];
    const triangles = [];
    let stuckCount = 0;

    const isPointInTri = (p, a, b, c) => {
      const d1 = (p[0] - b[0]) * (a[1] - b[1]) - (a[0] - b[0]) * (p[1] - b[1]);
      const d2 = (p[0] - c[0]) * (b[1] - c[1]) - (b[0] - c[0]) * (p[1] - c[1]);
      const d3 = (p[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (p[1] - a[1]);
      const hasNeg = (d1 < 0) || (d2 < 0) || (d3 < 0);
      const hasPos = (d1 > 0) || (d2 > 0) || (d3 > 0);
      return !(hasNeg && hasPos);
    };

    while (remaining.length > 3 && stuckCount < remaining.length) {
      let earFound = false;
      const m = remaining.length;

      for (let i = 0; i < m; i++) {
        const prevIdx = remaining[(i + m - 1) % m];
        const currIdx = remaining[i];
        const nextIdx = remaining[(i + 1) % m];

        const a = project(prevIdx);
        const b = project(currIdx);
        const c = project(nextIdx);

        // 2D cross product: (b - a) x (c - b)
        const cross2d = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
        const isConvex = isCcw ? (cross2d > 1e-12) : (cross2d < -1e-12);
        if (!isConvex) continue;

        let containsOther = false;
        for (let j = 0; j < m; j++) {
          if (j === (i + m - 1) % m || j === i || j === (i + 1) % m) continue;
          if (isPointInTri(project(remaining[j]), a, b, c)) {
            containsOther = true;
            break;
          }
        }

        if (!containsOther) {
          triangles.push(prevIdx, currIdx, nextIdx);
          remaining.splice(i, 1);
          earFound = true;
          stuckCount = 0;
          break;
        }
      }

      if (!earFound) {
        stuckCount++;
      }
    }

    if (remaining.length === 3) {
      triangles.push(remaining[0], remaining[1], remaining[2]);
    } else if (remaining.length > 3) {
      // Fan triangulation fallback around pivot
      const pivot = remaining[0];
      for (let i = 1; i < remaining.length - 1; i++) {
        triangles.push(pivot, remaining[i], remaining[i + 1]);
      }
    }

    return triangles;
  }

  /**
   * Detects boundary loops and iteratively patches holes up to maxHoleEdges.
   * @param {Float32Array} vertices
   * @param {Uint32Array} faces
   * @param {number} [maxHoleEdges=64]
   */
  function patchBoundaryHoles(vertices, faces, maxHoleEdges = 64) {
    let currentFaces = Array.from(faces);
    let totalPatched = 0;

    let iterations = 0;
    while (iterations < 5) {
      iterations++;
      const boundaryEdges = extractBoundaryDirectedEdges(new Uint32Array(currentFaces));
      if (boundaryEdges.length === 0) break;

      const loops = extractBoundaryLoops(boundaryEdges);
      if (loops.length === 0) break;

      let patchedThisPass = 0;
      for (const loop of loops) {
        if (maxHoleEdges > 0 && loop.length > maxHoleEdges) continue;

        const patch = triangulatePolygonLoop(loop, vertices);
        if (patch.length > 0) {
          for (let p = 0; p < patch.length; p++) {
            currentFaces.push(patch[p]);
          }
          patchedThisPass++;
        }
      }

      if (patchedThisPass === 0) break;
      totalPatched += patchedThisPass;
    }

    return {
      faces: new Uint32Array(currentFaces),
      holesPatched: totalPatched
    };
  }

  /**
   * Topological winding unification (BFS) and outward normal reorientation (Divergence Theorem).
   * @param {Float32Array} vertices
   * @param {Uint32Array} faces
   */
  function unifyWindingAndOrientOutward(vertices, faces) {
    const numFaces = (faces.length / 3) | 0;
    if (numFaces === 0) {
      return { faces, invertedCount: 0, signedVolume: 0 };
    }

    const faceArr = new Uint32Array(faces);

    // Map: undirected edge "min,max" -> array of face indices
    const edgeToFaces = new Map();
    const undirKey = (u, v) => u < v ? `${u},${v}` : `${v},${u}`;

    for (let f = 0; f < numFaces; f++) {
      const v0 = faceArr[f * 3], v1 = faceArr[f * 3 + 1], v2 = faceArr[f * 3 + 2];
      const e01 = undirKey(v0, v1);
      const e12 = undirKey(v1, v2);
      const e20 = undirKey(v2, v0);

      if (!edgeToFaces.has(e01)) edgeToFaces.set(e01, []);
      if (!edgeToFaces.has(e12)) edgeToFaces.set(e12, []);
      if (!edgeToFaces.has(e20)) edgeToFaces.set(e20, []);

      edgeToFaces.get(e01).push(f);
      edgeToFaces.get(e12).push(f);
      edgeToFaces.get(e20).push(f);
    }

    const visited = new Uint8Array(numFaces);
    let invertedComponents = 0;
    let totalSignedVolume = 0;

    for (let seed = 0; seed < numFaces; seed++) {
      if (visited[seed]) continue;

      const componentFaces = [];
      const queue = [seed];
      visited[seed] = 1;

      // BFS winding unification
      let qHead = 0;
      while (qHead < queue.length) {
        const curr = queue[qHead++];
        componentFaces.push(curr);

        const c0 = faceArr[curr * 3], c1 = faceArr[curr * 3 + 1], c2 = faceArr[curr * 3 + 2];
        const directedPairs = [[c0, c1], [c1, c2], [c2, c0]];

        for (let e = 0; e < 3; e++) {
          const u = directedPairs[e][0], v = directedPairs[e][1];
          const k = undirKey(u, v);
          const neighbors = edgeToFaces.get(k);
          if (!neighbors) continue;

          for (let n = 0; n < neighbors.length; n++) {
            const adj = neighbors[n];
            if (visited[adj]) continue;

            const a0 = faceArr[adj * 3], a1 = faceArr[adj * 3 + 1], a2 = faceArr[adj * 3 + 2];
            // In curr, edge is u -> v. In consistent neighbor, shared edge must be v -> u.
            // If neighbor has u -> v, it must be flipped (swap a1 and a2).
            const hasSameDir = (a0 === u && a1 === v) ||
                               (a1 === u && a2 === v) ||
                               (a2 === u && a0 === v);

            if (hasSameDir) {
              faceArr[adj * 3 + 1] = a2;
              faceArr[adj * 3 + 2] = a1;
            }

            visited[adj] = 1;
            queue.push(adj);
          }
        }
      }

      // Check outward orientation for this component
      // 1. Check topological closedness (all edges incident to exactly 2 faces)
      const compEdgeCounts = new Map();
      for (let i = 0; i < componentFaces.length; i++) {
        const f = componentFaces[i];
        const v0 = faceArr[f * 3], v1 = faceArr[f * 3 + 1], v2 = faceArr[f * 3 + 2];
        const e1 = undirKey(v0, v1), e2 = undirKey(v1, v2), e3 = undirKey(v2, v0);
        compEdgeCounts.set(e1, (compEdgeCounts.get(e1) || 0) + 1);
        compEdgeCounts.set(e2, (compEdgeCounts.get(e2) || 0) + 1);
        compEdgeCounts.set(e3, (compEdgeCounts.get(e3) || 0) + 1);
      }

      let isClosed = componentFaces.length > 0;
      for (const count of compEdgeCounts.values()) {
        if (count !== 2) {
          isClosed = false;
          break;
        }
      }

      let mustFlip = false;
      let compSignedVolume = 0;

      if (isClosed) {
        // Gauss Divergence Theorem signed volume: V = 1/6 * sum( p0 . (p1 x p2) )
        let signedVol6x = 0;
        for (let i = 0; i < componentFaces.length; i++) {
          const f = componentFaces[i];
          const v0 = faceArr[f * 3] * 3;
          const v1 = faceArr[f * 3 + 1] * 3;
          const v2 = faceArr[f * 3 + 2] * 3;

          const p0x = vertices[v0], p0y = vertices[v0 + 1], p0z = vertices[v0 + 2];
          const p1x = vertices[v1], p1y = vertices[v1 + 1], p1z = vertices[v1 + 2];
          const p2x = vertices[v2], p2y = vertices[v2 + 1], p2z = vertices[v2 + 2];

          // Cross product p1 x p2
          const cx = p1y * p2z - p1z * p2y;
          const cy = p1z * p2x - p1x * p2z;
          const cz = p1x * p2y - p1y * p2x;

          // Dot with p0
          signedVol6x += (p0x * cx + p0y * cy + p0z * cz);
        }

        compSignedVolume = signedVol6x / 6;
        totalSignedVolume += compSignedVolume;

        if (Math.abs(signedVol6x) > 1e-9) {
          mustFlip = signedVol6x < 0;
        }
      } else {
        // Open/flat surface: Extreme Z ray test from center of mass
        let comX = 0, comY = 0, comZ = 0, vCount = 0;
        let maxZ = -Infinity;
        let maxF = componentFaces[0];

        for (let i = 0; i < componentFaces.length; i++) {
          const f = componentFaces[i];
          const v0 = faceArr[f * 3] * 3;
          const v1 = faceArr[f * 3 + 1] * 3;
          const v2 = faceArr[f * 3 + 2] * 3;

          const zAvg = (vertices[v0 + 2] + vertices[v1 + 2] + vertices[v2 + 2]) / 3;
          if (zAvg > maxZ) {
            maxZ = zAvg;
            maxF = f;
          }

          comX += vertices[v0] + vertices[v1] + vertices[v2];
          comY += vertices[v0 + 1] + vertices[v1 + 1] + vertices[v2 + 1];
          comZ += vertices[v0 + 2] + vertices[v1 + 2] + vertices[v2 + 2];
          vCount += 3;
        }

        if (vCount > 0) {
          comX /= vCount; comY /= vCount; comZ /= vCount;
        }

        const v0 = faceArr[maxF * 3] * 3;
        const v1 = faceArr[maxF * 3 + 1] * 3;
        const v2 = faceArr[maxF * 3 + 2] * 3;

        const p0x = vertices[v0], p0y = vertices[v0 + 1], p0z = vertices[v0 + 2];
        const p1x = vertices[v1], p1y = vertices[v1 + 1], p1z = vertices[v1 + 2];
        const p2x = vertices[v2], p2y = vertices[v2 + 1], p2z = vertices[v2 + 2];

        const centX = (p0x + p1x + p2x) / 3;
        const centY = (p0y + p1y + p2y) / 3;
        const centZ = (p0z + p1z + p2z) / 3;

        const rayX = centX - comX;
        const rayY = centY - comY;
        const rayZ = centZ - comZ;

        const e1x = p1x - p0x, e1y = p1y - p0y, e1z = p1z - p0z;
        const e2x = p2x - p0x, e2y = p2y - p0y, e2z = p2z - p0z;

        const normX = e1y * e2z - e1z * e2y;
        const normY = e1z * e2x - e1x * e2z;
        const normZ = e1x * e2y - e1y * e2x;

        const dot = rayX * normX + rayY * normY + rayZ * normZ;
        if (Math.abs(dot) > 1e-12) {
          mustFlip = dot < 0;
        } else {
          mustFlip = normZ < 0;
        }
      }

      if (mustFlip) {
        invertedComponents++;
        for (let i = 0; i < componentFaces.length; i++) {
          const f = componentFaces[i];
          const tmp = faceArr[f * 3 + 1];
          faceArr[f * 3 + 1] = faceArr[f * 3 + 2];
          faceArr[f * 3 + 2] = tmp;
        }
      }
    }

    return {
      faces: faceArr,
      invertedComponents,
      signedVolume: totalSignedVolume
    };
  }

  /**
   * Computes unit normals for all faces.
   */
  function computeFaceNormals(vertices, faces) {
    const numTriangles = (faces.length / 3) | 0;
    const normals = new Float32Array(numTriangles * 3);

    for (let i = 0; i < numTriangles; i++) {
      const v0 = faces[i * 3] * 3;
      const v1 = faces[i * 3 + 1] * 3;
      const v2 = faces[i * 3 + 2] * 3;

      const e1x = vertices[v1] - vertices[v0];
      const e1y = vertices[v1 + 1] - vertices[v0 + 1];
      const e1z = vertices[v1 + 2] - vertices[v0 + 2];

      const e2x = vertices[v2] - vertices[v0];
      const e2y = vertices[v2 + 1] - vertices[v0 + 1];
      const e2z = vertices[v2 + 2] - vertices[v0 + 2];

      let nx = e1y * e2z - e1z * e2y;
      let ny = e1z * e2x - e1x * e2z;
      let nz = e1x * e2y - e1y * e2x;

      const len = Math.hypot(nx, ny, nz);
      if (len > 1e-12) {
        nx /= len; ny /= len; nz /= len;
      } else {
        nx = 0; ny = 0; nz = 1;
      }

      normals[i * 3] = nx;
      normals[i * 3 + 1] = ny;
      normals[i * 3 + 2] = nz;
    }

    return normals;
  }

  /**
   * Complete client-side mesh repair pipeline:
   * 1. Spatial hash welding (10^-6 mm)
   * 2. Degenerate facet elimination
   * 3. Hole boundary extraction & healing (optional)
   * 4. BFS winding unification & Divergence Theorem outward normal reorientation
   * 5. Recalculate face normals
   */
  function repairMeshPipeline(positions, options = {}) {
    const tolerance = options.tolerance !== undefined ? options.tolerance : VERTEX_TOLERANCE_DEFAULT;
    const doRepair = options.repair !== undefined ? options.repair : true;
    const maxHoleEdges = options.maxHoleEdges !== undefined ? options.maxHoleEdges : 64;

    // 1. Deduplicate vertices
    const weld = weldVertices(positions, tolerance);
    let currentVertices = weld.vertices;
    let currentFaces = weld.faces;

    // 2. Filter degenerate facets
    const degen = filterDegenerateTriangles(currentVertices, currentFaces);
    currentFaces = degen.faces;

    let holesPatched = 0;
    if (doRepair) {
      // 3. Patch boundary holes
      const holeResult = patchBoundaryHoles(currentVertices, currentFaces, maxHoleEdges);
      currentFaces = holeResult.faces;
      holesPatched = holeResult.holesPatched;

      // 4. Unify winding and reorient outward
      const orientResult = unifyWindingAndOrientOutward(currentVertices, currentFaces);
      currentFaces = orientResult.faces;
    }

    // Recalculate normals
    const normals = computeFaceNormals(currentVertices, currentFaces);

    // Extract remaining boundary edges for visualization
    const boundaryEdges = extractBoundaryDirectedEdges(currentFaces);

    return {
      vertices: currentVertices,
      faces: currentFaces,
      normals,
      boundaryEdges,
      stats: {
        rawVertices: (positions.length / 3) | 0,
        uniqueVertices: (currentVertices.length / 3) | 0,
        deduplicatedCount: weld.deduplicatedCount,
        prunedDegenerates: degen.prunedCount,
        holesPatched,
        remainingBoundaryEdges: boundaryEdges.length,
        isWatertight: boundaryEdges.length === 0,
        triangles: (currentFaces.length / 3) | 0
      }
    };
  }

  return {
    SpatialHashGrid,
    weldVertices,
    filterDegenerateTriangles,
    extractBoundaryDirectedEdges,
    extractBoundaryLoops,
    patchBoundaryHoles,
    unifyWindingAndOrientOutward,
    computeFaceNormals,
    repairMeshPipeline,
    VERTEX_TOLERANCE_DEFAULT
  };
}));
