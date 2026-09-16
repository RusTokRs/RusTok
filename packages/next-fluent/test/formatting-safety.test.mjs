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
