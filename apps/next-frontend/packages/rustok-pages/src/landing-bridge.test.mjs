/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { runInNewContext } from 'node:vm';
import { installLandingBridge } from './components/landing-bridge.ts';

const rustBridge = await readFile(new URL('../../../../../crates/modules/rustok-page-builder/storefront/assets/fly-storefront-bridge.js', import.meta.url), 'utf8');

class Element {
  constructor(attributes = {}, parent = null) {
    this.attributes = attributes;
    this.parentElement = parent;
    this.id = attributes.id || '';
    this.nodeType = 1;
    this.listeners = {};
  }
  closest(selector) {
    for (let node = this; node; node = node.parentElement) {
      if (selector.includes('data-fly-page-body') && node.attributes.root) return node;
      if (selector === 'a' && node.attributes.tag === 'a') return node;
      if (selector === '[data-fly-action]' && node.attributes['data-fly-action']) return node;
      if (selector === "[data-fly-island='cart_trigger']" && node.attributes.island === 'cart_trigger') return node;
    }
    return null;
  }
  contains(element) {
    for (let node = element; node; node = node.parentElement) if (node === this) return true;
    return false;
  }
  getAttribute(name) { return this.attributes[name] ?? null; }
  hasAttribute(name) { return name in this.attributes; }
  addEventListener(name, callback) { this.listeners[name] = callback; }
  removeEventListener(name) { delete this.listeners[name]; }
  querySelectorAll() { return this.candidates || []; }
  scrollIntoView(options) { this.scrolls.push({ ...options }); }
}

globalThis.Element = Element;

function fixture(host, reducedMotion = false) {
  const root = new Element({ root: true });
  const link = new Element({ tag: 'a', href: '#inside' }, root);
  const inner = new Element({ id: 'inside' }, root);
  const nestedRoot = new Element({ root: true }, root);
  const nested = new Element({ id: 'nested' }, nestedRoot);
  inner.scrolls = [];
  nested.scrolls = [];
  root.candidates = [inner, nested];
  const navigations = [];
  const view = {
    matchMedia: () => ({ matches: reducedMotion }),
    location: { origin: 'https://example.test', assign: (href) => navigations.push(href) },
    listeners: {},
    addEventListener(name, callback) { this.listeners[name] = callback; },
    dispatchEvent() { return true; },
  };
  root.ownerDocument = inner.ownerDocument = nested.ownerDocument = { defaultView: view };
  const document = {
    documentElement: {}, listeners: {},
    querySelectorAll() { return []; },
    addEventListener(name, callback) { this.listeners[name] = callback; },
  };
  if (host === 'next') {
    installLandingBridge(root, {
      openCart() {}, closeCart() {}, toggleCart() {},
      navigate: (href) => navigations.push(href), reportError() {}, clearError() {},
    });
  } else {
    runInNewContext(rustBridge, {
      window: view, document, URL, URLSearchParams,
      MutationObserver: class { observe() {} },
      CustomEvent: class { constructor(type, options) { this.type = type; this.detail = options?.detail; } },
    });
  }
  const click = (href) => {
    link.attributes.href = href;
    const event = {
      target: link, button: 0, defaultPrevented: false,
      preventDefault() { this.defaultPrevented = true; },
    };
    (host === 'next' ? root.listeners.click : document.listeners.click)(event);
    return event;
  };
  const navigate = (href) => {
    const event = { target: link, detail: { kind: 'emit_event', event: 'navigate', payload: { href } } };
    (host === 'next' ? root.listeners['fly:action'] : view.listeners['fly:action'])(event);
  };
  return { click, navigate, navigations, inner, nested, link };
}

for (const host of ['next', 'leptos']) {
  test(`${host}: fragments cannot escape the landing, including missing or malformed targets`, () => {
    const { click, navigate, navigations, inner, nested } = fixture(host);
    for (const fragment of ['#missing', '#nested', '#', '#%']) {
      assert.equal(click(fragment).defaultPrevented, true, fragment);
      navigate(fragment);
    }
    assert.deepEqual(navigations, []);
    assert.deepEqual(inner.scrolls, []);
    assert.deepEqual(nested.scrolls, []);
  });

  for (const reduced of [false, true]) {
    test(`${host}: local fragment links and events honor reduced motion=${reduced}`, () => {
      const { click, navigate, navigations, inner } = fixture(host, reduced);
      assert.equal(click('#%69nside').defaultPrevented, true);
      navigate('#%69nside');
      const behavior = reduced ? 'instant' : 'smooth';
      assert.deepEqual(inner.scrolls, [{ behavior }, { behavior }]);
      assert.deepEqual(navigations, []);
    });
  }

  test(`${host}: an unavailable action cannot expose a fragment to native navigation`, () => {
    const { click, inner, link } = fixture(host);
    link.attributes['data-fly-action'] = '{invalid';
    assert.equal(click('#inside').defaultPrevented, true);
    assert.deepEqual(inner.scrolls, []);
  });

  test(`${host}: non-fragment navigation events keep their existing host routing`, () => {
    const { navigate, navigations } = fixture(host);
    navigate('/shop?tag=1');
    assert.deepEqual(navigations, ['/shop?tag=1']);
  });
}
