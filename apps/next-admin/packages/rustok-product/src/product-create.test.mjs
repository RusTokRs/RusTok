/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { randomUUID } from 'node:crypto';
import vm from 'node:vm';
import { parsePriceAmount } from './money.ts';

// Execute the actual adapter without evaluating the host registry and UI barrel.
// All network IO is injected through the adapter's existing executor contract.
const source = readFileSync(new URL('./index.ts', import.meta.url), 'utf8');
const adapter = source.slice(source.indexOf('const CREATE_PRODUCT_MUTATION ='));
const createProduct = vm.runInNewContext(
  stripTypeScriptTypes(
    adapter.replace('export async function', 'async function')
  ) + '\ncreateProduct;',
  { crypto: { randomUUID }, parsePriceAmount }
);

async function capture(payload) {
  let variables;
  const result = await createProduct(
    {
      token: 'test-token',
      tenantSlug: 'test',
      tenantId: 'tenant',
      graphql: async (_query, input) => {
        variables = JSON.parse(JSON.stringify(input));
        return { createProduct: { id: 'created' } };
      }
    },
    payload
  );
  assert.equal(result.id, 'created');
  assert.match(variables.idempotencyKey, /^[0-9a-f-]{36}$/);
  return variables.input;
}

test('create preserves every supplied translation, category and initial variant field', async () => {
  const translations = [
    {
      locale: 'en',
      title: 'Chair',
      handle: 'chair',
      description: 'English',
      metaTitle: 'SEO',
      metaDescription: 'Summary'
    },
    {
      locale: 'ru',
      title: 'Стул',
      handle: 'stul',
      description: 'Русский',
      metaTitle: 'Заголовок',
      metaDescription: 'Описание'
    }
  ];
  const input = await capture({
    title: 'Chair',
    translations,
    primaryCategoryId: 'category',
    shippingProfileSlug: 'bulky',
    sku: 'CHAIR',
    barcode: '123',
    inventoryPolicy: 'continue',
    inventoryQuantity: 4,
    priceAmount: '12.50',
    compareAtAmount: '15.75',
    currencyCode: 'EUR',
    publish: false
  });
  assert.deepEqual(input.translations, translations);
  assert.equal(input.primaryCategoryId, 'category');
  assert.equal(input.shippingProfileSlug, 'bulky');
  assert.equal(input.variants[0].barcode, '123');
  assert.equal(input.variants[0].inventoryPolicy, 'continue');
  assert.deepEqual(input.variants[0].prices, [
    { currencyCode: 'EUR', amount: '12.5', compareAtAmount: '15.75' }
  ]);
});

test('zero is a valid explicit price, while an omitted price stays absent', async () => {
  const free = await capture({ title: 'Free sample', priceAmount: '0' });
  assert.equal(free.variants[0].prices[0].amount, '0');
  const unpriced = await capture({ title: 'Unpriced' });
  assert.deepEqual(unpriced.variants[0].prices, []);
});

test('single-translation callers retain the existing defaults', async () => {
  const input = await capture({ title: 'Test product' });
  assert.equal(input.translations[0].locale, 'en');
  assert.equal(input.translations[0].handle, 'test-product');
  assert.equal(input.primaryCategoryId, null);
  assert.equal(input.variants[0].inventoryPolicy, 'deny');
});

test('missing authentication is rejected before IO', async () => {
  await assert.rejects(
    () => createProduct({}, { title: 'Test' }),
    /Sign in again/
  );
});

test('price values must be decimal strings, not numeric GraphQL String inputs', async () => {
  await assert.rejects(
    () => capture({ title: 'Wrong scalar', priceAmount: 1250 }),
    /decimal string/
  );
});

test('create preserves high precision and never converts major units to cents', async () => {
  const input = await capture({
    title: 'Precision',
    priceAmount: '9007199254740993.000001'
  });
  assert.equal(input.variants[0].prices[0].amount, '9007199254740993.000001');
});

test('create includes localized value batches in the same canonical request', async () => {
  const attributeValues = [
    {
      locale: 'en',
      patches: [{ attributeId: 'material', kind: 'TEXT', text: 'cotton' }]
    },
    {
      locale: 'fr',
      patches: [{ attributeId: 'material', kind: 'TEXT', text: 'coton' }]
    }
  ];
  const input = await capture({
    title: 'Atomic product',
    attributeValues,
    publish: true
  });
  assert.deepEqual(input.attributeValues, attributeValues);
  assert.equal(input.publish, true);
});

test('create forwards the browser caller key unchanged on every invocation', async () => {
  const keys = [];
  const opts = {
    token: 'token',
    tenantSlug: 'tenant',
    tenantId: 'tenant',
    graphql: async (_query, input) => {
      keys.push(input.idempotencyKey);
      return { createProduct: { id: 'created', revision: 1 } };
    }
  };
  for (let i = 0; i < 2; i++) {
    await createProduct(
      opts,
      { title: 'Retried product' },
      'retained-browser-key'
    );
  }
  assert.deepEqual(keys, ['retained-browser-key', 'retained-browser-key']);
});
