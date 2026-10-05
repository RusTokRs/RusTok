/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

export interface PricingPriceListOption {
  id: string;
  name: string;
  listType: string;
  channelId?: string | null;
  channelSlug?: string | null;
  ruleKind?: string | null;
  adjustmentPercent?: string | null;
}

export interface PricingChannelOption {
  id: string;
  slug: string;
  name: string;
  isActive: boolean;
  isDefault: boolean;
  status: string;
}

export interface PricingPrice {
  currencyCode: string;
  amount: string;
  compareAtAmount?: string | null;
  discountPercent?: number | null;
  onSale: boolean;
}

export interface PricingEffectivePrice {
  currencyCode: string;
  amount: string;
  compareAtAmount?: string | null;
  discountPercent?: number | null;
  onSale: boolean;
  regionId?: string | null;
  priceListId?: string | null;
  channelId?: string | null;
  channelSlug?: string | null;
  minQuantity?: number | null;
  maxQuantity?: number | null;
}

export interface PricingVariant {
  id: string;
  title: string;
  sku: string;
  prices: PricingPrice[];
  effectivePrice?: PricingEffectivePrice | null;
}

export interface PricingProductTranslation {
  locale: string;
  title: string;
  handle: string;
  description?: string | null;
}

export interface PricingProductDetail {
  id: string;
  status: string;
  sellerId?: string | null;
  vendor?: string | null;
  productType?: string | null;
  publishedAt?: string | null;
  translations: PricingProductTranslation[];
  variants: PricingVariant[];
}

export interface PricingProductListItem {
  id: string;
  title: string;
  handle: string;
  sellerId?: string | null;
  vendor?: string | null;
  productType?: string | null;
  createdAt: string;
  publishedAt?: string | null;
  variantCount: number;
  saleVariantCount: number;
  currencies: string[];
}

export interface PricingProductList {
  items: PricingProductListItem[];
  total: number;
  page: number;
  perPage: number;
  hasNext: boolean;
}

export interface PricingResolutionContext {
  currencyCode: string;
  regionId?: string | null;
  priceListId?: string | null;
  channelId?: string | null;
  channelSlug?: string | null;
  quantity?: number | null;
}

export interface StorefrontPricingData {
  products: PricingProductList;
  selectedProduct?: PricingProductDetail | null;
  selectedHandle?: string | null;
  resolutionContext?: PricingResolutionContext | null;
  availableChannels: PricingChannelOption[];
  activePriceLists: PricingPriceListOption[];
}

export interface StorefrontPricingQuery {
  selectedHandle?: string | null;
  locale?: string | null;
  currencyCode?: string | null;
  regionId?: string | null;
  priceListId?: string | null;
  channelId?: string | null;
  channelSlug?: string | null;
  quantity?: number | null;
}
