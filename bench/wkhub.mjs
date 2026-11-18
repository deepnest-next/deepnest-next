// wkhub.mjs — tiny static + result-collector server for the WebKitGTK leg.
// Serves this directory and collects POST /result into results/webkit_result.json.
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { writeFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('.', import.meta.url));
const OUT = process.argv[2] || `${ROOT}results/webkit_result.json`;

const server = createServer(async (req, res) => {
  try {
    if (req.method === 'POST' && req.url.startsWith('/result')) {
      let body = '';
      req.on('data', c => (body += c));
      req.on('end', () => {
        writeFileSync(OUT, body);
        console.log('GOT_RESULT ' + body.length + ' bytes');
        res.writeHead(200, { 'Content-Type': 'text/plain' });
        res.end('ok');
      });
      return;
    }
    let path = decodeURIComponent((req.url || '/').split('?')[0]);
    if (path === '/') path = '/bench.html';
    const buf = await readFile(ROOT + path);
    res.writeHead(200, { 'Content-Type': path.endsWith('.html') ? 'text/html' : 'application/octet-stream' });
    res.end(buf);
  } catch (e) {
    res.writeHead(404); res.end('not found: ' + e.message);
  }
});
server.listen(8999, '127.0.0.1', () => console.log('hub on :8999'));
process.on('SIGTERM', () => process.exit(0));