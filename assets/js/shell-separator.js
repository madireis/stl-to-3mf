/**
 * StlTo3mf — Client-Side Multi-Shell Separator
 * Topological connectivity graph clustering via Disjoint Set Union (DSU).
 * Partitions compound meshes into discrete shells with compact re-indexing
 * and per-shell bounding boxes, center-of-mass, and divergence signed volume.
 */
(function (root, factory) {
  if (typeof define === 'function' && define.amd) {
    define([], factory);
  } else if (typeof module === 'object' && module.exports) {
    module.exports = factory();
  } else {
    root.ShellSeparator = factory();
  }
}(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  /**
   * Disjoint Set Union (DSU) with path compression and union by rank.
   */
  class DSU {
    constructor(size) {
      this.parent = new Int32Array(size);
      this.rank = new Uint8Array(size);
      for (let i = 0; i < size; i++) {
        this.parent[i] = i;
      }
    }

    find(i) {
      let root = i;
      while (root !== this.parent[root]) {
        root = this.parent[root];
      }
      let curr = i;
      while (curr !== root) {
        const nxt = this.parent[curr];
        this.parent[curr] = root;
        curr = nxt;
      }
      return root;
    }

    union(i, j) {
      const rootI = this.find(i);
      const rootJ = this.find(j);
      if (rootI === rootJ) return;

      if (this.rank[rootI] < this.rank[rootJ]) {
        this.parent[rootI] = rootJ;
      } else if (this.rank[rootI] > this.rank[rootJ]) {
        this.parent[rootJ] = rootI;
      } else {
        this.parent[rootJ] = rootI;
        this.rank[rootI]++;
      }
    }
  }

  /**
   * Separates an indexed mesh into discrete connected shells using edge adjacency.
   * @param {Float32Array} vertices
   * @param {Uint32Array} faces
   * @param {boolean} [splitShells=true]
   * @returns {Array<{
   *   id: number,
   *   name: string,
   *   vertices: Float32Array,
   *   faces: Uint32Array,
   *   bbox: [number, number, number, number, number, number],
   *   dimensions: [number, number, number],
   *   centerOfMass: [number, number, number],
   *   signedVolume: number,
   *   surfaceArea: number,
   *   isWatertight: boolean
   * }>}
   */
  function separateShells(vertices, faces, splitShells = true) {
    const numFaces = (faces.length / 3) | 0;
    const numVerts = (vertices.length / 3) | 0;

    if (numFaces === 0 || numVerts === 0) {
      return [];
    }

    if (!splitShells) {
      // Single shell containing everything
      return [extractSingleShell(1, 'Model', vertices, faces)];
    }

    // Step 1: DSU clustering by shared undirected edges
    const dsu = new DSU(numFaces);
    const edgeToFace = new Map();
    const undirKey = (u, v) => u < v ? `${u},${v}` : `${v},${u}`;

    for (let f = 0; f < numFaces; f++) {
      const v0 = faces[f * 3];
      const v1 = faces[f * 3 + 1];
      const v2 = faces[f * 3 + 2];

      const edges = [[v0, v1], [v1, v2], [v2, v0]];
      for (let e = 0; e < 3; e++) {
        const k = undirKey(edges[e][0], edges[e][1]);
        const existing = edgeToFace.get(k);
        if (existing !== undefined) {
          dsu.union(f, existing);
        } else {
          edgeToFace.set(k, f);
        }
      }
    }

    // Group face indices by root
    const compMap = new Map();
    for (let f = 0; f < numFaces; f++) {
      const root = dsu.find(f);
      let list = compMap.get(root);
      if (!list) {
        list = [];
        compMap.set(root, list);
      }
      list.push(f);
    }

    // Sort components by triangle count descending
    const sortedComps = Array.from(compMap.values()).sort((a, b) => b.length - a.length);

    // Step 2: Build discrete shells with compact re-indexing
    const shells = [];
    for (let i = 0; i < sortedComps.length; i++) {
      const compFaces = sortedComps[i];
      const shellId = i + 1;
      const shellName = `Shell_${shellId}`;
      shells.push(extractComponentMesh(shellId, shellName, vertices, faces, compFaces));
    }

    return shells;
  }

  /**
   * Extracts a single shell from selected face indices with compact 0-based re-indexing.
   */
  function extractComponentMesh(id, name, allVertices, allFaces, faceIndices) {
    const totalVerts = (allVertices.length / 3) | 0;
    const referenced = new Uint8Array(totalVerts);

    for (let i = 0; i < faceIndices.length; i++) {
      const f = faceIndices[i];
      referenced[allFaces[f * 3]] = 1;
      referenced[allFaces[f * 3 + 1]] = 1;
      referenced[allFaces[f * 3 + 2]] = 1;
    }

    // Compact re-index
    const oldToNew = new Int32Array(totalVerts).fill(-1);
    const newVertsList = [];

    for (let v = 0; v < totalVerts; v++) {
      if (referenced[v]) {
        oldToNew[v] = (newVertsList.length / 3) | 0;
        newVertsList.push(allVertices[v * 3], allVertices[v * 3 + 1], allVertices[v * 3 + 2]);
      }
    }

    const shellVertices = new Float32Array(newVertsList);
    const shellFaces = new Uint32Array(faceIndices.length * 3);

    for (let i = 0; i < faceIndices.length; i++) {
      const f = faceIndices[i];
      shellFaces[i * 3] = oldToNew[allFaces[f * 3]];
      shellFaces[i * 3 + 1] = oldToNew[allFaces[f * 3 + 1]];
      shellFaces[i * 3 + 2] = oldToNew[allFaces[f * 3 + 2]];
    }

    return computeShellMetrics(id, name, shellVertices, shellFaces);
  }

  /**
   * Treats entire mesh as a single shell.
   */
  function extractSingleShell(id, name, vertices, faces) {
    return computeShellMetrics(id, name, vertices, faces);
  }

  /**
   * Computes bounding box, center of mass, signed volume, surface area, and watertightness.
   */
  function computeShellMetrics(id, name, vertices, faces) {
    const numVerts = (vertices.length / 3) | 0;
    const numFaces = (faces.length / 3) | 0;

    let minX = Infinity, minY = Infinity, minZ = Infinity;
    let maxX = -Infinity, maxY = -Infinity, maxZ = -Infinity;
    let comX = 0, comY = 0, comZ = 0;

    for (let i = 0; i < numVerts; i++) {
      const x = vertices[i * 3], y = vertices[i * 3 + 1], z = vertices[i * 3 + 2];
      if (x < minX) minX = x;
      if (x > maxX) maxX = x;
      if (y < minY) minY = y;
      if (y > maxY) maxY = y;
      if (z < minZ) minZ = z;
      if (z > maxZ) maxZ = z;

      comX += x; comY += y; comZ += z;
    }

    if (numVerts > 0) {
      comX /= numVerts; comY /= numVerts; comZ /= numVerts;
    } else {
      minX = minY = minZ = maxX = maxY = maxZ = 0;
    }

    let signedVol6x = 0;
    let surfaceArea = 0;
    const undirEdgeCounts = new Map();
    const undirKey = (u, v) => u < v ? `${u},${v}` : `${v},${u}`;

    for (let f = 0; f < numFaces; f++) {
      const v0 = faces[f * 3] * 3;
      const v1 = faces[f * 3 + 1] * 3;
      const v2 = faces[f * 3 + 2] * 3;

      const p0x = vertices[v0], p0y = vertices[v0 + 1], p0z = vertices[v0 + 2];
      const p1x = vertices[v1], p1y = vertices[v1 + 1], p1z = vertices[v1 + 2];
      const p2x = vertices[v2], p2y = vertices[v2 + 1], p2z = vertices[v2 + 2];

      // Edge cross product
      const e1x = p1x - p0x, e1y = p1y - p0y, e1z = p1z - p0z;
      const e2x = p2x - p0x, e2y = p2y - p0y, e2z = p2z - p0z;

      const cx = e1y * e2z - e1z * e2y;
      const cy = e1z * e2x - e1x * e2z;
      const cz = e1x * e2y - e1y * e2x;

      surfaceArea += 0.5 * Math.hypot(cx, cy, cz);

      // Signed volume via Divergence Theorem: p0 . (p1 x p2)
      const px = p1y * p2z - p1z * p2y;
      const py = p1z * p2x - p1x * p2z;
      const pz = p1x * p2y - p1y * p2x;
      signedVol6x += (p0x * px + p0y * py + p0z * pz);

      // Edge counts
      const u0 = faces[f * 3], u1 = faces[f * 3 + 1], u2 = faces[f * 3 + 2];
      const e01 = undirKey(u0, u1), e12 = undirKey(u1, u2), e20 = undirKey(u2, u0);
      undirEdgeCounts.set(e01, (undirEdgeCounts.get(e01) || 0) + 1);
      undirEdgeCounts.set(e12, (undirEdgeCounts.get(e12) || 0) + 1);
      undirEdgeCounts.set(e20, (undirEdgeCounts.get(e20) || 0) + 1);
    }

    let isWatertight = numFaces > 0;
    for (const count of undirEdgeCounts.values()) {
      if (count !== 2) {
        isWatertight = false;
        break;
      }
    }

    const dx = Math.max(0, maxX - minX);
    const dy = Math.max(0, maxY - minY);
    const dz = Math.max(0, maxZ - minZ);

    return {
      id,
      name,
      vertices,
      faces,
      bbox: [minX, minY, minZ, maxX, maxY, maxZ],
      dimensions: [dx, dy, dz],
      centerOfMass: [comX, comY, comZ],
      signedVolume: signedVol6x / 6,
      surfaceArea,
      isWatertight
    };
  }

  return {
    DSU,
    separateShells
  };
}));
