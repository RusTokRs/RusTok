/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import type {
  ProductWriteRequests,
  ProductWriteScope,
  ProductWriteOutcome
} from './write-attempt';

export const PRODUCT_EDITOR_INTENT_QUERY_KEY = 'editor_intent' as const;
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
  | { phase: 'pending'; attempt: ProductPendingAttempt }
  | {
      phase: 'settled';
      operation: ProductWriteOperation;
      key: string;
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
  private readonly driver: ProductJournalDriver;
  constructor(
    scope: ProductWriteScope,
    resource: string,
    intent: string,
    driver: ProductJournalDriver
  ) {
    this.scope = Object.freeze({
      tenantId: scope.tenantId,
      actorId: scope.actorId
    });
    this.resource = resource;
    this.intent = intent;
    this.driver = driver;
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
    return `rustok.product.editor_attempt:${JSON.stringify([this.scope.tenantId, this.scope.actorId, this.resource, this.intent])}`;
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
    if (value.initialVariant !== undefined) {
      const variant = value.initialVariant;
      if (
        !object(variant) ||
        !only(variant, [
          'sku',
          'barcode',
          'priceAmount',
          'currencyCode',
          'compareAtAmount',
          'inventoryQuantity',
          'inventoryPolicy'
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
    }
    return true;
  }

  private validate(raw: unknown): ProductJournalRecord {
    if (
      !object(raw) ||
      !only(raw, ['scope', 'resource', 'intent', 'record']) ||
      !object(raw.scope) ||
      !only(raw.scope, ['tenantId', 'actorId']) ||
      raw.scope.tenantId !== this.scope.tenantId ||
      raw.scope.actorId !== this.scope.actorId ||
      raw.resource !== this.resource ||
      raw.intent !== this.intent ||
      !object(raw.record)
    )
      throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
    const record = raw.record;
    if (record.phase === 'pending') {
      const attempt = record.attempt;
      if (
        only(record, ['phase', 'attempt']) &&
        object(attempt) &&
        only(attempt, ['operation', 'key', 'request']) &&
        ['save', 'status', 'delete'].includes(attempt.operation as string) &&
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
      only(record, ['phase', 'operation', 'key', 'outcome']) &&
      ['save', 'status', 'delete'].includes(record.operation as string) &&
      (this.resource !== 'new' || record.operation === 'save') &&
      typeof record.key === 'string' &&
      uuid.test(record.key) &&
      object(record.outcome)
    ) {
      const outcome = record.outcome;
      if (
        outcome.status === 'rejected' &&
        only(outcome, ['status', 'code', 'message']) &&
        typeof outcome.code === 'string' &&
        outcome.code.length > 0 &&
        typeof outcome.message === 'string'
      )
        return record as ProductJournalRecord;
      if (
        outcome.status === 'confirmed' &&
        only(outcome, ['status', 'value']) &&
        object(outcome.value) &&
        only(outcome.value, ['id', 'revision']) &&
        typeof outcome.value.id === 'string' &&
        outcome.value.id.length > 0 &&
        (this.resource === 'new'
          ? record.operation === 'save'
          : outcome.value.id === this.resource) &&
        (record.operation === 'delete'
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

  write(record: ProductJournalRecord): ProductJournalRecord {
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
  if (!window.isSecureContext || !navigator.locks)
    throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
  const url = new URL(window.location.href);
  const intents = url.searchParams.getAll(PRODUCT_EDITOR_INTENT_QUERY_KEY);
  if (!newIntent && intents.length > 1)
    throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
  let intent = newIntent ? null : (intents[0] ?? null);
  if (intent !== null && !uuid.test(intent))
    throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
  if (!intent) {
    intent = crypto.randomUUID();
    url.searchParams.set(PRODUCT_EDITOR_INTENT_QUERY_KEY, intent);
    window.history.replaceState(window.history.state, '', url);
  }
  const storage = window.localStorage;
  const key = `rustok.product.editor_attempt:${JSON.stringify([scope.tenantId, scope.actorId, resource, intent])}`;
  return new ProductWriteJournal(scope, resource, intent, {
    read: () => storage.getItem(key),
    write: (value) => storage.setItem(key, value),
    exclusive: async (work) => {
      return await navigator.locks.request(
        key,
        { mode: 'exclusive', ifAvailable: true },
        async (lock) => {
          // Check after lock acquisition too: Web Locks schedules this callback.
          // A native query change must not submit an old mounted editor intent.
          const current = new URL(window.location.href).searchParams.getAll(
            PRODUCT_EDITOR_INTENT_QUERY_KEY
          );
          if (!lock || current.length !== 1 || current[0] !== intent) {
            throw new Error(PRODUCT_RECOVERY_UNAVAILABLE);
          }
          return work();
        }
      );
    }
  });
}
