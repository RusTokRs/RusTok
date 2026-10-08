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
  discountPercent?: string | null;
  onSale?: boolean;
  priceListId?: string | null;
  channelId?: string | null;
  channelSlug?: string | null;
  minQuantity?: number | null;
  maxQuantity?: number | null;
}

export interface ActivePriceList {
  id: string;
  name: string;
  listType: string;
  channelId?: string | null;
  channelSlug?: string | null;
  ruleKind?: string | null;
  adjustmentPercent?: string | null;
}

export interface PricingAdjustmentPreview {
  kind: string;
  currencyCode: string;
  currentAmount: string;
  baseAmount: string;
  adjustmentPercent: string;
  adjustedAmount: string;
  compareAtAmount?: string | null;
  priceListId?: string | null;
  channelId?: string | null;
  channelSlug?: string | null;
}

export interface UpsertVariantPriceInput {
  currencyCode: string;
  amount: string;
  compareAtAmount?: string | null;
  priceListId?: string | null;
  channelId?: string | null;
  channelSlug?: string | null;
  minQuantity?: number | null;
  maxQuantity?: number | null;
}

export interface VariantDiscountInput {
  currencyCode: string;
  discountPercent: string;
  priceListId?: string | null;
  channelId?: string | null;
  channelSlug?: string | null;
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

export type ProductRelationType =
  'CROSS_SELL' | 'UP_SELL' | 'RELATED' | 'ACCESSORY' | 'ALTERNATIVE';

export interface ProductRelation {
  id: string;
  productId: string;
  relatedProductId: string;
  relationType: ProductRelationType;
  position: number;
  metadata?: Record<string, unknown> | null;
  createdAt: string;
  updatedAt: string;
  relatedProduct?: {
    id: string;
    title: string;
    handle?: string;
    thumbnail?: string;
    sku?: string;
    status?: string;
    price?: string;
  };
}

export interface AddProductRelationInput {
  productId: string;
  relatedProductId: string;
  relationType: ProductRelationType;
  position?: number;
  metadata?: Record<string, unknown>;
}

export type BundleType = 'fixed' | 'flexible';
export type BundleStatus = 'draft' | 'active' | 'archived';
export type BundleDiscountType = 'none' | 'percentage' | 'fixed_amount';

export interface BundleItem {
  id: string;
  bundleId: string;
  productId: string;
  variantId?: string | null;
  quantity: number;
  isOptional: boolean;
  discountRate?: string | null;
  position: number;
  product?: {
    id: string;
    title: string;
    handle?: string;
    thumbnail?: string;
    price?: string;
  } | null;
  variant?: {
    id: string;
    title?: string | null;
    sku?: string | null;
  } | null;
}

export interface ProductBundle {
  id: string;
  tenantId: string;
  bundleProductId?: string | null;
  slug: string;
  name: string;
  description?: string | null;
  bundleType: BundleType | string;
  status: BundleStatus | string;
  discountType: BundleDiscountType | string;
  discountValue: string;
  metadata?: Record<string, unknown> | null;
  items: BundleItem[];
  createdAt: string;
  updatedAt: string;
}

export interface CreateBundleInput {
  bundleProductId?: string | null;
  slug: string;
  name: string;
  description?: string | null;
  bundleType: string;
  status: string;
  discountType: string;
  discountValue: string;
  metadata?: Record<string, unknown> | null;
}

export interface UpdateBundleInput {
  bundleProductId?: string | null;
  slug?: string | null;
  name?: string | null;
  description?: string | null;
  bundleType?: string | null;
  status?: string | null;
  discountType?: string | null;
  discountValue?: string | null;
  metadata?: Record<string, unknown> | null;
}

export interface AddBundleItemInput {
  bundleId: string;
  productId: string;
  variantId?: string | null;
  quantity?: number;
  isOptional?: boolean;
  discountRate?: string | null;
  position?: number;
}

export interface BundleListResponse {
  items: ProductBundle[];
  total: number;
  page: number;
  perPage: number;
}
