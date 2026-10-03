/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { graphqlRequest, type GqlOpts } from '@/lib/graphql';
import type {
  ActivePriceList,
  PricingAdjustmentPreview,
  ProductVariantPrice,
  UpsertVariantPriceInput,
  VariantDiscountInput
} from './types';

export const ACTIVE_PRICE_LISTS_QUERY = `
query PricingAdminActivePriceLists($channelId: UUID, $channelSlug: String) {
  storefrontActivePriceLists(channelId: $channelId, channelSlug: $channelSlug) {
    id
    name
    listType
    channelId
    channelSlug
    ruleKind
    adjustmentPercent
  }
}
`;

export const UPDATE_VARIANT_PRICE_MUTATION = `
mutation UpdateAdminPricingVariantPrice($tenantId: UUID!, $variantId: UUID!, $input: UpdateAdminPricingVariantPriceInput!) {
  updateAdminPricingVariantPrice(tenantId: $tenantId, variantId: $variantId, input: $input) {
    currencyCode
    amount
    compareAtAmount
    discountPercent
    onSale
    priceListId
    channelId
    channelSlug
    minQuantity
    maxQuantity
  }
}
`;

export const PREVIEW_VARIANT_DISCOUNT_MUTATION = `
mutation PreviewVariantDiscount($tenantId: UUID!, $variantId: UUID!, $input: AdminPricingVariantDiscountInput!) {
  previewAdminPricingVariantDiscount(tenantId: $tenantId, variantId: $variantId, input: $input) {
    kind
    currencyCode
    currentAmount
    baseAmount
    adjustmentPercent
    adjustedAmount
    compareAtAmount
    priceListId
    channelId
    channelSlug
  }
}
`;

export const APPLY_VARIANT_DISCOUNT_MUTATION = `
mutation ApplyVariantDiscount($tenantId: UUID!, $variantId: UUID!, $input: AdminPricingVariantDiscountInput!) {
  applyAdminPricingVariantDiscount(tenantId: $tenantId, variantId: $variantId, input: $input) {
    kind
    currencyCode
    currentAmount
    baseAmount
    adjustmentPercent
    adjustedAmount
    compareAtAmount
    priceListId
    channelId
    channelSlug
  }
}
`;

export async function fetchActivePriceLists(
  opts: GqlOpts,
  channelId?: string,
  channelSlug?: string
): Promise<ActivePriceList[]> {
  if (!opts.token || !opts.tenantSlug) {
    throw new Error('Unauthorized');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { channelId?: string; channelSlug?: string },
    { storefrontActivePriceLists: ActivePriceList[] }
  >(
    ACTIVE_PRICE_LISTS_QUERY,
    {
      channelId: channelId || undefined,
      channelSlug: channelSlug || undefined
    },
    opts.token,
    opts.tenantSlug
  );

  return data.storefrontActivePriceLists || [];
}

export async function upsertVariantPrice(
  opts: GqlOpts,
  variantId: string,
  input: UpsertVariantPriceInput
): Promise<ProductVariantPrice> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Unauthorized');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    {
      tenantId: string;
      variantId: string;
      input: UpsertVariantPriceInput;
    },
    {
      updateAdminPricingVariantPrice: {
        currencyCode: string;
        amount: string;
        compareAtAmount?: string | null;
        discountPercent?: string | null;
        onSale: boolean;
        priceListId?: string | null;
        channelId?: string | null;
        channelSlug?: string | null;
        minQuantity?: number | null;
        maxQuantity?: number | null;
      };
    }
  >(
    UPDATE_VARIANT_PRICE_MUTATION,
    {
      tenantId: opts.tenantId,
      variantId,
      input
    },
    opts.token,
    opts.tenantSlug
  );

  const raw = data.updateAdminPricingVariantPrice;
  return {
    currencyCode: raw.currencyCode,
    amount: Math.round(parseFloat(raw.amount) * 100),
    compareAtAmount: raw.compareAtAmount
      ? Math.round(parseFloat(raw.compareAtAmount) * 100)
      : null,
    discountPercent: raw.discountPercent,
    onSale: raw.onSale,
    priceListId: raw.priceListId,
    channelId: raw.channelId,
    channelSlug: raw.channelSlug,
    minQuantity: raw.minQuantity,
    maxQuantity: raw.maxQuantity
  };
}

export async function previewVariantDiscount(
  opts: GqlOpts,
  variantId: string,
  input: VariantDiscountInput
): Promise<PricingAdjustmentPreview> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Unauthorized');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    {
      tenantId: string;
      variantId: string;
      input: VariantDiscountInput;
    },
    { previewAdminPricingVariantDiscount: PricingAdjustmentPreview }
  >(
    PREVIEW_VARIANT_DISCOUNT_MUTATION,
    {
      tenantId: opts.tenantId,
      variantId,
      input
    },
    opts.token,
    opts.tenantSlug
  );

  return data.previewAdminPricingVariantDiscount;
}

export async function applyVariantDiscount(
  opts: GqlOpts,
  variantId: string,
  input: VariantDiscountInput
): Promise<PricingAdjustmentPreview> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Unauthorized');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    {
      tenantId: string;
      variantId: string;
      input: VariantDiscountInput;
    },
    { applyAdminPricingVariantDiscount: PricingAdjustmentPreview }
  >(
    APPLY_VARIANT_DISCOUNT_MUTATION,
    {
      tenantId: opts.tenantId,
      variantId,
      input
    },
    opts.token,
    opts.tenantSlug
  );

  return data.applyAdminPricingVariantDiscount;
}
