import assert from 'node:assert/strict';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';

// Bundle the production controller and its canonical validators, not a protocol replica.
const bundle = await build({
  entryPoints: [
    fileURLToPath(new URL('../src/frame/controller.ts', import.meta.url)),
  ],
  bundle: true,
  write: false,
  platform: 'node',
  format: 'esm',
  target: 'es2022',
});
const { RichTextFrameController } = await import(
  `data:text/javascript;base64,${Buffer.from(bundle.outputFiles[0].text).toString('base64')}`
);
const { createEnvelope, emptyRichTextDocument, RICH_TEXT_MESSAGE_KEYS } =
  await import('../dist/core.mjs');

function harness(t, options = {}) {
  const listeners = new Map(),
    channels = [],
    changes = [],
    errors = [],
    focus = [],
    sends = [];
  const contentWindow = { postMessage() {} };
  const previousWindow = Object.getOwnPropertyDescriptor(globalThis, 'window');
  Object.defineProperty(globalThis, 'window', {
    configurable: true,
    value: {
      location: { href: 'https://example.test/forum' },
      setTimeout: () => 1,
      clearTimeout() {},
      setInterval: () => 1,
      clearInterval() {},
      addEventListener: (name, fn) => listeners.set(name, fn),
      removeEventListener: (name) => listeners.delete(name),
    },
  });
  t.mock.method(
    globalThis,
    'MessageChannel',
    class {
      constructor() {
        this.port1 = {
          onmessage: null,
          start() {},
          close() {},
          postMessage: (value) => sends.push(value),
        };
        this.port2 = {};
        channels.push(this);
      }
    },
  );
  const iframe = { contentWindow, src: '' };
  const controller = new RichTextFrameController({
    iframe,
    frameUrl: '/richtext/frame',
    profile: 'discussion',
    document: emptyRichTextDocument(),
    messages: Object.fromEntries(
      RICH_TEXT_MESSAGE_KEYS.map((key) => [key, key]),
    ),
    contentLocale: 'en',
    onDocumentChange: (doc) => changes.push(doc),
    onError: (...args) => errors.push(args),
    onFocusChange: (value) => focus.push(value),
    ...options,
  });
  t.after(() => {
    controller.destroy();
    if (previousWindow)
      Object.defineProperty(globalThis, 'window', previousWindow);
    else delete globalThis.window;
  });
  const nonce = new URLSearchParams(new URL(iframe.src).hash.slice(1)).get(
    'nonce',
  );
  const handshake = listeners.get('message');
  const connect = () =>
    handshake({
      source: contentWindow,
      data: { protocol: 'rustok.richtext', type: 'ready', nonce },
    });
  const event = (sequence, type, payload) => ({
    data: createEnvelope(controller.session, sequence, { type, payload }),
  });
  return { controller, connect, channels, changes, errors, focus, event, sends };
}

test('destroyed controller ignores queued document, focus, error and malformed port events', async (t) => {
  const h = harness(t);
  h.connect();
  const queued = h.channels[0].port1.onmessage;
  queued(h.event(1, 'initialized', { document: emptyRichTextDocument() }));
  await h.controller.ready;
  h.controller.destroy();
  queued(h.event(2, 'document_changed', { document: emptyRichTextDocument() }));
  queued(h.event(3, 'document', { document: emptyRichTextDocument() }));
  queued(h.event(4, 'focus_changed', { focused: true }));
  queued(h.event(5, 'error', { code: 'late', message: 'old frame' }));
  queued({ data: {} });
  assert.deepEqual(h.changes, []);
  assert.deepEqual(h.focus, []);
  assert.deepEqual(h.errors, []);
});

test('a queued handshake cannot reconnect a destroyed controller', (t) => {
  const h = harness(t);
  h.controller.destroy();
  h.connect();
  assert.equal(h.channels.length, 0, 'disposal must not create a new channel');
});

test('a live controller still accepts validated document updates', async (t) => {
  const h = harness(t);
  h.connect();
  const receive = h.channels[0].port1.onmessage;
  const document = emptyRichTextDocument();
  receive(h.event(1, 'initialized', { document }));
  await h.controller.ready;
  receive(h.event(2, 'document_changed', { document }));
  assert.deepEqual(h.changes, [document]);
  assert.deepEqual(h.errors, []);
});

test('a poisoned persisted draft is dropped and never sent to the frame', async (t) => {
  const storage = new Map();
  const previousStorage = Object.getOwnPropertyDescriptor(
    globalThis,
    'localStorage'
  );
  Object.defineProperty(globalThis, 'localStorage', {
    configurable: true,
    value: {
      getItem: (key) => storage.get(key) ?? null,
      setItem: (key, value) => storage.set(key, value),
      removeItem: (key) => storage.delete(key),
    },
  });
  t.after(() => {
    if (previousStorage)
      Object.defineProperty(globalThis, 'localStorage', previousStorage);
    else delete globalThis.localStorage;
  });
  const key = 'post-42';
  storage.set(
    `richtext_draft_${key}`,
    JSON.stringify({
      timestamp: 1,
      doc: { type: 'doc', content: [{ type: 'javascript' }] }
    })
  );

  const hostDocument = emptyRichTextDocument();
  const h = harness(t, { draftKey: key, document: hostDocument });
  h.connect();
  const initialize = h.sends[0].message;
  assert.equal(initialize.type, 'initialize');
  assert.deepEqual(
    initialize.payload.document,
    hostDocument,
    'a draft that fails validation must not reach the frame'
  );
  assert.equal(
    storage.has(`richtext_draft_${key}`),
    false,
    'the poisoned draft must be cleared, not retried on every mount'
  );
  const receive = h.channels[0].port1.onmessage;
  receive(h.event(1, 'initialized', { document: hostDocument }));
  await h.controller.ready;
  assert.deepEqual(h.errors, []);
});

test('a valid persisted draft initializes the frame instead of the host document', async (t) => {
  const storage = new Map();
  const previousStorage = Object.getOwnPropertyDescriptor(
    globalThis,
    'localStorage'
  );
  Object.defineProperty(globalThis, 'localStorage', {
    configurable: true,
    value: {
      getItem: (key) => storage.get(key) ?? null,
      setItem: (key, value) => storage.set(key, value),
      removeItem: (key) => storage.delete(key),
    },
  });
  t.after(() => {
    if (previousStorage)
      Object.defineProperty(globalThis, 'localStorage', previousStorage);
    else delete globalThis.localStorage;
  });
  const key = 'post-42';
  const draft = {
    type: 'doc',
    content: [
      { type: 'paragraph', content: [{ type: 'text', text: 'Drafted' }] }
    ]
  };
  storage.set(
    `richtext_draft_${key}`,
    JSON.stringify({ timestamp: 1, doc: draft })
  );

  const h = harness(t, { draftKey: key });
  h.connect();
  const initialize = h.sends[0].message;
  assert.equal(initialize.type, 'initialize');
  assert.deepEqual(initialize.payload.document, draft);
  const receive = h.channels[0].port1.onmessage;
  receive(h.event(1, 'initialized', { document: draft }));
  await h.controller.ready;
  assert.deepEqual(h.errors, []);
});

test('a frame error before initialization rejects ready instead of pending forever', async (t) => {
  const h = harness(t);
  h.connect();
  const receive = h.channels[0].port1.onmessage;
  receive(
    h.event(1, 'error', { code: 'invalid_document', message: 'unsafe link' })
  );
  await assert.rejects(h.controller.ready, /unsafe link/);
  assert.deepEqual(h.errors, [['invalid_document', 'unsafe link']]);
});
