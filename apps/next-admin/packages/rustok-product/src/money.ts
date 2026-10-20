/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import Decimal from 'decimal.js-light';

// Decimal strings in major units are the owner wire contract. No currency exponent,
// minor-unit conversion or currency rounding policy belongs in the catalog editor.
const ExactDecimal = Decimal.clone({ precision: 64 });
const MAX_COEFFICIENT = 79228162514264337593543950335n;

export function parsePriceAmount(input: string): string {
  if (typeof input !== 'string')
    throw new Error('Price must be a decimal string.');
  const text = input.trim();
  if (!/^\d+(?:\.\d+)?$/.test(text)) {
    throw new Error('Price must be a non-negative decimal amount.');
  }
  const [whole, fraction = ''] = text.split('.');
  const significantFraction = fraction.replace(/0+$/, '');
  if (
    significantFraction.length > 28 ||
    BigInt(whole + significantFraction) > MAX_COEFFICIENT
  ) {
    throw new Error('Price is outside the supported decimal range.');
  }
  return new ExactDecimal(text).toFixed();
}

export function salePercent(
  amount: string,
  compareAt?: string | null
): string | null {
  if (!compareAt) return null;
  const current = new ExactDecimal(parsePriceAmount(amount));
  const previous = new ExactDecimal(parsePriceAmount(compareAt));
  if (previous.lte(current) || previous.isZero()) return null;
  return previous
    .minus(current)
    .div(previous)
    .times(100)
    .toDecimalPlaces(0)
    .toFixed();
}

export function discountedPrice(amount: string, percent: number): string {
  if (!Number.isInteger(percent) || percent < 0 || percent > 100) {
    throw new Error(
      'Discount must be an integer percentage between 0 and 100.'
    );
  }
  return parsePriceAmount(
    new ExactDecimal(parsePriceAmount(amount))
      .times(100 - percent)
      .div(100)
      .toFixed()
  );
}
