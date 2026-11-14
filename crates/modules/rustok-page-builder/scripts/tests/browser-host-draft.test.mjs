import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import vm from 'node:vm';

// Execute the exact embedded host bootstrap with a minimal DOM/adapter boundary.
// Browser navigation and the actual server-rendered Pages endpoint need separate integration tests.
const rust = readFileSync(new URL('../../src/browser_host.rs', import.meta.url), 'utf8');
const marker = 'pub const PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS: &str = r#"';
const start = rust.indexOf(marker);
assert.notEqual(start, -1);
const open = start + marker.length;
const close = rust.indexOf('"#;', open);
assert.notEqual(close, -1);
const script = new vm.Script(rust.slice(open, close));

class Element extends EventTarget {
  constructor(tag = 'div') {
    super();
    this.tag = tag;
    this.dataset = {};
    this.attrs = new Map();
    this.children = [];
    this.parentNode = null;
    this.previousElementSibling = null;
    this.inert = false;
  }
  setAttribute(key, value) { this.attrs.set(key, value); }
  hasAttribute(key) { return this.attrs.has(key); }
  append(child) { this.children.push(child); child.parentNode = this; }
  insertBefore(child, before) {
    assert.equal(before, this.root);
    this.children.push(child);
    child.parentNode = this;
    before.previousElementSibling = child;
  }
  querySelectorAll() { return []; }
  querySelector() { return null; }
}
class HTMLSelectElement extends Element {}
class HTMLFormElement extends Element {}
class HTMLIFrameElement extends Element {}

function harness({ route = 'https://admin.test/pages?page=home', dataset = {}, cached = null, failHistory = false } = {}) {
  const root = new Element();
  root.dataset = { flyDraftContract: 'rendered', ...dataset };
  const parent = new Element();
  parent.root = root;
  root.parentNode = parent;
  const location = { href: route };
  const history = {
    state: null,
    replaceState(_state, _title, url) {
      if (failHistory) throw new Error('history blocked');
      location.href = String(url);
    }
  };
  const controller = new AbortController();
  const adapter = {
    root, abortController: controller, draftSession: cached, selectedId: null,
    postIntents: true, intentEndpoint: '/intents', stopped: false,
    stop() { this.stopped = true; controller.abort(); },
    drawSelection() {}, emitIntent() {},
  };
  const document = new EventTarget();
  document.createElement = (tag) => new Element(tag);
  const context = vm.createContext({
    URL, EventTarget, CustomEvent, AbortSignal, Element, HTMLSelectElement, HTMLFormElement,
    HTMLIFrameElement, document, location, history, queueMicrotask,
    FlyBrowser: { bootstrap: () => [adapter] },
  });
  script.runInContext(context);
  const response = (detail) => root.dispatchEvent(new CustomEvent('fly:browser-intent-response', { detail }));
  return { root, parent, adapter, location, response };
}

function recoveryUrl(h) {
  const notice = h.parent.children.find((child) => child.hasAttribute('data-fly-draft-recovery'));
  assert.ok(notice);
  assert.equal(notice.attrs.get('role'), 'alert');
  return new URL(notice.children.find((child) => child.tag === 'a').href);
}

test('rendered generation wins over independently cached session state', () => {
  const h = harness({ route: 'https://admin.test/pages?page=home&fly_draft=d1',
    dataset: { flyRenderedDraftToken: 'd1', flyRenderedDraftGeneration: '7' },
    cached: { token: 'd1', generation: 99 } });
  assert.equal(h.adapter.stopped, false);
  assert.equal(h.adapter.draftSession.token, 'd1');
  assert.equal(h.adapter.draftSession.generation, 7);
});

test('saved-page render never posts to the hidden cached draft', () => {
  const cached = { token: 'kept-session', generation: 8 };
  const h = harness({ cached });
  assert.equal(h.adapter.stopped, true);
  assert.equal(h.adapter.postIntents, false);
  assert.equal(h.adapter.intentEndpoint, null);
  assert.equal(h.root.inert, true);
  assert.equal(h.adapter.draftSession, cached);
  assert.equal(recoveryUrl(h).searchParams.get('fly_draft'), 'kept-session');
  assert.equal(recoveryUrl(h).searchParams.get('page'), 'home');
});

test('a mismatched route, invalid or unsafe generation fails closed', () => {
  for (const [route, gen] of [
    ['https://admin.test/pages?fly_draft=other', '1'],
    ['https://admin.test/pages?fly_draft=d1', '0'],
    ['https://admin.test/pages?fly_draft=d1', '9007199254740992'],
    ['https://admin.test/pages?fly_draft=d1', 'oops'],
  ]) {
    const h = harness({ route, dataset: { flyRenderedDraftToken: 'd1', flyRenderedDraftGeneration: gen } });
    assert.equal(h.adapter.stopped, true);
    assert.equal(h.root.inert, true);
    assert.ok(recoveryUrl(h));
  }
});

test('a fresh render with no cached draft remains editable', () => {
  const h = harness();
  assert.equal(h.adapter.stopped, false);
  assert.equal(h.adapter.draftSession, null);
  assert.equal(h.parent.children.length, 0);
});

test('an older response cannot replace the current draft in the route', () => {
  const h = harness();
  h.response({ ok: true, current: true, result: { draft_token: 'new', reload: false } });
  h.response({ ok: true, current: false, result: { draft_token: 'old', reload: false } });
  assert.equal(new URL(h.location.href).searchParams.get('fly_draft'), 'new');
  assert.equal(h.adapter.stopped, false);
});

test('a route synchronization failure prevents reload and offers the response draft', () => {
  const h = harness({ failHistory: true });
  const result = { draft_token: 'new', reload: true, location: '/elsewhere' };
  h.response({ ok: true, current: true, result });
  assert.equal(result.reload, false);
  assert.equal(result.location, undefined);
  assert.equal(h.adapter.stopped, true);
  assert.equal(h.root.inert, true);
  assert.equal(recoveryUrl(h).searchParams.get('fly_draft'), 'new');
});


test('a response after a page change cannot change or navigate the new route', () => {
  const h = harness({ route: 'https://admin.test/pages?page_id=home' });
  h.location.href = 'https://admin.test/pages?page_id=other';
  const result = { draft_token: 'home-draft', reload: true, location: '/elsewhere' };
  h.response({ ok: true, current: true, result });
  assert.equal(h.location.href, 'https://admin.test/pages?page_id=other');
  assert.equal(result.reload, false);
  assert.equal(result.location, undefined);
  assert.equal(h.adapter.stopped, true);
  assert.equal(h.parent.children.length, 0);
});

test('a response cannot replace a draft selected after the request started', () => {
  const h = harness({ route: 'https://admin.test/pages?page_id=home' });
  h.location.href = 'https://admin.test/pages?page_id=home&fly_draft=selected';
  const result = { draft_token: 'response', reload: true };
  h.response({ ok: true, current: true, result });
  assert.equal(new URL(h.location.href).searchParams.get('fly_draft'), 'selected');
  assert.equal(result.reload, false);
  assert.equal(recoveryUrl(h).searchParams.get('fly_draft'), 'response');
});

test('a detached canvas cannot act on a later response', () => {
  const h = harness();
  h.root.isConnected = false;
  const result = { draft_token: 'draft', reload: true };
  h.response({ ok: true, current: true, result });
  assert.equal(result.reload, false);
  assert.equal(new URL(h.location.href).searchParams.get('fly_draft'), null);
  assert.equal(h.parent.children.length, 0);
});


test('an old canvas does not issue new intents after a page change', async () => {
  const h = harness({ route: 'https://admin.test/pages?page_id=home' });
  h.location.href = 'https://admin.test/pages?page_id=other';
  assert.equal(await h.adapter.emitIntent('save', {}), null);
  assert.equal(h.adapter.stopped, true);
  assert.equal(h.root.inert, true);
  assert.equal(h.parent.children.length, 0);
});
