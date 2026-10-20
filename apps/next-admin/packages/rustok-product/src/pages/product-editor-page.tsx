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

import type { UpdateVariantInput } from '../api/products';
import * as React from 'react';
import { parsePriceAmount } from '../money';
import {
  ProductWriteAttempt,
  type ProductSavePayload,
  type ProductWriteOutcome,
  type ProductWriteScope
} from '../write-attempt';
import {
  browserProductWriteJournal,
  PRODUCT_EDITOR_INTENT_QUERY_KEY,
  PRODUCT_RECOVERY_UNAVAILABLE,
  type ProductJournalRecord
} from '../write-journal';
import { useRouter } from 'next/navigation';
import { ProductHeaderBar } from '../components/products/product-header-bar';
import { ProductGeneralCard } from '../components/products/product-general-card';
import { ProductCategoryCard } from '../components/products/product-category-card';
import { ProductVariantsCard } from '../components/products/product-variants-card';
import { ProductVariantAxesCard } from '../components/products/product-variant-axes-card';
import { ProductMediaCard } from '../components/products/product-media-card';
import { ProductRelationsCard } from '../components/products/product-relations-card';
import { ProductBundleCard } from '../components/products/product-bundle-card';
import { ProductSeoCard } from '../components/products/product-seo-card';
import { Alert, AlertDescription } from '@/shared/ui/shadcn/alert';
import { AlertCircle, CheckCircle2 } from 'lucide-react';
import type {
  SetVariantAxesInput,
  VariantAxisConfig,
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
  ProductBundle,
  CreateBundleInput,
  UpdateBundleInput,
  AddBundleItemInput
} from '../api/types';

export interface ProductEditorPageProps {
  writeScope: ProductWriteScope;
  resourceId: string;
  recoveryOnly?: boolean;
  initialProduct?: ProductDetail | null;
  categories: CatalogCategorySummary[];
  initialEffectiveForm?: ProductEffectiveForm | null;
  initialAttributeValues?: ProductAttributeValueItem[];
  initialRelations?: ProductRelation[];
  isNew?: boolean;
  onSaveProduct: (
    payload: ProductSavePayload,
    idempotencyKey: string
  ) => Promise<ProductWriteOutcome>;
  onChangeStatus?: (
    id: string,
    status: string,
    revision: number | null,
    idempotencyKey: string,
    scope: ProductWriteScope
  ) => Promise<ProductWriteOutcome>;
  onDeleteProduct?: (
    id: string,
    idempotencyKey: string,
    scope: ProductWriteScope
  ) => Promise<ProductWriteOutcome>;
  onAddVariant?: (
    productId: string,
    variant: Omit<ProductVariant, 'id'>
  ) => Promise<void>;
  onUpdateVariant?: (
    id: string,
    variant: UpdateVariantInput
  ) => Promise<void>;
  onDeleteVariant?: (id: string) => Promise<void>;
  onSetVariantAxes?: (
    productId: string,
    input: SetVariantAxesInput
  ) => Promise<VariantAxisConfig[] | void>;
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
  writeScope,
  resourceId,
  recoveryOnly = false,
  initialProduct,
  categories,
  initialEffectiveForm = null,
  initialAttributeValues = [],
  initialRelations = [],
  initialBundles = [],
  isNew = false,
  onSaveProduct,
  onChangeStatus,
  onDeleteProduct,
  onAddVariant,
  onUpdateVariant,
  onDeleteVariant,
  onSetVariantAxes,
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
  // This predecessor belongs to the local document draft, not to the latest server
  // props. A router refresh must not silently rebase dirty fields onto someone else's
  // revision. Only this editor's confirmed catalog writes advance it; reload to rebase.
  const [documentRevision, setDocumentRevision] = React.useState<number | null>(
    initialProduct?.revision ?? null
  );
  const [activeLocale, setActiveLocale] = React.useState('en');
  const [isSaving, setIsSaving] = React.useState(false);
  const writeAttempt = React.useRef<ProductWriteAttempt | null>(null);
  const [recoveryReady, setRecoveryReady] = React.useState(false);
  const [recoveryError, setRecoveryError] = React.useState<string | null>(null);
  const [settledWrite, setSettledWrite] = React.useState<Extract<
    ProductJournalRecord,
    { phase: 'settled' }
  > | null>(null);
  const createdProduct = React.useRef<string | null>(null);
  const deletedProduct = React.useRef(false);
  const [pendingOperation, setPendingOperation] = React.useState<
    'save' | 'status' | 'delete' | null
  >(null);
  const initializeRecovery = React.useCallback(() => {
    try {
      if (!writeAttempt.current) {
        writeAttempt.current = new ProductWriteAttempt(
          browserProductWriteJournal(writeScope, resourceId)
        );
      }
      const record = writeAttempt.current.restore();
      if (recoveryOnly && !record)
        throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
      setPendingOperation(writeAttempt.current.pendingOperation);
      setSettledWrite(record?.phase === 'settled' ? record : null);
      setRecoveryError(null);
      setRecoveryReady(true);
    } catch {
      setRecoveryError(PRODUCT_RECOVERY_UNAVAILABLE);
      setRecoveryReady(false);
    }
  }, [writeScope, resourceId, recoveryOnly]);

  React.useEffect(() => {
    initializeRecovery();
    const changed = (event: StorageEvent) => {
      if (
        event.key === null ||
        event.key === writeAttempt.current?.storageKey
      ) {
        initializeRecovery();
      }
    };
    window.addEventListener('storage', changed);
    return () => window.removeEventListener('storage', changed);
  }, [initializeRecovery]);

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
    priceAmount: '0',
    currencyCode: 'USD',
    compareAtAmount: null as string | null,
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

  // A form belongs to an exact category/locale selection. Cleanup fences late
  // responses, including when a category is cleared or the editor is unmounted.
  React.useEffect(() => {
    if (!fetchEffectiveForm) return;
    let cancelled = false;
    setEffectiveForm(null);
    if (!selectedCategoryId) {
      setIsLoadingForm(false);
      return;
    }

    setIsLoadingForm(true);
    void fetchEffectiveForm(selectedCategoryId, activeLocale)
      .then((form) => {
        if (!cancelled) setEffectiveForm(form);
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setErrorMessage(
            err instanceof Error ? err.message : 'Failed to load category form.'
          );
        }
      })
      .finally(() => {
        if (!cancelled) setIsLoadingForm(false);
      });
    return () => {
      cancelled = true;
    };
  }, [selectedCategoryId, activeLocale, fetchEffectiveForm]);

  const handleSelectCategory = (categoryId: string) => {
    setSelectedCategoryId(categoryId);
    setEffectiveForm(null);
    setIsLoadingForm(Boolean(categoryId && fetchEffectiveForm));
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

  const syncRecoveryState = () => {
    const writer = writeAttempt.current;
    setPendingOperation(writer?.pendingOperation ?? null);
    const record = writer?.recovery;
    setSettledWrite(record?.phase === 'settled' ? record : null);
    setRecoveryError(writer?.recoveryError ?? null);
  };

  // Resume the stored command, not the current form. Recovery also works when
  // the deleted Product can no longer be loaded by the server route.
  const handleResume = async () => {
    const writer = writeAttempt.current;
    if (!writer || !recoveryReady || writer.isBusy || isSaving) return;
    setErrorMessage(null);
    setIsSaving(true);
    try {
      const unavailable = async (): Promise<ProductWriteOutcome> => ({
        status: 'unknown',
        message: PRODUCT_RECOVERY_UNAVAILABLE
      });
      const result = await writer.resume({
        save: onSaveProduct,
        status: onChangeStatus
          ? (request, key) =>
              onChangeStatus(
                request.id,
                request.status,
                request.revision,
                key,
                request.scope
              )
          : unavailable,
        delete: onDeleteProduct
          ? (request, key) => onDeleteProduct(request.id, key, request.scope)
          : unavailable
      });
      if (result && result.outcome.status !== 'confirmed') {
        setErrorMessage(result.outcome.message);
      }
      // A recovered acknowledgement deliberately does not rebase current fields.
      // The settled panel opens a fresh owner document (or the catalog).
    } catch {
      setRecoveryError(PRODUCT_RECOVERY_UNAVAILABLE);
      setRecoveryReady(false);
    } finally {
      syncRecoveryState();
      setIsSaving(false);
    }
  };

  const handleCorrectDraft = () => {
    const writer = writeAttempt.current;
    if (!writer || writer.isBusy || isSaving || recoveryOnly) return;
    try {
      const record = writer.restore();
      if (record?.phase !== 'settled' || record.outcome.status !== 'rejected')
        return;
      const next = new ProductWriteAttempt(
        browserProductWriteJournal(writeScope, resourceId, true)
      );
      next.restore();
      writeAttempt.current = next;
      setSettledWrite(null);
      setPendingOperation(null);
      setRecoveryError(null);
      setRecoveryReady(true);
      setErrorMessage(null);
    } catch {
      setRecoveryError(PRODUCT_RECOVERY_UNAVAILABLE);
      setRecoveryReady(false);
    }
  };

  const handleFreshDocument = () => {
    if (!recoveryReady || isSaving || writeAttempt.current?.isBusy) return;
    try {
      const record = writeAttempt.current?.restore();
      if (record?.phase !== 'settled') return;
      if (
        record.operation === 'delete' &&
        record.outcome.status === 'confirmed'
      ) {
        router.push('/dashboard/product');
        return;
      }
      const id =
        record.outcome.status === 'confirmed'
          ? record.outcome.value.id
          : resourceId;
      const url = new URL(
        `/dashboard/product/${encodeURIComponent(id)}`,
        window.location.origin
      );
      url.searchParams.set(
        PRODUCT_EDITOR_INTENT_QUERY_KEY,
        crypto.randomUUID()
      );
      // Full navigation is intentional: draft state and predecessor are not rebased
      // on new props by a router refresh. The new intent reads a fresh owner document.
      window.location.assign(url);
    } catch {
      setRecoveryError(PRODUCT_RECOVERY_UNAVAILABLE);
      setRecoveryReady(false);
    }
  };

  // Main Save Handler
  const handleSave = async () => {
    if (
      !recoveryReady ||
      !writeAttempt.current ||
      (isLoadingForm && !writeAttempt.current.pendingOperation) ||
      isSaving ||
      writeAttempt.current.isBusy
    )
      return;
    if (createdProduct.current) {
      router.push(`/dashboard/product/${createdProduct.current}`);
      return;
    }
    if (deletedProduct.current) return;
    if (writeAttempt.current.pendingOperation) {
      await handleResume();
      return;
    }
    setErrorMessage(null);
    setSuccessMessage(null);

    const primaryTitle =
      translations[activeLocale]?.title?.trim() ||
      translations.en?.title?.trim();
    if (!primaryTitle) {
      setErrorMessage('Product title is required.');
      return;
    }

    // Admit deterministic initial-Variant scalars before retaining a command.
    // A local decimal/Int error must leave the draft editable, not strand it as
    // an "unknown commit" when no request was ever sent.
    let initialVariant = isNew ? { ...newVariantDraft } : undefined;
    if (initialVariant && !writeAttempt.current.pendingOperation) {
      try {
        initialVariant.priceAmount = parsePriceAmount(
          initialVariant.priceAmount
        );
        initialVariant.compareAtAmount = initialVariant.compareAtAmount?.trim()
          ? parsePriceAmount(initialVariant.compareAtAmount)
          : null;
        if (
          !Number.isInteger(initialVariant.inventoryQuantity) ||
          initialVariant.inventoryQuantity < 0 ||
          initialVariant.inventoryQuantity > 2_147_483_647
        ) {
          throw new Error(
            'Initial inventory must be a non-negative 32-bit integer.'
          );
        }
      } catch (error) {
        setErrorMessage(
          error instanceof Error ? error.message : 'Initial variant is invalid.'
        );
        return;
      }
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

      const attempt = await writeAttempt.current.execute(
        'save',
        {
          scope: writeScope,
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
          revision: documentRevision,
          initialVariant,
          attributePatches: patches,
          activeLocale
        },
        onSaveProduct
      );
      if (!attempt) return;
      if (attempt.outcome.status !== 'confirmed') {
        setErrorMessage(attempt.outcome.message);
        return;
      }
      const res = attempt.outcome.value;

      if (typeof res.revision === 'number') {
        setDocumentRevision(res.revision);
      }

      setDirtyAttributeIds(new Set());
      setSuccessMessage(
        isNew ? 'Product created successfully!' : 'Changes saved successfully.'
      );

      if (isNew && res.id) {
        createdProduct.current = res.id;
        router.push(`/dashboard/product/${res.id}`);
      }
    } catch (err: unknown) {
      const msg =
        err instanceof Error ? err.message : 'Failed to save product.';
      if (!msg.includes('NEXT_REDIRECT')) {
        setErrorMessage(msg);
      }
    } finally {
      syncRecoveryState();
      setIsSaving(false);
    }
  };

  const handleStatusChange = async (newStatus: string) => {
    if (
      !recoveryReady ||
      !writeAttempt.current ||
      isSaving ||
      writeAttempt.current.isBusy ||
      deletedProduct.current ||
      isNew ||
      !initialProduct?.id ||
      !onChangeStatus
    )
      return;
    setErrorMessage(null);
    setSuccessMessage(null);
    setIsSaving(true);
    try {
      const attempt = await writeAttempt.current.execute(
        'status',
        {
          id: initialProduct.id,
          status: newStatus,
          revision: documentRevision,
          scope: writeScope
        },
        (request, key) =>
          onChangeStatus(
            request.id,
            request.status,
            request.revision,
            key,
            request.scope
          )
      );
      if (!attempt) return;
      if (attempt.outcome.status !== 'confirmed') {
        setErrorMessage(attempt.outcome.message);
        return;
      }
      const result = attempt.outcome.value;
      if (typeof result.revision === 'number')
        setDocumentRevision(result.revision);
      setStatus(attempt.request.status);
      setSuccessMessage(`Product status updated to ${attempt.request.status}.`);
    } finally {
      syncRecoveryState();
      setIsSaving(false);
    }
  };

  const handleDelete = async () => {
    if (
      !recoveryReady ||
      !writeAttempt.current ||
      isSaving ||
      writeAttempt.current.isBusy ||
      !initialProduct?.id ||
      !onDeleteProduct
    )
      return;
    if (deletedProduct.current) {
      router.push('/dashboard/product');
      return;
    }
    if (confirm('Are you sure you want to permanently delete this product?')) {
      setIsSaving(true);
      try {
        const attempt = await writeAttempt.current.execute(
          'delete',
          { id: initialProduct.id, scope: writeScope },
          (request, key) => onDeleteProduct(request.id, key, request.scope)
        );
        if (!attempt) return;
        if (attempt.outcome.status !== 'confirmed') {
          setErrorMessage(attempt.outcome.message);
          return;
        }
        deletedProduct.current = true;
        router.push('/dashboard/product');
      } finally {
        syncRecoveryState();
        setIsSaving(false);
      }
    }
  };

  const assertEditorialWriteResolved = () => {
    // Portal dialogs are outside the disabled fieldset. Guard the callbacks too,
    // including stale closures opened before the editorial request began.
    const writer = writeAttempt.current;
    if (
      !recoveryReady ||
      !writer ||
      writer.pendingOperation ||
      writer.isBusy ||
      writer.recoveryError ||
      writer.recovery?.phase === 'settled' ||
      deletedProduct.current
    ) {
      throw new Error(
        'Resolve the previous Product operation before making other changes.'
      );
    }
  };

  return (
    <div className='animate-in fade-in mx-auto flex max-w-6xl flex-col gap-6 pb-16 duration-150'>
      {recoveryError && (
        <Alert variant='destructive'>
          <AlertDescription>
            {recoveryError}
            <button
              type='button'
              className='ml-3 underline'
              disabled={isSaving}
              onClick={initializeRecovery}
            >
              Retry recovery
            </button>
          </AlertDescription>
        </Alert>
      )}
      {pendingOperation && (
        <Alert>
          <AlertDescription>
            The previous {pendingOperation} has an unconfirmed outcome. Its
            original request and key are saved in this browser. Do not start a
            different operation.
            <button
              type='button'
              className='ml-3 underline'
              disabled={isSaving || !recoveryReady}
              onClick={handleResume}
            >
              Retry previous operation
            </button>
          </AlertDescription>
        </Alert>
      )}
      {settledWrite && (
        <Alert>
          <AlertDescription>
            {settledWrite.outcome.status === 'rejected'
              ? settledWrite.outcome.message
              : `The previous ${settledWrite.operation} is confirmed. This editor intent is complete.`}
            Opening a fresh editor discards local unsaved fields.
            <button
              type='button'
              className='ml-3 underline'
              disabled={isSaving || !recoveryReady}
              onClick={handleFreshDocument}
            >
              {settledWrite.operation === 'delete' &&
              settledWrite.outcome.status === 'confirmed'
                ? 'Open catalog'
                : 'Open fresh editor'}
            </button>
            {!recoveryOnly && settledWrite.outcome.status === 'rejected' && (
              <button
                type='button'
                className='ml-3 underline'
                disabled={isSaving || !recoveryReady}
                onClick={handleCorrectDraft}
              >
                Correct this draft as a new operation
              </button>
            )}
          </AlertDescription>
        </Alert>
      )}
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

      {recoveryOnly && (
        <p>
          The Product could not be loaded. Only recovery of its saved operation
          is available.{' '}
          <a className='underline' href='/dashboard/product'>
            Open catalog
          </a>
        </p>
      )}
      {!recoveryOnly && (
        <>
          {/* Top Header Bar with Save & Lifecycle */}
          <ProductHeaderBar
            title={translations[activeLocale]?.title || ''}
            status={status}
            isNew={isNew}
            isSaving={isSaving}
            disabled={
              !recoveryReady ||
              !!recoveryError ||
              !!pendingOperation ||
              !!settledWrite
            }
            onSave={handleSave}
            onStatusChange={
              onChangeStatus && !pendingOperation
                ? handleStatusChange
                : undefined
            }
            onDelete={!pendingOperation ? handleDelete : undefined}
          />

          <fieldset
            disabled={
              isSaving ||
              !recoveryReady ||
              !!recoveryError ||
              !!pendingOperation ||
              !!settledWrite
            }
            className='contents'
          >
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
                  disabled={isSaving || pendingOperation !== null}
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
                  disabled={isSaving || pendingOperation !== null}
                />

                {/* Variant axes (ADR identity model) */}
                {initialProduct?.id ? (
                  <ProductVariantAxesCard
                    axes={initialProduct.variantAxes ?? []}
                    effectiveForm={initialEffectiveForm}
                    disabled={isSaving || pendingOperation !== null}
                    onSaveAxes={
                      onSetVariantAxes
                        ? async (input) => {
                            assertEditorialWriteResolved();
                            return onSetVariantAxes(initialProduct.id, input);
                          }
                        : undefined
                    }
                  />
                ) : null}

                {/* Pricing & Variants Matrix */}
                <ProductVariantsCard
                  variants={variants}
                  isNew={isNew}
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
                          assertEditorialWriteResolved();
                          await onAddVariant(initialProduct.id, v);
                          router.refresh();
                        }
                      : undefined
                  }
                  onUpdateVariant={
                    onUpdateVariant
                      ? async (id, patch) => {
                          assertEditorialWriteResolved();
                          await onUpdateVariant(id, patch);
                          router.refresh();
                        }
                      : undefined
                  }
                  onDeleteVariant={
                    onDeleteVariant
                      ? async (id) => {
                          assertEditorialWriteResolved();
                          await onDeleteVariant(id);
                          router.refresh();
                        }
                      : undefined
                  }
                  disabled={isSaving || pendingOperation !== null}
                />

                {/* Product Relations (Cross-sell, Up-sell, Accessories, Alternatives) */}
                {!isNew && initialProduct?.id && (
                  <ProductRelationsCard
                    productId={initialProduct.id}
                    relations={relations}
                    onAddRelation={
                      onAddRelation
                        ? async (input) => {
                            assertEditorialWriteResolved();
                            await onAddRelation(input);
                            router.refresh();
                          }
                        : undefined
                    }
                    onRemoveRelation={
                      onRemoveRelation
                        ? async (id) => {
                            assertEditorialWriteResolved();
                            await onRemoveRelation(id);
                            setRelations((prev) =>
                              prev.filter((r) => r.id !== id)
                            );
                            router.refresh();
                          }
                        : undefined
                    }
                    onReorderRelations={
                      onReorderRelations
                        ? async (pId, type, orderedIds) => {
                            assertEditorialWriteResolved();
                            await onReorderRelations(pId, type, orderedIds);
                            router.refresh();
                          }
                        : undefined
                    }
                    onSearchProducts={onSearchProducts}
                    disabled={isSaving || pendingOperation !== null}
                  />
                )}

                {/* Product Bundle & Kit Composition */}
                {!isNew && initialProduct?.id && (
                  <ProductBundleCard
                    productId={initialProduct.id}
                    productTitle={translations[activeLocale]?.title || ''}
                    productHandle={translations[activeLocale]?.handle || ''}
                    bundle={
                      bundles.find(
                        (b) => b.bundleProductId === initialProduct.id
                      ) ||
                      bundles[0] ||
                      null
                    }
                    onCreateBundle={
                      onCreateBundle
                        ? async (input) => {
                            assertEditorialWriteResolved();
                            await onCreateBundle(input);
                            router.refresh();
                          }
                        : undefined
                    }
                    onUpdateBundle={
                      onUpdateBundle
                        ? async (bundleId, input) => {
                            assertEditorialWriteResolved();
                            await onUpdateBundle(bundleId, input);
                            router.refresh();
                          }
                        : undefined
                    }
                    onAddBundleItem={
                      onAddBundleItem
                        ? async (input) => {
                            assertEditorialWriteResolved();
                            await onAddBundleItem(input);
                            router.refresh();
                          }
                        : undefined
                    }
                    onRemoveBundleItem={
                      onRemoveBundleItem
                        ? async (bundleId, itemId) => {
                            assertEditorialWriteResolved();
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
                    disabled={isSaving || pendingOperation !== null}
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
                          assertEditorialWriteResolved();
                          await onAddImage(initialProduct.id, img);
                          router.refresh();
                        }
                      : undefined
                  }
                  onDeleteImage={
                    initialProduct?.id && onDeleteImage
                      ? async (imgId) => {
                          assertEditorialWriteResolved();
                          await onDeleteImage(initialProduct.id, imgId);
                          router.refresh();
                        }
                      : undefined
                  }
                  onReorderImages={
                    initialProduct?.id && onReorderImages
                      ? async (ids) => {
                          assertEditorialWriteResolved();
                          await onReorderImages(initialProduct.id, ids);
                          router.refresh();
                        }
                      : undefined
                  }
                  disabled={isSaving || pendingOperation !== null}
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
                  disabled={isSaving || pendingOperation !== null}
                />
              </div>
            </div>
          </fieldset>
        </>
      )}
    </div>
  );
}
