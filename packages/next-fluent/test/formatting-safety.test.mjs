import test from 'node:test';
import assert from 'node:assert/strict';

import { createFluentBundle, createTranslator } from '../dist/index.js';

const stripBidiIsolates = (value) => value.replace(/[\u2068\u2069]/g, '');

test('format errors never expose partial Fluent output or fall through locale fallback', () => {
  const primary = createFluentBundle('ru', 'welcome = Привет, { $name }!');
  const fallback = createFluentBundle('en', 'welcome = Fallback without arguments');
  const t = createTranslator(primary, { fallbackBundle: fallback });

  // The primary message exists, so a missing required variable is a formatting
  // failure, not a missing-message condition. Do not render Fluent's partial
  // result and do not hide the broken primary translation behind another locale.
  assert.equal(t('welcome'), 'welcome');

  assert.equal(
    stripBidiIsolates(t('welcome', { name: 'Иван' })),
    'Привет, Иван!'
  );
});

test('t.raw rejects partial values and attributes when formatting needs arguments', () => {
  const bundle = createFluentBundle(
    'en',
    `
raw-value = Value for { $name }
raw-list =
    .item0 = Static item
    .item1 = Item for { $name }
`
  );
  const t = createTranslator(bundle);

  assert.equal(t.raw('raw-value'), 'raw-value');
  assert.equal(t.raw('raw-list'), 'raw-list');
});
