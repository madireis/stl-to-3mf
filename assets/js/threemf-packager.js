/**
 * StlTo3mf — Client-Side 3MF Container Packager
 * Pure JavaScript OPC/ZIP archive builder conforming to 3MF Core Specification v1.x.
 * Packages multi-shell geometry XML, relationships, content-types, slicer profile sanitization,
 * and 256x256 embedded thumbnail using fflate.
 */
(function (root, factory) {
  if (typeof define === 'function' && define.amd) {
    define(['fflate'], factory);
  } else if (typeof module === 'object' && module.exports) {
    const fflate = require('../vendor/fflate.min.js');
    module.exports = factory(fflate);
  } else {
    root.ThreeMfPackager = factory(root.fflate);
  }
}(typeof self !== 'undefined' ? self : this, function (fflate) {
  'use strict';

  const COLOR_PALETTE_HEX = [
    '#00A3E0FF', // Cyan
    '#FF7A00FF', // Orange
    '#2ECC71FF', // Green
    '#9B59B6FF', // Purple
    '#E74C3CFF', // Red
    '#F1C40FFF', // Yellow
    '#1ABC9CFF', // Turquoise
    '#E67E22FF', // Carrot
  ];

  const CONTENT_TYPES_XML = `<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/>
  <Default Extension="png" ContentType="image/png"/>
</Types>
`;

  const ROOT_RELS_WITH_THUMB = `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rel-model" Target="/3D/3dmodel.model" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/>
  <Relationship Id="rel-thumb" Target="/Metadata/thumbnail.png" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/thumbnail"/>
</Relationships>
`;

  const ROOT_RELS_NO_THUMB = `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rel-model" Target="/3D/3dmodel.model" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/>
</Relationships>
`;

  const MODEL_RELS_XML = `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rel-thumb" Target="/Metadata/thumbnail.png" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/thumbnail"/>
</Relationships>
`;

  /**
   * Generates 3D/3dmodel.model XML for an array of shells.
   * @param {Array<Object>} shells
   * @param {Object} [options]
   * @param {number} [options.scale=1.0]
   * @param {boolean} [options.stripSlicerConfig=false]
   * @returns {string} XML string
   */
  function generateModelXml(shells, options = {}) {
    const scale = Number(options.scale) > 0 ? Number(options.scale) : 1.0;
    const title = options.title || 'StlTo3mf Model';

    let xml = `<?xml version="1.0" encoding="UTF-8"?>\n`;
    xml += `<model unit="millimeter" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02" xmlns:m="http://schemas.microsoft.com/3dmanufacturing/material/2015/02">\n`;
    xml += `  <metadata name="Application">StlTo3mf Studio / Omni3MF Engine</metadata>\n`;
    xml += `  <metadata name="Title">${escapeXml(title)}</metadata>\n`;

    // Resources
    xml += `  <resources>\n`;
    xml += `    <m:colorgroup id="1">\n`;
    for (let c = 0; c < COLOR_PALETTE_HEX.length; c++) {
      xml += `      <m:color color="${COLOR_PALETTE_HEX[c]}"/>\n`;
    }
    xml += `    </m:colorgroup>\n`;

    // Objects per shell
    for (let s = 0; s < shells.length; s++) {
      const shell = shells[s];
      const objectId = shell.id + 1;
      const pindex = (s % COLOR_PALETTE_HEX.length);
      const name = shell.name || `Shell_${shell.id}`;

      xml += `    <object id="${objectId}" type="model" name="${escapeXml(name)}" pid="1" pindex="${pindex}">\n`;
      xml += `      <mesh>\n`;
      xml += `        <vertices>\n`;

      const verts = shell.vertices;
      const vCount = (verts.length / 3) | 0;
      for (let v = 0; v < vCount; v++) {
        const x = (verts[v * 3] * scale).toFixed(6);
        const y = (verts[v * 3 + 1] * scale).toFixed(6);
        const z = (verts[v * 3 + 2] * scale).toFixed(6);
        xml += `          <vertex x="${x}" y="${y}" z="${z}"/>\n`;
      }
      xml += `        </vertices>\n`;

      xml += `        <triangles>\n`;
      const faces = shell.faces;
      const tCount = (faces.length / 3) | 0;
      for (let t = 0; t < tCount; t++) {
        const v1 = faces[t * 3];
        const v2 = faces[t * 3 + 1];
        const v3 = faces[t * 3 + 2];
        xml += `          <triangle v1="${v1}" v2="${v2}" v3="${v3}"/>\n`;
      }
      xml += `        </triangles>\n`;
      xml += `      </mesh>\n`;
      xml += `    </object>\n`;
    }

    xml += `  </resources>\n`;

    // Build items
    xml += `  <build>\n`;
    for (let s = 0; s < shells.length; s++) {
      const objectId = shells[s].id + 1;
      xml += `    <item objectid="${objectId}"/>\n`;
    }
    xml += `  </build>\n`;
    xml += `</model>\n`;

    return xml;
  }

  function escapeXml(str) {
    return String(str)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&apos;');
  }

  /**
   * Packages shells into a valid 3MF ZIP archive.
   * @param {Array<Object>} shells
   * @param {Object} [options]
   * @param {number} [options.scale=1.0]
   * @param {boolean} [options.stripSlicerConfig=false]
   * @param {Uint8Array} [options.thumbnailPng]
   * @returns {Uint8Array} Zipped 3MF byte buffer
   */
  function package3mf(shells, options = {}) {
    const ffl = fflate || (typeof window !== 'undefined' ? window.fflate : null);
    if (!ffl) {
      throw new Error('fflate ZIP library not loaded');
    }

    const modelXmlStr = generateModelXml(shells, options);
    const modelXmlBytes = ffl.strToU8(modelXmlStr);

    const hasThumbnail = Boolean(options.thumbnailPng && options.thumbnailPng.length > 0);

    const zipFiles = {
      '[Content_Types].xml': ffl.strToU8(CONTENT_TYPES_XML),
      '_rels/.rels': ffl.strToU8(hasThumbnail ? ROOT_RELS_WITH_THUMB : ROOT_RELS_NO_THUMB),
      '3D/3dmodel.model': modelXmlBytes
    };

    if (hasThumbnail) {
      zipFiles['Metadata/thumbnail.png'] = options.thumbnailPng;
      zipFiles['3D/_rels/3dmodel.model.rels'] = ffl.strToU8(MODEL_RELS_XML);
    }

    // If options.extraFiles provided (and not stripped)
    if (!options.stripSlicerConfig && options.extraFiles) {
      for (const [filePath, content] of Object.entries(options.extraFiles)) {
        zipFiles[filePath] = content instanceof Uint8Array ? content : ffl.strToU8(content);
      }
    }

    // Zip with standard deflate compression
    return ffl.zipSync(zipFiles, { level: 6 });
  }

  /**
   * Helper to trigger browser download of a 3MF Uint8Array.
   */
  function download3mf(bytes, filename = 'model.3mf') {
    const blob = new Blob([bytes], { type: 'application/vnd.ms-package.3dmanufacturing-3dmodel+xml' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename.endsWith('.3mf') ? filename : `${filename}.3mf`;
    document.body.appendChild(a);
    a.click();
    setTimeout(() => {
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    }, 100);
  }

  return {
    generateModelXml,
    package3mf,
    download3mf,
    COLOR_PALETTE_HEX
  };
}));
