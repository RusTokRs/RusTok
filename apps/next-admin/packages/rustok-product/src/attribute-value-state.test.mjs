/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import {
  buildProductAttributeValueBatches,
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

test('server value records map to editable patches without detached metadata', () => {
  const values = toProductAttributeValueMap([
    {
      attributeId: 'material',
      kind: 'TEXT',
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
