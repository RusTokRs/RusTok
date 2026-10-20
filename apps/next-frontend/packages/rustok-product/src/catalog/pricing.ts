/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import type {
  StorefrontProductVariant,
  StorefrontProductPricing
} from '../api/types';

export function mergeProductPricing(
  variants: StorefrontProductVariant[],
  pricingVariants: StorefrontProductPricing[]
): StorefrontProductVariant[] {
  const pricingById = new Map(
    pricingVariants.map((price) => [price.id, price])
  );
  return variants.map((variant) => {
    const pricing = pricingById.get(variant.id);
    if (!pricing) return { ...variant, prices: [], effectivePrice: null };
    // An explicit empty Pricing result is authoritative, not permission to fall
    // back to an out-of-context catalog price. Null SKUs are never join keys.
    return {
      ...variant,
      prices: pricing.prices,
      effectivePrice: pricing.effectivePrice ?? null
    };
  });
}
