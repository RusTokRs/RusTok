import test from 'node:test';
import assert from 'node:assert/strict';

import {
  matchSupportedLocale,
  resolveAcceptLanguage,
} from '../dist/index.js';

test('structured locale fallback preserves script before language', () => {
  assert.equal(
    matchSupportedLocale('zh-Hant-TW', ['zh-Hant', 'zh', 'en']),
    'zh-Hant'
  );
  assert.equal(
    matchSupportedLocale('sr-Latn-RS', ['sr-Latn', 'sr', 'en']),
    'sr-Latn'
  );
});

test('structured locale fallback removes variants as one layer', () => {
  assert.equal(
    matchSupportedLocale('de-DE-1901', ['de-DE', 'de', 'en']),
    'de-DE'
  );

  // Canonical variant ordering must not manufacture a partial-variant parent.
  assert.equal(
    matchSupportedLocale('sl-rozaj-biske', ['sl-biske', 'sl-rozaj', 'sl', 'en']),
    'sl'
  );
});

test('extension-bearing locale falls back through extension-free structural parents', () => {
  assert.equal(
    matchSupportedLocale('en-US-u-ca-gregory', ['en-US', 'en']),
    'en-US'
  );
});

test('Accept-Language negotiation uses the same structured locale matching', () => {
  assert.equal(
    resolveAcceptLanguage('zh-Hant-TW;q=1,en;q=0.9', ['zh-Hant', 'zh', 'en']),
    'zh-Hant'
  );
});
