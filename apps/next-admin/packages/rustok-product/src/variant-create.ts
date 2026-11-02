/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

import type { CreateVariantInput } from './api/types';
import { parsePriceAmount } from './money.ts';

const object = (value: unknown): value is Record<string, unknown> =>
  !!value && typeof value === 'object' && !Array.isArray(value);
const only = (value: Record<string, unknown>, keys: string[]) =>
  Object.keys(value).every((key) => keys.includes(key));
const shortText = (value: unknown) =>
  value === null || (typeof value === 'string' && [...value].length <= 100);
const idPattern = /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i;
export const isVariantId = (value: unknown): value is string =>
  typeof value === 'string' &&
  idPattern.test(value) &&
  value !== '00000000-0000-0000-0000-000000000000';
const amount = (value: unknown): boolean => {
  if (typeof value !== 'string' || value.length > 64 || value.trim() !== value)
    return false;
  try {
    return parsePriceAmount(value) === value;
  } catch {
    return false;
  }
};

/** Complete explicit wire projection, validated without consulting newer Product metadata. */
export function isVariantCreateInput(
  value: unknown
): value is CreateVariantInput {
  if (
    !object(value) ||
    !only(value, [
      'sku',
      'barcode',
      'shippingProfileSlug',
      'axisValues',
      'prices',
      'inventoryQuantity',
      'inventoryPolicy'
    ]) ||
    !shortText(value.sku) ||
    !shortText(value.barcode) ||
    !shortText(value.shippingProfileSlug) ||
    !Array.isArray(value.axisValues) ||
    value.axisValues.length > 32 ||
    !Number.isInteger(value.inventoryQuantity) ||
    (value.inventoryQuantity as number) < 0 ||
    (value.inventoryQuantity as number) > 2147483647 ||
    !['deny', 'continue'].includes(value.inventoryPolicy as string) ||
    !Array.isArray(value.prices) ||
    value.prices.length > 64
  )
    return false;
  const attributes = new Set<string>();
  return (
    value.axisValues.every((entry) => {
      if (
        !object(entry) ||
        !only(entry, ['attributeId', 'optionId']) ||
        !isVariantId(entry.attributeId) ||
        !isVariantId(entry.optionId) ||
        attributes.has(entry.attributeId)
      )
        return false;
      attributes.add(entry.attributeId);
      return true;
    }) &&
    value.prices.every(
      (price) =>
        object(price) &&
        only(price, ['currencyCode', 'amount', 'compareAtAmount']) &&
        typeof price.currencyCode === 'string' &&
        /^[A-Z]{3}$/.test(price.currencyCode) &&
        amount(price.amount) &&
        (price.compareAtAmount === null || amount(price.compareAtAmount))
    )
  );
}

/** Fresh form admission only. SKU/barcode text retains the existing create semantics.
 * Unknown requests are replayed from the journal, never normalized a second time. */
export function prepareVariantCreateInput(
  input: CreateVariantInput
): CreateVariantInput {
  if (
    !object(input) ||
    !only(input, [
      'sku',
      'barcode',
      'shippingProfileSlug',
      'axisValues',
      'prices',
      'inventoryQuantity',
      'inventoryPolicy'
    ]) ||
    !Array.isArray(input.prices) ||
    input.prices.length > 64 ||
    input.prices.some(
      (price) =>
        !object(price) ||
        !only(price, ['currencyCode', 'amount', 'compareAtAmount'])
    ) ||
    (input.axisValues !== undefined &&
      (!Array.isArray(input.axisValues) ||
        input.axisValues.length > 32 ||
        input.axisValues.some(
          (value) => !object(value) || !only(value, ['attributeId', 'optionId'])
        )))
  )
    throw new Error(
      'The SKU request contains unsupported fields or exceeds the editor bounds.'
    );
  const result: CreateVariantInput = {
    sku: input.sku ?? null,
    barcode: input.barcode ?? null,
    shippingProfileSlug: input.shippingProfileSlug ?? null,
    axisValues: (input.axisValues ?? []).map(({ attributeId, optionId }) => ({
      attributeId,
      optionId
    })),
    prices: input.prices.map((price) => ({
      currencyCode: price.currencyCode.trim().toUpperCase(),
      amount: parsePriceAmount(price.amount),
      compareAtAmount:
        price.compareAtAmount == null || !price.compareAtAmount.trim()
          ? null
          : parsePriceAmount(price.compareAtAmount)
    })),
    inventoryQuantity: input.inventoryQuantity ?? 0,
    inventoryPolicy: (input.inventoryPolicy ?? 'deny').toLowerCase()
  };
  if (!isVariantCreateInput(result))
    throw new Error('Review the full SKU request before creating it.');
  return result;
}

const refusals = new Set([
  'DUPLICATE_SKU',
  'PRODUCT_VALIDATION',
  'PRODUCT_CONFLICT',
  'PRODUCT_VARIANT_AXIS_POLICY_CONFLICT',
  'PRODUCT_NOT_FOUND',
  'PRODUCT_VARIANT_NOT_FOUND',
  'PRODUCT_DEPENDENCY_NOT_FOUND'
]);
export const isVariantCreateRefusalCode = (value: unknown): value is string =>
  typeof value === 'string' && refusals.has(value);

export function variantCreateRefusalKey(code: string) {
  switch (code) {
    case 'DUPLICATE_SKU':
      return 'duplicateSku' as const;
    case 'PRODUCT_NOT_FOUND':
    case 'PRODUCT_VARIANT_NOT_FOUND':
    case 'PRODUCT_DEPENDENCY_NOT_FOUND':
      return 'notFound' as const;
    case 'PRODUCT_CONFLICT':
    case 'PRODUCT_VARIANT_AXIS_POLICY_CONFLICT':
      return 'conflict' as const;
    default:
      return 'invalid' as const;
  }
}
