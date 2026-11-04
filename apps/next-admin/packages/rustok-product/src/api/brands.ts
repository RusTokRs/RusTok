/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

import { graphqlRequest, type GqlOpts } from '@/lib/graphql';
import type {
  BrandSummary,
  BrandFilterInput,
  BrandListResponse,
  CreateBrandPayload,
  UpdateBrandPayload
} from './types';

const BRANDS_QUERY = `
query ProductAdminBrands(
  $page: Int
  $perPage: Int
  $search: String
  $isActive: Boolean
  $categoryId: UUID
  $locale: String
) {
  brands(
    page: $page
    perPage: $perPage
    locale: $locale
    filter: {
      search: $search
      isActive: $isActive
      categoryId: $categoryId
    }
  ) {
    total
    page
    perPage
    items {
      id
      tenantId
      slug
      name
      description
      websiteUrl
      logoUrl
      isActive
      sortOrder
      productsCount
      createdAt
      updatedAt
    }
  }
}`;

const CATEGORY_BRANDS_QUERY = `
query ProductAdminCategoryBrands($categoryId: UUID!, $locale: String) {
  categoryBrands(categoryId: $categoryId, locale: $locale) {
    id
    tenantId
    slug
    name
    description
    websiteUrl
    logoUrl
    isActive
    sortOrder
    productsCount
    createdAt
    updatedAt
  }
}`;

const CREATE_BRAND_MUTATION = `
mutation ProductAdminCreateBrand($input: CreateBrandInputGql!) {
  createBrand(input: $input) {
    id
    tenantId
    slug
    name
    description
    websiteUrl
    logoUrl
    isActive
    sortOrder
    createdAt
    updatedAt
  }
}`;

const UPDATE_BRAND_MUTATION = `
mutation ProductAdminUpdateBrand($id: UUID!, $input: UpdateBrandInputGql!) {
  updateBrand(id: $id, input: $input) {
    id
    tenantId
    slug
    name
    description
    websiteUrl
    logoUrl
    isActive
    sortOrder
    createdAt
    updatedAt
  }
}`;

const DELETE_BRAND_MUTATION = `
mutation ProductAdminDeleteBrand($id: UUID!) {
  deleteBrand(id: $id)
}`;

export async function listBrands(
  opts: GqlOpts,
  filter?: BrandFilterInput,
  locale?: string
): Promise<BrandListResponse> {
  const data = await graphqlRequest<{ brands: BrandListResponse }>(
    BRANDS_QUERY,
    {
      page: filter?.page ?? 1,
      perPage: filter?.perPage ?? 50,
      search: filter?.search || null,
      isActive: filter?.isActive ?? null,
      categoryId: filter?.categoryId || null,
      locale: locale || opts.locale || 'en'
    },
    opts.token,
    opts.tenantSlug,
    { tenantId: opts.tenantId, locale: locale || opts.locale }
  );

  return data.brands;
}

export async function listCategoryBrands(
  opts: GqlOpts,
  categoryId: string,
  locale?: string
): Promise<BrandSummary[]> {
  const data = await graphqlRequest<{ categoryBrands: BrandSummary[] }>(
    CATEGORY_BRANDS_QUERY,
    {
      categoryId,
      locale: locale || opts.locale || 'en'
    },
    opts.token,
    opts.tenantSlug,
    { tenantId: opts.tenantId, locale: locale || opts.locale }
  );

  return data.categoryBrands;
}

export async function createBrand(
  opts: GqlOpts,
  input: CreateBrandPayload
): Promise<BrandSummary> {
  const data = await graphqlRequest<{ createBrand: BrandSummary }>(
    CREATE_BRAND_MUTATION,
    {
      input: {
        slug: input.slug,
        name: input.name,
        description: input.description || null,
        websiteUrl: input.websiteUrl || null,
        isActive: input.isActive ?? true
      }
    },
    opts.token,
    opts.tenantSlug,
    { tenantId: opts.tenantId, locale: opts.locale }
  );

  return data.createBrand;
}

export async function updateBrand(
  opts: GqlOpts,
  id: string,
  input: UpdateBrandPayload
): Promise<BrandSummary> {
  const data = await graphqlRequest<{ updateBrand: BrandSummary }>(
    UPDATE_BRAND_MUTATION,
    {
      id,
      input: {
        slug: input.slug || null,
        name: input.name || null,
        description: input.description || null,
        websiteUrl: input.websiteUrl || null,
        isActive: input.isActive ?? null
      }
    },
    opts.token,
    opts.tenantSlug,
    { tenantId: opts.tenantId, locale: opts.locale }
  );

  return data.updateBrand;
}

export async function deleteBrand(
  opts: GqlOpts,
  id: string
): Promise<boolean> {
  const data = await graphqlRequest<{ deleteBrand: boolean }>(
    DELETE_BRAND_MUTATION,
    { id },
    opts.token,
    opts.tenantSlug,
    { tenantId: opts.tenantId, locale: opts.locale }
  );

  return data.deleteBrand;
}
