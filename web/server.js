const http = require('http');
const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');

const PORT = process.env.PORT || 3000;
const PUBLIC_DIR = path.resolve(__dirname, 'public');
const UPLOAD_DIR = path.resolve(__dirname, 'temp');

const CANDIDATE_BINS = [
  path.resolve(__dirname, '..', 'target', 'release', 'stl-to-3mf.exe'),
  path.resolve(__dirname, '..', 'target', 'release', 'stl-to-3mf'),
  path.resolve(__dirname, '..', 'target', 'release', 'stls-to-3mf.exe'),
  path.resolve(__dirname, '..', 'target', 'release', 'stls-to-3mf'),
  path.resolve(__dirname, '..', 'target', 'release', 'omni3mf.exe'),
  path.resolve(__dirname, '..', 'target', 'release', 'omni3mf'),
  path.resolve(__dirname, '..', 'target', 'debug', 'stl-to-3mf.exe'),
  path.resolve(__dirname, '..', 'target', 'debug', 'stl-to-3mf'),
  path.resolve(__dirname, '..', 'target', 'debug', 'stls-to-3mf.exe'),
  path.resolve(__dirname, '..', 'target', 'debug', 'stls-to-3mf'),
  path.resolve(__dirname, '..', 'target', 'debug', 'omni3mf.exe'),
  path.resolve(__dirname, '..', 'target', 'debug', 'omni3mf'),
];

function resolveBinary() {
  return CANDIDATE_BINS.find(p => fs.existsSync(p)) || CANDIDATE_BINS[0];
}

const BIN_PATH = resolveBinary();

if (!fs.existsSync(UPLOAD_DIR)) {
  fs.mkdirSync(UPLOAD_DIR, { recursive: true });
}

function parseMultipart(buffer, boundary) {
  const parts = [];
  const boundaryBuffer = Buffer.from('--' + boundary);
  let start = buffer.indexOf(boundaryBuffer);

  while (start !== -1) {
    const nextStart = buffer.indexOf(boundaryBuffer, start + boundaryBuffer.length);
    if (nextStart === -1) break;

    const partBuffer = buffer.slice(start + boundaryBuffer.length, nextStart);
    const headerEnd = partBuffer.indexOf('\r\n\r\n');
    if (headerEnd !== -1) {
      const headers = partBuffer.slice(0, headerEnd).toString('utf-8');
      const body = partBuffer.slice(headerEnd + 4, partBuffer.length - 2); // trim trailing \r\n

      const dispositionMatch = headers.match(/name="([^"]+)"(?:;\s*filename="([^"]+)")?/);
      if (dispositionMatch) {
        parts.push({
          name: dispositionMatch[1],
          filename: dispositionMatch[2] || null,
          data: body
        });
      }
    }
    start = nextStart;
  }
  return parts;
}

const server = http.createServer((req, res) => {
  const url = new URL(req.url, `http://${req.headers.host}`);

  if (url.pathname === '/' || url.pathname === '/index.html') {
    const html = fs.readFileSync(path.join(PUBLIC_DIR, 'index.html'), 'utf-8');
    res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
    return res.end(html);
  }

  if (url.pathname === '/api/convert' && req.method === 'POST') {
    const contentType = req.headers['content-type'] || '';
    const boundaryMatch = contentType.match(/boundary=(?:([^;]+)|"([^"]+)")/);
    if (!boundaryMatch) {
      res.writeHead(400, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify({ error: 'Missing multipart boundary' }));
    }
    const boundary = boundaryMatch[1] || boundaryMatch[2];

    const chunks = [];
    req.on('data', chunk => chunks.push(chunk));
    req.on('end', () => {
      const fullBuffer = Buffer.concat(chunks);
      const parts = parseMultipart(fullBuffer, boundary);

      const filePart = parts.find(p => p.filename);
      if (!filePart) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        return res.end(JSON.stringify({ error: 'No STL file uploaded' }));
      }

      const getField = (name, fallback) => {
        const p = parts.find(x => x.name === name);
        return p ? p.data.toString().trim() : fallback;
      };

      const doSplit = getField('split_shells', 'true') === 'true';
      const doRepair = getField('repair', 'true') === 'true';
      const doSanitize = getField('strip_slicer_config', 'false') === 'true';
      const scaleVal = getField('scale', '');

      const id = Date.now() + '_' + Math.random().toString(36).substring(2, 8);
      const tempInput = path.join(UPLOAD_DIR, `${id}_input.stl`);
      const tempOutput = path.join(UPLOAD_DIR, `${id}_output.3mf`);

      fs.writeFileSync(tempInput, filePart.data);

      const args = ['convert', tempInput, '-o', tempOutput, '-q'];
      if (!doSplit) args.push('--no-split');
      if (!doRepair) args.push('--no-repair');
      if (doSanitize) args.push('--strip-slicer-config');
      if (scaleVal && !isNaN(parseFloat(scaleVal))) {
        args.push('--scale', scaleVal);
      }

      const proc = spawn(resolveBinary(), args);
      let stderr = '';
      proc.stderr.on('data', d => stderr += d.toString());

      proc.on('close', code => {
        try { fs.unlinkSync(tempInput); } catch (_) {}

        if (code !== 0 || !fs.existsSync(tempOutput)) {
          res.writeHead(500, { 'Content-Type': 'application/json' });
          return res.end(JSON.stringify({ error: stderr || 'StlTo3mf conversion process failed' }));
        }

        const threeMfBuffer = fs.readFileSync(tempOutput);
        try { fs.unlinkSync(tempOutput); } catch (_) {}

        const rawBase = path.basename(filePart.filename, path.extname(filePart.filename)) || 'model';
        const safeBase = rawBase.replace(/[^\w.-]/g, '_');
        const downloadName = `${safeBase}_stlto3mf.3mf`;
        const encodedName = encodeURIComponent(`${rawBase}_stlto3mf.3mf`);

        res.writeHead(200, {
          'Content-Type': 'application/vnd.ms-package.3dmanufacturing-3dmodel+xml',
          'Content-Disposition': `attachment; filename="${downloadName}"; filename*=UTF-8''${encodedName}`,
          'Content-Length': threeMfBuffer.length
        });
        res.end(threeMfBuffer);
      });
    });
    return;
  }

  if (url.pathname === '/api/inspect' && req.method === 'POST') {
    const contentType = req.headers['content-type'] || '';
    const boundaryMatch = contentType.match(/boundary=(?:([^;]+)|"([^"]+)")/);
    if (!boundaryMatch) {
      res.writeHead(400, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify({ error: 'Missing multipart boundary' }));
    }
    const boundary = boundaryMatch[1] || boundaryMatch[2];

    const chunks = [];
    req.on('data', chunk => chunks.push(chunk));
    req.on('end', () => {
      const fullBuffer = Buffer.concat(chunks);
      const parts = parseMultipart(fullBuffer, boundary);
      const filePart = parts.find(p => p.filename);
      if (!filePart) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        return res.end(JSON.stringify({ error: 'No STL file uploaded' }));
      }

      const id = Date.now() + '_' + Math.random().toString(36).substring(2, 8);
      const tempInput = path.join(UPLOAD_DIR, `${id}_inspect.stl`);
      fs.writeFileSync(tempInput, filePart.data);

      const proc = spawn(resolveBinary(), ['inspect', tempInput, '--json']);
      let stdout = '';
      let stderr = '';
      proc.stdout.on('data', d => stdout += d.toString());
      proc.stderr.on('data', d => stderr += d.toString());

      proc.on('close', code => {
        try { fs.unlinkSync(tempInput); } catch (_) {}
        if (code !== 0) {
          res.writeHead(500, { 'Content-Type': 'application/json' });
          return res.end(JSON.stringify({ error: stderr || 'Inspection failed' }));
        }

        try {
          const json = JSON.parse(stdout);
          res.writeHead(200, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify(json));
        } catch (e) {
          res.writeHead(200, { 'Content-Type': 'text/plain' });
          res.end(stdout);
        }
      });
    });
    return;
  }

  res.writeHead(404, { 'Content-Type': 'text/plain' });
  res.end('Not Found');
});

server.listen(PORT, () => {
  const currentBin = resolveBinary();
  console.log(`===============================================`);
  console.log(` StlTo3mf Studio Web App is running!`);
  console.log(` Open in your browser: http://localhost:${PORT}`);
  console.log(` Engine Binary: ${currentBin} (exists: ${fs.existsSync(currentBin)})`);
  console.log(` Powered by Omni3MF Core Geometry Engine`);
  console.log(`===============================================`);
});
