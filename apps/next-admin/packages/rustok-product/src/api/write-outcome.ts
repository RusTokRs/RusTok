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
  PRODUCT_VALIDATION:
    'Product request is invalid. Review the fields and try again.',
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
