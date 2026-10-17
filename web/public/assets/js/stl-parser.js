/**
 * StlTo3mf — Client-Side STL Parser
 * Fast, zero-copy binary & streaming ASCII STL parser for browser and Node.js.
 * Produces TypedArray vertex coordinates with format auto-detection.
 */
(function (root, factory) {
  if (typeof define === 'function' && define.amd) {
    define([], factory);
  } else if (typeof module === 'object' && module.exports) {
    module.exports = factory();
  } else {
    root.StlParser = factory();
  }
}(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  /**
   * Deterministically detects whether an ArrayBuffer/Uint8Array is binary or ASCII STL.
   * @param {ArrayBuffer|Uint8Array} input
   * @returns {{ format: 'binary'|'ascii', facetCount?: number }}
   */
  function detectStlFormat(input) {
    const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
    const len = bytes.byteLength;

    if (len < 84) {
      if (isAsciiStl(bytes)) {
        return { format: 'ascii' };
      }
      throw new Error(`File too small to be a valid STL (${len} bytes)`);
    }

    // Read uint32 facet count at offset 80 (little endian)
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const facetCount = view.getUint32(80, true);

    // Check 1: Exact binary length formula: 84 + 50 * facetCount
    const expectedLen = 84 + facetCount * 50;
    if (expectedLen === len) {
      return { format: 'binary', facetCount };
    }

    // Check 2: Check for ASCII patterns
    if (isAsciiStl(bytes)) {
      return { format: 'ascii' };
    }

    // Check 3: Binary with trailing metadata or padding
    if (facetCount > 0 && len >= expectedLen) {
      return { format: 'binary', facetCount };
    }

    // Fallback: If header starts with 'solid' or is printable ASCII
    if (isAsciiStl(bytes, true)) {
      return { format: 'ascii' };
    }

    // Default to binary if non-empty
    if (facetCount > 0) {
      return { format: 'binary', facetCount };
    }

    throw new Error('Unrecognized STL format: buffer does not match binary or ASCII specification');
  }

  /**
   * Inspects bytes to determine if content is ASCII STL.
   */
  function isAsciiStl(bytes, relaxed = false) {
    const sampleLen = Math.min(bytes.length, 1024);
    for (let i = 0; i < sampleLen; i++) {
      if (bytes[i] === 0) return false; // Null byte indicates binary
    }

    const decoder = new TextDecoder('utf-8', { fatal: false });
    const text = decoder.decode(bytes.subarray(0, sampleLen)).toLowerCase();

    const hasSolid = text.includes('solid');
    if (relaxed && hasSolid) return true;

    return hasSolid && (
      text.includes('facet') ||
      text.includes('vertex') ||
      text.includes('endsolid') ||
      text.trim().startsWith('solid')
    );
  }

  /**
   * Parses binary STL into TypedArrays.
   * @param {ArrayBuffer|Uint8Array} input
   * @param {number} [facetCountHint]
   */
  function parseBinaryStl(input, facetCountHint) {
    const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);

    const facetCount = facetCountHint !== undefined ? facetCountHint : view.getUint32(80, true);
    const minRequired = 84 + facetCount * 50;
    if (bytes.byteLength < minRequired) {
      throw new Error(`Incomplete binary STL: expected at least ${minRequired} bytes, found ${bytes.byteLength}`);
    }

    // Read 80-byte header
    const headerBytes = bytes.subarray(0, 80);
    const decoder = new TextDecoder('ascii', { fatal: false });
    const header = decoder.decode(headerBytes).replace(/\0/g, '').trim();

    // Allocate flat TypedArrays: 9 floats per triangle (3 vertices * 3 coords)
    const positions = new Float32Array(facetCount * 9);
    const normals = new Float32Array(facetCount * 3);

    let srcOffset = 84;
    let posOffset = 0;
    let normOffset = 0;

    for (let i = 0; i < facetCount; i++) {
      // Normal vector (3x float32)
      normals[normOffset++] = view.getFloat32(srcOffset, true);
      normals[normOffset++] = view.getFloat32(srcOffset + 4, true);
      normals[normOffset++] = view.getFloat32(srcOffset + 8, true);

      // Vertex 0 (x, y, z)
      positions[posOffset++] = view.getFloat32(srcOffset + 12, true);
      positions[posOffset++] = view.getFloat32(srcOffset + 16, true);
      positions[posOffset++] = view.getFloat32(srcOffset + 20, true);

      // Vertex 1 (x, y, z)
      positions[posOffset++] = view.getFloat32(srcOffset + 24, true);
      positions[posOffset++] = view.getFloat32(srcOffset + 28, true);
      positions[posOffset++] = view.getFloat32(srcOffset + 32, true);

      // Vertex 2 (x, y, z)
      positions[posOffset++] = view.getFloat32(srcOffset + 36, true);
      positions[posOffset++] = view.getFloat32(srcOffset + 40, true);
      positions[posOffset++] = view.getFloat32(srcOffset + 44, true);

      srcOffset += 50; // Skip 2 attribute bytes
    }

    return {
      format: 'binary',
      triangleCount: facetCount,
      positions,
      normals,
      header
    };
  }

  /**
   * Parses ASCII STL into TypedArrays.
   * Handles arbitrary whitespace, scientifically formatted floating-point numbers.
   * @param {ArrayBuffer|Uint8Array|string} input
   */
  function parseAsciiStl(input) {
    let text;
    if (typeof input === 'string') {
      text = input;
    } else {
      const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
      text = new TextDecoder('utf-8').decode(bytes);
    }

    // Extract solid name
    let header = 'STL Model';
    const solidMatch = text.match(/solid\s+([^\r\n]+)/i);
    if (solidMatch && solidMatch[1]) {
      header = solidMatch[1].trim();
    }

    const posList = [];
    const normList = [];

    // Match all facets
    // Pattern: facet normal nx ny nz ... endfacet
    const facetRegex = /facet\s+normal\s+([^\r\n]+)[\s\S]*?outer\s+loop[\s\S]*?vertex\s+([^\r\n]+)[\s\S]*?vertex\s+([^\r\n]+)[\s\S]*?vertex\s+([^\r\n]+)[\s\S]*?endloop[\s\S]*?endfacet/gi;

    let match;
    while ((match = facetRegex.exec(text)) !== null) {
      // Normals
      const normParts = match[1].trim().split(/\s+/).map(Number);
      normList.push(normParts[0] || 0, normParts[1] || 0, normParts[2] || 0);

      // Vertices v0, v1, v2
      for (let v = 2; v <= 4; v++) {
        const vParts = match[v].trim().split(/\s+/).map(Number);
        posList.push(vParts[0] || 0, vParts[1] || 0, vParts[2] || 0);
      }
    }

    const triangleCount = (posList.length / 9) | 0;
    return {
      format: 'ascii',
      triangleCount,
      positions: new Float32Array(posList),
      normals: new Float32Array(normList),
      header
    };
  }

  /**
   * High-level entrypoint: detects format and parses STL buffer.
   * @param {ArrayBuffer|Uint8Array} buffer
   */
  function parseStl(buffer) {
    const detection = detectStlFormat(buffer);
    if (detection.format === 'binary') {
      return parseBinaryStl(buffer, detection.facetCount);
    } else {
      return parseAsciiStl(buffer);
    }
  }

  return {
    detectStlFormat,
    parseBinaryStl,
    parseAsciiStl,
    parseStl
  };
}));
