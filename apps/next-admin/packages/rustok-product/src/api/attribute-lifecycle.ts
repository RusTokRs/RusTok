/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import { graphqlRequest } from '@/shared/api/graphql';
import type { GqlOpts } from './types';
import { productWriteOutcome } from './write-outcome';
import {
  UNKNOWN_PRODUCT_WRITE,
  type ProductWriteOutcome
} from '../write-attempt';
import {
  isAttributeDefinitionRequest,
  isAttributeDefinitionResult,
  isAttributeLifecycleRequest,
  isAttributeWorkspace,
  isDefinitionId,
  isDefinitionLocale,
  attributeDefinitionRefusalCode,
  type AttributeDefinitionWriteRequest,
  type AttributeLifecycleRequest,
  type AttributeLifecycleWorkspace
} from '../attribute-lifecycle';

const READ = `query ProductAttributeLifecycle($tenantId: UUID!, $locale: String!, $attributeId: UUID, $optionId: UUID, $includeArchived: Boolean!) {
  productAttributeLifecycle(tenantId: $tenantId, locale: $locale, attributeId: $attributeId, optionId: $optionId, includeArchived: $includeArchived)
}`;
const WRITE = `mutation ProductUpdateAttributeDefinition($idempotencyKey: String!, $input: UpdateProductAttributeDefinitionInput!) {
  updateProductAttributeDefinition(idempotencyKey: $idempotencyKey, input: $input)
}`;

export async function readAttributeLifecycle(
  opts: GqlOpts,
  locale: string,
  request: AttributeLifecycleRequest
): Promise<AttributeLifecycleWorkspace> {
  if (
    !opts.token ||
    !isDefinitionId(opts.tenantId) ||
    !opts.tenantSlug ||
    !isDefinitionLocale(locale) ||
    !isAttributeLifecycleRequest(request)
  )
    throw new Error('Dictionary is unavailable');
  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { tenantId: string; locale: string } & AttributeLifecycleRequest,
    { productAttributeLifecycle: unknown }
  >(
    READ,
    { tenantId: opts.tenantId, locale, ...request },
    opts.token,
    opts.tenantSlug,
    { tenantId: opts.tenantId, locale }
  );
  const value = data.productAttributeLifecycle;
  if (
    !isAttributeWorkspace(value) ||
    (request.attributeId === null
      ? value.options !== null || value.impact !== null
      : value.attributes.length > 1 ||
        value.attributes.some((row) => row.id !== request.attributeId) ||
        value.options?.some((row) => row.attributeId !== request.attributeId) ||
        (value.attributes.length === 0
          ? value.options !== null || value.impact !== null
          : value.options === null ||
            value.options.some(
              (row) => row.attributeArchived !== value.attributes[0].isArchived
            ) ||
            (value.impact !== null) !==
              (request.optionId === null ||
                value.options.some((row) => row.id === request.optionId)))) ||
    (!request.includeArchived && value.attributes.some((row) => row.isArchived))
  )
    throw new Error('Dictionary projection is unavailable');
  return value;
}

/** Caller supplies a frozen command and key. This adapter never mints/normalizes retry identity. */
export async function updateAttributeDefinition(
  opts: GqlOpts,
  request: AttributeDefinitionWriteRequest,
  key: string
): Promise<ProductWriteOutcome> {
  const unknown = (): ProductWriteOutcome => ({
    status: 'unknown',
    message: UNKNOWN_PRODUCT_WRITE
  });
  if (
    !isAttributeDefinitionRequest(request) ||
    !opts.token ||
    !opts.tenantSlug ||
    opts.tenantId !== request.scope.tenantId ||
    !/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(
      key
    )
  )
    return unknown();
  try {
    const executor = opts.graphql ?? graphqlRequest;
    const input = structuredClone(request.input);
    const data = await executor<
      {
        idempotencyKey: string;
        input: Omit<typeof input, 'action'> & { action: string };
      },
      { updateProductAttributeDefinition: unknown }
    >(
      WRITE,
      {
        idempotencyKey: key,
        input: { ...input, action: input.action.toUpperCase() }
      },
      opts.token,
      opts.tenantSlug,
      { tenantId: opts.tenantId, locale: input.locale }
    );
    const result = data.updateProductAttributeDefinition;
    if (!isAttributeDefinitionResult(result, request.input)) return unknown();
    if (result.applied)
      return {
        status: 'confirmed',
        value: { id: request.id, revision: null, definition: result }
      };
    if (result.refusal === null) return unknown();
    return {
      status: 'rejected',
      code: attributeDefinitionRefusalCode(result.refusal),
      message:
        'Dictionary change was refused. Review the retained request and owner impact.',
      definition: result
    };
  } catch (error) {
    // Only the established explicit owner rejection metadata can settle validation errors.
    const result = await productWriteOutcome(async () => {
      throw error;
    });
    return result.status === 'rejected' && result.code === 'PRODUCT_VALIDATION'
      ? result
      : unknown();
  }
}
