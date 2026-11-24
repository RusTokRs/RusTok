/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import { GraphqlError } from '@/lib/graphql';
import type {
  ProductWriteAcknowledgement,
  ProductWriteOutcome
} from '../write-attempt';
import { UNKNOWN_PRODUCT_WRITE } from '../write-attempt';

// Do not turn an auth/preflight refusal, missing resource, unknown receipt conflict,
// or arbitrary redacted Server Action exception into permission to create again.
const definitiveRefusals: Record<string, string> = {
  PRODUCT_CONFLICT:
    'Product request conflicts with the current state. Review the saved request.',
  PRODUCT_DEPENDENCY_NOT_FOUND:
    'A resource in the saved request no longer exists. Review the request.',
  PRODUCT_NOT_FOUND:
    'The Product no longer exists. This operation was not applied.',
  PRODUCT_VARIANT_NOT_FOUND:
    'An SKU in this request no longer exists. Preview a corrected proposal.',
  PRODUCT_VARIANT_AXIS_POLICY_CONFLICT:
    'The schema axis policy conflicts with existing Product configurations.',
  PRODUCT_VALIDATION:
    'Product request is invalid. Review the fields and try again.',
  PRODUCT_VARIANT_AXES_PREVIEW_REQUIRED:
    'Preview the exact axes proposal before applying it.',
  PRODUCT_VARIANT_AXES_STALE:
    'Axes or assignments changed. Preview the proposal again; no changes were applied.',
  PRODUCT_VARIANT_AXES_CONFLICT:
    'Resolve the missing assignments and combination collisions shown by preview.',
  PRODUCT_VARIANT_AXES_LIMIT_EXCEEDED:
    'Atomic reconfiguration supports at most 256 variants, 32 axes and 256 options per axis.',
  PRODUCT_REVISION_CONFLICT:
    'Product changed in another editor. Reload it before saving again.',
  DUPLICATE_HANDLE: 'This product handle is already in use.',
  DUPLICATE_SKU: 'This SKU is already in use.',
  NO_VARIANTS: 'Product requires at least one variant.',
  CANNOT_DELETE_ONLY_VARIANT: 'Product must have at least one variant.',
  PRODUCT_INVENTORY_DELETION_INVALID:
    'The Inventory deletion request is invalid. Review the variant before retrying.',
  PRODUCT_INVENTORY_DELETION_LIMIT:
    'The Inventory deletion request exceeds the supported limit.',
  PRODUCT_INVENTORY_DELETION_HISTORY_PRESENT:
    'This variant has inventory history and cannot be deleted.',
  PRODUCT_INVENTORY_DELETION_STOCK_PRESENT:
    'Remove the remaining stock before deleting this variant.',
  PRODUCT_INVENTORY_DELETION_CHANGED:
    'Inventory changed while deleting this variant. Reload and review the current state.',
  PRODUCT_INVENTORY_QUANTITY_INVALID:
    'The requested inventory quantity is invalid.',
  PRODUCT_INVENTORY_QUANTITY_LIMIT:
    'The requested inventory quantity exceeds the supported limit.',
  PRODUCT_INVENTORY_QUANTITY_LOCATION_UNAVAILABLE:
    'No eligible stock location is available for this variant.',
  PRODUCT_INVENTORY_QUANTITY_UNREPRESENTABLE:
    'Inventory cannot represent the requested quantity.',
  PRODUCT_INVENTORY_QUANTITY_INSUFFICIENT:
    'There is not enough stock for the requested quantity change.',
  PRODUCT_INVENTORY_QUANTITY_CHANGED:
    'Inventory changed concurrently. Reload the variant and try again.',
  PRODUCT_INVENTORY_QUANTITY_HISTORY_PRESENT:
    'Inventory history prevents this quantity operation.',
  CANNOT_DELETE_PUBLISHED: 'Archive the published product before deleting it.',
  PRODUCT_OPERATION_FAILED:
    'The operation could not be completed safely. Review the request and try again.',
  IDEMPOTENCY_KEY_CONFLICT:
    'This attempt was already used with a different request. Try again with the current values.'
};

/**
 * True when the owner definitively refused the write: retrying the exact same
 * request can never succeed (validation, permission, conflict, or a receipt
 * bound to a different payload).
 *
 * Callers that reuse one idempotency key across retries of an uncertain
 * submission must rotate the key when this returns true; only ambiguous
 * (network-level) failures keep the key for a receipt replay.
 */
export function isDefinitiveProductWriteRefusal(error: unknown): boolean {
  return (
    error instanceof GraphqlError &&
    error.retryable === false &&
    error.outcome === 'rejected'
  );
}

/** Serializable boundary: production Server Action exception redaction is irrelevant. */
export async function productWriteOutcome(
  write: () => Promise<ProductWriteAcknowledgement>
): Promise<ProductWriteOutcome> {
  try {
    return { status: 'confirmed', value: await write() };
  } catch (error) {
    if (
      error instanceof GraphqlError &&
      error.retryable === false &&
      error.outcome === 'rejected' &&
      error.code &&
      Object.hasOwn(definitiveRefusals, error.code)
    ) {
      return {
        status: 'rejected',
        code: error.code,
        message: definitiveRefusals[error.code]
      };
    }
    return { status: 'unknown', message: UNKNOWN_PRODUCT_WRITE };
  }
}
