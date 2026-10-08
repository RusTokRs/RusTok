/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

'use client';

import * as React from 'react';
import { useRouter } from 'next/navigation';
import { ProductHeaderBar } from '../components/products/product-header-bar';
import { ProductGeneralCard } from '../components/products/product-general-card';
import { ProductCategoryCard } from '../components/products/product-category-card';
import { ProductVariantsCard } from '../components/products/product-variants-card';
import { ProductMediaCard } from '../components/products/product-media-card';
import { ProductRelationsCard } from '../components/products/product-relations-card';
import { ProductBundleCard } from '../components/products/product-bundle-card';
import { ProductSeoCard } from '../components/products/product-seo-card';
import { Alert, AlertDescription } from '@/shared/ui/shadcn/alert';
import { AlertCircle, CheckCircle2 } from 'lucide-react';
import type {
  ProductListItem,
  ProductDetail,
  ProductTranslation,
  ProductVariant,
  ProductImage,
  CatalogCategorySummary,
  ProductEffectiveForm,
  ProductAttributeValuePatch,
  ProductAttributeValueItem,
  ProductRelation,
  ProductRelationType,
  AddProductRelationInput,
  ActivePriceList,
  ProductBundle,
  CreateBundleInput,
  UpdateBundleInput,
  AddBundleItemInput
} from '../api/types';

export interface ProductEditorPageProps {
  initialProduct?: ProductDetail | null;
  categories: CatalogCategorySummary[];
  initialEffectiveForm?: ProductEffectiveForm | null;
  initialAttributeValues?: ProductAttributeValueItem[];
  initialRelations?: ProductRelation[];
  activePriceLists?: ActivePriceList[];
  isNew?: boolean;
  onSaveProduct: (payload: {
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
    // New product initial variant
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
  }) => Promise<{ id: string } | void>;
  onDeleteProduct?: (id: string) => Promise<void>;
  onAddVariant?: (
    productId: string,
    variant: Omit<ProductVariant, 'id'>
  ) => Promise<void>;
  onUpdateVariant?: (
    id: string,
    variant: Partial<ProductVariant>
  ) => Promise<void>;
  onDeleteVariant?: (id: string) => Promise<void>;
  onAddImage?: (
    productId: string,
    input: { mediaId: string; altText?: string }
  ) => Promise<void>;
  onDeleteImage?: (productId: string, id: string) => Promise<void>;
  onReorderImages?: (productId: string, imageIds: string[]) => Promise<void>;
  onAddRelation?: (input: AddProductRelationInput) => Promise<void>;
  onRemoveRelation?: (id: string) => Promise<void>;
  onReorderRelations?: (
    productId: string,
    relationType: ProductRelationType,
    orderedIds: string[]
  ) => Promise<void>;
  onSearchProducts?: (query: string) => Promise<ProductListItem[]>;
  fetchEffectiveForm?: (
    categoryId: string,
    locale: string
  ) => Promise<ProductEffectiveForm | null>;
  initialBundles?: ProductBundle[];
  onCreateBundle?: (input: CreateBundleInput) => Promise<void>;
  onUpdateBundle?: (
    bundleId: string,
    input: UpdateBundleInput
  ) => Promise<void>;
  onAddBundleItem?: (input: AddBundleItemInput) => Promise<void>;
  onRemoveBundleItem?: (bundleId: string, itemId: string) => Promise<void>;
}

export function ProductEditorPage({
  initialProduct,
  categories,
  initialEffectiveForm = null,
  initialAttributeValues = [],
  initialRelations = [],
  activePriceLists = [],
  initialBundles = [],
  isNew = false,
  onSaveProduct,
  onDeleteProduct,
  onAddVariant,
  onUpdateVariant,
  onDeleteVariant,
  onAddImage,
  onDeleteImage,
  onReorderImages,
  onAddRelation,
  onRemoveRelation,
  onReorderRelations,
  onSearchProducts,
  fetchEffectiveForm,
  onCreateBundle,
  onUpdateBundle,
  onAddBundleItem,
  onRemoveBundleItem
}: ProductEditorPageProps) {
  const router = useRouter();
  const [activeLocale, setActiveLocale] = React.useState('en');
  const [isSaving, setIsSaving] = React.useState(false);
  const [errorMessage, setErrorMessage] = React.useState<string | null>(null);
  const [successMessage, setSuccessMessage] = React.useState<string | null>(
    null
  );

  // General fields
  const [translations, setTranslations] = React.useState<
    Record<string, ProductTranslation>
  >(() => {
    const map: Record<string, ProductTranslation> = {
      en: {
        locale: 'en',
        title: '',
        handle: '',
        description: '',
        metaTitle: '',
        metaDescription: ''
      },
      ru: {
        locale: 'ru',
        title: '',
        handle: '',
        description: '',
        metaTitle: '',
        metaDescription: ''
      }
    };
    if (initialProduct?.translations) {
      for (const t of initialProduct.translations) {
        map[t.locale] = { ...t };
      }
    }
    return map;
  });

  const [vendor, setVendor] = React.useState(initialProduct?.vendor || '');
  const [sellerId, setSellerId] = React.useState(
    initialProduct?.sellerId || ''
  );
  const [productType, setProductType] = React.useState(
    initialProduct?.productType || 'simple'
  );
  const [shippingProfileSlug, setShippingProfileSlug] = React.useState(
    initialProduct?.shippingProfileSlug || ''
  );
  const [tags, setTags] = React.useState<string[]>(initialProduct?.tags || []);
  const [status, setStatus] = React.useState(initialProduct?.status || 'DRAFT');

  // Category & Effective Form fields
  const [selectedCategoryId, setSelectedCategoryId] = React.useState(
    initialProduct?.primaryCategoryId || ''
  );
  const [effectiveForm, setEffectiveForm] =
    React.useState<ProductEffectiveForm | null>(initialEffectiveForm);
  const [isLoadingForm, setIsLoadingForm] = React.useState(false);

  // Attribute Values State
  const [attributeValues, setAttributeValues] = React.useState<
    Record<string, ProductAttributeValuePatch>
  >(() => {
    const map: Record<string, ProductAttributeValuePatch> = {};
    for (const v of initialAttributeValues) {
      map[v.attributeId] = {
        attributeId: v.attributeId,
        kind: v.kind,
        text: v.text,
        integer: v.integer,
        decimal: v.decimal,
        boolean: v.boolean,
        date: v.date,
        datetime: v.datetime,
        optionId: v.optionId,
        optionIds: v.optionIds,
        json: v.json
      };
    }
    return map;
  });
  const [dirtyAttributeIds, setDirtyAttributeIds] = React.useState<Set<string>>(
    new Set()
  );

  // Variants & Media State
  const [variants, setVariants] = React.useState<ProductVariant[]>(
    initialProduct?.variants || []
  );
  const [newVariantDraft, setNewVariantDraft] = React.useState({
    sku: '',
    barcode: '',
    priceAmount: 0,
    currencyCode: 'USD',
    compareAtAmount: null as number | null,
    inventoryQuantity: 0,
    inventoryPolicy: 'deny'
  });
  const [images, setImages] = React.useState<ProductImage[]>(
    initialProduct?.images || []
  );

  // Relations State
  const [relations, setRelations] = React.useState<ProductRelation[]>(
    initialRelations || []
  );

  React.useEffect(() => {
    if (initialRelations) {
      setRelations(initialRelations);
    }
  }, [initialRelations]);

  // Bundles State
  const [bundles, setBundles] = React.useState<ProductBundle[]>(
    initialBundles || []
  );

  React.useEffect(() => {
    if (initialBundles) {
      setBundles(initialBundles);
    }
  }, [initialBundles]);

  // Dynamic Effective Form Loader
  const handleSelectCategory = async (categoryId: string) => {
    setSelectedCategoryId(categoryId);
    if (!categoryId || !fetchEffectiveForm) {
      setEffectiveForm(null);
      return;
    }

    setIsLoadingForm(true);
    try {
      const form = await fetchEffectiveForm(categoryId, activeLocale);
      setEffectiveForm(form);
    } catch (err) {
      console.error('Failed to load category effective form:', err);
    } finally {
      setIsLoadingForm(false);
    }
  };

  const handleTranslationChange = (
    locale: string,
    field: keyof ProductTranslation,
    value: string
  ) => {
    setTranslations((prev) => {
      const current = prev[locale] || {
        locale,
        title: '',
        handle: '',
        description: '',
        metaTitle: '',
        metaDescription: ''
      };
      return {
        ...prev,
        [locale]: {
          ...current,
          [field]: value
        }
      };
    });
  };

  const handleAttributeValueChange = (
    attributeId: string,
    patch: Partial<ProductAttributeValuePatch>
  ) => {
    setAttributeValues((prev) => ({
      ...prev,
      [attributeId]: {
        ...(prev[attributeId] || { attributeId, kind: 'TEXT' }),
        ...patch
      }
    }));
    setDirtyAttributeIds((prev) => new Set(prev).add(attributeId));
  };

  // Main Save Handler
  const handleSave = async () => {
    setErrorMessage(null);
    setSuccessMessage(null);

    const primaryTitle =
      translations[activeLocale]?.title?.trim() ||
      translations.en?.title?.trim();
    if (!primaryTitle) {
      setErrorMessage('Product title is required.');
      return;
    }

    setIsSaving(true);
    try {
      const translationList = Object.values(translations).filter(
        (t) => t.title.trim().length > 0
      );

      // Collect only dirty attribute patches
      const patches: ProductAttributeValuePatch[] = Array.from(
        dirtyAttributeIds
      )
        .map((id) => attributeValues[id])
        .filter(Boolean);

      const res = await onSaveProduct({
        id: initialProduct?.id,
        isNew,
        translations: translationList,
        vendor: vendor.trim() || undefined,
        sellerId: sellerId.trim() || undefined,
        productType,
        shippingProfileSlug: shippingProfileSlug.trim() || undefined,
        primaryCategoryId: selectedCategoryId || null,
        tags,
        status,
        initialVariant: isNew ? newVariantDraft : undefined,
        attributePatches: patches,
        activeLocale
      });

      setDirtyAttributeIds(new Set());
      setSuccessMessage(
        isNew ? 'Product created successfully!' : 'Changes saved successfully.'
      );

      if (isNew && res && res.id) {
        router.push(`/dashboard/product/${res.id}`);
      }
    } catch (err: unknown) {
      const msg =
        err instanceof Error ? err.message : 'Failed to save product.';
      if (!msg.includes('NEXT_REDIRECT')) {
        setErrorMessage(msg);
      }
    } finally {
      setIsSaving(false);
    }
  };

  const handleStatusChange = async (newStatus: string) => {
    setStatus(newStatus);
    if (!isNew && initialProduct?.id) {
      setIsSaving(true);
      try {
        await onSaveProduct({
          id: initialProduct.id,
          isNew: false,
          translations: Object.values(translations).filter(
            (t) => t.title.trim().length > 0
          ),
          primaryCategoryId: selectedCategoryId || null,
          tags,
          status: newStatus,
          attributePatches: [],
          activeLocale
        });
        setSuccessMessage(`Product status updated to ${newStatus}.`);
      } catch (err) {
        setErrorMessage(
          err instanceof Error ? err.message : 'Failed to update status.'
        );
      } finally {
        setIsSaving(false);
      }
    }
  };

  const handleDelete = async () => {
    if (!initialProduct?.id || !onDeleteProduct) return;
    if (confirm('Are you sure you want to permanently delete this product?')) {
      setIsSaving(true);
      try {
        await onDeleteProduct(initialProduct.id);
        router.push('/dashboard/product');
      } catch (err) {
        setErrorMessage(
          err instanceof Error ? err.message : 'Failed to delete product.'
        );
        setIsSaving(false);
      }
    }
  };

  return (
    <div className='animate-in fade-in mx-auto flex max-w-6xl flex-col gap-6 pb-16 duration-150'>
      {/* Top Header Bar with Save & Lifecycle */}
      <ProductHeaderBar
        title={translations[activeLocale]?.title || ''}
        status={status}
        isNew={isNew}
        isSaving={isSaving}
        onSave={handleSave}
        onStatusChange={handleStatusChange}
        onDelete={handleDelete}
      />

      {/* Alerts */}
      {errorMessage && (
        <Alert variant='destructive' className='rounded-2xl'>
          <AlertCircle className='h-4 w-4' />
          <AlertDescription className='text-xs'>
            {errorMessage}
          </AlertDescription>
        </Alert>
      )}
      {successMessage && (
        <Alert className='rounded-2xl border-emerald-500/30 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'>
          <CheckCircle2 className='h-4 w-4' />
          <AlertDescription className='text-xs'>
            {successMessage}
          </AlertDescription>
        </Alert>
      )}

      {/* Two Column Layout */}
      <div className='grid grid-cols-1 items-start gap-6 lg:grid-cols-3'>
        {/* Left (Main) Column - 2 Cols */}
        <div className='space-y-6 lg:col-span-2'>
          {/* General Information */}
          <ProductGeneralCard
            translations={translations}
            activeLocale={activeLocale}
            onActiveLocaleChange={setActiveLocale}
            onTranslationChange={handleTranslationChange}
            vendor={vendor}
            onVendorChange={setVendor}
            sellerId={sellerId}
            onSellerIdChange={setSellerId}
            productType={productType}
            onProductTypeChange={setProductType}
            shippingProfileSlug={shippingProfileSlug}
            onShippingProfileSlugChange={setShippingProfileSlug}
            tags={tags}
            onTagsChange={setTags}
            disabled={isSaving}
          />

          {/* Category & Effective Attributes Form */}
          <ProductCategoryCard
            categories={categories}
            selectedCategoryId={selectedCategoryId}
            onSelectCategory={handleSelectCategory}
            effectiveForm={effectiveForm}
            isLoadingForm={isLoadingForm}
            attributeValues={attributeValues}
            onAttributeValueChange={handleAttributeValueChange}
            disabled={isSaving}
          />

          {/* Pricing & Variants Matrix */}
          <ProductVariantsCard
            variants={variants}
            isNew={isNew}
            activePriceLists={activePriceLists}
            newVariantDraft={newVariantDraft}
            onNewVariantDraftChange={(draft) =>
              setNewVariantDraft({
                ...draft,
                compareAtAmount: draft.compareAtAmount ?? null
              })
            }
            onAddVariant={
              initialProduct?.id && onAddVariant
                ? async (v) => {
                    await onAddVariant(initialProduct.id, v);
                    router.refresh();
                  }
                : undefined
            }
            onUpdateVariant={
              onUpdateVariant
                ? async (id, patch) => {
                    await onUpdateVariant(id, patch);
                    router.refresh();
                  }
                : undefined
            }
            onDeleteVariant={
              onDeleteVariant
                ? async (id) => {
                    await onDeleteVariant(id);
                    router.refresh();
                  }
                : undefined
            }
            disabled={isSaving}
          />

          {/* Product Relations (Cross-sell, Up-sell, Accessories, Alternatives) */}
          {!isNew && initialProduct?.id && (
            <ProductRelationsCard
              productId={initialProduct.id}
              relations={relations}
              onAddRelation={
                onAddRelation
                  ? async (input) => {
                      await onAddRelation(input);
                      router.refresh();
                    }
                  : undefined
              }
              onRemoveRelation={
                onRemoveRelation
                  ? async (id) => {
                      await onRemoveRelation(id);
                      setRelations((prev) => prev.filter((r) => r.id !== id));
                      router.refresh();
                    }
                  : undefined
              }
              onReorderRelations={
                onReorderRelations
                  ? async (pId, type, orderedIds) => {
                      await onReorderRelations(pId, type, orderedIds);
                      router.refresh();
                    }
                  : undefined
              }
              onSearchProducts={onSearchProducts}
              disabled={isSaving}
            />
          )}

          {/* Product Bundle & Kit Composition */}
          {!isNew && initialProduct?.id && (
            <ProductBundleCard
              productId={initialProduct.id}
              productTitle={translations[activeLocale]?.title || ''}
              productHandle={translations[activeLocale]?.handle || ''}
              bundle={
                bundles.find((b) => b.bundleProductId === initialProduct.id) ||
                bundles[0] ||
                null
              }
              onCreateBundle={
                onCreateBundle
                  ? async (input) => {
                      await onCreateBundle(input);
                      router.refresh();
                    }
                  : undefined
              }
              onUpdateBundle={
                onUpdateBundle
                  ? async (bundleId, input) => {
                      await onUpdateBundle(bundleId, input);
                      router.refresh();
                    }
                  : undefined
              }
              onAddBundleItem={
                onAddBundleItem
                  ? async (input) => {
                      await onAddBundleItem(input);
                      router.refresh();
                    }
                  : undefined
              }
              onRemoveBundleItem={
                onRemoveBundleItem
                  ? async (bundleId, itemId) => {
                      await onRemoveBundleItem(bundleId, itemId);
                      router.refresh();
                    }
                  : undefined
              }
              onSearchProducts={
                onSearchProducts
                  ? async (q) => {
                      const items = await onSearchProducts(q);
                      return items.map((p) => ({
                        id: p.id,
                        title: p.title,
                        handle: p.handle,
                        sku: undefined,
                        thumbnail: undefined,
                        price: undefined
                      }));
                    }
                  : undefined
              }
              disabled={isSaving}
            />
          )}
        </div>

        {/* Right (Sidebar) Column - 1 Col */}
        <div className='space-y-6'>
          {/* Media Gallery */}
          <ProductMediaCard
            images={images}
            onAddImage={
              initialProduct?.id && onAddImage
                ? async (img) => {
                    await onAddImage(initialProduct.id, img);
                    router.refresh();
                  }
                : undefined
            }
            onDeleteImage={
              initialProduct?.id && onDeleteImage
                ? async (imgId) => {
                    await onDeleteImage(initialProduct.id, imgId);
                    router.refresh();
                  }
                : undefined
            }
            onReorderImages={
              initialProduct?.id && onReorderImages
                ? async (ids) => {
                    await onReorderImages(initialProduct.id, ids);
                    router.refresh();
                  }
                : undefined
            }
            disabled={isSaving}
          />

          {/* SEO Card */}
          <ProductSeoCard
            translation={
              translations[activeLocale] || {
                locale: activeLocale,
                title: '',
                handle: '',
                description: '',
                metaTitle: '',
                metaDescription: ''
              }
            }
            activeLocale={activeLocale}
            onChange={(field, val) =>
              handleTranslationChange(activeLocale, field, val)
            }
            disabled={isSaving}
          />
        </div>
      </div>
    </div>
  );
}
