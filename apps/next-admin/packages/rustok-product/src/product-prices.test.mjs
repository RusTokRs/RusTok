/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import vm from 'node:vm';
import { parsePriceAmount } from './money.ts';
import { variantDraftCore } from './variant-draft-test-support.mjs';

const card = readFileSync(
  new URL('./components/products/product-variants-card.tsx', import.meta.url),
  'utf8'
);
const handler = card.slice(
  card.indexOf('  const handleSaveDialog ='),
  card.indexOf('  const formatPrice =')
);

function loadSave(rows, overrides = {}) {
  const state = {
    writes: [],
    busy: Boolean(overrides.isBusy),
    open: true,
    error: null
  };
  const context = {
    Error,
    disabled: false,
    isBusy: state.busy,
    combinationsCopy: (key) => key,
    parsePriceAmount,
    axes: [],
    formAxisValues: [],
    validateAxisValues: variantDraftCore.validateAxisValues,
    formPrices: rows,
    formStock: '2',
    formSku: 'SKU',
    formBarcode: '',
    formShippingProfile: '',
    formPolicy: 'deny',
    editingVariant: { id: 'variant' },
    onUpdateVariant: async (...args) => {
      state.writes.push(JSON.parse(JSON.stringify(args)));
    },
    onAddVariant: undefined,
    setError: (value) => {
      state.error = value;
    },
    setIsBusy: (value) => {
      state.busy = value;
    },
    setDialogOpen: (value) => {
      state.open = value;
    },
    ...overrides
  };
  return {
    state,
    save: vm.runInNewContext(
      stripTypeScriptTypes(handler) + '\nhandleSaveDialog;',
      context
    )
  };
}

test('existing variant edit round-trips exact major-unit prices and explicit compare-at zero', async () => {
  const { state, save } = loadSave([
    {
      currencyCode: 'KWD',
      amount: '9007199254740993.001',
      compareAtAmount: '0'
    }
  ]);
  await save();
  assert.deepEqual(state.writes[0][1].prices, [
    {
      currencyCode: 'KWD',
      amount: '9007199254740993.001',
      compareAtAmount: '0'
    }
  ]);
  assert.equal(state.open, false);
  assert.equal(state.busy, false);
});

test('invalid amounts cannot turn into a zero-price write or close the editor', async () => {
  for (const amount of ['', '1e2', '-1', 'NaN']) {
    const { state, save } = loadSave([
      { currencyCode: 'USD', amount, compareAtAmount: '' }
    ]);
    await save();
    assert.equal(state.writes.length, 0);
    assert.equal(state.open, true);
    assert.ok(state.error);
    assert.equal(state.busy, false);
  }
});

test('empty catalog prices remain empty, and explicit zero remains a decimal string', async () => {
  const empty = loadSave([]);
  await empty.save();
  assert.deepEqual(empty.state.writes[0][1].prices, []);
  const zero = loadSave([
    { currencyCode: 'JPY', amount: '0', compareAtAmount: '' }
  ]);
  await zero.save();
  assert.deepEqual(zero.state.writes[0][1].prices, [
    { currencyCode: 'JPY', amount: '0', compareAtAmount: null }
  ]);
});

test('owner errors keep the price draft open and release its busy state', async () => {
  const { state, save } = loadSave([], {
    onUpdateVariant: async () => {
      throw new Error('Owner refused');
    }
  });
  await save();
  assert.equal(state.open, true);
  assert.equal(state.busy, false);
  assert.equal(state.error, 'Owner refused');
});

test('inventory is not silently truncated or clamped while saving prices', async () => {
  for (const formStock of ['', '-1', '1.5', '2147483648']) {
    const { state, save } = loadSave([], { formStock });
    await save();
    assert.equal(state.writes.length, 0);
    assert.ok(state.error);
  }
});

test('scalar/price edits do not validate or resubmit a stale dialog assignment tuple', async () => {
  const { state, save } = loadSave([], {
    axes: [{ attributeId: 'color', allowedValues: [{ optionId: 'blue' }] }],
    formAxisValues: [{ attributeId: 'color', optionId: 'old-red' }]
  });
  await save();
  assert.equal(state.writes.length, 1);
  assert.equal(state.writes[0][0], 'variant');
  assert.equal(Object.hasOwn(state.writes[0][1], 'axisValues'), false);
  assert.equal(state.open, false);
  assert.equal(state.error, null);
});

test('an edit with a missing updater never falls through to the available creator', async () => {
  const creates = [];
  const { state, save } = loadSave([], {
    onUpdateVariant: undefined,
    onAddVariant: async (input) => {
      creates.push(input);
    }
  });
  await save();
  assert.equal(creates.length, 0);
  assert.equal(state.writes.length, 0);
  assert.equal(state.open, true);
  assert.equal(state.busy, false);
  assert.equal(state.error, 'writeUnavailable');
});

test('a missing intended writer keeps both create and edit drafts open without fake success', async () => {
  for (const editingVariant of [null, { id: 'variant' }]) {
    const { state, save } = loadSave([], {
      editingVariant,
      onUpdateVariant: undefined,
      onAddVariant: undefined
    });
    await save();
    assert.equal(state.writes.length, 0);
    assert.equal(state.open, true);
    assert.equal(state.busy, false);
    assert.equal(state.error, 'writeUnavailable');
  }
});

test('a create with only an updater available does not call it or close the draft', async () => {
  const { state, save } = loadSave([], { editingVariant: null });
  await save();
  assert.equal(state.writes.length, 0);
  assert.equal(state.open, true);
  assert.equal(state.error, 'writeUnavailable');
});

test('creation still validates the complete current tuple and sends only an explicit valid selection', async () => {
  const axes = [
    { attributeId: 'color', allowedValues: [{ optionId: 'blue' }] }
  ];
  for (const formAxisValues of [
    [],
    [{ attributeId: 'color', optionId: 'old-red' }]
  ]) {
    const creates = [];
    const { state, save } = loadSave([], {
      axes,
      formAxisValues,
      editingVariant: null,
      onAddVariant: async (input) => {
        creates.push(input);
      }
    });
    await save();
    assert.equal(creates.length, 0);
    assert.equal(state.open, true);
    assert.ok(state.error);
  }
  const creates = [];
  const values = [{ attributeId: 'color', optionId: 'blue' }];
  const { state, save } = loadSave([], {
    axes,
    formAxisValues: values,
    editingVariant: null,
    onAddVariant: async (input) => {
      creates.push(JSON.parse(JSON.stringify(input)));
    }
  });
  await save();
  assert.equal(creates.length, 1);
  assert.deepEqual(creates[0].axisValues, values);
  assert.deepEqual(creates[0].prices, []);
  assert.equal(state.writes.length, 0);
  assert.equal(state.open, false);
});

test('disabled and already-busy handlers leave the draft and current busy state untouched', async () => {
  for (const overrides of [{ disabled: true }, { isBusy: true }]) {
    const { state, save } = loadSave([], overrides);
    await save();
    assert.equal(state.writes.length, 0);
    assert.equal(state.open, true);
    assert.equal(state.busy, Boolean(overrides.isBusy));
    assert.equal(state.error, null);
  }
});

const pricing = readFileSync(
  new URL('./api/pricing.ts', import.meta.url),
  'utf8'
);
const upsertSource = pricing
  .slice(
    pricing.indexOf('export async function upsertVariantPrice('),
    pricing.indexOf('export async function previewVariantDiscount(')
  )
  .replace('export async function', 'async function');
const upsert = vm.runInNewContext(
  stripTypeScriptTypes(upsertSource) + '\nupsertVariantPrice;',
  { UPDATE_VARIANT_PRICE_MUTATION: 'mutation', Error }
);

test('Pricing owner response is not converted to approximate integer cents', async () => {
  const raw = {
    currencyCode: 'KWD',
    amount: '9007199254740993.001',
    compareAtAmount: '0',
    discountPercent: '0.001',
    onSale: false
  };
  const result = await upsert(
    {
      token: 'token',
      tenantSlug: 'tenant',
      tenantId: 'id',
      graphql: async () => ({ updateAdminPricingVariantPrice: raw })
    },
    'variant',
    raw
  );
  assert.equal(result.amount, raw.amount);
  assert.equal(result.compareAtAmount, '0');
  assert.equal(result.discountPercent, '0.001');
});
