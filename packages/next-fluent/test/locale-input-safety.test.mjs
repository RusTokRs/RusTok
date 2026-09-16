import test from 'node:test';
import assert from 'node:assert/strict';

import {
  normalizeLocaleTag,
  resolveAcceptLanguage,
} from '../dist/index.js';

test('oversized locale input is rejected before normalization work', () => {
  const oversized = `en-${'a'.repeat(62)}`;
  assert.ok(oversized.length > 64);
  assert.equal(normalizeLocaleTag(oversized), undefined);
});

test('oversized Accept-Language candidate does not block a later supported locale', () => {
  const oversized = `en-${'a'.repeat(62)}`;
  assert.equal(
    resolveAcceptLanguage(`${oversized},ru;q=0.9`, ['en', 'ru']),
    'ru'
  );
});

test('bounded surrounding whitespace is still trimmed', () => {
  assert.equal(
    normalizeLocaleTag('        ru_RU        '),
    'ru-RU'
  );
});

test('oversized raw padding is rejected before trim work', () => {
  const padded = `${' '.repeat(32)}ru_RU${' '.repeat(32)}`;
  assert.ok(padded.length > 64);
  assert.equal(normalizeLocaleTag(padded), undefined);
});
