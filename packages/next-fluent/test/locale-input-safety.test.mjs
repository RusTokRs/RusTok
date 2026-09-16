import test from 'node:test';
import assert from 'node:assert/strict';

import {
  normalizeLocaleTag,
  resolveAcceptLanguage,
  validateI18nConfig,
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

test('Accept-Language streaming preserves q priority and first-seen ties', () => {
  const locales = ['en', 'ru', 'de'];

  assert.equal(
    resolveAcceptLanguage('ru;q=0.8,en;q=0.8,de;q=0.7', locales),
    'ru'
  );
  assert.equal(
    resolveAcceptLanguage('ru;q=0.8,en;q=0.9,de;q=0.7', locales),
    'en'
  );
  assert.equal(
    resolveAcceptLanguage('*;q=0.7,ru;q=0.8', locales),
    'ru'
  );
  assert.equal(
    resolveAcceptLanguage('*;q=0.9,ru;q=0.8', locales),
    'en'
  );
});

test('Accept-Language rejects malformed or out-of-range qvalues', () => {
  const locales = ['en', 'ru'];

  for (const invalidQuality of ['bogus', '2', '1.5', '.8', '0.1234', '1.001', '.']) {
    assert.equal(
      resolveAcceptLanguage(`ru;q=${invalidQuality},en;q=0.5`, locales),
      'en',
      `invalid q=${invalidQuality} must not promote ru`
    );
  }

  assert.equal(resolveAcceptLanguage('ru;Q=1.000,en;q=0.9', locales), 'ru');
  assert.equal(resolveAcceptLanguage('ru;q=0.123,en;q=0.5', locales), 'en');
  assert.equal(resolveAcceptLanguage('ru;q=0,en', locales), 'en');
});

test('large Accept-Language candidate sets resolve without candidate-array materialization', () => {
  const unsupported = Array.from({ length: 4096 }, (_, index) => `x-${index}`).join(',');
  const header = `${unsupported},ru;q=0.9`;

  assert.equal(resolveAcceptLanguage(header, ['en', 'ru']), 'ru');
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

test('oversized configuration diagnostics do not retain locale payloads', () => {
  const oversized = `en-${'a'.repeat(512)}`;

  assert.throws(
    () => validateI18nConfig({ locales: [oversized], defaultLocale: 'en' }),
    (error) => {
      assert.ok(error instanceof Error);
      assert.equal(error.message.includes(oversized), false);
      assert.match(error.message, new RegExp(`oversized locale: ${oversized.length} code units`));
      return true;
    }
  );

  assert.throws(
    () => validateI18nConfig({ locales: ['en'], defaultLocale: oversized }),
    (error) => {
      assert.ok(error instanceof Error);
      assert.equal(error.message.includes(oversized), false);
      assert.match(error.message, new RegExp(`oversized locale: ${oversized.length} code units`));
      return true;
    }
  );
});
