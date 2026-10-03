/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

import { graphqlRequest } from '@/shared/api/graphql';
import type {
  GqlOpts,
  ProductAttributeSummary,
  CreateProductAttributePayload,
  CreateProductAttributeOptionPayload,
  ProductAttributeSchemaSummary,
  CreateProductAttributeSchemaPayload,
  BindSchemaAttributePayload
} from './types';

const PRODUCT_ATTRIBUTES_QUERY = `
query ProductAdminAttributes($tenantId: UUID!, $locale: String!) {
  productAttributes(tenantId: $tenantId, locale: $locale) {
    total
    items {
      id
      code
      valueType
      isLocalized
      isFilterable
      isSearchable
      isSortable
      showOnStorefront
      label
    }
  }
}`;

const PRODUCT_ATTRIBUTE_SCHEMAS_QUERY = `
query ProductAdminAttributeSchemas($tenantId: UUID!, $locale: String!) {
  productAttributeSchemas(tenantId: $tenantId, locale: $locale) {
    total
    items {
      id
      code
      name
    }
  }
}`;

const CREATE_PRODUCT_ATTRIBUTE_MUTATION = `
mutation ProductAdminCreateAttribute(
  $idempotencyKey: String!
  $locale: String!
  $input: CreateProductAttributeInput!
) {
  createProductAttribute(
    idempotencyKey: $idempotencyKey
    locale: $locale
    input: $input
  )
}`;

const CREATE_PRODUCT_ATTRIBUTE_OPTION_MUTATION = `
mutation ProductAdminCreateAttributeOption(
  $idempotencyKey: String!
  $locale: String!
  $input: CreateProductAttributeOptionInput!
) {
  createProductAttributeOption(
    idempotencyKey: $idempotencyKey
    locale: $locale
    input: $input
  )
}`;

const CREATE_PRODUCT_ATTRIBUTE_SCHEMA_MUTATION = `
mutation ProductAdminCreateAttributeSchema(
  $idempotencyKey: String!
  $locale: String!
  $input: CreateProductAttributeSchemaInput!
) {
  createProductAttributeSchema(
    idempotencyKey: $idempotencyKey
    locale: $locale
    input: $input
  )
}`;

const BIND_SCHEMA_ATTRIBUTE_MUTATION = `
mutation ProductAdminBindSchemaAttribute(
  $idempotencyKey: String!
  $input: BindSchemaAttributeInput!
) {
  bindProductAttributeSchemaAttribute(
    idempotencyKey: $idempotencyKey
    input: $input
  )
}`;

type AttributesResponse = {
  productAttributes: {
    total: number;
    items: ProductAttributeSummary[];
  };
};

type AttributeSchemasResponse = {
  productAttributeSchemas: {
    total: number;
    items: ProductAttributeSchemaSummary[];
  };
};

export async function listProductAttributes(
  opts: GqlOpts,
  locale = 'en'
): Promise<ProductAttributeSummary[]> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    return [];
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<{ tenantId: string; locale: string }, AttributesResponse>(
    PRODUCT_ATTRIBUTES_QUERY,
    { tenantId: opts.tenantId, locale },
    opts.token,
    opts.tenantSlug
  );

  return data.productAttributes.items;
}

export async function createProductAttribute(
  opts: GqlOpts,
  payload: CreateProductAttributePayload,
  locale = 'en'
): Promise<boolean> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to create product attribute.');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const idempotencyKey = crypto.randomUUID();

  const input = {
    code: payload.code.trim(),
    valueType: payload.valueType,
    label: payload.label.trim(),
    helpText: payload.helpText?.trim() || null,
    isLocalized: Boolean(payload.isLocalized),
    isFilterable: Boolean(payload.isFilterable),
    isSearchable: Boolean(payload.isSearchable),
    isSortable: Boolean(payload.isSortable),
    showOnStorefront: Boolean(payload.showOnStorefront)
  };

  const data = await executor<
    {
      idempotencyKey: string;
      locale: string;
      input: typeof input;
    },
    { createProductAttribute: boolean }
  >(
    CREATE_PRODUCT_ATTRIBUTE_MUTATION,
    { idempotencyKey, locale, input },
    opts.token,
    opts.tenantSlug
  );

  return Boolean(data.createProductAttribute);
}

export async function createProductAttributeOption(
  opts: GqlOpts,
  payload: CreateProductAttributeOptionPayload,
  locale = 'en'
): Promise<boolean> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to create attribute option.');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const idempotencyKey = crypto.randomUUID();

  const input = {
    attributeId: payload.attributeId,
    code: payload.code.trim(),
    label: payload.label.trim(),
    position: payload.position ?? 0
  };

  const data = await executor<
    {
      idempotencyKey: string;
      locale: string;
      input: typeof input;
    },
    { createProductAttributeOption: boolean }
  >(
    CREATE_PRODUCT_ATTRIBUTE_OPTION_MUTATION,
    { idempotencyKey, locale, input },
    opts.token,
    opts.tenantSlug
  );

  return Boolean(data.createProductAttributeOption);
}

export async function listProductAttributeSchemas(
  opts: GqlOpts,
  locale = 'en'
): Promise<ProductAttributeSchemaSummary[]> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    return [];
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<{ tenantId: string; locale: string }, AttributeSchemasResponse>(
    PRODUCT_ATTRIBUTE_SCHEMAS_QUERY,
    { tenantId: opts.tenantId, locale },
    opts.token,
    opts.tenantSlug
  );

  return data.productAttributeSchemas.items;
}

export async function createProductAttributeSchema(
  opts: GqlOpts,
  payload: CreateProductAttributeSchemaPayload,
  locale = 'en'
): Promise<boolean> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to create attribute schema.');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const idempotencyKey = crypto.randomUUID();

  const input = {
    code: payload.code.trim(),
    name: payload.name.trim(),
    description: payload.description?.trim() || null
  };

  const data = await executor<
    {
      idempotencyKey: string;
      locale: string;
      input: typeof input;
    },
    { createProductAttributeSchema: boolean }
  >(
    CREATE_PRODUCT_ATTRIBUTE_SCHEMA_MUTATION,
    { idempotencyKey, locale, input },
    opts.token,
    opts.tenantSlug
  );

  return Boolean(data.createProductAttributeSchema);
}

export async function bindSchemaAttribute(
  opts: GqlOpts,
  payload: BindSchemaAttributePayload
): Promise<boolean> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to bind schema attribute.');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const idempotencyKey = crypto.randomUUID();

  const input = {
    schemaId: payload.schemaId,
    attributeId: payload.attributeId,
    groupCode: payload.groupCode || null,
    isRequired: Boolean(payload.isRequired),
    isDisabled: Boolean(payload.isDisabled),
    position: payload.position ?? 0
  };

  const data = await executor<
    {
      idempotencyKey: string;
      input: typeof input;
    },
    { bindProductAttributeSchemaAttribute: boolean }
  >(
    BIND_SCHEMA_ATTRIBUTE_MUTATION,
    { idempotencyKey, input },
    opts.token,
    opts.tenantSlug
  );

  return Boolean(data.bindProductAttributeSchemaAttribute);
}
