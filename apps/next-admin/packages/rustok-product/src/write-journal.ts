/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import type {
  ProductWriteRequests,
  ProductWriteScope,
  ProductWriteOutcome,
  ProductVariantCreateRequest
} from './write-attempt';

import {
  isDefinitionId,
  isAttributeDefinitionRequest,
  isAttributeDefinitionResult,
  isAttributeDefinitionRefusalCode,
  attributeDefinitionRefusalCode
} from './attribute-lifecycle.ts';
import type { AttributeDefinitionWriteRequest } from './attribute-lifecycle';

import {
  isAxesApplicationInput,
  isAxesDurableRefusalCode
} from './axis-reconfiguration.ts';

import {
  isVariantCreateInput,
  isVariantCreateRefusalCode,
  isVariantId
} from './variant-create.ts';

export const PRODUCT_EDITOR_INTENT_QUERY_KEY = 'editor_intent' as const;
export const ATTRIBUTE_DEFINITION_INTENT_QUERY_KEY =
  'attribute_recovery' as const;
type CatalogJournalKind = 'product' | 'attribute_definition';
const journalPrefix = (kind: CatalogJournalKind) =>
  kind === 'product'
    ? 'rustok.product.editor_attempt'
    : 'rustok.product.attribute_definition_attempt';
const intentQuery = (kind: CatalogJournalKind) =>
  kind === 'product'
    ? PRODUCT_EDITOR_INTENT_QUERY_KEY
    : ATTRIBUTE_DEFINITION_INTENT_QUERY_KEY;
export const PRODUCT_RECOVERY_UNAVAILABLE =
  'Product recovery is unavailable. No new operation will be sent. Restore browser storage and retry recovery; do not discard an unresolved attempt.';
export type ProductWriteOperation = keyof ProductWriteRequests;
export type ProductPendingAttempt = {
  [K in ProductWriteOperation]: {
    operation: K;
    key: string;
    request: ProductWriteRequests[K];
  };
}[ProductWriteOperation];
export type ProductJournalRecord =
  | AttributeDefinitionJournalRecord
  | { phase: 'pending'; attempt: ProductPendingAttempt }
  | {
      phase: 'settled';
      operation: Exclude<ProductWriteOperation, 'variant' | 'definition'>;
      key: string;
      outcome: Exclude<ProductWriteOutcome, { status: 'unknown' }>;
    }
  | {
      phase: 'settled';
      operation: 'variant';
      key: string;
      request: ProductVariantCreateRequest;
      outcome: Exclude<ProductWriteOutcome, { status: 'unknown' }>;
    };

// Terminal dictionary requests retain their full body, just like standalone SKU attempts.
export type AttributeDefinitionJournalRecord = {
  phase: 'settled';
  operation: 'definition';
  key: string;
  request: AttributeDefinitionWriteRequest;
  outcome: Exclude<ProductWriteOutcome, { status: 'unknown' }>;
};

export interface ProductJournalDriver {
  read(): string | null;
  write(value: string): void;
  exclusive<T>(work: () => Promise<T>): Promise<T>;
}
const uuid =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
export const isProductEditorIntent = (value: unknown): value is string =>
  typeof value === 'string' && uuid.test(value);
const MAX_RECORD_LENGTH = 2 * 1024 * 1024;
const object = (value: unknown): value is Record<string, unknown> =>
  !!value && typeof value === 'object' && !Array.isArray(value);
const strings = (value: unknown): value is string[] =>
  Array.isArray(value) && value.every((item) => typeof item === 'string');
const only = (value: Record<string, unknown>, keys: string[]) =>
  Object.keys(value).every((key) => keys.includes(key));
const optionalText = (value: unknown) =>
  value === undefined || value === null || typeof value === 'string';
const revision = (value: unknown) =>
  value === null ||
  (Number.isInteger(value) &&
    (value as number) >= 0 &&
    (value as number) <= 2147483647);

/** Strictly validate a local projection; corrupt/foreign records are not silently discarded. */
export class ProductWriteJournal {
  readonly scope: ProductWriteScope;
  readonly resource: string;
  readonly intent: string;
  readonly resourceKind: CatalogJournalKind;
  private readonly driver: ProductJournalDriver;
  constructor(
    scope: ProductWriteScope,
    resource: string,
    intent: string,
    driver: ProductJournalDriver,
    resourceKind: CatalogJournalKind = 'product'
  ) {
    this.scope = Object.freeze({
      tenantId: scope.tenantId,
      actorId: scope.actorId
    });
    this.resource = resource;
    this.intent = intent;
    this.driver = driver;
    this.resourceKind = resourceKind;
    if (
      typeof scope.tenantId !== 'string' ||
      !scope.tenantId ||
      typeof scope.actorId !== 'string' ||
      !scope.actorId ||
      typeof resource !== 'string' ||
      !resource ||
      !isProductEditorIntent(intent)
    )
      throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
  }

  get storageKey(): string {
    return `${journalPrefix(this.resourceKind)}:${JSON.stringify([this.scope.tenantId, this.scope.actorId, this.resource, this.intent])}`;
  }

  private requestValid(
    operation: ProductWriteOperation,
    value: unknown
  ): boolean {
    if (
      !object(value) ||
      !object(value.scope) ||
      !only(value.scope, ['tenantId', 'actorId']) ||
      value.scope.tenantId !== this.scope.tenantId ||
      value.scope.actorId !== this.scope.actorId
    )
      return false;
    if (this.resourceKind === 'attribute_definition') {
      return (
        operation === 'definition' &&
        this.resource !== 'new' &&
        isAttributeDefinitionRequest(value) &&
        value.id === this.resource
      );
    }
    if (operation === 'definition') return false;
    if (operation === 'variant') {
      return (
        this.resource !== 'new' &&
        value.id === this.resource &&
        only(value, ['id', 'scope', 'input', 'locale']) &&
        typeof value.locale === 'string' &&
        value.locale.length > 0 &&
        value.locale.length <= 32 &&
        isVariantCreateInput(value.input)
      );
    }
    if (operation === 'axes') {
      return (
        this.resource !== 'new' &&
        value.id === this.resource &&
        only(value, ['id', 'scope', 'input', 'locale']) &&
        typeof value.locale === 'string' &&
        value.locale.length > 0 &&
        value.locale.length <= 32 &&
        isAxesApplicationInput(value.input)
      );
    }
    if (operation !== 'save') {
      return (
        this.resource !== 'new' &&
        value.id === this.resource &&
        only(
          value,
          operation === 'status'
            ? ['id', 'scope', 'status', 'revision']
            : ['id', 'scope']
        ) &&
        (operation === 'delete' ||
          (['ACTIVE', 'DRAFT', 'ARCHIVED'].includes(value.status as string) &&
            revision(value.revision)))
      );
    }
    if (
      !only(value, [
        'scope',
        'id',
        'isNew',
        'translations',
        'vendor',
        'sellerId',
        'productType',
        'shippingProfileSlug',
        'primaryCategoryId',
        'tags',
        'status',
        'revision',
        'initialVariant',
        'variantAxes',
        'initialVariants',
        'attributePatches',
        'activeLocale'
      ]) ||
      value.isNew !== (this.resource === 'new') ||
      (this.resource === 'new'
        ? value.id !== undefined
        : value.id !== this.resource) ||
      !strings(value.tags) ||
      typeof value.activeLocale !== 'string' ||
      !Array.isArray(value.translations) ||
      value.translations.length === 0 ||
      !Array.isArray(value.attributePatches)
    )
      return false;
    if (
      ![
        'vendor',
        'sellerId',
        'productType',
        'shippingProfileSlug',
        'primaryCategoryId',
        'status'
      ].every((key) => optionalText(value[key])) ||
      (value.revision !== undefined && !revision(value.revision))
    )
      return false;
    if (
      !value.translations.every(
        (tr) =>
          object(tr) &&
          only(tr, [
            'locale',
            'title',
            'handle',
            'description',
            'metaTitle',
            'metaDescription'
          ]) &&
          typeof tr.locale === 'string' &&
          typeof tr.title === 'string' &&
          ['handle', 'description', 'metaTitle', 'metaDescription'].every(
            (key) => optionalText(tr[key])
          )
      )
    )
      return false;
    if (
      !value.attributePatches.every(
        (patch) =>
          object(patch) &&
          only(patch, [
            'attributeId',
            'kind',
            'text',
            'integer',
            'decimal',
            'boolean',
            'date',
            'datetime',
            'optionId',
            'optionIds',
            'json'
          ]) &&
          typeof patch.attributeId === 'string' &&
          typeof patch.kind === 'string' &&
          ['text', 'decimal', 'date', 'datetime', 'optionId', 'json'].every(
            (key) => optionalText(patch[key])
          ) &&
          (patch.integer === undefined ||
            patch.integer === null ||
            Number.isSafeInteger(patch.integer)) &&
          (patch.boolean === undefined ||
            patch.boolean === null ||
            typeof patch.boolean === 'boolean') &&
          (patch.optionIds === undefined ||
            patch.optionIds === null ||
            strings(patch.optionIds))
      )
    )
      return false;
    const scalarVariantValid = (
      variant: unknown,
      withAxes: boolean
    ): boolean => {
      if (
        !object(variant) ||
        !only(variant, [
          'sku',
          'barcode',
          'priceAmount',
          'currencyCode',
          'compareAtAmount',
          'inventoryQuantity',
          'inventoryPolicy',
          ...(withAxes ? ['axisValues'] : [])
        ]) ||
        ![
          'sku',
          'barcode',
          'priceAmount',
          'currencyCode',
          'inventoryPolicy'
        ].every((key) => typeof variant[key] === 'string') ||
        !optionalText(variant.compareAtAmount) ||
        !Number.isInteger(variant.inventoryQuantity) ||
        (variant.inventoryQuantity as number) < 0 ||
        (variant.inventoryQuantity as number) > 2147483647
      )
        return false;
      return (
        !withAxes ||
        (object(variant) &&
          Array.isArray(variant.axisValues) &&
          variant.axisValues.length <= 32 &&
          variant.axisValues.every(
            (value) =>
              object(value) &&
              only(value, ['attributeId', 'optionId']) &&
              typeof value.attributeId === 'string' &&
              typeof value.optionId === 'string'
          ))
      );
    };
    if (
      value.initialVariant !== undefined &&
      !scalarVariantValid(value.initialVariant, false)
    )
      return false;
    if (
      value.initialVariants !== undefined &&
      (!value.isNew ||
        value.initialVariant !== undefined ||
        !Array.isArray(value.initialVariants) ||
        value.initialVariants.length < 1 ||
        value.initialVariants.length > 128 ||
        !value.initialVariants.every((variant) =>
          scalarVariantValid(variant, true)
        ))
    )
      return false;
    if (
      value.variantAxes !== undefined &&
      (!value.isNew ||
        !Array.isArray(value.variantAxes) ||
        value.variantAxes.length > 32 ||
        !value.variantAxes.every(
          (axis) =>
            object(axis) &&
            only(axis, ['attributeId', 'position', 'allowedOptionIds']) &&
            typeof axis.attributeId === 'string' &&
            (axis.position === undefined ||
              axis.position === null ||
              (Number.isInteger(axis.position) &&
                (axis.position as number) >= 0 &&
                (axis.position as number) <= 2147483647)) &&
            strings(axis.allowedOptionIds) &&
            axis.allowedOptionIds.length >= 1 &&
            axis.allowedOptionIds.length <= 256
        ))
    )
      return false;
    return true;
  }

  private validate(raw: unknown): ProductJournalRecord | null {
    if (
      !object(raw) ||
      !only(raw, ['scope', 'resource', 'intent', 'record']) ||
      !object(raw.scope) ||
      !only(raw.scope, ['tenantId', 'actorId']) ||
      raw.scope.tenantId !== this.scope.tenantId ||
      raw.scope.actorId !== this.scope.actorId ||
      raw.resource !== this.resource ||
      raw.intent !== this.intent ||
      (raw.record !== null && !object(raw.record))
    )
      throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
    const record = raw.record;
    // An explicit empty baseline differs from missing browser storage after reload.
    if (record === null) return null;
    if (record.phase === 'pending') {
      const attempt = record.attempt;
      if (
        only(record, ['phase', 'attempt']) &&
        object(attempt) &&
        only(attempt, ['operation', 'key', 'request']) &&
        (this.resourceKind === 'product'
          ? ['save', 'status', 'delete', 'axes', 'variant']
          : ['definition']
        ).includes(attempt.operation as string) &&
        typeof attempt.key === 'string' &&
        uuid.test(attempt.key) &&
        this.requestValid(
          attempt.operation as ProductWriteOperation,
          attempt.request
        )
      )
        return record as ProductJournalRecord;
    } else if (
      record.phase === 'settled' &&
      only(
        record,
        record.operation === 'variant' || record.operation === 'definition'
          ? ['phase', 'operation', 'key', 'request', 'outcome']
          : ['phase', 'operation', 'key', 'outcome']
      ) &&
      (this.resourceKind === 'product'
        ? ['save', 'status', 'delete', 'axes', 'variant']
        : ['definition']
      ).includes(record.operation as string) &&
      (this.resource !== 'new' || record.operation === 'save') &&
      typeof record.key === 'string' &&
      uuid.test(record.key) &&
      object(record.outcome) &&
      (!['variant', 'definition'].includes(record.operation as string) ||
        this.requestValid(
          record.operation as 'variant' | 'definition',
          record.request
        ))
    ) {
      const outcome = record.outcome;
      if (
        outcome.status === 'rejected' &&
        only(
          outcome,
          record.operation === 'definition'
            ? ['status', 'code', 'message', 'definition']
            : ['status', 'code', 'message']
        ) &&
        typeof outcome.code === 'string' &&
        outcome.code.length > 0 &&
        (record.operation !== 'axes' ||
          isAxesDurableRefusalCode(outcome.code)) &&
        (record.operation !== 'variant' ||
          isVariantCreateRefusalCode(outcome.code)) &&
        (record.operation !== 'definition' ||
          (isAttributeDefinitionRefusalCode(outcome.code) &&
            (outcome.code === 'PRODUCT_VALIDATION'
              ? outcome.definition === undefined
              : isAttributeDefinitionResult(
                  outcome.definition,
                  (record.request as AttributeDefinitionWriteRequest).input
                ) &&
                !outcome.definition.applied &&
                outcome.definition.refusal !== null &&
                outcome.code ===
                  attributeDefinitionRefusalCode(
                    outcome.definition.refusal
                  )))) &&
        typeof outcome.message === 'string'
      )
        return record as ProductJournalRecord;
      if (
        outcome.status === 'confirmed' &&
        only(outcome, ['status', 'value']) &&
        object(outcome.value) &&
        only(
          outcome.value,
          record.operation === 'variant'
            ? ['id', 'revision', 'variantId']
            : record.operation === 'definition'
              ? ['id', 'revision', 'definition']
              : ['id', 'revision']
        ) &&
        (record.operation !== 'variant' ||
          isVariantId(outcome.value.variantId)) &&
        (record.operation !== 'definition' ||
          (isAttributeDefinitionResult(
            outcome.value.definition,
            (record.request as AttributeDefinitionWriteRequest).input
          ) &&
            outcome.value.definition.applied)) &&
        typeof outcome.value.id === 'string' &&
        outcome.value.id.length > 0 &&
        (this.resource === 'new'
          ? record.operation === 'save'
          : outcome.value.id === this.resource) &&
        (record.operation === 'delete' ||
        record.operation === 'axes' ||
        record.operation === 'variant' ||
        record.operation === 'definition'
          ? outcome.value.revision === null
          : outcome.value.revision !== null && revision(outcome.value.revision))
      )
        return record as ProductJournalRecord;
    }
    throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
  }

  read(): ProductJournalRecord | null {
    try {
      const text = this.driver.read();
      if (text === null) return null;
      if (text.length > MAX_RECORD_LENGTH)
        throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
      return this.validate(JSON.parse(text));
    } catch {
      throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
    }
  }

  write(
    record: ProductJournalRecord,
    expected: ProductJournalRecord | null
  ): ProductJournalRecord {
    const projection = {
      scope: this.scope,
      resource: this.resource,
      intent: this.intent,
      record
    };
    // Validate before JSON as well: NaN/Infinity must not silently become null
    // (an explicit value-clear), nor may a serializer normalize foreign fields.
    this.validate(projection);
    const text = JSON.stringify(projection);
    if (text.length > MAX_RECORD_LENGTH)
      throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
    const canonical = this.validate(JSON.parse(text));
    if (!canonical || JSON.stringify(this.read()) !== JSON.stringify(expected))
      throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
    const transition =
      expected === null
        ? canonical.phase === 'pending'
        : expected.phase === 'pending' &&
          canonical.phase === 'settled' &&
          expected.attempt.operation === canonical.operation &&
          expected.attempt.key === canonical.key &&
          (!['variant', 'definition'].includes(canonical.operation) ||
            JSON.stringify(expected.attempt.request) ===
              JSON.stringify(
                (
                  canonical as Extract<
                    ProductJournalRecord,
                    { operation: 'variant' | 'definition' }
                  >
                ).request
              ));
    if (!transition) throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
    this.driver.write(text);
    // Read-after-write detects an unsuccessful storage adapter before owner IO.
    if (this.driver.read() !== text)
      throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
    return canonical;
  }

  exclusive<T>(work: () => Promise<T>): Promise<T> {
    return this.driver.exclusive(work);
  }
}

/** No in-memory production fallback: durable admission and Web Locks are mandatory. */
export function browserProductWriteJournal(
  scope: ProductWriteScope,
  resource: string,
  newIntent = false
): ProductWriteJournal {
  return browserCatalogWriteJournal(scope, resource, newIntent, 'product');
}
export function browserAttributeDefinitionJournal(
  scope: ProductWriteScope,
  resource: string,
  newIntent = false,
  optionId?: string | null
): ProductWriteJournal {
  return browserCatalogWriteJournal(
    scope,
    resource,
    newIntent,
    'attribute_definition',
    optionId
  );
}
function attributeDocumentOption(
  url: URL,
  resource: string,
  expected?: string | null
): string | null {
  const attributes = url.searchParams.getAll('attribute_id');
  const options = url.searchParams.getAll('option_id');
  const option = options[0] ?? null;
  if (
    attributes.length !== 1 ||
    attributes[0] !== resource ||
    !isDefinitionId(resource) ||
    options.length > 1 ||
    (option !== null && !isDefinitionId(option)) ||
    (expected !== undefined && option !== expected)
  )
    throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
  return option;
}
function browserCatalogWriteJournal(
  scope: ProductWriteScope,
  resource: string,
  newIntent: boolean,
  kind: CatalogJournalKind,
  expectedOptionId?: string | null
): ProductWriteJournal {
  const queryKey = intentQuery(kind);
  if (!window.isSecureContext || !navigator.locks)
    throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
  const url = new URL(window.location.href);
  let documentOption =
    kind === 'attribute_definition'
      ? attributeDocumentOption(url, resource, expectedOptionId)
      : null;
  const verifySelection = () => {
    if (kind === 'attribute_definition')
      attributeDocumentOption(
        new URL(window.location.href),
        resource,
        documentOption
      );
  };
  const intents = url.searchParams.getAll(queryKey);
  if (intents.length > 1) throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
  let intent: string | null = intents[0] ?? null;
  if (intent !== null && !uuid.test(intent))
    throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
  const storage = window.localStorage;
  const storageKey = (intent: string) =>
    `${journalPrefix(kind)}:${JSON.stringify([scope.tenantId, scope.actorId, resource, intent])}`;
  const journal = (intent: string) => {
    const key = storageKey(intent);
    return new ProductWriteJournal(
      scope,
      resource,
      intent,
      {
        read: () => {
          verifySelection();
          const raw = storage.getItem(key);
          if (raw === null) throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
          return raw;
        },
        write: (value) => {
          verifySelection();
          storage.setItem(key, value);
        },
        exclusive: async (work) => {
          return await navigator.locks.request(
            key,
            { mode: 'exclusive', ifAvailable: true },
            async (lock) => {
              // Check after lock acquisition too: native query changes cannot submit an old page.
              verifySelection();
              const current = new URL(window.location.href).searchParams.getAll(
                queryKey
              );
              if (!lock || current.length !== 1 || current[0] !== intent)
                throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
              return work();
            }
          );
        }
      },
      kind
    );
  };
  if (newIntent) {
    // Rotation itself requires durable terminal proof, not merely a caller's boolean.
    if (!intent) throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
    const previous = journal(intent).read();
    if (previous?.phase !== 'settled')
      throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
    if (kind === 'attribute_definition') {
      if (previous.operation !== 'definition')
        throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
      // Ready has no body: publish its original target before committing a new baseline.
      // A failed full navigation/reload must not reinterpret the old URL as another option.
      documentOption = previous.request.input.optionId;
      url.searchParams.set('attribute_id', previous.request.id);
      if (documentOption === null) url.searchParams.delete('option_id');
      else url.searchParams.set('option_id', documentOption);
      window.history.replaceState(window.history.state, '', url);
      verifySelection();
    }
    intent = null;
  }
  if (!intent) {
    intent = crypto.randomUUID();
    // Validate scope before touching storage. Initialize/read back before the URL is attached;
    // no owner write can occur if either storage or the host history annotation fails.
    const initialized = journal(intent);
    const key = storageKey(intent);
    try {
      if (storage.getItem(key) !== null)
        throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
      const baseline = JSON.stringify({
        scope: { tenantId: scope.tenantId, actorId: scope.actorId },
        resource,
        intent,
        record: null
      });
      storage.setItem(key, baseline);
      if (storage.getItem(key) !== baseline)
        throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
    } catch (error) {
      if (error instanceof Error && error.message === PRODUCT_RECOVERY_UNAVAILABLE) {
        throw error;
      }
      throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
    }
    initialized.read();
    url.searchParams.set(queryKey, intent);
    window.history.replaceState(window.history.state, '', url);
    const attached = new URL(window.location.href).searchParams.getAll(
      queryKey
    );
    if (attached.length !== 1 || attached[0] !== intent)
      throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
  }
  const result = journal(intent);
  result.read(); // Existing pending/terminal records retain their original envelope and hash.
  return result;
}
