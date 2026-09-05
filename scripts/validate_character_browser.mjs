// Browser GPU acceptance without dependencies. Build the opt-in WASM fixture
// first; this serves only its generated files and writes review artifacts.
// Usage: node scripts/validate_character_browser.mjs webgpu|gl [output-dir]
import http from 'node:http';
import { spawn } from 'node:child_process';
import { readFile, writeFile, mkdir, mkdtemp } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { tmpdir } from 'node:os';

const backend = process.argv[2] ?? 'webgpu';
if (!['webgpu', 'gl'].includes(backend)) throw new Error('Expected webgpu or gl');
const output = resolve(process.argv[3] ?? `docs/baselines/magic-characters/phase3/${backend}`);
const bindings = resolve('target/phase3-browser');
const chrome = process.env.CHARACTER_CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const profile = await mkdtemp(join(tmpdir(), 'character-gpu-chrome-'));
let finish;
const done = new Promise((resolve) => { finish = resolve; });
const server = http.createServer(async (request, response) => {
  try {
    response.setHeader('Cross-Origin-Opener-Policy', 'same-origin');
    response.setHeader('Cross-Origin-Embedder-Policy', 'require-corp');
    if (request.method === 'POST' && request.url === '/result') {
      let body = '';
      for await (const chunk of request) {
        body += chunk;
        if (body.length > 32 * 1024 * 1024) throw new Error('Report too large');
      }
      const result = JSON.parse(body);
      if (result.error) throw new Error(result.error);
      await mkdir(output, { recursive: true });
      for (const image of result.images) {
        if (!/^[a-z0-9-]+$/.test(image.name)) throw new Error('Invalid artifact name');
        await writeFile(join(output, `${image.name}.png`), Buffer.from(image.png));
      }
      result.report.browser = result.browser;
      await writeFile(join(output, 'phase3_report.json'), JSON.stringify(result.report, null, 2) + '\n');
      response.end('ok');
      finish({ report: result.report });
      return;
    }
    const files = { '/fixture.js': ['cubacadabra_renderer.js', 'text/javascript'], '/cubacadabra_renderer_bg.wasm': ['cubacadabra_renderer_bg.wasm', 'application/wasm'] };
    if (files[request.url]) {
      const [file, type] = files[request.url];
      response.setHeader('Content-Type', type);
      response.end(await readFile(join(bindings, file)));
      return;
    }
    if (request.url !== '/') { response.writeHead(404).end(); return; }
    response.setHeader('Content-Type', 'text/html');
    response.end(`<!doctype html><meta charset="utf-8"><title>Character GPU validation</title><canvas id="canvas" width="640" height="360"></canvas><pre id="status">Running ${backend} validation…</pre>
      <script type="module">
      import init, { validate_character_gpu } from '/fixture.js';
      try {
        await init();
        const result = JSON.parse(await validate_character_gpu(document.querySelector('canvas'), '${backend}'));
        result.browser = navigator.userAgent;
        await fetch('/result', { method: 'POST', body: JSON.stringify(result) });
        document.querySelector('#status').textContent = 'Passed';
      } catch (error) {
        document.querySelector('#status').textContent = String(error);
        await fetch('/result', { method: 'POST', body: JSON.stringify({ error: String(error) }) });
      }
      </script>`);
  } catch (error) {
    response.writeHead(500).end(String(error));
    finish({ error: String(error) });
  }
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const child = spawn(chrome, [
  '--headless=new', '--no-first-run', '--no-default-browser-check',
  '--enable-unsafe-webgpu', '--disable-background-timer-throttling',
  `--user-data-dir=${profile}`, `http://127.0.0.1:${server.address().port}/`,
], { stdio: ['ignore', 'ignore', 'pipe'] });
let errors = '';
child.stderr.on('data', (data) => { errors = (errors + data).slice(-8000); });
child.on('error', (error) => finish({ error: String(error) }));
child.on('exit', (code) => { if (code) finish({ error: `Chrome exited ${code}: ${errors}` }); });
const timeout = setTimeout(() => finish({ error: `Browser validation timed out: ${errors}` }), 180_000);
const result = await done;
clearTimeout(timeout);
child.kill('SIGTERM');
server.close();
if (result.error) throw new Error(result.error);
console.log(`Verified ${backend}: ${result.report.adapter}; report=${join(output, 'phase3_report.json')}`);
