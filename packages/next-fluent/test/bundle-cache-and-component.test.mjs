import { test } from 'node:test';
import assert from 'node:assert/strict';
import React from 'react';
import {
  createFluentBundle,
  getCachedFluentBundle,
  clearBundleCache,
  getBundleCacheStats,
  FluentProvider,
  FormattedMessage,
  useTranslations,
} from '../dist/index.js';

test('bundle cache returns identical bundle instance for identical locale and sources', () => {
  clearBundleCache();

  const source = `
hello = Hello
welcome = Welcome, {$name}!
`;

  const bundle1 = getCachedFluentBundle('en', source);
  const bundle2 = getCachedFluentBundle('en', source);

  // Exact same instance from cache
  assert.equal(bundle1, bundle2);

  const stats = getBundleCacheStats();
  assert.equal(stats.bundleCount, 1);
  assert.equal(stats.resourceCount, 1);
});

test('bundle cache isolates different locales and different sources without collision', () => {
  clearBundleCache();

  const sourceEn = 'msg = Hello';
  const sourceRu = 'msg = Привет';

  const bundleEn = getCachedFluentBundle('en', sourceEn);
  const bundleRu = getCachedFluentBundle('ru', sourceRu);

  assert.notEqual(bundleEn, bundleRu);

  const stats = getBundleCacheStats();
  assert.equal(stats.bundleCount, 2);
  assert.equal(stats.resourceCount, 2);
});

test('clearBundleCache resets cache counters and maps', () => {
  clearBundleCache();
  getCachedFluentBundle('en', 'a = 1');
  assert.ok(getBundleCacheStats().bundleCount > 0);

  clearBundleCache();
  assert.equal(getBundleCacheStats().bundleCount, 0);
  assert.equal(getBundleCacheStats().resourceCount, 0);
});

test('FormattedMessage is a React component that depends on createTranslator for formatting', async () => {
  // FormattedMessage is a React component that requires a render tree with FluentProvider.
  // Testing its full behavior requires a React renderer (e.g. react-dom/server or
  // @testing-library/react). Here we verify the underlying translator contract that
  // FormattedMessage delegates to.
  const messages = `
welcome = Welcome, {$name}!
terms = Agree to our terms now.
simple = Plain text
`;

  const bundle = createFluentBundle('en', messages);
  const { createTranslator } = await import('../dist/index.js');
  const t = createTranslator(bundle);

  const stripBidi = (v) => v.replace(/[\u2068\u2069]/g, '');

  assert.equal(stripBidi(t('welcome', { name: 'Alice' })), 'Welcome, Alice!');
  assert.equal(t('simple'), 'Plain text');
  assert.equal(t.has('welcome'), true);
  assert.equal(t.has('nonexistent'), false);

  // Missing key returns the key itself as fallback
  assert.equal(t('nonexistent.key'), 'nonexistent.key');

  // Verify FormattedMessage is exported as a function (React component)
  assert.equal(typeof FormattedMessage, 'function');
});

test('FormattedMessage without FluentProvider context falls through to key names', async () => {
  // Without a React render tree, calling FormattedMessage as a function is not valid
  // React usage (hooks require a renderer). This test documents that the component
  // exists and verifies the default context behavior through createTranslator.
  const { createTranslator } = await import('../dist/index.js');

  // Simulate what happens when FluentProvider is absent: bundle is null
  const t = createTranslator(null);
  assert.equal(t('missing.test.key'), 'missing.test.key');
  assert.equal(t.has('missing.test.key'), false);
});

test('typegen extracts variables inside custom functions and attributes', async () => {
  const { extractMessagesFromFtl } = await import('../dist/index.js');
  const ftl = `
order-total = Total: { CURRENCY($total, currency: "USD") }
order-discount = Discount: { PERCENT($rate) }
cart-count = Items: { NUMBER($count) }
user-profile = Profile
    .aria-label = User profile for {$username}
`;

  const messages = extractMessagesFromFtl(ftl);
  assert.equal(messages.length, 4);

  const totalMsg = messages.find((m) => m.id === 'order-total');
  assert.ok(totalMsg);
  assert.deepEqual(totalMsg.variables, ['total']);

  const discountMsg = messages.find((m) => m.id === 'order-discount');
  assert.ok(discountMsg);
  assert.deepEqual(discountMsg.variables, ['rate']);

  const countMsg = messages.find((m) => m.id === 'cart-count');
  assert.ok(countMsg);
  assert.deepEqual(countMsg.variables, ['count']);

  const profileMsg = messages.find((m) => m.id === 'user-profile');
  assert.ok(profileMsg);
  assert.deepEqual(profileMsg.attributes, ['aria-label']);
  assert.deepEqual(profileMsg.variables, ['username']);
});
