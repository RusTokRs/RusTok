import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { runInNewContext } from 'node:vm';

const runtime = await readFile(new URL('../assets/fly-actions.js', import.meta.url), 'utf8');
const nextRuntime = await readFile(
  new URL('../../../../../apps/next-frontend/packages/rustok-pages/src/components/fly-actions.js', import.meta.url),
  'utf8',
);

// Exercise the shipped runtime with a minimal DOM boundary. Real browser scrolling and media
// emulation are covered separately by the Playwright suite.
function fixture(reducedMotion) {
  const calls = [];
  const events = [];
  const view = reducedMotion === undefined ? {} : {
    matchMedia(query) {
      assert.equal(query, '(prefers-reduced-motion: reduce)');
      return { matches: reducedMotion };
    },
  };
  const document = { defaultView: view };
  const root = {
    nodeType: 1,
    ownerDocument: document,
    contains(element) { return element === root || element === trigger || element === target; },
    closest() { return root; },
    getAttribute() { return null; },
    querySelectorAll() { return [target]; },
  };
  document.body = root;
  const target = {
    nodeType: 1,
    ownerDocument: document,
    closest() { return root; },
    getAttribute(attribute) { return attribute === 'id' ? 'destination' : null; },
    scrollIntoView(options) { calls.push(['scroll', { ...options }]); },
    focus(options) { calls.push(['focus', { ...options }]); },
    animate(keyframes, options) { calls.push(['animate', keyframes, options]); },
    style: {},
  };
  const trigger = {
    nodeType: 1,
    ownerDocument: document,
    parentElement: root,
    closest() { return root; },
    getAttribute() { return null; },
    querySelectorAll() { return []; },
    dispatchEvent(event) { events.push(event); return true; },
  };
  const context = {
    CustomEvent: class {
      constructor(type, options = {}) {
        this.type = type;
        this.detail = options.detail;
        this.bubbles = options.bubbles;
        this.cancelable = options.cancelable;
      }
    },
  };
  runInNewContext(runtime, context);
  return { calls, events, target, trigger, actions: context.FlyActions };
}

for (const [label, preference, behavior] of [
  ['reduced motion', true, 'instant'],
  ['no preference', false, 'smooth'],
  ['unavailable media query', undefined, 'smooth'],
]) {
  test(`scroll action uses ${behavior} for ${label}`, () => {
    const { actions, calls, trigger } = fixture(preference);
    actions.handleAction({ kind: 'scroll_to', target_id: 'destination' }, trigger);
    assert.deepEqual(calls, [
      ['scroll', { behavior, block: 'start' }],
      ['focus', { preventScroll: true }],
    ]);
  });
}

test('animation uses the target document motion preference', () => {
  const reduced = fixture(true);
  reduced.actions.handleAction({ kind: 'animate', target_id: 'destination', animation: 'fade_in' }, reduced.trigger);
  assert.equal(reduced.target.style.opacity, '1');
  assert.equal(reduced.calls.length, 0);

  const normal = fixture(false);
  normal.actions.handleAction({ kind: 'animate', target_id: 'destination', animation: 'fade_in' }, normal.trigger);
  assert.equal(normal.calls[0][0], 'animate');
});

for (const [animation, transform] of [
  ['slide_up', 'translateY(0)'],
  ['slide_down', 'translateY(0)'],
  ['zoom_in', 'scale(1)'],
  ['pulse', 'scale(1)'],
]) {
  test('reduced motion applies the final transform for ' + animation, () => {
    const { actions, calls, target, trigger } = fixture(true);
    actions.handleAction({ kind: 'animate', target_id: 'destination', animation }, trigger);
    assert.equal(target.style.transform, transform);
    assert.equal(calls.length, 0);
  });
}

test('animation fallback applies final transform when Web Animations is unavailable', () => {
  const { actions, calls, target, trigger } = fixture(false);
  delete target.animate;
  actions.handleAction({ kind: 'animate', target_id: 'destination', animation: 'slide_up' }, trigger);
  assert.equal(target.style.opacity, '1');
  assert.equal(target.style.transform, 'translateY(0)');
  assert.equal(calls.length, 0);
});


test('the Next published-page asset stays identical to the Fly source asset', () => {
  assert.equal(nextRuntime, runtime);
});

for (const payload of [false, 0, '']) {
  test('emit_event preserves falsy payload ' + String(payload), () => {
    const { actions, events, trigger } = fixture();
    actions.handleAction({ kind: 'emit_event', event: 'test.payload', payload }, trigger);
    assert.equal(events.length, 1);
    assert.equal(events[0].type, 'fly:action');
    assert.strictEqual(events[0].detail.payload, payload);
  });
}

for (const input of [false, 0, '']) {
  test('provider_action preserves falsy input ' + String(input), () => {
    const { actions, events, trigger } = fixture();
    actions.handleAction({
      kind: 'provider_action',
      provider: 'test',
      action: 'submit',
      input,
    }, trigger);
    assert.equal(events.length, 1);
    assert.strictEqual(events[0].detail.input, input);
  });
}

test('null custom JSON values use the empty-object default', () => {
  const { actions, events, trigger } = fixture();
  actions.handleAction({ kind: 'emit_event', event: 'test.empty', payload: null }, trigger);
  actions.handleAction({
    kind: 'provider_action',
    provider: 'test',
    action: 'submit',
    input: null,
  }, trigger);
  assert.equal(JSON.stringify(events[0].detail.payload), '{}');
  assert.equal(JSON.stringify(events[1].detail.input), '{}');
});

for (const animation of ['constructor', 'toString', '__proto__', 'unknown']) {
  test('animation rejects non-allowlisted name ' + animation, () => {
    for (const reduced of [true, false]) {
      const { actions, calls, target, trigger } = fixture(reduced);
      actions.handleAction({ kind: 'animate', target_id: 'destination', animation }, trigger);
      assert.equal(calls.length, 0);
      assert.equal(target.style.opacity, undefined);
    }
  });
}

test('stale stop handles cannot unregister a replacement document adapter', () => {
  const { actions } = fixture();
  function mockDocument() {
    const listeners = new Map();
    return {
      listeners,
      addEventListener(type, listener) {
        if (!listeners.has(type)) listeners.set(type, new Set());
        listeners.get(type).add(listener);
      },
      removeEventListener(type, listener) {
        listeners.get(type)?.delete(listener);
      },
    };
  }

  const doc = mockDocument();
  const otherDoc = mockDocument();
  const first = actions.start(doc);
  const other = actions.start(otherDoc);
  assert.strictEqual(actions.start(doc), first);
  first.stop();
  for (const listeners of doc.listeners.values()) assert.equal(listeners.size, 0);

  const replacement = actions.start(doc);
  assert.notStrictEqual(replacement, first);
  first.stop();
  assert.strictEqual(actions.start(doc), replacement);
  assert.strictEqual(actions.start(otherDoc), other);
  assert.equal(doc.listeners.size, 4);
  for (const listeners of doc.listeners.values()) assert.equal(listeners.size, 1);
  for (const listeners of otherDoc.listeners.values()) assert.equal(listeners.size, 1);

  replacement.stop();
  replacement.stop();
  for (const listeners of doc.listeners.values()) assert.equal(listeners.size, 0);
  other.stop();
  for (const listeners of otherDoc.listeners.values()) assert.equal(listeners.size, 0);
});
