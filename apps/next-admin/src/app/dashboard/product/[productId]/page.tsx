/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { auth } from '@/auth';
import { graphqlRequest } from '@/shared/api/graphql';
import { PageContainer } from '@/widgets/app-shell';
import { notFound } from 'next/navigation';
import {
  fetchProductDetail,
  fetchProductAttributeValues,
  fetchProductRelations,
  fetchProductBundles,
  listCatalogCategories,
  listBrands,
  getCategoryEffectiveForm,
  ProductEditorPage,
  isProductEditorIntent,
  PRODUCT_EDITOR_INTENT_QUERY_KEY,
  PRODUCT_VARIANT_INTENT_QUERY_KEY
} from '@rustok/product-admin';
import {
  saveProductAction,
  changeProductStatusAction,
  deleteProductAction,
  addVariantAction,
  updateVariantAction,
  deleteVariantAction,
  setVariantAxesAction,
  previewVariantAxesAction,
  addImageAction,
  deleteImageAction,
  reorderImagesAction,
  updateImageAction,
  fetchActivePriceListsAction,
  upsertVariantPriceAction,
  previewVariantDiscountAction,
  applyVariantDiscountAction,
  listProductMediaAction,
  fetchEffectiveFormAction,
  addProductRelationAction,
  removeProductRelationAction,
  reorderProductRelationsAction,
  searchProductsAction,
  createBundleAction,
  updateBundleAction,
  addBundleItemAction,
  removeBundleItemAction
} from '../actions';

export const metadata = {
  title: 'RusTok Admin: Product Editor'
};

type PageProps = {
  params: Promise<{ productId: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
};

export default async function ProductDetailPage({
  params,
  searchParams
}: PageProps) {
  const { productId } = await params;
  const session = await auth();
  const query = await searchParams;
  const editorIntent = query[PRODUCT_EDITOR_INTENT_QUERY_KEY];
  const variantIntent = query[PRODUCT_VARIANT_INTENT_QUERY_KEY];
  const hasProductRecovery = isProductEditorIntent(editorIntent);
  const hasVariantRecovery = isProductEditorIntent(variantIntent);

  const opts = {
    graphql: graphqlRequest,
    token: session?.user?.rustokToken ?? null,
    tenantSlug: session?.user?.tenantSlug ?? null,
    tenantId: session?.user?.tenantId ?? null
  };

  const isNew = productId === 'new';
  const writeScope = {
    tenantId: session?.user?.tenantId ?? '',
    actorId: session?.user?.id ?? ''
  };

  const editorKey = JSON.stringify([
    writeScope.tenantId,
    writeScope.actorId,
    productId,
    editorIntent ?? null,
    variantIntent ?? null
  ]);

  // Load category taxonomy
  let categories: Awaited<ReturnType<typeof listCatalogCategories>> = [];
  try {
    categories = await listCatalogCategories(opts, 'en');
  } catch (err) {
    console.error('Failed to load catalog categories for product editor:', err);
  }

  // Load the brand directory for the product's brand picker
  let brands: Awaited<ReturnType<typeof listBrands>>['items'] = [];
  try {
    brands = (await listBrands(opts)).items;
  } catch (err) {
    console.error('Failed to load brands for product editor:', err);
  }

  if (isNew) {
    return (
      <PageContainer
        pageTitle='Create Product'
        pageDescription='Configure catalog product entity, primary category, pricing, and specifications.'
      >
        <ProductEditorPage
          key={editorKey}
          resourceId={productId}
          writeScope={writeScope}
          isNew={true}
          categories={categories}
          brands={brands}
          onSaveProduct={saveProductAction}
          onChangeStatus={changeProductStatusAction}
          fetchEffectiveForm={fetchEffectiveFormAction}
        />
      </PageContainer>
    );
  }

  // Fetch product detail for existing product
  let product: Awaited<ReturnType<typeof fetchProductDetail>> = null;
  try {
    product = await fetchProductDetail(opts, productId);
  } catch (err) {
    console.error('Failed to load product detail:', err);
  }

  if (!product) {
    if (!hasProductRecovery && !hasVariantRecovery) notFound();
    // A delete may have committed before its response was lost. Do not hide the
    // caller's durable recovery behind a read-side 404; no editable draft here.
    return (
      <PageContainer pageTitle='Recover Product operation'>
        <ProductEditorPage
          key={editorKey}
          resourceId={productId}
          writeScope={writeScope}
          recoveryOnly
          variantRecoveryOnly={!hasProductRecovery && hasVariantRecovery}
          categories={[]}
          onSaveProduct={saveProductAction}
          onChangeStatus={changeProductStatusAction}
          onDeleteProduct={deleteProductAction}
          onSetVariantAxes={setVariantAxesAction}
          onAddVariant={addVariantAction}
          onUpdateVariant={updateVariantAction}
          onDeleteVariant={deleteVariantAction}
        />
      </PageContainer>
    );
  }

  // Pre-load effective form and attribute values if category is assigned
  let effectiveForm = null;
  let attributeValues: Awaited<ReturnType<typeof fetchProductAttributeValues>> =
    [];
  if (product.primaryCategoryId) {
    try {
      effectiveForm = await getCategoryEffectiveForm(
        opts,
        product.primaryCategoryId,
        'en'
      );
    } catch (err) {
      console.error('Failed to pre-load category effective form:', err);
    }
  }

  try {
    attributeValues = await fetchProductAttributeValues(opts, productId, 'en');
  } catch (err) {
    console.error('Failed to pre-load product attribute values:', err);
  }

  let relations: Awaited<ReturnType<typeof fetchProductRelations>> = [];
  try {
    relations = await fetchProductRelations(opts, productId);
  } catch (err) {
    console.error('Failed to pre-load product relations:', err);
  }

  let bundles: Awaited<ReturnType<typeof fetchProductBundles>> = [];
  try {
    bundles = await fetchProductBundles(opts, productId);
  } catch (err) {
    console.error('Failed to pre-load product bundles:', err);
  }

  const primaryTranslation = product.translations[0] || null;

  return (
    <PageContainer
      pageTitle={primaryTranslation?.title || 'Product Editor'}
      pageDescription='RusTok product catalog editor, variant matrix, media gallery, relations, bundles, and effective specifications.'
    >
      <ProductEditorPage
        key={editorKey}
        resourceId={productId}
        writeScope={writeScope}
        isNew={false}
        initialProduct={product}
        categories={categories}
        brands={brands}
        initialEffectiveForm={effectiveForm}
        initialAttributeValues={attributeValues}
        initialRelations={relations}
        initialBundles={bundles}
        onSaveProduct={saveProductAction}
        onChangeStatus={changeProductStatusAction}
        onDeleteProduct={deleteProductAction}
        onAddVariant={addVariantAction}
        onUpdateVariant={updateVariantAction}
        onPreviewVariantAxes={previewVariantAxesAction}
        onSetVariantAxes={setVariantAxesAction}
        onDeleteVariant={deleteVariantAction}
        onAddImage={async (pid, img, idempotencyKey) => {
          await addImageAction(pid, img, idempotencyKey);
        }}
        onDeleteImage={async (pid, id) => {
          await deleteImageAction(pid, id);
        }}
        onReorderImages={async (pid, ids) => {
          await reorderImagesAction(pid, ids);
        }}
        onUpdateImage={async (pid, imageId, input, idempotencyKey) => {
          await updateImageAction(pid, imageId, input, idempotencyKey);
        }}
        onFetchPriceLists={fetchActivePriceListsAction}
        onUpsertVariantPrice={async (pid, variantId, input) => {
          await upsertVariantPriceAction(pid, variantId, input);
        }}
        onPreviewVariantDiscount={async (variantId, input) => {
          return previewVariantDiscountAction(variantId, input);
        }}
        onApplyVariantDiscount={async (pid, variantId, input) => {
          await applyVariantDiscountAction(pid, variantId, input);
        }}
        onListMedia={listProductMediaAction}
        onAddRelation={async (input, idempotencyKey) => {
          await addProductRelationAction(input, idempotencyKey);
        }}
        onRemoveRelation={async (id) => {
          await removeProductRelationAction(id, productId);
        }}
        onReorderRelations={async (pid, type, orderedIds) => {
          await reorderProductRelationsAction(pid, type, orderedIds);
        }}
        onCreateBundle={async (input, idempotencyKey) => {
          await createBundleAction(input, undefined, idempotencyKey);
        }}
        onUpdateBundle={async (bundleId, input) => {
          await updateBundleAction(productId, bundleId, input);
        }}
        onAddBundleItem={async (input, idempotencyKey) => {
          await addBundleItemAction(productId, input, idempotencyKey);
        }}
        onRemoveBundleItem={async (bundleId, itemId) => {
          await removeBundleItemAction(productId, bundleId, itemId);
        }}
        onSearchProducts={async (query) => {
          return searchProductsAction(query);
        }}
        fetchEffectiveForm={fetchEffectiveFormAction}
      />
    </PageContainer>
  );
}
