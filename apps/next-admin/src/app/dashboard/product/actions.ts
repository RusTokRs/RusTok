/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

'use server';

import { auth } from '@/auth';
import { graphqlRequest } from '@/shared/api/graphql';
import {
  createProduct,
  updateProductDetail,
  deleteProductDetail,
  createVariant,
  updateVariant,
  deleteVariant,
  addProductImage,
  deleteProductImage,
  reorderProductImages,
  saveProductAttributeValues,
  getCategoryEffectiveForm,
  type ProductTranslation,
  type ProductVariant,
  type ProductAttributeValuePatch,
  type UpdateProductInput,
  type CreateVariantInput,
  type UpdateVariantInput
} from '@rustok/product-admin';
import { revalidatePath } from 'next/cache';
import { redirect } from 'next/navigation';

async function getAuthOpts() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;

  if (!token || !tenantSlug || !tenantId) {
    throw new Error('Sign in again to manage products.');
  }

  return {
    graphql: graphqlRequest,
    token,
    tenantSlug,
    tenantId
  };
}

export async function saveProductAction(payload: {
  id?: string;
  isNew: boolean;
  translations: ProductTranslation[];
  vendor?: string;
  sellerId?: string;
  productType?: string;
  shippingProfileSlug?: string;
  primaryCategoryId?: string | null;
  tags: string[];
  status?: string;
  initialVariant?: {
    sku: string;
    barcode: string;
    priceAmount: number;
    currencyCode: string;
    compareAtAmount?: number | null;
    inventoryQuantity: number;
    inventoryPolicy: string;
  };
  attributePatches: ProductAttributeValuePatch[];
  activeLocale: string;
}): Promise<{ id: string }> {
  const opts = await getAuthOpts();

  if (payload.isNew) {
    const primaryTranslation = payload.translations[0];
    if (!primaryTranslation) {
      throw new Error('At least one translation with title is required.');
    }

    const created = await createProduct(opts, {
      title: primaryTranslation.title,
      handle: primaryTranslation.handle || undefined,
      description: primaryTranslation.description || undefined,
      locale: primaryTranslation.locale,
      vendor: payload.vendor,
      sellerId: payload.sellerId,
      productType: payload.productType,
      tags: payload.tags,
      sku: payload.initialVariant?.sku,
      priceAmount: payload.initialVariant?.priceAmount,
      currencyCode: payload.initialVariant?.currencyCode,
      inventoryQuantity: payload.initialVariant?.inventoryQuantity,
      publish: payload.status === 'ACTIVE'
    });

    // If there were attribute values specified, save them
    if (payload.attributePatches.length > 0 && created.id) {
      try {
        await saveProductAttributeValues(
          opts,
          created.id,
          payload.activeLocale,
          payload.attributePatches
        );
      } catch (err) {
        console.error('Failed to save attributes for new product:', err);
      }
    }

    revalidatePath('/dashboard/product');
    return { id: created.id };
  } else {
    if (!payload.id) {
      throw new Error('Product ID is required for update.');
    }

    const updateInput: UpdateProductInput = {
      translations: payload.translations.map((t) => ({
        locale: t.locale,
        title: t.title,
        handle: t.handle || null,
        description: t.description || null,
        metaTitle: t.metaTitle || null,
        metaDescription: t.metaDescription || null
      })),
      vendor: payload.vendor || null,
      sellerId: payload.sellerId || null,
      productType: payload.productType || null,
      shippingProfileSlug: payload.shippingProfileSlug || null,
      primaryCategoryId: payload.primaryCategoryId,
      tags: payload.tags,
      status: payload.status || null
    };

    const updated = await updateProductDetail(opts, payload.id, updateInput);

    // Save attribute patches if any
    if (payload.attributePatches.length > 0) {
      await saveProductAttributeValues(
        opts,
        payload.id,
        payload.activeLocale,
        payload.attributePatches
      );
    }

    revalidatePath('/dashboard/product');
    revalidatePath(`/dashboard/product/${payload.id}`);
    return { id: updated.id };
  }
}

export async function deleteProductAction(id: string): Promise<boolean> {
  const opts = await getAuthOpts();
  const success = await deleteProductDetail(opts, id);
  revalidatePath('/dashboard/product');
  return success;
}

export async function addVariantAction(
  productId: string,
  variant: Omit<ProductVariant, 'id'>
): Promise<ProductVariant> {
  const opts = await getAuthOpts();
  const input: CreateVariantInput = {
    sku: variant.sku,
    barcode: variant.barcode,
    shippingProfileSlug: null,
    inventoryQuantity: variant.inventoryQuantity,
    inventoryPolicy: variant.inventoryPolicy,
    prices: variant.prices.map((p) => ({
      currencyCode: p.currencyCode,
      amount: p.amount,
      compareAtAmount: p.compareAtAmount
    }))
  };

  const created = await createVariant(opts, productId, input);
  revalidatePath(`/dashboard/product/${productId}`);
  return created;
}

export async function updateVariantAction(
  id: string,
  variant: Partial<ProductVariant>
): Promise<ProductVariant> {
  const opts = await getAuthOpts();
  const input: UpdateVariantInput = {
    sku: variant.sku,
    barcode: variant.barcode,
    inventoryQuantity: variant.inventoryQuantity,
    inventoryPolicy: variant.inventoryPolicy,
    prices: variant.prices?.map((p) => ({
      currencyCode: p.currencyCode,
      amount: p.amount,
      compareAtAmount: p.compareAtAmount
    }))
  };

  const updated = await updateVariant(opts, id, input);
  return updated;
}

export async function deleteVariantAction(id: string): Promise<boolean> {
  const opts = await getAuthOpts();
  return deleteVariant(opts, id);
}

export async function addImageAction(
  productId: string,
  input: { mediaId: string; altText?: string; locale?: string }
) {
  const opts = await getAuthOpts();
  const res = await addProductImage(opts, productId, {
    mediaId: input.mediaId,
    altText: input.altText,
    locale: input.locale
  });
  revalidatePath(`/dashboard/product/${productId}`);
  return res;
}

export async function deleteImageAction(productId: string, id: string) {
  const opts = await getAuthOpts();
  const res = await deleteProductImage(opts, productId, id);
  revalidatePath(`/dashboard/product/${productId}`);
  return res;
}

export async function reorderImagesAction(
  productId: string,
  imageIds: string[]
) {
  const opts = await getAuthOpts();
  const res = await reorderProductImages(opts, productId, imageIds);
  revalidatePath(`/dashboard/product/${productId}`);
  return res;
}

export async function fetchEffectiveFormAction(
  categoryId: string,
  locale: string
) {
  const opts = await getAuthOpts();
  return getCategoryEffectiveForm(opts, categoryId, locale);
}
