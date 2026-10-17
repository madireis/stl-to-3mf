/**
 * StlTo3mf Studio — Application Controller
 * Coordinates client-side STL ingestion, geometry repair, DSU shell separation,
 * Three.js 3D viewport, UI controls, scale sanity alerts, and 3MF packaging.
 */
(function () {
  'use strict';

  // DOM Elements
  const dropZone = document.getElementById('dropZone');
  const fileInput = document.getElementById('fileInput');
  const fileNameDisplay = document.getElementById('fileNameDisplay');
  const statusBox = document.getElementById('statusBox');
  const scaleWarningBox = document.getElementById('scaleWarningBox');
  const btnConvert = document.getElementById('btnConvert');
  const btnResetCamera = document.getElementById('btnResetCamera');
  const btnToggleWireframe = document.getElementById('btnToggleWireframe');
  const btnToggleHoles = document.getElementById('btnToggleHoles');

  // Toggles & Inputs
  const splitShellsCheckbox = document.getElementById('splitShells');
  const repairMeshCheckbox = document.getElementById('repairMesh');
  const stripConfigCheckbox = document.getElementById('stripConfig');
  const genThumbnailCheckbox = document.getElementById('genThumbnail');
  const scaleFactorInput = document.getElementById('scaleFactor');

  // Stats Elements
  const statsCard = document.getElementById('statsCard');
  const statFormat = document.getElementById('statFormat');
  const statTriangles = document.getElementById('statTriangles');
  const statVertices = document.getElementById('statVertices');
  const statShells = document.getElementById('statShells');
  const statWatertight = document.getElementById('statWatertight');
  const statDimensions = document.getElementById('statDimensions');
  const statVolume = document.getElementById('statVolume');
  const statDeduplicated = document.getElementById('statDeduplicated');
  const statPruned = document.getElementById('statPruned');
  const statHolesPatched = document.getElementById('statHolesPatched');
  const shellDetailsList = document.getElementById('shellDetailsList');

  // State
  let currentFile = null;
  let rawStlBuffer = null;
  let parsedStl = null;
  let repairedMesh = null;
  let separatedShells = null;
  let viewer = null;
  let wireframeState = false;
  let holesVisibleState = true;

  // Initialize 3D Viewport
  function initViewport() {
    const container = document.getElementById('viewport3d');
    if (!container) return;
    try {
      viewer = new Viewer3D(container);
    } catch (err) {
      console.error('Failed to initialize 3D viewer:', err);
      showStatus('WebGL 3D preview unavailable: ' + err.message, 'warning');
    }
  }

  // Status message helper
  function showStatus(text, type = 'info') {
    if (!statusBox) return;
    statusBox.textContent = text;
    statusBox.className = `status-box ${type}`;
    statusBox.style.display = 'block';
  }

  function hideStatus() {
    if (statusBox) statusBox.style.display = 'none';
  }

  // Setup Event Listeners
  function initEvents() {
    // Dropzone
    dropZone.addEventListener('click', () => fileInput.click());
    dropZone.addEventListener('dragover', (e) => {
      e.preventDefault();
      dropZone.classList.add('dragover');
    });
    dropZone.addEventListener('dragleave', () => dropZone.classList.remove('dragover'));
    dropZone.addEventListener('drop', (e) => {
      e.preventDefault();
      dropZone.classList.remove('dragover');
      if (e.dataTransfer.files.length > 0) {
        handleFileSelect(e.dataTransfer.files[0]);
      }
    });

    fileInput.addEventListener('change', (e) => {
      if (e.target.files.length > 0) {
        handleFileSelect(e.target.files[0]);
      }
    });

    // Conversion & Download
    btnConvert.addEventListener('click', onConvertAndDownload);

    // Viewport control buttons
    if (btnResetCamera) {
      btnResetCamera.addEventListener('click', () => {
        if (viewer) viewer.resetCamera();
      });
    }

    if (btnToggleWireframe) {
      btnToggleWireframe.addEventListener('click', () => {
        wireframeState = !wireframeState;
        if (viewer) viewer.setWireframe(wireframeState);
        btnToggleWireframe.classList.toggle('active', wireframeState);
      });
    }

    if (btnToggleHoles) {
      btnToggleHoles.addEventListener('click', () => {
        holesVisibleState = !holesVisibleState;
        if (viewer) viewer.setHolesVisible(holesVisibleState);
        btnToggleHoles.classList.toggle('active', holesVisibleState);
      });
    }

    // Option toggles re-trigger pipeline on already loaded file
    const reprocessInputs = [splitShellsCheckbox, repairMeshCheckbox, scaleFactorInput];
    reprocessInputs.forEach(el => {
      if (el) {
        el.addEventListener('change', () => {
          if (parsedStl) processGeometry();
        });
      }
    });
  }

  // Handle file selection
  function handleFileSelect(file) {
    if (!file.name.toLowerCase().endsWith('.stl')) {
      showStatus('Please select a valid .stl file', 'error');
      return;
    }

    currentFile = file;
    fileNameDisplay.textContent = `${file.name} (${(file.size / (1024 * 1024)).toFixed(2)} MB)`;
    showStatus('Reading STL file into memory...', 'info');

    const reader = new FileReader();
    reader.onload = function (e) {
      rawStlBuffer = e.target.result;
      try {
        parseAndProcess();
      } catch (err) {
        console.error(err);
        showStatus('Error parsing STL: ' + err.message, 'error');
      }
    };
    reader.onerror = function () {
      showStatus('Error reading file from disk', 'error');
    };
    reader.readAsArrayBuffer(file);
  }

  // Ingest & run pipeline
  function parseAndProcess() {
    showStatus('Parsing STL binary/ASCII geometry...', 'info');

    // 1. Ingest via StlParser
    const t0 = performance.now();
    parsedStl = StlParser.parseStl(rawStlBuffer);
    const tParse = (performance.now() - t0).toFixed(1);

    processGeometry(tParse);
  }

  function processGeometry(tParse = '0') {
    if (!parsedStl) return;

    showStatus('Executing mesh repair (spatial hash welding, hole healing, normal flipping)...', 'info');

    const doRepair = repairMeshCheckbox.checked;
    const doSplit = splitShellsCheckbox.checked;
    const userScale = parseFloat(scaleFactorInput.value) || 1.0;

    // 2. Repair pipeline
    const t0 = performance.now();
    repairedMesh = MeshRepair.repairMeshPipeline(parsedStl.positions, {
      repair: doRepair,
      tolerance: MeshRepair.VERTEX_TOLERANCE_DEFAULT
    });

    // 3. Multi-shell separation
    separatedShells = ShellSeparator.separateShells(repairedMesh.vertices, repairedMesh.faces, doSplit);
    const tProcess = (performance.now() - t0).toFixed(1);

    // 4. Update 3D Viewport
    if (viewer) {
      viewer.loadModel({
        shells: separatedShells,
        vertices: repairedMesh.vertices,
        boundaryEdges: repairedMesh.boundaryEdges,
        showHoles: holesVisibleState
      });
      if (wireframeState) viewer.setWireframe(true);
    }

    // 5. Update UI stats & scale check
    updateStatsDisplay(tParse, tProcess);
    checkScaleSanity(userScale);

    btnConvert.disabled = false;
    showStatus(`Processing complete in ${(Number(tParse) + Number(tProcess)).toFixed(1)} ms. Ready to export!`, 'success');
  }

  // Update metrics card
  function updateStatsDisplay(tParse, tProcess) {
    if (!repairedMesh || !separatedShells) return;

    statsCard.style.display = 'block';

    statFormat.textContent = parsedStl.format.toUpperCase();
    statTriangles.textContent = repairedMesh.stats.triangles.toLocaleString();
    statVertices.textContent = repairedMesh.stats.uniqueVertices.toLocaleString();
    statShells.textContent = separatedShells.length;

    statWatertight.textContent = repairedMesh.stats.isWatertight ? 'Yes (Solid 2-Manifold)' : `No (${repairedMesh.stats.remainingBoundaryEdges} open edges)`;
    statWatertight.className = repairedMesh.stats.isWatertight ? 'badge badge-success' : 'badge badge-warning';

    statDeduplicated.textContent = repairedMesh.stats.deduplicatedCount.toLocaleString();
    statPruned.textContent = repairedMesh.stats.prunedDegenerates.toLocaleString();
    statHolesPatched.textContent = repairedMesh.stats.holesPatched;

    // Collective bounding box and volume
    let totalVol = 0;
    let minX = Infinity, minY = Infinity, minZ = Infinity;
    let maxX = -Infinity, maxY = -Infinity, maxZ = -Infinity;

    separatedShells.forEach(s => {
      totalVol += s.signedVolume;
      minX = Math.min(minX, s.bbox[0]);
      minY = Math.min(minY, s.bbox[1]);
      minZ = Math.min(minZ, s.bbox[2]);
      maxX = Math.max(maxX, s.bbox[3]);
      maxY = Math.max(maxY, s.bbox[4]);
      maxZ = Math.max(maxZ, s.bbox[5]);
    });

    const dx = Math.max(0, maxX - minX).toFixed(2);
    const dy = Math.max(0, maxY - minY).toFixed(2);
    const dz = Math.max(0, maxZ - minZ).toFixed(2);

    statDimensions.textContent = `${dx} × ${dy} × ${dz} mm`;
    const volCm3 = (totalVol / 1000).toFixed(2);
    statVolume.textContent = `${totalVol.toFixed(1)} mm³ (${volCm3} cm³)`;

    // Populate shell details list
    shellDetailsList.innerHTML = '';
    separatedShells.forEach((shell, idx) => {
      const li = document.createElement('li');
      li.className = 'shell-item';
      const colorHex = ThreeMfPackager.COLOR_PALETTE_HEX[idx % ThreeMfPackager.COLOR_PALETTE_HEX.length];
      li.innerHTML = `
        <span class="shell-color-swatch" style="background-color: ${colorHex}"></span>
        <strong>${shell.name}</strong>: ${(shell.faces.length / 3).toLocaleString()} triangles &bull;
        ${(shell.signedVolume).toFixed(1)} mm³ &bull;
        ${shell.isWatertight ? '<span class="text-success">watertight</span>' : '<span class="text-warning">open mesh</span>'}
      `;
      shellDetailsList.appendChild(li);
    });
  }

  // Auto scale sanity check
  function checkScaleSanity(scale) {
    if (!separatedShells || separatedShells.length === 0) return;

    let minX = Infinity, minY = Infinity, minZ = Infinity;
    let maxX = -Infinity, maxY = -Infinity, maxZ = -Infinity;

    separatedShells.forEach(s => {
      minX = Math.min(minX, s.bbox[0]);
      minY = Math.min(minY, s.bbox[1]);
      minZ = Math.min(minZ, s.bbox[2]);
      maxX = Math.max(maxX, s.bbox[3]);
      maxY = Math.max(maxY, s.bbox[4]);
      maxZ = Math.max(maxZ, s.bbox[5]);
    });

    const maxDim = Math.max(maxX - minX, maxY - minY, maxZ - minZ) * scale;
    const minDim = Math.min(maxX - minX, maxY - minY, maxZ - minZ) * scale;

    scaleWarningBox.style.display = 'none';

    if (maxDim > 5000) {
      scaleWarningBox.innerHTML = `⚠️ <strong>Scale Warning:</strong> Model maximum dimension is <strong>${maxDim.toFixed(1)} mm</strong> (> 5 meters). If model was designed in microns or custom units, consider a scale multiplier (e.g. 0.001 or 0.1).`;
      scaleWarningBox.style.display = 'block';
    } else if (minDim > 0 && maxDim < 2.0) {
      scaleWarningBox.innerHTML = `⚠️ <strong>Scale Warning:</strong> Model maximum dimension is only <strong>${maxDim.toFixed(2)} mm</strong>. If model was designed in inches, set Scale Multiplier to <strong>25.4</strong>.`;
      scaleWarningBox.style.display = 'block';
    }
  }

  // Convert & Download
  async function onConvertAndDownload() {
    if (!separatedShells || separatedShells.length === 0) {
      showStatus('No valid mesh loaded to convert', 'error');
      return;
    }

    btnConvert.disabled = true;
    showStatus('Generating 3MF container, OPC XML, and packaging ZIP archive...', 'info');

    try {
      const userScale = parseFloat(scaleFactorInput.value) || 1.0;
      const doStrip = stripConfigCheckbox.checked;
      const doThumbnail = genThumbnailCheckbox.checked;

      let thumbnailBytes = null;
      if (doThumbnail && viewer) {
        showStatus('Rasterizing 256x256 embedded thumbnail...', 'info');
        thumbnailBytes = await viewer.captureThumbnail(256);
      }

      showStatus('Compressing into 3MF ZIP archive...', 'info');
      const baseName = currentFile ? currentFile.name.replace(/\.[^/.]+$/, '') : 'model';

      const threeMfBytes = ThreeMfPackager.package3mf(separatedShells, {
        scale: userScale,
        stripSlicerConfig: doStrip,
        thumbnailPng: thumbnailBytes,
        title: baseName
      });

      const outFileName = `${baseName}_stlto3mf.3mf`;
      ThreeMfPackager.download3mf(threeMfBytes, outFileName);

      const sizeKb = (threeMfBytes.length / 1024).toFixed(1);
      showStatus(`🎉 Conversion complete! Downloaded ${outFileName} (${sizeKb} KB). Ready for 3D slicer!`, 'success');
    } catch (err) {
      console.error(err);
      showStatus('Packaging error: ' + err.message, 'error');
    } finally {
      btnConvert.disabled = false;
    }
  }

  // Initialize on DOM load
  window.addEventListener('DOMContentLoaded', () => {
    initViewport();
    initEvents();
  });
})();
