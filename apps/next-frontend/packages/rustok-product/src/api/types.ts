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

export type StorefrontProductListImage = {
  mediaId: string;
  url: string;
  altText?: string | null;
  position: number;
};

export type StorefrontProductListPrice = {
  currencyCode: string;
  amount: string;
  compareAtAmount?: string | null;
  onSale: boolean;
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
  primaryImage?: StorefrontProductListImage | null;
  priceFrom?: StorefrontProductListPrice | null;
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

export type StorefrontProductImage = {
  mediaId: string;
  url: string;
  altText?: string | null;
  position: number;
};

/**
 * One display-ready storefront specification value.
 *
 * `text` is already localized by the Product owner (localized text, option label or formatted
 * number/date). Booleans stay `true`/`false`: product data carries no locale copy, so the
 * storefront maps them to its own yes/no wording.
 */
export type StorefrontProductAttributeValue = {
  text: string;
};

/** A storefront-safe product specification: service attributes never reach this list. */
export type StorefrontProductAttribute = {
  code: string;
  label: string;
  /** Stored attribute value type, e.g. `select`; drives the boolean vocabulary mapping. */
  valueType: string;
  isLocalized: boolean;
  values: StorefrontProductAttributeValue[];
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
  images: StorefrontProductImage[];
  translations: StorefrontProductTranslation[];
  variants: StorefrontProductVariant[];
  /** Specifications the owner resolved for the storefront; empty when nothing is published. */
  attributes?: StorefrontProductAttribute[];
};

export type StorefrontCatalogFilter = {
  search?: string;
  categoryId?: string;
  sortBy?: string;
  sortDirection?: string;
  attributeFilters?: string[];
  currencyCode?: string;
  page?: number;
  perPage?: number;
};

/** One bucket of an enumerable facet: an option id for dictionaries, `true`/`false` for booleans. */
export type ProductCatalogFacetValue = {
  value: string;
  label: string;
  count: number;
};

/** A facet the Product owner counted for the current catalog filter set. */
export type ProductCatalogFacet = {
  code: string;
  label: string;
  /** Stored attribute value type, e.g. `select`. */
  valueType: string;
  isLocalized: boolean;
  /** False for unbounded domains (text, numeric, date): `values` stays empty. */
  isEnumerable: boolean;
  /** True when the owner cut the bucket list at its facet-value limit. */
  isTruncated: boolean;
  /** Products matching every other active facet that carry a value for this attribute. */
  totalProducts: number;
  values: ProductCatalogFacetValue[];
};
