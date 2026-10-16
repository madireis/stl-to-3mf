# StlTo3mf Web Studio & API Guide

This guide covers running, configuring, deploying, and integrating the **StlTo3mf Web Studio** and its underlying HTTP and WebAssembly APIs.

---

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Local Quickstart](#local-quickstart)
- [Web Studio User Interface Walkthrough](#web-studio-user-interface-walkthrough)
- [REST API Reference](#rest-api-reference)
  - [`GET /`](#get-)
  - [`POST /api/convert`](#post-apiconvert)
  - [`POST /api/inspect`](#post-apiinspect)
- [Production Deployment Recipes](#production-deployment-recipes)
  - [Recipe 1: PM2 Process Manager](#recipe-1-pm2-process-manager)
  - [Recipe 2: Docker & Docker Compose](#recipe-2-docker--docker-compose)
  - [Recipe 3: systemd Service Unit (Linux)](#recipe-3-systemd-service-unit-linux)
  - [Recipe 4: Nginx Reverse Proxy Configuration](#recipe-4-nginx-reverse-proxy-configuration)
- [In-Browser WebAssembly Integration](#in-browser-webassembly-integration)
  - [Compiling Wasm Bindings](#compiling-wasm-bindings)
  - [JavaScript / TypeScript Integration Example](#javascript--typescript-integration-example)
  - [Serverless Static Hosting (GitHub / Cloudflare Pages)](#serverless-static-hosting-github--cloudflare-pages)

---

## Architecture Overview

StlTo3mf Web Studio is built as a lightweight, zero-dependency Node.js HTTP service paired with the high-performance native Rust engine:

```
[ Browser Client ]
       |
       | HTTP Multipart Uploads (up to 250MB)
       v
[ Node.js HTTP Server: web/server.js ] (Zero npm runtime dependencies)
       |
       | Subprocess spawn with streaming pipes
       v
[ StlTo3mf Native Engine: target/release/stl-to-3mf ]
       |
       | Heals geometry, splits shells, renders PNG thumbnail, builds OPC ZIP
       v
[ Compliant .3mf Streamed back to Browser ]
```

### Key Architectural Strengths:
1. **Zero External Runtime Dependencies:** `web/server.js` uses only native Node.js standard library modules (`http`, `fs`, `path`, `child_process`). No vulnerable `express`, `multer`, or third-party packages to maintain.
2. **Subprocess Isolation:** Geometry parsing and memory-intensive rasterization run inside the compiled native Rust binary (`stl-to-3mf`), preventing Node.js event-loop blocking.
3. **Safe Ephemeral Cleanup:** Temporary scratch files in `web/temp/` are deterministically deleted in process completion hooks.
4. **Dual-Path Architecture:** Can run either as a client-server web app or 100% client-side via WebAssembly (`crates/omni3mf-wasm`).

---

## Local Quickstart

### Prerequisites
- Node.js 18.0 or higher.
- Built release binary: `cargo build --release` (or `cargo build` in debug mode).

### Starting the Server
From the workspace root:

```bash
# Ensure release binary is compiled
cargo build --release

# Start the web studio on port 3000
node web/server.js
```

Or configure via environment variables:
```bash
# Specify custom port
PORT=8080 node web/server.js
```

Terminal output:
```text
===============================================
 StlTo3mf Studio Web App is running!
 Open in your browser: http://localhost:3000
 Engine Binary: target/release/stl-to-3mf.exe (exists: true)
 Powered by Omni3MF Core Geometry Engine
===============================================
```

Navigate to `http://localhost:3000` in your web browser.

---

## Web Studio User Interface Walkthrough

The web interface (`web/public/index.html`) provides a responsive, dark-mode 3D conversion studio:

### 1. Ingestion Drop Zone
Drag and drop any `.stl` file (binary or ASCII) up to 250MB into the dotted drop zone, or click to browse files. The UI immediately displays file size and filename.

### 2. Processing Configuration Toggles
- **Separate Multi-Body Shells (Default: ON):** Identifies disconnected closed shells and splits them into discrete `<object>` elements in the 3MF package. Essential for multi-material and multi-color 3D printing.
- **Heal Holes & Inverted Normals (Default: ON):** Performs spatial vertex welding ($10^{-6}\text{ mm}$), boundary cycle ear-clipping hole patching, and Gauss Divergence Theorem outward normal reorientation.
- **Strip Slicer Profiles & Overrides (Default: OFF):** Sanitizes vendor profile configurations (`.config`, printer temperatures, acceleration limits), protecting your slicer profiles from being overwritten.
- **Manual Scale Multiplier (Default: empty / 1.0):** Enter a scaling factor (e.g. `25.4` to convert an inch-exported model to metric).

### 3. Action Buttons
- **Inspect STL Geometry:** Queries `/api/inspect` and displays an interactive modal with bounding box dimensions, shell count, vertex count, face count, and watertightness status.
- **Convert to 3MF & Download:** Posts the file and options to `/api/convert` and triggers an automatic browser download of the resulting `.3mf` package.

---

## REST API Reference

### `GET /`

Serves the Web Studio single-page application.

- **Request:** `GET /` or `GET /index.html`
- **Response:** `200 OK`, `Content-Type: text/html; charset=utf-8`

---

### `POST /api/convert`

Ingests an STL file via multipart/form-data, repairs geometry, separates shells, and streams back a valid `.3mf` package.

- **Content-Type:** `multipart/form-data; boundary=----WebKitFormBoundary...`

#### Form Fields:
| Field Name | Type | Default | Description |
|---|:---:|:---:|---|
| `file` | Binary | *(required)* | STL file payload (binary or ASCII). |
| `split_shells` | String (`"true"`/`"false"`) | `"true"` | When `"true"`, splits disconnected shells. |
| `repair` | String (`"true"`/`"false"`) | `"true"` | When `"true"`, executes vertex welding and hole healing. |
| `strip_slicer_config` | String (`"true"`/`"false"`) | `"false"` | When `"true"`, purges vendor profiles and printer overrides. |
| `scale` | String (float) | `""` | Optional scale factor override (e.g. `"25.4"`). |

#### Response:
- **Status:** `200 OK`
- **Content-Type:** `application/vnd.ms-package.3dmanufacturing-3dmodel+xml`
- **Content-Disposition:** `attachment; filename="model_stlto3mf.3mf"`
- **Body:** Binary 3MF archive (ZIP format).

#### Example `curl` Call:
```bash
curl -X POST http://localhost:3000/api/convert \
  -F "file=@sample_model.stl" \
  -F "split_shells=true" \
  -F "repair=true" \
  -F "strip_slicer_config=true" \
  -o sample_model.3mf
```

---

### `POST /api/inspect`

Analyzes an uploaded STL model and returns structured geometric and diagnostic metrics.

- **Content-Type:** `multipart/form-data; boundary=----WebKitFormBoundary...`

#### Form Fields:
| Field Name | Type | Default | Description |
|---|:---:|:---:|---|
| `file` | Binary | *(required)* | STL file payload. |

#### Response:
- **Status:** `200 OK`
- **Content-Type:** `application/json`
- **Schema:**
```json
{
  "vertex_count": 12,
  "face_count": 20,
  "shell_count": 1,
  "is_watertight": true,
  "dimensions": [50.0, 50.0, 50.0],
  "bounding_box_min": [-25.0, -25.0, 0.0],
  "bounding_box_max": [25.0, 25.0, 50.0],
  "volume": 62500.0,
  "surface_area": 15000.0,
  "scale_anomaly": "Normal",
  "suggested_scale": 1.0,
  "scale_warning": null
}
```

#### Example `curl` Call:
```bash
curl -X POST http://localhost:3000/api/inspect \
  -F "file=@part.stl" | jq .
```

---

## Production Deployment Recipes

### Recipe 1: PM2 Process Manager

For high-availability production environments:

```bash
# Install PM2 globally
npm install -g pm2

# Build the release binary first
cargo build --release

# Start server under PM2 supervision
pm2 start web/server.js --name "stl-to-3mf-studio" --env PORT=3000

# Save process list for system reboot persistence
pm2 save
pm2 startup
```

---

### Recipe 2: Docker & Docker Compose

#### Multi-Stage Dockerfile (`Dockerfile`):

```dockerfile
# Stage 1: Build Rust native binary
FROM rust:1.75-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin stl-to-3mf

# Stage 2: Minimal Node.js production runtime
FROM node:20-slim
WORKDIR /app

# Copy compiled Rust binary
COPY --from=builder /app/target/release/stl-to-3mf /usr/local/bin/stl-to-3mf
# Symlink alternative alias
RUN ln -s /usr/local/bin/stl-to-3mf /usr/local/bin/omni3mf

# Copy web server code
COPY web/ ./web/

ENV PORT=3000
EXPOSE 3000

USER node
CMD ["node", "web/server.js"]
```

#### Docker Compose (`docker-compose.yml`):
```yaml
version: '3.8'

services:
  stl-to-3mf-web:
    build:
      context: .
      dockerfile: Dockerfile
    container_name: stlto3mf_studio
    restart: unless-stopped
    ports:
      - "3000:3000"
    environment:
      - PORT=3000
    tmpfs:
      - /app/web/temp:size=512M,mode=1777
```

Run with:
```bash
docker compose up -d
```

---

### Recipe 3: systemd Service Unit (Linux)

Create `/etc/systemd/system/stlto3mf.service`:

```ini
[Unit]
Description=StlTo3mf Web Studio Service
After=network.target

[Service]
Type=simple
User=www-data
Group=www-data
WorkingDirectory=/var/www/StlTo3mf
Environment=PORT=3000
ExecStart=/usr/bin/node web/server.js
Restart=on-failure
RestartSec=5s

# Security Hardening
ProtectSystem=full
ProtectHome=true
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl daemon-reload
sudo systemctl enable --now stlto3mf
```

---

### Recipe 4: Nginx Reverse Proxy Configuration

Configure Nginx to front the service and handle large 3D file uploads (e.g. up to 250MB):

```nginx
server {
    listen 80;
    server_name 3mf.yourdomain.com;

    client_max_body_size 250M;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_cache_bypass $http_upgrade;
        proxy_read_timeout 180s;
        proxy_send_timeout 180s;
    }
}
```

---

## In-Browser WebAssembly Integration

For serverless web applications where conversion runs directly in the client's browser (e.g. static site hosting on GitHub Pages or Cloudflare Pages), use `omni3mf-wasm`.

### Compiling Wasm Bindings

```bash
# Install wasm-pack
cargo install wasm-pack

# Build the WebAssembly package
wasm-pack build crates/omni3mf-wasm --target web --release
```

The output in `crates/omni3mf-wasm/pkg/` contains:
- `omni3mf_wasm.js`: JavaScript glue code.
- `omni3mf_wasm_bg.wasm`: Optimized WebAssembly binary.
- `omni3mf_wasm.d.ts`: Full TypeScript definitions.

---

### JavaScript / TypeScript Integration Example

```typescript
import init, {
  convert_stl_to_3mf,
  inspect_stl,
  WasmMeshInspection,
  WasmPipelineOptions
} from './pkg/omni3mf_wasm.js';

async function processUserStlFile(file: File): Promise<void> {
  // 1. Initialize WebAssembly runtime
  await init();

  // 2. Read file bytes into Uint8Array
  const arrayBuffer = await file.arrayBuffer();
  const stlBytes = new Uint8Array(arrayBuffer);

  // 3. Inspect mesh geometry in memory
  const report: WasmMeshInspection = inspect_stl(stlBytes);
  console.log('Topology Diagnosis:', report);

  if (!report.is_watertight) {
    console.warn('Mesh is non-manifold; automated hole patching will be applied.');
  }

  // 4. Configure pipeline options
  const options: WasmPipelineOptions = {
    merge_tolerance: 1e-6,
    repair: true,
    hole_max_edges: 64,
    split_shells: true,
    strip_slicer_config: true,
    generate_thumbnail: true,
  };

  // 5. Convert to 3MF package in browser memory
  const threeMfBytes: Uint8Array = convert_stl_to_3mf(stlBytes, options);

  // 6. Trigger instantaneous browser download
  const blob = new Blob([threeMfBytes], {
    type: 'application/vnd.ms-package.3dmanufacturing-3dmodel+xml',
  });
  const downloadUrl = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = downloadUrl;
  a.download = `${file.name.replace(/\.[^/.]+$/, '')}_converted.3mf`;
  a.click();
  URL.revokeObjectURL(downloadUrl);
}
```

---

### Serverless Static Hosting (GitHub / Cloudflare Pages)

Because `omni3mf-wasm` executes 100% on the client side:
- **Zero Server Costs:** Host the HTML, JS, and `.wasm` bundle on any static CDN (GitHub Pages, Vercel, Netlify, Cloudflare Pages).
- **100% Privacy:** User STL CAD files never leave their machine.
- **Infinite Scalability:** Client machines handle compute, eliminating backend scaling bottlenecks.
