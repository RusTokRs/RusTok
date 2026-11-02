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
  CANNOT_DELETE_PUBLISHED: 'Archive the published product before deleting it.'
};

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
