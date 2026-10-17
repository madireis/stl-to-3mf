/**
 * Automated Verification Suite for Interactive Documentation Portal at /docs
 */
const assert = require('assert');
const fs = require('fs');
const path = require('path');
const http = require('http');

console.log('Running Documentation Portal Automated Verification Suite...\n');

const docsDir = path.resolve(__dirname, '..', 'web', 'public', 'docs');
const htmlPath = path.join(docsDir, 'index.html');
const cssPath = path.join(docsDir, 'docs.css');
const jsPath = path.join(docsDir, 'docs.js');

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

async function asyncTest(name, fn) {
  totalTests++;
  try {
    await fn();
    passedTests++;
    console.log(`  ✓ ${name}`);
  } catch (err) {
    console.error(`  ✗ ${name}`);
    console.error(`    ${err.stack || err.message}`);
    process.exitCode = 1;
  }
}

// 1. File existence & size
test('Documentation files exist and have substantial size', () => {
  assert.strictEqual(fs.existsSync(htmlPath), true, 'index.html exists');
  assert.strictEqual(fs.existsSync(cssPath), true, 'docs.css exists');
  assert.strictEqual(fs.existsSync(jsPath), true, 'docs.js exists');

  const htmlSize = fs.statSync(htmlPath).size;
  const cssSize = fs.statSync(cssPath).size;
  const jsSize = fs.statSync(jsPath).size;

  assert.strictEqual(htmlSize > 10000, true, `index.html is comprehensive (${htmlSize} bytes)`);
  assert.strictEqual(cssSize > 4000, true, `docs.css has complete dark mode styles (${cssSize} bytes)`);
  assert.strictEqual(jsSize > 2500, true, `docs.js has interactive behaviors (${jsSize} bytes)`);
});

// 2. Strict relative path verification
test('Strict relative path compliance for GitHub Pages and local server compatibility', () => {
  const html = fs.readFileSync(htmlPath, 'utf-8');
  assert.strictEqual(html.includes('href="../"'), true, 'Contains relative link back to Studio (../)');
  assert.strictEqual(html.includes('href="./docs.css"'), true, 'Contains relative CSS link (./docs.css)');
  assert.strictEqual(html.includes('src="./docs.js"'), true, 'Contains relative JS link (./docs.js)');
  assert.strictEqual(!html.includes('href="/docs/'), true, 'Does not contain hardcoded absolute root /docs/ hrefs');
  assert.strictEqual(!html.includes('src="/docs/'), true, 'Does not contain hardcoded absolute root /docs/ src');
});

// 3. Technical coverage verification
test('Complete technical coverage of 6 required topic areas', () => {
  const html = fs.readFileSync(htmlPath, 'utf-8');

  // 1. Overview & 30-Second Quickstart
  assert.strictEqual(html.includes('id="overview"'), true, 'Has #overview section');
  assert.strictEqual(html.includes('30-Second Quickstart'), true, 'Covers 30-Second Quickstart');
  assert.strictEqual(html.includes('id="stl-vs-3mf"'), true, 'Has STL vs 3MF comparison');

  // 2. CLI Guide
  assert.strictEqual(html.includes('id="cli-guide"'), true, 'Has #cli-guide section');
  assert.strictEqual(html.includes('--merge-tolerance'), true, 'Covers --merge-tolerance flag');
  assert.strictEqual(html.includes('--strip-slicer-config'), true, 'Covers --strip-slicer-config flag');
  assert.strictEqual(html.includes('--hole-max-edges'), true, 'Covers --hole-max-edges flag');
  assert.strictEqual(html.includes('--threads'), true, 'Covers --threads flag');
  assert.strictEqual(html.includes('inspect'), true, 'Covers inspect command and --json diagnostics');

  // 3. Web Studio Guide
  assert.strictEqual(html.includes('id="web-guide"'), true, 'Has #web-guide section');
  assert.strictEqual(html.includes('id="client-pipeline"'), true, 'Has client-side pipeline guide');
  assert.strictEqual(html.includes('stl-parser.js'), true, 'References stl-parser.js');
  assert.strictEqual(html.includes('mesh-repair.js'), true, 'References mesh-repair.js');
  assert.strictEqual(html.includes('shell-separator.js'), true, 'References shell-separator.js');
  assert.strictEqual(html.includes('threemf-packager.js'), true, 'References threemf-packager.js');
  assert.strictEqual(html.includes('POST /api/convert'), true, 'Covers REST API endpoint POST /api/convert');

  // 4. Architecture & Mathematical Proofs
  assert.strictEqual(html.includes('id="pipeline-stages"'), true, 'Has #pipeline-stages section');
  assert.strictEqual(html.includes('11-Stage'), true, 'Covers 11-stage pipeline');
  assert.strictEqual(html.includes('id="mathematical-proofs"'), true, 'Has #mathematical-proofs section');
  assert.strictEqual(html.includes('Gauss-Ostrogradsky Divergence Theorem'), true, 'Covers Gauss Divergence Theorem');
  assert.strictEqual(html.includes('1/6'), true, 'Contains signed volume formula (1/6 sum p0 . p1 x p2)');
  assert.strictEqual(html.includes("Newell's Method"), true, 'Covers Newell Method');
  assert.strictEqual(html.includes('Ear-Clipping Triangulation'), true, 'Covers Ear Clipping');
  assert.strictEqual(html.includes('Barycentric'), true, 'Covers Barycentric coordinates');
  assert.strictEqual(html.includes('id="opc-spec"'), true, 'Has 3MF OPC ZIP specification');
  assert.strictEqual(html.includes('[Content_Types].xml'), true, 'References [Content_Types].xml');

  // 5. Slicer Compatibility Matrix
  assert.strictEqual(html.includes('id="slicers"'), true, 'Has #slicers section');
  assert.strictEqual(html.includes('Bambu Studio'), true, 'Covers Bambu Studio');
  assert.strictEqual(html.includes('OrcaSlicer'), true, 'Covers OrcaSlicer');
  assert.strictEqual(html.includes('PrusaSlicer'), true, 'Covers PrusaSlicer');
  assert.strictEqual(html.includes('UltiMaker Cura'), true, 'Covers Cura');
  assert.strictEqual(html.includes('SuperSlicer'), true, 'Covers SuperSlicer');

  // 6. Contributing & Testing
  assert.strictEqual(html.includes('id="contributing"'), true, 'Has #contributing section');
  assert.strictEqual(html.includes('#![forbid(unsafe_code)]'), true, 'Covers forbid unsafe code invariant');
  assert.strictEqual(html.includes('85'), true, 'Mentions 85 synthetic tests harness');
});

// 4. Interactive UX controls presence
test('Interactive features: real-time search, copy buttons, mobile drawer', () => {
  const html = fs.readFileSync(htmlPath, 'utf-8');
  assert.strictEqual(html.includes('id="docsSearch"'), true, 'Search input exists');
  assert.strictEqual(html.includes('id="clearSearch"'), true, 'Clear search button exists');
  assert.strictEqual(html.includes('id="searchStatus"'), true, 'Search status banner exists');
  assert.strictEqual(html.includes('class="copy-btn"'), true, 'Copy snippet buttons exist');
  assert.strictEqual(html.includes('id="mobileMenuBtn"'), true, 'Mobile drawer button exists');

  const js = fs.readFileSync(jsPath, 'utf-8');
  assert.strictEqual(js.includes('navigator.clipboard'), true, 'docs.js handles clipboard copying');
  assert.strictEqual(js.includes('search-highlight'), true, 'docs.js implements search highlighting');
  assert.strictEqual(js.includes('updateSidebarActiveState'), true, 'docs.js tracks active scrollspy section');
});

// 5. HTTP Delivery via Node Server
async function runHttpDeliveryTests() {
  const TEST_PORT = 3199;
  process.env.PORT = TEST_PORT;

  // Launch web server in-process
  const serverPath = path.resolve(__dirname, '..', 'web', 'server.js');
  // Clear require cache if any
  delete require.cache[require.resolve(serverPath)];
  
  // Require and start server
  const serverModule = require(serverPath);

  // Helper for HTTP GET
  function httpGet(pathname) {
    return new Promise((resolve, reject) => {
      http.get(`http://localhost:${TEST_PORT}${pathname}`, (res) => {
        let data = '';
        res.on('data', chunk => data += chunk);
        res.on('end', () => resolve({ status: res.statusCode, headers: res.headers, body: data }));
      }).on('error', reject);
    });
  }

  // Small delay for server bind
  await new Promise(r => setTimeout(r, 200));

  await asyncTest('Server delivers /docs/ as text/html with 200 OK', async () => {
    const res = await httpGet('/docs/');
    assert.strictEqual(res.status, 200);
    assert.strictEqual(res.headers['content-type'].includes('text/html'), true);
    assert.strictEqual(res.body.includes('StlTo3mf Documentation Portal'), true);
  });

  await asyncTest('Server delivers /docs as text/html with 200 OK (clean directory resolution)', async () => {
    const res = await httpGet('/docs');
    assert.strictEqual(res.status, 200);
    assert.strictEqual(res.headers['content-type'].includes('text/html'), true);
    assert.strictEqual(res.body.includes('StlTo3mf Documentation Portal'), true);
  });

  await asyncTest('Server delivers /docs/docs.css as text/css with 200 OK', async () => {
    const res = await httpGet('/docs/docs.css');
    assert.strictEqual(res.status, 200);
    assert.strictEqual(res.headers['content-type'].includes('text/css'), true);
    assert.strictEqual(res.body.includes('--accent: #38bdf8'), true);
  });

  await asyncTest('Server delivers /docs/docs.js as application/javascript with 200 OK', async () => {
    const res = await httpGet('/docs/docs.js');
    assert.strictEqual(res.status, 200);
    assert.strictEqual(res.headers['content-type'].includes('javascript'), true);
    assert.strictEqual(res.body.includes('docsSearch'), true);
  });

  // Finish tests
  console.log(`\n===========================================`);
  console.log(`Results: ${passedTests}/${totalTests} tests passed`);
  console.log(`===========================================\n`);

  process.exit(passedTests === totalTests ? 0 : 1);
}

runHttpDeliveryTests().catch(err => {
  console.error('Fatal error during HTTP delivery test:', err);
  process.exit(1);
});
