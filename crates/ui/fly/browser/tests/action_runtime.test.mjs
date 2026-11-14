import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { runInNewContext } from 'node:vm';

const runtime = await readFile(new URL('../assets/fly-actions.js', import.meta.url), 'utf8');

// Exercise the shipped runtime with a minimal DOM boundary. Real browser scrolling and media
// emulation are covered separately by the Playwright suite.
function fixture(reducedMotion) {
  const calls = [];
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
  };
  const context = {};
  runInNewContext(runtime, context);
  return { calls, target, trigger, actions: context.FlyActions };
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
