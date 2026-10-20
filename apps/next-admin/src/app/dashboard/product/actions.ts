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
  productWriteOutcome,
  type ProductSavePayload,
  type ProductWriteOutcome,
  type ProductWriteScope,
  updateProductDetail,
  deleteProductDetail,
  createVariant,
  updateVariant,
  deleteVariant,
  addProductImage,
  deleteProductImage,
  reorderProductImages,
  setVariantAxes,
  getCategoryEffectiveForm,
  fetchProductRelations,
  addProductRelation,
  removeProductRelation,
  reorderProductRelations,
  fetchActivePriceLists,
  upsertVariantPrice,
  previewVariantDiscount,
  applyVariantDiscount,
  listProducts,
  fetchProductBundles,
  fetchBundles,
  createBundle,
  updateBundle,
  deleteBundle,
  addBundleItem,
  removeBundleItem,
  type ProductVariant,
  type UpdateProductInput,
  type CreateVariantInput,
  type UpdateVariantInput,
  type ProductRelationType,
  type AddProductRelationInput,
  type UpsertVariantPriceInput,
  type VariantDiscountInput,
  type CreateBundleInput,
  type UpdateBundleInput,
  type AddBundleItemInput,
  type ProductBundle,
  type VariantAxisConfig,
  type SetVariantAxesInput
} from '@rustok/product-admin';
import { revalidatePath } from 'next/cache';
import { redirect } from 'next/navigation';

async function getAuthOpts(expectedScope?: ProductWriteScope) {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;

  if (
    expectedScope &&
    (expectedScope.tenantId !== tenantId ||
      expectedScope.actorId !== session?.user?.id)
  ) {
    throw new Error('The Product editor authentication context changed');
  }

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

async function getProductWriteAuthOpts(
  scope: ProductWriteScope,
  idempotencyKey: string
) {
  if (
    typeof idempotencyKey !== 'string' ||
    !/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(
      idempotencyKey
    )
  ) {
    throw new Error('Product editor attempt identity is missing or invalid');
  }
  if (!scope?.tenantId || !scope?.actorId)
    throw new Error('Product editor context is missing');
  return getAuthOpts(scope);
}

export async function saveProductAction(
  payload: ProductSavePayload,
  idempotencyKey: string
): Promise<ProductWriteOutcome> {
  const outcome = await productWriteOutcome(async () => {
    const opts = await getProductWriteAuthOpts(payload.scope, idempotencyKey);

    if (payload.isNew) {
      const primaryTranslation = payload.translations[0];
      if (!primaryTranslation) {
        throw new Error('At least one translation with title is required.');
      }

      const created = await createProduct(
        opts,
        {
          title: primaryTranslation.title,
          translations: payload.translations,
          primaryCategoryId: payload.primaryCategoryId,
          shippingProfileSlug: payload.shippingProfileSlug,
          handle: primaryTranslation.handle || undefined,
          description: primaryTranslation.description || undefined,
          locale: primaryTranslation.locale,
          vendor: payload.vendor,
          sellerId: payload.sellerId,
          productType: payload.productType,
          tags: payload.tags,
          sku: payload.initialVariant?.sku,
          barcode: payload.initialVariant?.barcode,
          compareAtAmount: payload.initialVariant?.compareAtAmount,
          inventoryPolicy: payload.initialVariant?.inventoryPolicy,
          priceAmount: payload.initialVariant?.priceAmount,
          currencyCode: payload.initialVariant?.currencyCode,
          inventoryQuantity: payload.initialVariant?.inventoryQuantity,
          attributeValues: payload.attributePatches.length
            ? [
                {
                  locale: payload.activeLocale,
                  patches: payload.attributePatches
                }
              ]
            : [],
          publish: payload.status === 'ACTIVE'
        },
        idempotencyKey
      );

      return { id: created.id, revision: created.revision };
    } else {
      if (!payload.id) {
        throw new Error('Product ID is required for update.');
      }

      const updateInput: UpdateProductInput = {
        attributeValues: payload.attributePatches.length
          ? [
              {
                locale: payload.activeLocale,
                patches: payload.attributePatches
              }
            ]
          : [],
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
        status: payload.status || null,
        // The document revision the editor loaded; the owner refuses a save that would
        // overwrite a concurrent edit instead of silently losing it.
        revision: payload.revision ?? null
      };

      const updated = await updateProductDetail(
        opts,
        payload.id,
        updateInput,
        idempotencyKey
      );

      return { id: updated.id, revision: updated.revision ?? null };
    }
  });
  await refreshProductAfterWrite(outcome);
  return outcome;
}

export async function changeProductStatusAction(
  id: string,
  status: string,
  revision: number | null,
  idempotencyKey: string,
  scope: ProductWriteScope
): Promise<ProductWriteOutcome> {
  const outcome = await productWriteOutcome(async () => {
    const opts = await getProductWriteAuthOpts(scope, idempotencyKey);
    const updated = await updateProductDetail(
      opts,
      id,
      { status, revision },
      idempotencyKey
    );
    return { id: updated.id, revision: updated.revision ?? null };
  });
  await refreshProductAfterWrite(outcome);
  return outcome;
}

export async function deleteProductAction(
  id: string,
  idempotencyKey: string,
  scope: ProductWriteScope
): Promise<ProductWriteOutcome> {
  const outcome = await productWriteOutcome(async () => {
    const opts = await getProductWriteAuthOpts(scope, idempotencyKey);
    const success = await deleteProductDetail(opts, id, idempotencyKey);
    if (!success) throw new Error('Product deletion was not acknowledged');
    return { id, revision: null };
  });
  await refreshProductAfterWrite(outcome);
  return outcome;
}

async function refreshProductAfterWrite(outcome: ProductWriteOutcome) {
  if (outcome.status !== 'confirmed') return;
  try {
    revalidatePath('/dashboard/product');
    revalidatePath(`/dashboard/product/${outcome.value.id}`);
  } catch {
    // Cache invalidation is not the owner commit. Keep its acknowledged success.
    // The next page read can refresh independently; never mint another write.
  }
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
  variant: UpdateVariantInput
): Promise<ProductVariant> {
  const opts = await getAuthOpts();
  const input: UpdateVariantInput = {
    sku: variant.sku,
    barcode: variant.barcode,
    shippingProfileSlug: variant.shippingProfileSlug,
    weight: variant.weight,
    weightUnit: variant.weightUnit,
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

export async function setVariantAxesAction(
  productId: string,
  input: SetVariantAxesInput
): Promise<VariantAxisConfig[]> {
  const opts = await getAuthOpts();
  const axes = await setVariantAxes(opts, productId, input);
  revalidatePath(`/dashboard/product/${productId}`);
  return axes;
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

export async function addProductRelationAction(input: AddProductRelationInput) {
  const opts = await getAuthOpts();
  const res = await addProductRelation(opts, input);
  revalidatePath(`/dashboard/product/${input.productId}`);
  return res;
}

export async function removeProductRelationAction(
  id: string,
  productId: string
) {
  const opts = await getAuthOpts();
  const res = await removeProductRelation(opts, id);
  revalidatePath(`/dashboard/product/${productId}`);
  return res;
}

export async function reorderProductRelationsAction(
  productId: string,
  relationType: ProductRelationType,
  orderedIds: string[]
) {
  const opts = await getAuthOpts();
  const res = await reorderProductRelations(
    opts,
    productId,
    relationType,
    orderedIds
  );
  revalidatePath(`/dashboard/product/${productId}`);
  return res;
}

export async function searchProductsAction(query: string) {
  const opts = await getAuthOpts();
  const res = await listProducts(opts, { search: query, perPage: 20 });
  return res.items || [];
}

export async function fetchActivePriceListsAction(
  channelId?: string,
  channelSlug?: string
) {
  const opts = await getAuthOpts();
  return fetchActivePriceLists(opts, channelId, channelSlug);
}

export async function upsertVariantPriceAction(
  productId: string,
  variantId: string,
  input: UpsertVariantPriceInput
) {
  const opts = await getAuthOpts();
  const res = await upsertVariantPrice(opts, variantId, input);
  revalidatePath(`/dashboard/product/${productId}`);
  return res;
}

export async function previewVariantDiscountAction(
  variantId: string,
  input: VariantDiscountInput
) {
  const opts = await getAuthOpts();
  return previewVariantDiscount(opts, variantId, input);
}

export async function applyVariantDiscountAction(
  productId: string,
  variantId: string,
  input: VariantDiscountInput
) {
  const opts = await getAuthOpts();
  const res = await applyVariantDiscount(opts, variantId, input);
  revalidatePath(`/dashboard/product/${productId}`);
  return res;
}

export async function fetchProductBundlesAction(
  productId: string,
  locale?: string
): Promise<ProductBundle[]> {
  const opts = await getAuthOpts();
  return fetchProductBundles(opts, productId, locale);
}

export async function fetchBundlesAction(
  filter?: {
    search?: string;
    status?: string;
    bundleType?: string;
    page?: number;
    perPage?: number;
  },
  locale?: string
) {
  const opts = await getAuthOpts();
  return fetchBundles(opts, filter, locale);
}

export async function createBundleAction(
  input: CreateBundleInput,
  locale?: string
) {
  const opts = await getAuthOpts();
  const res = await createBundle(opts, input, locale);
  if (input.bundleProductId) {
    revalidatePath(`/dashboard/product/${input.bundleProductId}`);
  }
  revalidatePath('/dashboard/product/bundles');
  return res;
}

export async function updateBundleAction(
  productId: string,
  bundleId: string,
  input: UpdateBundleInput,
  locale?: string
) {
  const opts = await getAuthOpts();
  const res = await updateBundle(opts, bundleId, input, locale);
  revalidatePath(`/dashboard/product/${productId}`);
  revalidatePath('/dashboard/product/bundles');
  return res;
}

export async function deleteBundleAction(id: string) {
  const opts = await getAuthOpts();
  const res = await deleteBundle(opts, id);
  revalidatePath('/dashboard/product/bundles');
  return res;
}

export async function addBundleItemAction(
  productId: string,
  input: AddBundleItemInput
) {
  const opts = await getAuthOpts();
  const res = await addBundleItem(opts, input);
  revalidatePath(`/dashboard/product/${productId}`);
  revalidatePath('/dashboard/product/bundles');
  return res;
}

export async function removeBundleItemAction(
  productId: string,
  bundleId: string,
  itemId: string
) {
  const opts = await getAuthOpts();
  const res = await removeBundleItem(opts, bundleId, itemId);
  revalidatePath(`/dashboard/product/${productId}`);
  revalidatePath('/dashboard/product/bundles');
  return res;
}
