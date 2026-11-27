/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import {
  buildProductAttributeValueBatches,
  formatProductDateTimeLocal,
  parseProductDateTimeLocal,
  parseProductIntegerInput,
  toProductAttributeValueMap
} from './attribute-value-state.ts';

const textPatch = (attributeId, text) => ({
  attributeId,
  kind: 'TEXT',
  text
});

test('localized edits remain in their own locale while shared edits appear once', () => {
  const batches = buildProductAttributeValueBatches({
    activeLocale: 'ru',
    valuesByLocale: {
      en: { title: textPatch('title', 'Chair') },
      ru: { title: textPatch('title', 'Стул') }
    },
    nonLocalizedValues: {
      material: textPatch('material', 'Wool')
    },
    dirtyLocalizedAttributeIdsByLocale: {
      en: new Set(['title']),
      ru: new Set(['title'])
    },
    dirtyNonLocalizedAttributeIds: new Set(['material'])
  });

  assert.deepEqual(batches, [
    { locale: 'en', patches: [textPatch('title', 'Chair')] },
    {
      locale: 'ru',
      patches: [
        textPatch('material', 'Wool'),
        textPatch('title', 'Стул')
      ]
    }
  ]);
});

test('nonlocalized-only edits use one active-locale batch', () => {
  assert.deepEqual(
    buildProductAttributeValueBatches({
      activeLocale: 'fr',
      valuesByLocale: {},
      nonLocalizedValues: {
        material: textPatch('material', 'Cotton')
      },
      dirtyLocalizedAttributeIdsByLocale: {},
      dirtyNonLocalizedAttributeIds: new Set(['material'])
    }),
    [{ locale: 'fr', patches: [textPatch('material', 'Cotton')] }]
  );
});

test('empty dirty state produces no empty locale batches', () => {
  assert.deepEqual(
    buildProductAttributeValueBatches({
      activeLocale: 'en',
      valuesByLocale: {},
      nonLocalizedValues: {},
      dirtyLocalizedAttributeIdsByLocale: { en: new Set() },
      dirtyNonLocalizedAttributeIds: new Set()
    }),
    []
  );
});

test('category changes exclude dirty values outside the new effective form', () => {
  const batches = buildProductAttributeValueBatches(
    {
      activeLocale: 'ru',
      valuesByLocale: {
        en: {
          oldTitle: textPatch('oldTitle', 'Old'),
          shared: textPatch('shared', 'Chair')
        },
        ru: { oldTitle: textPatch('oldTitle', 'Старый') }
      },
      nonLocalizedValues: {
        oldMaterial: textPatch('oldMaterial', 'Wool'),
        sharedGlobal: textPatch('sharedGlobal', 'Wood')
      },
      dirtyLocalizedAttributeIdsByLocale: {
        en: new Set(['oldTitle', 'shared']),
        ru: new Set(['oldTitle'])
      },
      dirtyNonLocalizedAttributeIds: new Set(['oldMaterial', 'sharedGlobal'])
    },
    new Set(['shared', 'sharedGlobal'])
  );

  assert.deepEqual(batches, [
    { locale: 'en', patches: [textPatch('shared', 'Chair')] },
    { locale: 'ru', patches: [textPatch('sharedGlobal', 'Wood')] }
  ]);
});

test('integer EAV parsing never truncates decimals or unsafe values', () => {
  assert.equal(parseProductIntegerInput(''), null);
  assert.equal(parseProductIntegerInput('1e3'), 1000);
  assert.equal(parseProductIntegerInput('42'), 42);
  assert.equal(parseProductIntegerInput('1.5'), undefined);
  assert.equal(parseProductIntegerInput('9007199254740992'), undefined);
});

test('datetime-local values round-trip using local wall-clock time', () => {
  const instant = '2026-01-02T03:04:00.000Z';
  const date = new Date(instant);
  const pad = (value) => String(value).padStart(2, '0');
  const expectedLocal = [
    date.getFullYear(),
    '-',
    pad(date.getMonth() + 1),
    '-',
    pad(date.getDate()),
    'T',
    pad(date.getHours()),
    ':',
    pad(date.getMinutes())
  ].join('');

  const localValue = formatProductDateTimeLocal(instant);
  assert.equal(localValue, expectedLocal);
  assert.equal(parseProductDateTimeLocal(localValue), instant);
  assert.equal(parseProductDateTimeLocal(''), null);
  assert.equal(parseProductDateTimeLocal('not-a-date'), undefined);
});

test('server value records map to editable patches without detached metadata', () => {
  const values = toProductAttributeValueMap([
    {
      attributeId: 'material',
      kind: 'text',
      text: 'Cotton',
      detached: false
    }
  ]);
  assert.deepEqual(values, {
    material: {
      attributeId: 'material',
      kind: 'TEXT',
      text: 'Cotton',
      integer: undefined,
      decimal: undefined,
      boolean: undefined,
      date: undefined,
      datetime: undefined,
      optionId: undefined,
      optionIds: undefined,
      json: undefined
    }
  });
});


test('unset GraphQL value records become explicit CLEAR mutation patches', () => {
  const values = toProductAttributeValueMap([
    { attributeId: 'material', kind: 'unset', detached: false }
  ]);

  assert.deepEqual(values.material, {
    attributeId: 'material',
    kind: 'CLEAR',
    text: undefined,
    integer: undefined,
    decimal: undefined,
    boolean: undefined,
    date: undefined,
    datetime: undefined,
    optionId: undefined,
    optionIds: undefined,
    json: undefined
  });
});
