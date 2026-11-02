/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import { isVariantCreateRefusalCode, isVariantId } from './variant-create.ts';
import { isAxesDurableRefusalCode } from './axis-reconfiguration.ts';
import {
  isAttributeDefinitionResult,
  isAttributeDefinitionRefusalCode,
  attributeDefinitionRefusalCode
} from './attribute-lifecycle.ts';
import type {
  AttributeDefinitionWriteRequest,
  AttributeDefinitionResult
} from './attribute-lifecycle';
import type {
  ProductAttributeValuePatch,
  ProductTranslation
} from './api/types';

export type ProductWriteScope = { tenantId: string; actorId: string };

export type ProductSavePayload = {
  scope: ProductWriteScope;
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
  revision?: number | null;
  /** Persisted pre-axis editor requests remain replayable without changing their payload. */
  initialVariant?: import('./variant-draft').InitialVariantScalars;
  variantAxes?: import('./api/types').VariantAxisDraftInput[];
  initialVariants?: import('./variant-draft').InitialVariantDraft[];
  attributePatches: ProductAttributeValuePatch[];
  activeLocale: string;
};

export type ProductWriteAcknowledgement = {
  id: string;
  revision: number | null;
  /** Only standalone creation supplies a stable Variant acknowledgement, not a revision. */
  variantId?: string;
  /** Dictionary receipt evidence; never a Product document revision. */
  definition?: AttributeDefinitionResult;
};
export type ProductWriteOutcome =
  | { status: 'confirmed'; value: ProductWriteAcknowledgement }
  | {
      status: 'rejected';
      code: string;
      message: string;
      definition?: AttributeDefinitionResult;
    }
  | { status: 'unknown'; message: string };

export type ProductAxesWriteRequest = {
  locale: string;
  id: string;
  scope: ProductWriteScope;
  input: import('./api/types').SetVariantAxesInput;
};

export type ProductVariantCreateRequest = {
  locale: string;
  id: string;
  scope: ProductWriteScope;
  input: import('./api/types').CreateVariantInput;
};

export type ProductWriteRequests = {
  definition: AttributeDefinitionWriteRequest;
  variant: ProductVariantCreateRequest;
  axes: ProductAxesWriteRequest;
  save: ProductSavePayload;
  status: {
    id: string;
    status: string;
    revision: number | null;
    scope: ProductWriteScope;
  };
  delete: { id: string; scope: ProductWriteScope };
};
type Operation = keyof ProductWriteRequests;
import type {
  ProductWriteJournal,
  ProductJournalRecord,
  ProductPendingAttempt
} from './write-journal';

export const UNKNOWN_PRODUCT_WRITE =
  'The outcome is not confirmed. Recover the saved operation before making other changes.';

function freeze<T>(value: T): T {
  if (value && typeof value === 'object') {
    Object.values(value).forEach(freeze);
    Object.freeze(value);
  }
  return value;
}

export type ProductWriteHandlers = {
  [K in Operation]: (
    request: ProductWriteRequests[K],
    key: string
  ) => Promise<ProductWriteOutcome>;
};

/** Durable caller projection; the Product owner receipt remains authoritative. */
export class ProductWriteAttempt {
  private cached: ProductJournalRecord | null = null;
  private busy = false;
  private failure: string | null = null;

  private readonly journal: ProductWriteJournal;
  constructor(journal: ProductWriteJournal) {
    this.journal = journal;
  }

  get recovery(): ProductJournalRecord | null {
    return this.cached;
  }
  get recoveryError(): string | null {
    return this.failure;
  }
  get storageKey(): string {
    return this.journal.storageKey;
  }
  get pendingOperation(): Operation | null {
    return this.cached?.phase === 'pending'
      ? this.cached.attempt.operation
      : null;
  }
  get isBusy(): boolean {
    return this.busy;
  }

  restore(): ProductJournalRecord | null {
    try {
      const record = this.journal.read();
      // Browser data disappearing/changing underneath a live editor is not a
      // terminal decision about its earlier request. Never silently mint a new key.
      const oldKey =
        this.cached?.phase === 'pending'
          ? this.cached.attempt.key
          : this.cached?.key;
      const newKey =
        record?.phase === 'pending' ? record.attempt.key : record?.key;
      if (oldKey && oldKey !== newKey) throw new Error(UNKNOWN_PRODUCT_WRITE);
      if (this.cached && record) {
        const oldOperation =
          this.cached.phase === 'pending'
            ? this.cached.attempt.operation
            : this.cached.operation;
        const newOperation =
          record.phase === 'pending'
            ? record.attempt.operation
            : record.operation;
        if (
          oldOperation !== newOperation ||
          (this.cached.phase === 'settled' &&
            JSON.stringify(this.cached) !== JSON.stringify(record)) ||
          (this.cached.phase === 'pending' &&
            record.phase === 'pending' &&
            JSON.stringify(this.cached) !== JSON.stringify(record)) ||
          (this.cached.phase === 'pending' &&
            record.phase === 'settled' &&
            (record.operation === 'variant' ||
              record.operation === 'definition') &&
            JSON.stringify(this.cached.attempt.request) !==
              JSON.stringify(record.request))
        ) {
          throw new Error(UNKNOWN_PRODUCT_WRITE);
        }
      }
      this.cached = record && freeze(record);
      this.failure = null;
      return this.cached;
    } catch (error) {
      this.failure = UNKNOWN_PRODUCT_WRITE;
      throw error;
    }
  }

  async resume(handlers: Partial<ProductWriteHandlers>) {
    const record = this.restore();
    if (record?.phase !== 'pending') return null;
    const attempt = record.attempt;
    switch (attempt.operation) {
      case 'definition':
        return handlers.definition
          ? this.execute('definition', attempt.request, handlers.definition)
          : null;
      case 'variant':
        return handlers.variant
          ? this.execute('variant', attempt.request, handlers.variant)
          : null;
      case 'axes':
        return handlers.axes
          ? this.execute('axes', attempt.request, handlers.axes)
          : null;
      case 'save':
        return handlers.save
          ? this.execute('save', attempt.request, handlers.save)
          : null;
      case 'status':
        return handlers.status
          ? this.execute('status', attempt.request, handlers.status)
          : null;
      case 'delete':
        return handlers.delete
          ? this.execute('delete', attempt.request, handlers.delete)
          : null;
    }
  }

  async execute<K extends Operation>(
    operation: K,
    request: ProductWriteRequests[K],
    send: (
      request: ProductWriteRequests[K],
      key: string
    ) => Promise<ProductWriteOutcome>
  ): Promise<{
    operation: K;
    request: ProductWriteRequests[K];
    outcome: ProductWriteOutcome;
  } | null> {
    if (this.busy) return null;
    this.busy = true;
    try {
      return await this.journal.exclusive(async () => {
        let record = this.restore();
        if (record?.phase === 'settled') return null;
        if (record && record.attempt.operation !== operation) {
          return {
            operation,
            request,
            outcome: {
              status: 'unknown',
              message: UNKNOWN_PRODUCT_WRITE
            } as ProductWriteOutcome
          };
        }
        if (!record) {
          record = this.journal.write(
            {
              phase: 'pending',
              attempt: {
                operation,
                key: crypto.randomUUID(),
                request: structuredClone(request)
              } as ProductPendingAttempt
            },
            null
          );
          this.cached = freeze(record);
        }
        if (record.phase !== 'pending') return null;
        const attempt = record.attempt;
        const snapshot = freeze(attempt.request) as ProductWriteRequests[K];
        let outcome: ProductWriteOutcome;
        try {
          outcome = await send(snapshot, attempt.key);
        } catch {
          outcome = { status: 'unknown', message: UNKNOWN_PRODUCT_WRITE };
        }
        if (outcome?.status === 'confirmed') {
          const value = outcome.value;
          const creating =
            operation === 'save' && (snapshot as ProductSavePayload).isNew;
          if (
            !value ||
            typeof value.id !== 'string' ||
            !value.id ||
            (!creating && value.id !== snapshot.id) ||
            (operation === 'variant'
              ? !isVariantId(value.variantId)
              : value.variantId !== undefined) ||
            (operation === 'definition'
              ? !isAttributeDefinitionResult(
                  value.definition,
                  (snapshot as AttributeDefinitionWriteRequest).input
                ) || !value.definition?.applied
              : value.definition !== undefined) ||
            (operation === 'delete' ||
            operation === 'axes' ||
            operation === 'variant' ||
            operation === 'definition'
              ? value.revision !== null
              : !Number.isInteger(value.revision) ||
                value.revision === null ||
                value.revision < 0 ||
                value.revision > 2147483647)
          ) {
            outcome = { status: 'unknown', message: UNKNOWN_PRODUCT_WRITE };
          }
        } else if (
          outcome?.status !== 'rejected' ||
          !outcome.code ||
          typeof outcome.code !== 'string' ||
          typeof outcome.message !== 'string' ||
          (operation === 'axes' && !isAxesDurableRefusalCode(outcome.code)) ||
          (operation === 'variant' &&
            !isVariantCreateRefusalCode(outcome.code)) ||
          (operation !== 'definition' && outcome.definition !== undefined) ||
          (operation === 'definition' &&
            (!isAttributeDefinitionRefusalCode(outcome.code) ||
              (outcome.code === 'PRODUCT_VALIDATION'
                ? outcome.definition !== undefined
                : !isAttributeDefinitionResult(
                    outcome.definition,
                    (snapshot as AttributeDefinitionWriteRequest).input
                  ) ||
                  outcome.definition?.applied !== false ||
                  outcome.definition.refusal === null ||
                  outcome.code !==
                    attributeDefinitionRefusalCode(
                      outcome.definition.refusal
                    ))))
        ) {
          outcome = { status: 'unknown', message: UNKNOWN_PRODUCT_WRITE };
        }
        if (outcome.status !== 'unknown') {
          // Keep a terminal tombstone for this URL intent; old tabs/history cannot
          // reinterpret an acknowledged create as permission for a new create.
          this.cached = freeze(
            this.journal.write(
              {
                phase: 'settled',
                operation,
                key: attempt.key,
                outcome,
                ...(operation === 'variant' || operation === 'definition'
                  ? { request: snapshot }
                  : {})
              } as ProductJournalRecord,
              record
            )
          );
        }
        return { operation, request: snapshot, outcome };
      });
    } catch {
      this.failure =
        'Recovery storage or the editor lock is unavailable. Restore the saved operation before continuing.';
      try {
        this.restore();
      } catch {
        /* Retain the last known identity on corruption/unavailability. */
      }
      // restore() may clear a read error; a failed write/lock still blocks this editor.
      this.failure =
        'Recovery storage or the editor lock is unavailable. Restore the saved operation before continuing.';
      return {
        operation,
        request,
        outcome: { status: 'unknown', message: this.failure }
      };
    } finally {
      this.busy = false;
    }
  }
}
