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
  type ProductTranslation,
  type ProductVariant,
  type ProductAttributeValuePatch,
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
  type ProductBundle
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

    // Attribute values need the created product id, so they are written after
    // creation. Attribute failure must not be swallowed: a draft product is
    // rolled back so the operator can retry the form safely, while a published
    // product is kept and reported as a partial success.
    if (payload.attributePatches.length > 0 && created.id) {
      try {
        await saveProductAttributeValues(
          opts,
          created.id,
          payload.activeLocale,
          payload.attributePatches
        );
      } catch (err) {
        const reason =
          err instanceof Error ? err.message : 'unknown attribute error';

        let rollbackSucceeded = false;
        if (payload.status !== 'ACTIVE') {
          try {
            await deleteProductDetail(opts, created.id);
            rollbackSucceeded = true;
          } catch (rollbackError) {
            console.error(
              'Failed to roll back product after attribute error:',
              rollbackError
            );
          }
        }

        revalidatePath('/dashboard/product');
        revalidatePath(`/dashboard/product/${created.id}`);

        if (rollbackSucceeded) {
          throw new Error(
            `Product creation was rolled back because attribute values could not be saved: ${reason}. Nothing was saved — fix the values and save again.`
          );
        }

        throw new Error(
          `Product was created, but attribute values could not be saved: ${reason}. Open /dashboard/product/${created.id} to review the product and retry the attributes.`
        );
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

    // Product details are already persisted at this point: attribute values are
    // saved afterwards because their effective schema depends on the category
    // assigned above. Surface a partial-success error instead of claiming the
    // whole save failed, and keep the paths revalidated so the saved product is
    // visible for a retry.
    if (payload.attributePatches.length > 0) {
      try {
        await saveProductAttributeValues(
          opts,
          payload.id,
          payload.activeLocale,
          payload.attributePatches
        );
      } catch (err) {
        revalidatePath('/dashboard/product');
        revalidatePath(`/dashboard/product/${payload.id}`);
        const reason =
          err instanceof Error ? err.message : 'unknown attribute error';
        throw new Error(
          `Product details were saved, but attribute values could not be saved: ${reason}. Fix the issue and save again to retry the attributes.`
        );
      }
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



