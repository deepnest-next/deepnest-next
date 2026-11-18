// cdp.mjs — tiny raw-CDP driver: launch headless chrome, run window.bench.all(), print JSON.
// Usage: node cdp.mjs <url> [chrome flags...]
import { spawn } from 'node:child_process';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const url = process.argv[2];
const flags = process.argv.slice(3);

const CHROME = '/usr/lib64/chromium-browser/chromium-browser';

const chrome = spawn(CHROME, [
  '--headless=new',
  '--remote-debugging-port=0',
  '--no-sandbox',
  '--disable-dev-shm-usage',
  '--no-first-run',
  '--no-default-browser-check',
  '--window-size=1600,900',
  `--user-data-dir=${mkdtempSync(join(tmpdir(), 'bench-chrome-'))}`,
  ...flags,
  url,
], { stdio: ['ignore', 'pipe', 'pipe'] });

let devWsUrl = null;
chrome.stderr.setEncoding('utf8');
chrome.stderr.on('data', (d) => {
  const m = d.match(/DevTools listening on (ws:\/\/[^\s]+)/);
  if (m) devWsUrl = m[1];
});

function timeout(ms, msg) { return new Promise((_, rej) => setTimeout(() => rej(new Error(msg)), ms)); }

// Minimal CDP client
class CDP {
  constructor(ws, sessionId) { this.ws = ws; this.sessionId = sessionId; this.id = 0; this.pending = new Map(); }
  send(method, params = {}) {
    const id = ++this.id;
    return new Promise((res, rej) => {
      this.pending.set(id, { res, rej });
      const msg = { id, method, params };
      if (this.sessionId) msg.sessionId = this.sessionId;
      this.ws.send(JSON.stringify(msg));
    });
  }
  onMessage(ev) {
    const msg = JSON.parse(ev.data);
    if (msg.id && this.pending.has(msg.id)) {
      const { res, rej } = this.pending.get(msg.id);
      this.pending.delete(msg.id);
      if (msg.error) rej(new Error(msg.error.message)); else res(msg.result);
    }
  }
}

async function connect(wsUrl, ms) {
  const ws = new WebSocket(wsUrl);
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws connect failed')); ws.onmessage = (e) => ws._buffer.push(e); });
  // we'll do buffered processing via the CDP client after open
  await new Promise(r => setTimeout(r, 50));
  return ws;
}

async function main() {
  // wait for ws url
  const wsUrl = await Promise.race([
    new Promise(res => { const i = setInterval(() => { if (devWsUrl) { clearInterval(i); res(devWsUrl); } }, 50); }),
    timeout(25000, 'chrome never reported DevTools url'),
  ]);
  // Get list of targets on the browser endpoint, find the page.
  const raw = new WebSocket(wsUrl);
  await new Promise((res, rej) => { raw.onopen = res; raw.onerror = () => rej(new Error('browser ws failed')); });
  const c1 = new CDP(raw);
  raw.onmessage = (e) => c1.onMessage(e);
  const { targetInfos } = await c1.send('Target.getTargets');
  const page = targetInfos.find(t => t.type === 'page' && t.url && t.url.indexOf('bench.html') !== -1);
  if (!page) throw new Error('no page target: ' + JSON.stringify(targetInfos.map(t=>t.url)));
  // Attach in flatten mode -> sessionId-based dispatch
  const attach = await c1.send('Target.attachToTarget', { targetId: page.targetId, flatten: true });
  const sessionId = attach.sessionId;
  const c = new CDP(raw, sessionId);
  raw.onmessage = (e) => { c1.onMessage(e); c.onMessage(e); };
  await c.send('Runtime.enable');
  // wait for window.bench
  let ready = false;
  for (let i = 0; i < 100 && !ready; i++) {
    const r = await c.send('Runtime.evaluate', { expression: 'typeof window.bench', returnByValue: true });
    ready = r.result.value === 'object';
    if (!ready) await new Promise(r2 => setTimeout(r2, 100));
  }
  if (!ready) throw new Error('bench not ready on page');
  const res = await c.send('Runtime.evaluate', {
    expression: 'window.bench.all().then(r => JSON.stringify({ __bench: r }))',
    awaitPromise: true,
    returnByValue: true,
  });
  if (res.exceptionDetails) {
    console.error('PAGE_EXCEPTION: ' + JSON.stringify(res.exceptionDetails.exception ? res.exceptionDetails.exception.description : res.exceptionDetails.text));
    chrome.kill(); process.exit(3);
  }
  console.log('RESULT ' + res.result.value);
  chrome.kill();
  process.exit(0);
}

main().catch((e) => { console.error('DRIVER_ERROR: ' + e.message); try { chrome.kill(); } catch {} process.exit(2); });