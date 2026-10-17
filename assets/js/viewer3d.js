/**
 * StlTo3mf — 3D Interactive WebGL Viewport
 * Powered by Three.js & OrbitControls.
 * Features per-shell vibrant color coding, glowing red hole wireframe highlighting,
 * responsive grid floor, and offscreen 256x256 PNG thumbnail generation.
 */
(function (root, factory) {
  if (typeof define === 'function' && define.amd) {
    define(['three', 'OrbitControls'], factory);
  } else if (typeof module === 'object' && module.exports) {
    module.exports = factory(root.THREE);
  } else {
    root.Viewer3D = factory(root.THREE);
  }
}(typeof self !== 'undefined' ? self : this, function (THREE) {
  'use strict';

  const SHELL_COLORS = [
    0x00A3E0, // Vivid Cyan
    0xFF7A00, // Vibrant Orange
    0x2ECC71, // Emerald Green
    0x9B59B6, // Amethyst Purple
    0xE74C3C, // Coral Red
    0xF1C40F, // Sun Yellow
    0x1ABC9C, // Turquoise
    0xE67E22, // Amber Carrot
  ];

  class Viewer3D {
    /**
     * @param {HTMLElement} container
     */
    constructor(container) {
      if (!THREE) {
        throw new Error('Three.js library not loaded');
      }
      this.container = container;
      this.width = container.clientWidth || 600;
      this.height = container.clientHeight || 450;

      // 1. Scene
      this.scene = new THREE.Scene();
      this.scene.background = new THREE.Color(0x0b1120);

      // 2. Camera
      this.camera = new THREE.PerspectiveCamera(45, this.width / this.height, 0.1, 100000);
      this.camera.position.set(100, 100, 150);

      // 3. Renderer with preserveDrawingBuffer for thumbnail capture
      this.renderer = new THREE.WebGLRenderer({
        antialias: true,
        alpha: true,
        preserveDrawingBuffer: true,
        powerPreference: 'high-performance'
      });
      this.renderer.setSize(this.width, this.height);
      this.renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
      this.renderer.shadowMap.enabled = true;
      this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;
      this.container.appendChild(this.renderer.domElement);

      // 4. OrbitControls
      const ControlsClass = THREE.OrbitControls || (typeof OrbitControls !== 'undefined' ? OrbitControls : null);
      if (ControlsClass) {
        this.controls = new ControlsClass(this.camera, this.renderer.domElement);
        this.controls.enableDamping = true;
        this.controls.dampingFactor = 0.08;
        this.controls.screenSpacePanning = true;
        this.controls.maxDistance = 50000;
      }

      // 5. Lights
      this.initLights();

      // 6. Grid floor
      this.gridHelper = null;
      this.initGrid(200);

      // Tracked objects
      this.meshGroup = new THREE.Group();
      this.scene.add(this.meshGroup);

      this.holeLineMesh = null;
      this.animId = null;

      // Bind animation loop
      this.animate = this.animate.bind(this);
      this.onResize = this.onResize.bind(this);

      window.addEventListener('resize', this.onResize);
      this.animate();
    }

    initLights() {
      // Hemisphere ambient light
      const hemiLight = new THREE.HemisphereLight(0xffffff, 0x1e293b, 0.65);
      this.scene.add(hemiLight);

      // Primary Key Light
      const dirLight1 = new THREE.DirectionalLight(0xffffff, 0.85);
      dirLight1.position.set(150, 250, 200);
      this.scene.add(dirLight1);

      // Soft Blue Fill Light
      const dirLight2 = new THREE.DirectionalLight(0x38bdf8, 0.4);
      dirLight2.position.set(-150, -100, -150);
      this.scene.add(dirLight2);

      // Back rim light
      const rimLight = new THREE.DirectionalLight(0xffffff, 0.3);
      rimLight.position.set(0, 100, -200);
      this.scene.add(rimLight);
    }

    initGrid(size = 200) {
      if (this.gridHelper) {
        this.scene.remove(this.gridHelper);
        this.gridHelper.geometry.dispose();
      }
      this.gridHelper = new THREE.GridHelper(size, 20, 0x38bdf8, 0x1e293b);
      this.gridHelper.position.y = 0;
      this.scene.add(this.gridHelper);
    }

    animate() {
      this.animId = requestAnimationFrame(this.animate);
      if (this.controls) {
        this.controls.update();
      }
      this.renderer.render(this.scene, this.camera);
    }

    onResize() {
      if (!this.container) return;
      this.width = this.container.clientWidth;
      this.height = this.container.clientHeight || 450;
      this.camera.aspect = this.width / this.height;
      this.camera.updateProjectionMatrix();
      this.renderer.setSize(this.width, this.height);
    }

    /**
     * Clears all existing meshes and hole lines from scene.
     */
    clearModel() {
      while (this.meshGroup.children.length > 0) {
        const obj = this.meshGroup.children[0];
        if (obj.geometry) obj.geometry.dispose();
        if (obj.material) {
          if (Array.isArray(obj.material)) obj.material.forEach(m => m.dispose());
          else obj.material.dispose();
        }
        this.meshGroup.remove(obj);
      }
      if (this.holeLineMesh) {
        this.scene.remove(this.holeLineMesh);
        if (this.holeLineMesh.geometry) this.holeLineMesh.geometry.dispose();
        if (this.holeLineMesh.material) this.holeLineMesh.material.dispose();
        this.holeLineMesh = null;
      }
    }

    /**
     * Loads multi-shell model and boundary hole edges into 3D viewport.
     * @param {Object} data
     * @param {Array<Object>} data.shells
     * @param {Float32Array} [data.vertices] (If single mesh)
     * @param {Array<{u: number, v: number}>} [data.boundaryEdges]
     * @param {boolean} [data.showHoles=true]
     */
    loadModel({ shells, vertices, boundaryEdges, showHoles = true }) {
      this.clearModel();

      if (!shells || shells.length === 0) return;

      const overallBbox = new THREE.Box3();

      // 1. Add each shell as a separate Three.js Mesh
      for (let i = 0; i < shells.length; i++) {
        const shell = shells[i];
        const geometry = new THREE.BufferGeometry();

        geometry.setAttribute('position', new THREE.BufferAttribute(shell.vertices, 3));
        geometry.setIndex(new THREE.BufferAttribute(shell.faces, 1));
        geometry.computeVertexNormals();
        geometry.computeBoundingBox();

        overallBbox.union(geometry.boundingBox);

        const colorHex = SHELL_COLORS[i % SHELL_COLORS.length];
        const material = new THREE.MeshStandardMaterial({
          color: colorHex,
          roughness: 0.35,
          metalness: 0.15,
          side: THREE.DoubleSide
        });

        const mesh = new THREE.Mesh(geometry, material);
        mesh.name = shell.name || `Shell_${i + 1}`;
        this.meshGroup.add(mesh);
      }

      // 2. Add glowing red boundary/hole wireframe lines
      if (boundaryEdges && boundaryEdges.length > 0 && vertices) {
        const linePositions = [];
        for (let e = 0; e < boundaryEdges.length; e++) {
          const u = boundaryEdges[e].u * 3;
          const v = boundaryEdges[e].v * 3;
          linePositions.push(
            vertices[u], vertices[u + 1], vertices[u + 2],
            vertices[v], vertices[v + 1], vertices[v + 2]
          );
        }

        const lineGeom = new THREE.BufferGeometry();
        lineGeom.setAttribute('position', new THREE.Float32BufferAttribute(linePositions, 3));

        const lineMat = new THREE.LineBasicMaterial({
          color: 0xef4444, // Glowing Red
          linewidth: 2,
          depthTest: false,
          transparent: true,
          opacity: 0.95
        });

        this.holeLineMesh = new THREE.LineSegments(lineGeom, lineMat);
        this.holeLineMesh.renderOrder = 999; // Draw on top
        this.holeLineMesh.visible = showHoles;
        this.scene.add(this.holeLineMesh);
      }

      // 3. Center and zoom camera to fit model
      this.fitCameraToBox(overallBbox);
    }

    /**
     * Positions camera and grid nicely around model bounding box.
     */
    fitCameraToBox(bbox) {
      if (bbox.isEmpty()) return;

      const center = new THREE.Vector3();
      bbox.getCenter(center);

      const size = new THREE.Vector3();
      bbox.getSize(size);

      const maxDim = Math.max(size.x, size.y, size.z, 1.0);

      // Adjust grid size and position under model
      const gridSize = Math.max(maxDim * 2.5, 50);
      this.initGrid(gridSize);
      if (this.gridHelper) {
        this.gridHelper.position.set(center.x, bbox.min.y, center.z);
      }

      // Position camera
      const fov = this.camera.fov * (Math.PI / 180);
      let cameraDistance = (maxDim / 2) / Math.tan(fov / 2);
      cameraDistance *= 1.6;

      this.camera.position.set(
        center.x + cameraDistance * 0.7,
        center.y + cameraDistance * 0.5,
        center.z + cameraDistance * 0.9
      );
      this.camera.near = maxDim / 100;
      this.camera.far = maxDim * 100;
      this.camera.updateProjectionMatrix();

      if (this.controls) {
        this.controls.target.copy(center);
        this.controls.update();
      }
    }

    setHolesVisible(visible) {
      if (this.holeLineMesh) {
        this.holeLineMesh.visible = visible;
      }
    }

    setWireframe(enabled) {
      this.meshGroup.children.forEach(mesh => {
        if (mesh.material) {
          mesh.material.wireframe = enabled;
        }
      });
    }

    resetCamera() {
      const bbox = new THREE.Box3().setFromObject(this.meshGroup);
      this.fitCameraToBox(bbox);
    }

    /**
     * Captures a 256x256 PNG thumbnail of the current model.
     * Returns a Promise resolving to a Uint8Array containing PNG bytes.
     * @param {number} [size=256]
     * @returns {Promise<Uint8Array>}
     */
    captureThumbnail(size = 256) {
      return new Promise((resolve, reject) => {
        try {
          const prevWidth = this.width;
          const prevHeight = this.height;

          // Temporarily set renderer to thumbnail dimensions
          this.renderer.setSize(size, size);
          this.camera.aspect = 1;
          this.camera.updateProjectionMatrix();

          // Render
          this.renderer.render(this.scene, this.camera);

          // Extract data URL
          const dataUrl = this.renderer.domElement.toDataURL('image/png');

          // Restore previous size
          this.renderer.setSize(prevWidth, prevHeight);
          this.camera.aspect = prevWidth / prevHeight;
          this.camera.updateProjectionMatrix();

          // Convert data URL base64 to Uint8Array
          const base64 = dataUrl.split(',')[1];
          const binaryStr = atob(base64);
          const bytes = new Uint8Array(binaryStr.length);
          for (let i = 0; i < binaryStr.length; i++) {
            bytes[i] = binaryStr.charCodeAt(i);
          }

          resolve(bytes);
        } catch (err) {
          reject(err);
        }
      });
    }

    destroy() {
      if (this.animId) cancelAnimationFrame(this.animId);
      window.removeEventListener('resize', this.onResize);
      this.clearModel();
      if (this.renderer && this.renderer.domElement) {
        this.container.removeChild(this.renderer.domElement);
        this.renderer.dispose();
      }
    }
  }

  return Viewer3D;
}));
