/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import { parsePriceAmount, salePercent, discountedPrice } from './money.ts';

test('major-unit prices are exact decimal strings without exponent or fixed cent rounding', () => {
  assert.equal(parsePriceAmount(' 12.50 '), '12.5');
  assert.equal(parsePriceAmount('0'), '0');
  assert.equal(parsePriceAmount('0.000001'), '0.000001');
  assert.equal(
    parsePriceAmount('9007199254740993.125'),
    '9007199254740993.125'
  );
  assert.equal(
    parsePriceAmount('79228162514264337593543950335'),
    '79228162514264337593543950335'
  );
});

test('invalid prices are rejected, never clamped to a free price', () => {
  for (const value of [
    '',
    'NaN',
    'Infinity',
    '1e3',
    '-1',
    '1.2x',
    '1,25',
    '79228162514264337593543950336',
    '0.00000000000000000000000000001'
  ]) {
    assert.throws(() => parsePriceAmount(value));
  }
  assert.throws(() => parsePriceAmount(12.5));
});

test('discount drafting uses decimal arithmetic and respects zero/precision', () => {
  assert.equal(discountedPrice('0.3', 10), '0.27');
  assert.equal(discountedPrice('12.5', 20), '10');
  assert.equal(discountedPrice('0.001', 15), '0.00085');
  assert.equal(discountedPrice('9007199254740993', 10), '8106479329266893.7');
  assert.equal(discountedPrice('10', 100), '0');
  assert.equal(salePercent('0', '10'), '100');
  assert.equal(salePercent('10', '12.5'), '20');
  assert.equal(salePercent('20', '12.5'), null);
  assert.equal(salePercent('0', '0'), null);
});
