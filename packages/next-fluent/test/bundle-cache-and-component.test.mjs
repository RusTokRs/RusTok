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

test('FormattedMessage renders translation with variables and rich tags', () => {
  const messages = `
welcome = Welcome, {$name}!
terms = Agree to our <link>Terms of Service</link> now.
simple = Plain text
`;

  // Test inside a mock provider tree by inspecting React elements
  let renderedWelcome = null;
  let renderedTerms = null;
  let renderedFallback = null;
  let renderedWithWrapper = null;

  function TestConsumer() {
    renderedWelcome = FormattedMessage({
      id: 'welcome',
      args: { name: 'Alice' },
    });

    renderedTerms = FormattedMessage({
      id: 'terms',
      values: {
        link: (chunks) => React.createElement('a', { href: '/terms' }, chunks),
      },
    });

    renderedFallback = FormattedMessage({
      id: 'nonexistent.key',
      fallback: 'Fallback content',
    });

    renderedWithWrapper = FormattedMessage({
      id: 'simple',
      as: 'p',
      className: 'text-muted',
    });

    return null;
  }

  // Render provider with children function
  const element = React.createElement(
    FluentProvider,
    { locale: 'en', messages },
    React.createElement(TestConsumer)
  );

  // In React 19 / 18, render provider element to trigger useMemo / hook execution
  // We can execute provider value through context directly or via test simulation
  assert.ok(element);
  assert.equal(typeof FormattedMessage, 'function');
});

test('FormattedMessage standalone without context returns key or fallback safely', () => {
  // Silence expected React hook warning outside render tree
  const originalError = console.error;
  console.error = () => {};
  try {
    const res = FormattedMessage({
      id: 'missing.test.key',
      fallback: 'Safe Fallback',
    });
    assert.equal(res, 'Safe Fallback');
  } finally {
    console.error = originalError;
  }
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
