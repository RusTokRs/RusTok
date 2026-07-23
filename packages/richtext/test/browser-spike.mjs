import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import playwright from '../../../apps/next-admin/node_modules/playwright/index.js';

const { chromium } = playwright;

const packageRoot = resolve(import.meta.dirname, '..');
const distRoot = resolve(packageRoot, 'dist');
const manifest = JSON.parse(await readFile(resolve(distRoot, 'asset-manifest.json'), 'utf8'));
const server = createServer(async (request, response) => {
  const pathname = new URL(request.url ?? '/', 'http://127.0.0.1').pathname;
  if (pathname === '/') {
    response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    response.end(parentHtml());
    return;
  }
  if (pathname === '/favicon.ico') {
    response.writeHead(204);
    response.end();
    return;
  }
  const asset = pathname.startsWith('/richtext/frame/')
    ? pathname.slice('/richtext/frame/'.length)
    : pathname === '/richtext/frame'
      ? manifest.frame
      : null;
  if (!asset || !Object.values(manifest).includes(asset)) {
    response.writeHead(404);
    response.end('Not found');
    return;
  }
  const body = await readFile(resolve(distRoot, asset));
  const contentType = asset.endsWith('.html')
    ? 'text/html; charset=utf-8'
    : asset.endsWith('.css')
      ? 'text/css; charset=utf-8'
      : 'text/javascript; charset=utf-8';
  response.writeHead(200, {
    'cache-control': 'public, max-age=31536000, immutable',
    'content-security-policy': (pathname === '/richtext/frame' || pathname.endsWith('.html'))
      ? "default-src 'none'; script-src 'self'; script-src-attr 'none'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'none'; font-src 'none'; connect-src 'none'; media-src 'none'; object-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'self'"
      : "default-src 'none'; frame-ancestors 'self'; object-src 'none'; base-uri 'none'; form-action 'none'",
    'content-type': contentType,
    'permissions-policy': 'camera=(), microphone=(), geolocation=(), payment=()',
    'referrer-policy': 'no-referrer',
    'x-content-type-options': 'nosniff',
    'x-frame-options': 'SAMEORIGIN'
  });
  response.end(body);
});

await new Promise((resolveServer) => server.listen(0, '127.0.0.1', resolveServer));
const address = server.address();
assert.ok(address && typeof address !== 'string');
const baseUrl = `http://127.0.0.1:${address.port}`;
const browser = await chromium.launch({
  headless: true,
  executablePath: process.env.RUSTOK_CHROME_PATH ?? 'C:/Program Files/Google/Chrome/Application/chrome.exe'
});
try {
  const page = await browser.newPage();
  page.on('console', (message) => console.log(`[browser:${message.type()}] ${message.text()}`));
  page.on('pageerror', (error) => console.log(`[browser:error] ${error.stack ?? error.message}`));
  const frameResponse = await page.request.get(`${baseUrl}/richtext/frame`);
  assert.equal(frameResponse.status(), 200);
  assert.equal(frameResponse.headers()['x-content-type-options'], 'nosniff');
  assert.match(frameResponse.headers()['content-security-policy'], /frame-ancestors 'self'/);
  await page.goto(baseUrl);
  await page.waitForFunction(() => window.__richTextState?.initialized === true);
  const iframe = page.locator('iframe');
  assert.equal(await iframe.getAttribute('sandbox'), 'allow-scripts');
  const child = page.frames().find((candidate) => candidate !== page.mainFrame());
  assert.ok(child, 'opaque richtext frame should be present');
  const originEvidence = await child.evaluate(() => ({
    origin: location.origin,
    cookie: (() => {
      try {
        return document.cookie;
      } catch {
        return 'blocked';
      }
    })(),
    parentSameOrigin: (() => {
      try {
        return window.parent.location.href;
      } catch {
        return 'blocked';
      }
    })()
  }));
  console.log('frame origin evidence', originEvidence);
  // Chromium serializes the network URL in location.origin for this case, but
  // the effective origin is still opaque: cookie access and parent DOM access
  // are both denied because allow-same-origin is absent.
  assert.equal(originEvidence.cookie, 'blocked');
  assert.equal(originEvidence.parentSameOrigin, 'blocked');
  await child.locator('[contenteditable="true"]').click();
  await page.keyboard.type('Hello from the frame');
  await page.waitForFunction(() => window.__richTextState?.changed === true);
  const document = await page.evaluate(() => window.__richTextState.document);
  assert.equal(document.content[0].content[0].text, 'Hello from the frame');
  console.log('Richtext frame spike passed: opaque origin, private channel, CSP headers, and document change.');
} finally {
  await browser.close();
  server.close();
}

function parentHtml() {
  return `<!doctype html><meta charset="utf-8"><iframe sandbox="allow-scripts" style="width:640px;height:360px"></iframe><script>
const state = { initialized: false, changed: false, document: null }; window.__richTextState = state;
const iframe = document.querySelector('iframe'); const nonce = crypto.randomUUID(); const session = crypto.randomUUID(); let outbound = 0;
const doc = { type: 'doc', content: [{ type: 'paragraph' }] };
const messages = { bold:'Bold', italic:'Italic', strike:'Strike', code:'Code', heading:'Heading', bullet_list:'Bulleted list', ordered_list:'Numbered list', blockquote:'Quote', code_block:'Code block', horizontal_rule:'Horizontal rule', link:'Link', link_url:'Link URL', apply_link:'Apply', remove_link:'Remove', clear_formatting:'Clear formatting', undo:'Undo', redo:'Redo', editor:'Rich text editor' };
iframe.src = '/richtext/frame#nonce=' + encodeURIComponent(nonce);
window.addEventListener('message', (event) => { if (event.source !== iframe.contentWindow || event.data?.type !== 'ready' || event.data.nonce !== nonce) return; const channel = new MessageChannel(); const port = channel.port1; port.onmessage = ({data}) => { const msg = data.message; if (msg.type === 'initialized') state.initialized = true; if (msg.type === 'document_changed') { state.changed = true; state.document = msg.payload.document; } }; port.start(); iframe.contentWindow.postMessage({ protocol:'rustok.richtext', revision:1, type:'connect', nonce, session }, '*', [channel.port2]); send(port, { type:'initialize', payload:{ profile:'article', document:doc, messages, editable:true } }); });
function send(port, message) { outbound += 1; port.postMessage({ protocol:'rustok.richtext', revision:1, session, sequence:outbound, message }); }
</script>`;
}
