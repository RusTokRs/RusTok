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
  ProductDetail,
  ProductImage,
  ProductVariant,
  ProductVariantPrice,
  ProductAttributeValueItem,
  ProductAttributeValuePatch
} from './types';

export const PRODUCT_DETAIL_QUERY = `
query ProductAdminProductDetail($tenantId: UUID!, $id: UUID!, $locale: String) {
  product(tenantId: $tenantId, id: $id, locale: $locale) {
    id
    status
    sellerId
    vendor
    productType
    shippingProfileSlug
    primaryCategoryId
    tags
    createdAt
    updatedAt
    publishedAt
    translations {
      locale
      title
      handle
      description
      metaTitle
      metaDescription
    }
    variants {
      id
      sku
      barcode
      title
      inventoryQuantity
      inventoryPolicy
      inStock
      prices {
        currencyCode
        amount
        compareAtAmount
        onSale
      }
    }
    images {
      id
      mediaId
      url
      altText
      position
    }
  }
}`;

export const PRODUCT_ATTRIBUTE_VALUES_QUERY = `
query ProductAdminAttributeValues($tenantId: UUID!, $productId: UUID!, $locale: String!) {
  productAttributeValues(tenantId: $tenantId, productId: $productId, locale: $locale) {
    attributeId
    kind
    text
    integer
    decimal
    boolean
    date
    datetime
    optionId
    optionIds
    json
    detached
  }
}`;

export const UPDATE_PRODUCT_MUTATION = `
mutation ProductAdminUpdateProduct($idempotencyKey: String!, $id: UUID!, $input: UpdateProductInput!) {
  updateProduct(idempotencyKey: $idempotencyKey, id: $id, input: $input) {
    id
    status
    sellerId
    vendor
    productType
    shippingProfileSlug
    primaryCategoryId
    tags
    createdAt
    updatedAt
    publishedAt
    translations {
      locale
      title
      handle
      description
      metaTitle
      metaDescription
    }
    variants {
      id
      sku
      barcode
      title
      inventoryQuantity
      inventoryPolicy
      inStock
      prices {
        currencyCode
        amount
        compareAtAmount
        onSale
      }
    }
    images {
      id
      mediaId
      url
      altText
      position
    }
  }
}`;

export const DELETE_PRODUCT_MUTATION = `
mutation ProductAdminDeleteProduct($idempotencyKey: String!, $id: UUID!) {
  deleteProduct(idempotencyKey: $idempotencyKey, id: $id)
}`;

export const CREATE_VARIANT_MUTATION = `
mutation ProductAdminCreateVariant($idempotencyKey: String!, $productId: UUID!, $input: CreateVariantInput!) {
  createProductVariant(idempotencyKey: $idempotencyKey, productId: $productId, input: $input) {
    id
    sku
    barcode
    title
    inventoryQuantity
    inventoryPolicy
    inStock
    prices {
      currencyCode
      amount
      compareAtAmount
      onSale
    }
  }
}`;

export const UPDATE_VARIANT_MUTATION = `
mutation ProductAdminUpdateVariant($idempotencyKey: String!, $id: UUID!, $input: UpdateVariantInput!) {
  updateProductVariant(idempotencyKey: $idempotencyKey, id: $id, input: $input) {
    id
    sku
    barcode
    title
    inventoryQuantity
    inventoryPolicy
    inStock
    prices {
      currencyCode
      amount
      compareAtAmount
      onSale
    }
  }
}`;

export const DELETE_VARIANT_MUTATION = `
mutation ProductAdminDeleteVariant($idempotencyKey: String!, $id: UUID!) {
  deleteProductVariant(idempotencyKey: $idempotencyKey, id: $id)
}`;

export const ADD_PRODUCT_IMAGE_MUTATION = `
mutation ProductAdminAddImage($idempotencyKey: String!, $productId: UUID!, $input: AddProductImageInput!) {
  addProductImage(idempotencyKey: $idempotencyKey, productId: $productId, input: $input) {
    id
    mediaId
    url
    altText
    position
  }
}`;

export const UPDATE_PRODUCT_IMAGE_MUTATION = `
mutation ProductAdminUpdateImage($idempotencyKey: String!, $productId: UUID!, $id: UUID!, $input: UpdateProductImageInput!) {
  updateProductImage(idempotencyKey: $idempotencyKey, productId: $productId, id: $id, input: $input) {
    id
    mediaId
    url
    altText
    position
  }
}`;

export const DELETE_PRODUCT_IMAGE_MUTATION = `
mutation ProductAdminDeleteImage($idempotencyKey: String!, $productId: UUID!, $id: UUID!) {
  deleteProductImage(idempotencyKey: $idempotencyKey, productId: $productId, id: $id)
}`;

export const REORDER_PRODUCT_IMAGES_MUTATION = `
mutation ProductAdminReorderImages($idempotencyKey: String!, $productId: UUID!, $imageIds: [UUID!]!) {
  reorderProductImages(idempotencyKey: $idempotencyKey, productId: $productId, imageIds: $imageIds)
}`;

export const SAVE_ATTRIBUTE_VALUES_MUTATION = `
mutation ProductAdminSaveAttributeValues($idempotencyKey: String!, $productId: UUID!, $locale: String!, $patches: [ProductAttributeValuePatchInput!]!) {
  saveProductAttributeValues(idempotencyKey: $idempotencyKey, productId: $productId, locale: $locale, patches: $patches) {
    attributeId
    kind
    text
    integer
    decimal
    boolean
    date
    datetime
    optionId
    optionIds
    json
    detached
  }
}`;

export type UpdateProductInput = {
  translations?: Array<{
    locale: string;
    title: string;
    handle?: string | null;
    description?: string | null;
    metaTitle?: string | null;
    metaDescription?: string | null;
  }>;
  sellerId?: string | null;
  vendor?: string | null;
  productType?: string | null;
  shippingProfileSlug?: string | null;
  primaryCategoryId?: string | null;
  tags?: string[];
  status?: string | null;
};

export type CreateVariantInput = {
  sku?: string | null;
  barcode?: string | null;
  shippingProfileSlug?: string | null;
  prices: Array<{
    currencyCode: string;
    amount: number;
    compareAtAmount?: number | null;
  }>;
  inventoryQuantity?: number;
  inventoryPolicy?: string;
};

export type UpdateVariantInput = {
  sku?: string | null;
  barcode?: string | null;
  shippingProfileSlug?: string | null;
  prices?: Array<{
    currencyCode: string;
    amount: number;
    compareAtAmount?: number | null;
  }>;
  inventoryQuantity?: number;
  inventoryPolicy?: string;
};

export type AddProductImageInput = {
  mediaId: string;
  position?: number | null;
  altText?: string | null;
  locale?: string | null;
};

export type UpdateProductImageInput = {
  position?: number | null;
  altText?: string | null;
  locale?: string | null;
};

export async function fetchProductDetail(
  opts: GqlOpts,
  id: string,
  locale?: string
): Promise<ProductDetail | null> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage products.');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { tenantId: string; id: string; locale?: string },
    { product: ProductDetail | null }
  >(
    PRODUCT_DETAIL_QUERY,
    { tenantId: opts.tenantId, id, locale },
    opts.token,
    opts.tenantSlug
  );

  return data.product;
}

export async function fetchProductAttributeValues(
  opts: GqlOpts,
  productId: string,
  locale: string
): Promise<ProductAttributeValueItem[]> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    return [];
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { tenantId: string; productId: string; locale: string },
    { productAttributeValues: ProductAttributeValueItem[] }
  >(
    PRODUCT_ATTRIBUTE_VALUES_QUERY,
    { tenantId: opts.tenantId, productId, locale },
    opts.token,
    opts.tenantSlug
  );

  return data.productAttributeValues ?? [];
}

export async function updateProductDetail(
  opts: GqlOpts,
  id: string,
  input: UpdateProductInput
): Promise<ProductDetail> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage products.');
  }

  const idempotencyKey = crypto.randomUUID();
  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { idempotencyKey: string; id: string; input: UpdateProductInput },
    { updateProduct: ProductDetail }
  >(
    UPDATE_PRODUCT_MUTATION,
    { idempotencyKey, id, input },
    opts.token,
    opts.tenantSlug
  );

  return data.updateProduct;
}

export async function deleteProductDetail(
  opts: GqlOpts,
  id: string
): Promise<boolean> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage products.');
  }

  const idempotencyKey = crypto.randomUUID();
  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { idempotencyKey: string; id: string },
    { deleteProduct: boolean }
  >(
    DELETE_PRODUCT_MUTATION,
    { idempotencyKey, id },
    opts.token,
    opts.tenantSlug
  );

  return data.deleteProduct;
}

export async function createVariant(
  opts: GqlOpts,
  productId: string,
  input: CreateVariantInput
): Promise<ProductVariant> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage variants.');
  }

  const idempotencyKey = crypto.randomUUID();
  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { idempotencyKey: string; productId: string; input: CreateVariantInput },
    { createProductVariant: ProductVariant }
  >(
    CREATE_VARIANT_MUTATION,
    { idempotencyKey, productId, input },
    opts.token,
    opts.tenantSlug
  );

  return data.createProductVariant;
}

export async function updateVariant(
  opts: GqlOpts,
  id: string,
  input: UpdateVariantInput
): Promise<ProductVariant> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage variants.');
  }

  const idempotencyKey = crypto.randomUUID();
  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { idempotencyKey: string; id: string; input: UpdateVariantInput },
    { updateProductVariant: ProductVariant }
  >(
    UPDATE_VARIANT_MUTATION,
    { idempotencyKey, id, input },
    opts.token,
    opts.tenantSlug
  );

  return data.updateProductVariant;
}

export async function deleteVariant(
  opts: GqlOpts,
  id: string
): Promise<boolean> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage variants.');
  }

  const idempotencyKey = crypto.randomUUID();
  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { idempotencyKey: string; id: string },
    { deleteProductVariant: boolean }
  >(
    DELETE_VARIANT_MUTATION,
    { idempotencyKey, id },
    opts.token,
    opts.tenantSlug
  );

  return data.deleteProductVariant;
}

export async function addProductImage(
  opts: GqlOpts,
  productId: string,
  input: AddProductImageInput
): Promise<ProductImage> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage images.');
  }

  const idempotencyKey = crypto.randomUUID();
  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { idempotencyKey: string; productId: string; input: AddProductImageInput },
    { addProductImage: ProductImage }
  >(
    ADD_PRODUCT_IMAGE_MUTATION,
    { idempotencyKey, productId, input },
    opts.token,
    opts.tenantSlug
  );

  return data.addProductImage;
}

export async function deleteProductImage(
  opts: GqlOpts,
  productId: string,
  id: string
): Promise<boolean> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage images.');
  }

  const idempotencyKey = crypto.randomUUID();
  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { idempotencyKey: string; productId: string; id: string },
    { deleteProductImage: boolean }
  >(
    DELETE_PRODUCT_IMAGE_MUTATION,
    { idempotencyKey, productId, id },
    opts.token,
    opts.tenantSlug
  );

  return data.deleteProductImage;
}

export async function reorderProductImages(
  opts: GqlOpts,
  productId: string,
  imageIds: string[]
): Promise<boolean> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage images.');
  }

  const idempotencyKey = crypto.randomUUID();
  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { idempotencyKey: string; productId: string; imageIds: string[] },
    { reorderProductImages: boolean }
  >(
    REORDER_PRODUCT_IMAGES_MUTATION,
    { idempotencyKey, productId, imageIds },
    opts.token,
    opts.tenantSlug
  );

  return data.reorderProductImages;
}

export async function saveProductAttributeValues(
  opts: GqlOpts,
  productId: string,
  locale: string,
  patches: ProductAttributeValuePatch[]
): Promise<ProductAttributeValueItem[]> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to save attribute values.');
  }

  const idempotencyKey = crypto.randomUUID();
  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    {
      idempotencyKey: string;
      productId: string;
      locale: string;
      patches: ProductAttributeValuePatch[];
    },
    { saveProductAttributeValues: ProductAttributeValueItem[] }
  >(
    SAVE_ATTRIBUTE_VALUES_MUTATION,
    { idempotencyKey, productId, locale, patches },
    opts.token,
    opts.tenantSlug
  );

  return data.saveProductAttributeValues;
}
