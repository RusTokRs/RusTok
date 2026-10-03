/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

export type ProductCatalogSearchOption = {
  value: string;
  label: string;
};

export type ProductCatalogSearchOptions = {
  categoryOptions: ProductCatalogSearchOption[];
  attributeOptions: ProductCatalogSearchOption[];
};

export type ProductCatalogSearchOptionsRequest = {
  graphql: import("./products").ProductGraphqlExecutor;
  locale: string;
  token?: string | null;
  tenantSlug?: string | null;
  graphqlUrl?: string;
};

export type StorefrontProductPrice = {
  currencyCode: string;
  amount: number;
  compareAtAmount?: number | null;
  onSale?: boolean | null;
  discountPercent?: number | null;
};

export type StorefrontEffectivePrice = {
  currencyCode: string;
  amount: number;
  compareAtAmount?: number | null;
  discountPercent?: number | null;
  onSale?: boolean | null;
  priceListId?: string | null;
  channelId?: string | null;
  channelSlug?: string | null;
};

export type StorefrontProductVariant = {
  id: string;
  title: string;
  sku: string;
  inventoryQuantity?: number | null;
  inStock: boolean;
  prices: StorefrontProductPrice[];
  effectivePrice?: StorefrontEffectivePrice | null;
};

export type StorefrontProductTranslation = {
  locale: string;
  title: string;
  handle: string;
  description?: string | null;
};

export type StorefrontProductListItem = {
  id: string;
  status: string;
  title: string;
  handle: string;
  sellerId: string;
  vendor?: string | null;
  productType?: string | null;
  tags: string[];
  createdAt: string;
  publishedAt?: string | null;
};

export type StorefrontProductListResponse = {
  total: number;
  page: number;
  perPage: number;
  hasNext: boolean;
  items: StorefrontProductListItem[];
};

export type StorefrontProductDetail = {
  id: string;
  status: string;
  handle?: string | null;
  sellerId: string;
  vendor?: string | null;
  productType?: string | null;
  tags: string[];
  publishedAt?: string | null;
  translations: StorefrontProductTranslation[];
  variants: StorefrontProductVariant[];
};

export type StorefrontCatalogFilter = {
  search?: string;
  categoryId?: string;
  sortBy?: string;
  sortDirection?: string;
  attributeFilters?: string[];
  page?: number;
  perPage?: number;
};
