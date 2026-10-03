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
  CatalogCategorySummary,
  CreateCatalogCategoryPayload,
  CreateCategoryAttributeGroupPayload,
  SetCategorySchemaModePayload,
  BindCategoryAttributePayload,
  ProductEffectiveForm
} from './types';

const CATALOG_CATEGORIES_QUERY = `
query ProductAdminCatalogCategories($tenantId: UUID!, $locale: String!) {
  catalogCategories(tenantId: $tenantId, locale: $locale) {
    total
    items {
      id
      parentId
      code
      slug
      path
      kind
      name
    }
  }
}`;

const CREATE_CATALOG_CATEGORY_MUTATION = `
mutation ProductAdminCreateCatalogCategory(
  $idempotencyKey: String!
  $locale: String!
  $input: CreateCatalogCategoryInput!
) {
  createCatalogCategory(
    idempotencyKey: $idempotencyKey
    locale: $locale
    input: $input
  )
}`;

const CREATE_CATEGORY_GROUP_MUTATION = `
mutation ProductAdminCreateCategoryGroup(
  $idempotencyKey: String!
  $locale: String!
  $input: CreateCategoryAttributeGroupInput!
) {
  createCatalogCategoryAttributeGroup(
    idempotencyKey: $idempotencyKey
    locale: $locale
    input: $input
  )
}`;

const SET_CATEGORY_SCHEMA_MODE_MUTATION = `
mutation ProductAdminSetCategorySchemaMode(
  $idempotencyKey: String!
  $input: SetCategorySchemaModeInput!
) {
  setCatalogCategorySchemaMode(
    idempotencyKey: $idempotencyKey
    input: $input
  )
}`;

const BIND_CATEGORY_ATTRIBUTE_MUTATION = `
mutation ProductAdminBindCategoryAttribute(
  $idempotencyKey: String!
  $input: BindCategoryAttributeInput!
) {
  bindCatalogCategoryAttribute(
    idempotencyKey: $idempotencyKey
    input: $input
  )
}`;

const CATEGORY_EFFECTIVE_FORM_QUERY = `
query ProductAdminCategoryEffectiveForm(
  $tenantId: UUID!
  $categoryId: UUID!
  $locale: String!
) {
  productEffectiveForm(
    tenantId: $tenantId
    categoryId: $categoryId
    locale: $locale
  ) {
    categoryId
    detachedAttributeIds
    attributes {
      attributeId
      code
      label
      valueType
      isLocalized
      options {
        id
        code
        label
        position
      }
      groupCode
      groupLabel
      isRequired
      isDisabled
      position
      source
      variantAxisPolicy
      defaultVariantAxis
    }
  }
}`;

type CategoriesResponse = {
  catalogCategories: {
    total: number;
    items: CatalogCategorySummary[];
  };
};

type EffectiveFormResponse = {
  productEffectiveForm: ProductEffectiveForm | null;
};

export async function listCatalogCategories(
  opts: GqlOpts,
  locale = 'en'
): Promise<CatalogCategorySummary[]> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    return [];
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { tenantId: string; locale: string },
    CategoriesResponse
  >(
    CATALOG_CATEGORIES_QUERY,
    { tenantId: opts.tenantId, locale },
    opts.token,
    opts.tenantSlug
  );

  return data.catalogCategories.items;
}

export async function createCatalogCategory(
  opts: GqlOpts,
  payload: CreateCatalogCategoryPayload,
  locale = 'en'
): Promise<boolean> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to create catalog category.');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const idempotencyKey = crypto.randomUUID();

  const input = {
    parentId: payload.parentId || null,
    code: payload.code.trim(),
    slug: payload.slug.trim(),
    kind: payload.kind || 'standard',
    name: payload.name.trim(),
    description: payload.description?.trim() || null
  };

  const data = await executor<
    {
      idempotencyKey: string;
      locale: string;
      input: typeof input;
    },
    { createCatalogCategory: boolean }
  >(
    CREATE_CATALOG_CATEGORY_MUTATION,
    { idempotencyKey, locale, input },
    opts.token,
    opts.tenantSlug
  );

  return Boolean(data.createCatalogCategory);
}

export async function createCategoryAttributeGroup(
  opts: GqlOpts,
  payload: CreateCategoryAttributeGroupPayload,
  locale = 'en'
): Promise<boolean> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to create category attribute group.');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const idempotencyKey = crypto.randomUUID();

  const input = {
    categoryId: payload.categoryId,
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
    { createCatalogCategoryAttributeGroup: boolean }
  >(
    CREATE_CATEGORY_GROUP_MUTATION,
    { idempotencyKey, locale, input },
    opts.token,
    opts.tenantSlug
  );

  return Boolean(data.createCatalogCategoryAttributeGroup);
}

export async function setCatalogCategorySchemaMode(
  opts: GqlOpts,
  payload: SetCategorySchemaModePayload
): Promise<boolean> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to update category schema mode.');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const idempotencyKey = crypto.randomUUID();

  const input = {
    categoryId: payload.categoryId,
    mode: payload.mode,
    schemaId: payload.schemaId || null,
    cloneFromCategoryId: payload.cloneFromCategoryId || null
  };

  const data = await executor<
    {
      idempotencyKey: string;
      input: typeof input;
    },
    { setCatalogCategorySchemaMode: boolean }
  >(
    SET_CATEGORY_SCHEMA_MODE_MUTATION,
    { idempotencyKey, input },
    opts.token,
    opts.tenantSlug
  );

  return Boolean(data.setCatalogCategorySchemaMode);
}

export async function bindCategoryAttribute(
  opts: GqlOpts,
  payload: BindCategoryAttributePayload
): Promise<boolean> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to bind category attribute.');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const idempotencyKey = crypto.randomUUID();

  const input = {
    categoryId: payload.categoryId,
    attributeId: payload.attributeId,
    groupCode: payload.groupCode || null,
    bindingKind: payload.bindingKind,
    isRequired: payload.isRequired ?? null,
    isDisabled: Boolean(payload.isDisabled),
    position: payload.position ?? null
  };

  const data = await executor<
    {
      idempotencyKey: string;
      input: typeof input;
    },
    { bindCatalogCategoryAttribute: boolean }
  >(
    BIND_CATEGORY_ATTRIBUTE_MUTATION,
    { idempotencyKey, input },
    opts.token,
    opts.tenantSlug
  );

  return Boolean(data.bindCatalogCategoryAttribute);
}

export async function getCategoryEffectiveForm(
  opts: GqlOpts,
  categoryId: string,
  locale = 'en'
): Promise<ProductEffectiveForm | null> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    return null;
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { tenantId: string; categoryId: string; locale: string },
    EffectiveFormResponse
  >(
    CATEGORY_EFFECTIVE_FORM_QUERY,
    { tenantId: opts.tenantId, categoryId, locale },
    opts.token,
    opts.tenantSlug
  );

  return data.productEffectiveForm;
}
