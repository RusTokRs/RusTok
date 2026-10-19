/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
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
  initialVariant?: {
    sku: string;
    barcode: string;
    priceAmount: string;
    currencyCode: string;
    compareAtAmount?: string | null;
    inventoryQuantity: number;
    inventoryPolicy: string;
  };
  attributePatches: ProductAttributeValuePatch[];
  activeLocale: string;
};

export type ProductWriteAcknowledgement = {
  id: string;
  revision: number | null;
};
export type ProductWriteOutcome =
  | { status: 'confirmed'; value: ProductWriteAcknowledgement }
  | { status: 'rejected'; code: string; message: string }
  | { status: 'unknown'; message: string };

export type ProductWriteRequests = {
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
            JSON.stringify(this.cached) !== JSON.stringify(record))
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

  async resume(handlers: ProductWriteHandlers) {
    const record = this.restore();
    if (record?.phase !== 'pending') return null;
    const attempt = record.attempt;
    switch (attempt.operation) {
      case 'save':
        return this.execute('save', attempt.request, handlers.save);
      case 'status':
        return this.execute('status', attempt.request, handlers.status);
      case 'delete':
        return this.execute('delete', attempt.request, handlers.delete);
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
          record = this.journal.write({
            phase: 'pending',
            attempt: {
              operation,
              key: crypto.randomUUID(),
              request: structuredClone(request)
            } as ProductPendingAttempt
          });
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
            (operation === 'delete'
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
          typeof outcome.message !== 'string'
        ) {
          outcome = { status: 'unknown', message: UNKNOWN_PRODUCT_WRITE };
        }
        if (outcome.status !== 'unknown') {
          // Keep a terminal tombstone for this URL intent; old tabs/history cannot
          // reinterpret an acknowledged create as permission for a new create.
          this.cached = freeze(
            this.journal.write({
              phase: 'settled',
              operation,
              key: attempt.key,
              outcome
            })
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
