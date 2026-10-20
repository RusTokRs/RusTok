/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { mergeProductPricing } from './pricing.ts';

const price = {
  currencyCode: 'KWD',
  amount: '9007199254740993.001',
  compareAtAmount: '9007199254740994.001',
  onSale: true
};
const variants = [
  { id: 'a', sku: null, inventoryQuantity: 7, inStock: true, prices: [price] },
  { id: 'b', sku: null, inventoryQuantity: 2, inStock: true, prices: [] }
];

test('pricing overlay joins only by ID and leaves Product inventory intact', () => {
  const result = mergeProductPricing(variants, [
    { id: 'b', sku: null, prices: [price], inventoryQuantity: 999 }
  ]);
  assert.equal(result[0].inventoryQuantity, 7);
  assert.deepEqual(result[0].prices, []);
  assert.deepEqual(result[1].prices, [price]);
  assert.equal(result[1].inventoryQuantity, 2);
  assert.equal(result[1].prices[0].amount, '9007199254740993.001');
});

test('explicitly absent Pricing price cannot fall back to a stale catalog price', () => {
  const [result] = mergeProductPricing(variants, [
    { id: 'a', prices: [], effectivePrice: null }
  ]);
  assert.deepEqual(result.prices, []);
  assert.equal(result.effectivePrice, null);
});

test('detail view renders decimal strings and owner onSale without numeric coercion', () => {
  const source = readFileSync(
    new URL('../components/product-detail-view.tsx', import.meta.url),
    'utf8'
  );
  assert.ok(source.includes('{priceInfo.amount}'));
  assert.ok(source.includes('priceInfo.onSale && priceInfo.compareAt != null'));
  assert.ok(!source.includes('.toFixed('));
  assert.ok(!source.includes('compareAt > priceInfo.amount'));
});

test('unavailable Pricing fails closed without using catalog price fallback', () => {
  const results = mergeProductPricing(variants, []);
  for (const result of results) {
    assert.deepEqual(result.prices, []);
    assert.equal(result.effectivePrice, null);
  }
});
