/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

export interface GqlOpts {
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
  graphql?: <TVariables, TData>(
    query: string,
    variables: TVariables,
    token?: string | null,
    tenantSlug?: string | null
  ) => Promise<TData>;
}

export interface CatalogCategorySummary {
  id: string;
  parentId: string | null;
  code: string;
  slug: string;
  path: string;
  kind: string;
  name: string;
}

export interface CreateCatalogCategoryPayload {
  parentId?: string | null;
  code: string;
  slug: string;
  kind?: string;
  name: string;
  description?: string;
}

export interface ProductAttributeSummary {
  id: string;
  code: string;
  valueType: string;
  isLocalized: boolean;
  isFilterable: boolean;
  isSearchable: boolean;
  isSortable: boolean;
  showOnStorefront: boolean;
  label: string;
}

export interface CreateProductAttributePayload {
  code: string;
  valueType: string;
  label: string;
  helpText?: string;
  isLocalized: boolean;
  isFilterable: boolean;
  isSearchable: boolean;
  isSortable: boolean;
  showOnStorefront: boolean;
}

export interface CreateProductAttributeOptionPayload {
  attributeId: string;
  code: string;
  label: string;
  position: number;
}

export interface ProductAttributeSchemaSummary {
  id: string;
  code: string;
  name: string;
}

export interface CreateProductAttributeSchemaPayload {
  code: string;
  name: string;
  description?: string;
}

export interface CreateCategoryAttributeGroupPayload {
  categoryId: string;
  code: string;
  label: string;
  position: number;
}

export interface SetCategorySchemaModePayload {
  categoryId: string;
  mode: string;
  schemaId?: string | null;
  cloneFromCategoryId?: string | null;
}

export interface BindCategoryAttributePayload {
  categoryId: string;
  attributeId: string;
  groupCode?: string | null;
  bindingKind: string;
  isRequired?: boolean | null;
  isDisabled: boolean;
  position?: number | null;
}

export interface BindSchemaAttributePayload {
  schemaId: string;
  attributeId: string;
  groupCode?: string | null;
  isRequired: boolean;
  isDisabled: boolean;
  position: number;
}

export interface ProductAttributeOptionSummary {
  id: string;
  code: string;
  label: string;
  position: number;
}

export interface ProductEffectiveFormAttribute {
  attributeId: string;
  code: string;
  label: string;
  valueType: string;
  isLocalized: boolean;
  options: ProductAttributeOptionSummary[];
  groupCode: string | null;
  groupLabel: string | null;
  isRequired: boolean;
  isDisabled: boolean;
  position: number;
  source: string;
  variantAxisPolicy: string;
  defaultVariantAxis: boolean;
}

export interface ProductEffectiveForm {
  categoryId: string;
  detachedAttributeIds: string[];
  attributes: ProductEffectiveFormAttribute[];
}

export interface ProductImage {
  id: string;
  mediaId: string;
  url: string;
  altText: string | null;
  position: number;
}

export interface ProductVariantPrice {
  currencyCode: string;
  amount: number;
  compareAtAmount: number | null;
  onSale?: boolean;
}

export interface ProductVariant {
  id: string;
  sku: string | null;
  barcode: string | null;
  title: string | null;
  inventoryQuantity: number;
  inventoryPolicy: string;
  inStock: boolean;
  prices: ProductVariantPrice[];
}

export interface ProductTranslation {
  locale: string;
  title: string;
  handle: string;
  description: string | null;
  metaTitle: string | null;
  metaDescription: string | null;
}

export interface ProductDetail {
  id: string;
  status: string;
  sellerId: string | null;
  vendor: string | null;
  productType: string | null;
  shippingProfileSlug: string | null;
  primaryCategoryId: string | null;
  tags: string[];
  createdAt: string | null;
  updatedAt: string | null;
  publishedAt: string | null;
  translations: ProductTranslation[];
  variants: ProductVariant[];
  images: ProductImage[];
}

export interface ProductListItem {
  id: string;
  status: string;
  title: string;
  handle: string;
  sellerId: string | null;
  vendor: string | null;
  productType: string | null;
  shippingProfileSlug: string | null;
  tags: string[];
  createdAt: string | null;
  publishedAt: string | null;
}

export interface ProductAttributeValueItem {
  attributeId: string;
  kind: string;
  text?: string | null;
  integer?: number | null;
  decimal?: string | null;
  boolean?: boolean | null;
  date?: string | null;
  datetime?: string | null;
  optionId?: string | null;
  optionIds?: string[] | null;
  json?: string | null;
  detached: boolean;
}

export interface ProductAttributeValuePatch {
  attributeId: string;
  kind: string;
  text?: string | null;
  integer?: number | null;
  decimal?: string | null;
  boolean?: boolean | null;
  date?: string | null;
  datetime?: string | null;
  optionId?: string | null;
  optionIds?: string[] | null;
  json?: string | null;
}
