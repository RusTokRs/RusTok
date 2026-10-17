/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

import { graphqlRequest } from '@/lib/graphql';
import type {
  GqlOpts,
  ProductBundle,
  BundleItem,
  CreateBundleInput,
  UpdateBundleInput,
  AddBundleItemInput,
  BundleListResponse
} from './types';

const BUNDLE_FIELDS = `
  id
  tenantId
  bundleProductId
  slug
  name
  description
  bundleType
  status
  discountType
  discountValue
  metadata
  createdAt
  updatedAt
  items {
    id
    bundleId
    productId
    variantId
    quantity
    isOptional
    discountRate
    position
  }
`;

const PRODUCT_BUNDLES_QUERY = `
query ProductBundles($productId: UUID!, $locale: String) {
  productBundles(productId: $productId, locale: $locale) {
    ${BUNDLE_FIELDS}
  }
}`;

const BUNDLE_QUERY = `
query Bundle($id: UUID!, $locale: String) {
  bundle(id: $id, locale: $locale) {
    ${BUNDLE_FIELDS}
  }
}`;

const BUNDLES_QUERY = `
query Bundles($filter: GqlBundleFilter, $page: Int, $perPage: Int, $locale: String) {
  bundles(filter: $filter, page: $page, perPage: $perPage, locale: $locale) {
    total
    page
    perPage
    items {
      ${BUNDLE_FIELDS}
    }
  }
}`;

const CREATE_BUNDLE_MUTATION = `
mutation CreateBundle($idempotencyKey: String!, $input: CreateBundleInputGql!, $locale: String) {
  createBundle(idempotencyKey: $idempotencyKey, input: $input, locale: $locale) {
    ${BUNDLE_FIELDS}
  }
}`;

const UPDATE_BUNDLE_MUTATION = `
mutation UpdateBundle($idempotencyKey: String!, $id: UUID!, $input: UpdateBundleInputGql!, $locale: String) {
  updateBundle(idempotencyKey: $idempotencyKey, id: $id, input: $input, locale: $locale) {
    ${BUNDLE_FIELDS}
  }
}`;

const DELETE_BUNDLE_MUTATION = `
mutation DeleteBundle($idempotencyKey: String!, $id: UUID!) {
  deleteBundle(idempotencyKey: $idempotencyKey, id: $id)
}`;

const ADD_BUNDLE_ITEM_MUTATION = `
mutation AddBundleItem($idempotencyKey: String!, $bundleId: UUID!, $item: BundleItemInputGql!) {
  addBundleItem(idempotencyKey: $idempotencyKey, bundleId: $bundleId, item: $item) {
    id
    bundleId
    productId
    variantId
    quantity
    isOptional
    discountRate
    position
  }
}`;

const REMOVE_BUNDLE_ITEM_MUTATION = `
mutation RemoveBundleItem($idempotencyKey: String!, $bundleId: UUID!, $itemId: UUID!) {
  removeBundleItem(idempotencyKey: $idempotencyKey, bundleId: $bundleId, itemId: $itemId)
}`;

export async function fetchProductBundles(
  opts: GqlOpts,
  productId: string,
  locale?: string
): Promise<ProductBundle[]> {
  if (!opts.token || !opts.tenantSlug) {
    return [];
  }

  const executor = opts.graphql ?? graphqlRequest;
  try {
    const data = await executor<
      { productId: string; locale?: string },
      { productBundles: ProductBundle[] }
    >(
      PRODUCT_BUNDLES_QUERY,
      { productId, locale },
      opts.token,
      opts.tenantSlug
    );
    return data?.productBundles ?? [];
  } catch (error) {
    console.error('Failed to fetch product bundles:', error);
    return [];
  }
}

export async function fetchBundle(
  opts: GqlOpts,
  id: string,
  locale?: string
): Promise<ProductBundle | null> {
  if (!opts.token || !opts.tenantSlug) {
    return null;
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { id: string; locale?: string },
    { bundle: ProductBundle | null }
  >(BUNDLE_QUERY, { id, locale }, opts.token, opts.tenantSlug);

  return data?.bundle ?? null;
}

export async function fetchBundles(
  opts: GqlOpts,
  filter?: {
    search?: string;
    status?: string;
    bundleType?: string;
    page?: number;
    perPage?: number;
  },
  locale?: string
): Promise<BundleListResponse> {
  if (!opts.token || !opts.tenantSlug) {
    return { items: [], total: 0, page: 1, perPage: 20 };
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    {
      filter?: { search?: string; status?: string; bundleType?: string };
      page?: number;
      perPage?: number;
      locale?: string;
    },
    { bundles: BundleListResponse }
  >(
    BUNDLES_QUERY,
    {
      filter: filter
        ? {
            search: filter.search,
            status: filter.status,
            bundleType: filter.bundleType
          }
        : undefined,
      page: filter?.page,
      perPage: filter?.perPage,
      locale
    },
    opts.token,
    opts.tenantSlug
  );

  return (
    data?.bundles ?? {
      items: [],
      total: 0,
      page: 1,
      perPage: 20
    }
  );
}

/**
 * Pass the same `idempotencyKey` again when retrying an uncertain request.
 */
export async function createBundle(
  opts: GqlOpts,
  input: CreateBundleInput,
  locale?: string,
  idempotencyKey = crypto.randomUUID()
): Promise<ProductBundle> {
  if (!opts.token || !opts.tenantSlug) {
    throw new Error('Sign in again to create a bundle.');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    {
      idempotencyKey: string;
      input: {
        bundleProductId?: string | null;
        slug: string;
        name: string;
        description?: string | null;
        bundleType: string;
        status: string;
        discountType: string;
        discountValue: string;
        metadata?: Record<string, unknown> | null;
      };
      locale?: string;
    },
    { createBundle: ProductBundle }
  >(
    CREATE_BUNDLE_MUTATION,
    {
      idempotencyKey,
      input: {
        bundleProductId: input.bundleProductId || null,
        slug: input.slug,
        name: input.name,
        description: input.description || null,
        bundleType: input.bundleType,
        status: input.status,
        discountType: input.discountType,
        discountValue: input.discountValue || '0',
        metadata: input.metadata || null
      },
      locale
    },
    opts.token,
    opts.tenantSlug
  );

  return data.createBundle;
}

/**
 * Pass the same `idempotencyKey` again when retrying an uncertain request.
 */
export async function updateBundle(
  opts: GqlOpts,
  id: string,
  input: UpdateBundleInput,
  locale?: string,
  idempotencyKey = crypto.randomUUID()
): Promise<ProductBundle> {
  if (!opts.token || !opts.tenantSlug) {
    throw new Error('Sign in again to update bundle.');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    {
      idempotencyKey: string;
      id: string;
      input: {
        bundleProductId?: string | null;
        slug?: string | null;
        name?: string | null;
        description?: string | null;
        bundleType?: string | null;
        status?: string | null;
        discountType?: string | null;
        discountValue?: string | null;
        metadata?: Record<string, unknown> | null;
      };
      locale?: string;
    },
    { updateBundle: ProductBundle }
  >(
    UPDATE_BUNDLE_MUTATION,
    {
      idempotencyKey,
      id,
      // Conditional fields preserve GraphQL's meaningful omitted/null/value distinction.
      // In particular, null clears owner/description/metadata while undefined keeps them.
      input: {
        ...(input.bundleProductId === undefined
          ? {}
          : { bundleProductId: input.bundleProductId }),
        ...(input.slug === undefined ? {} : { slug: input.slug }),
        ...(input.name === undefined ? {} : { name: input.name }),
        ...(input.description === undefined
          ? {}
          : { description: input.description }),
        ...(input.bundleType === undefined
          ? {}
          : { bundleType: input.bundleType }),
        ...(input.status === undefined ? {} : { status: input.status }),
        ...(input.discountType === undefined
          ? {}
          : { discountType: input.discountType }),
        ...(input.discountValue === undefined
          ? {}
          : { discountValue: input.discountValue }),
        ...(input.metadata === undefined ? {} : { metadata: input.metadata })
      },
      locale
    },
    opts.token,
    opts.tenantSlug
  );

  return data.updateBundle;
}

/**
 * Pass the same `idempotencyKey` again when retrying an uncertain request.
 */
export async function deleteBundle(
  opts: GqlOpts,
  id: string,
  idempotencyKey = crypto.randomUUID()
): Promise<boolean> {
  if (!opts.token || !opts.tenantSlug) {
    throw new Error('Sign in again to delete bundle.');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { idempotencyKey: string; id: string },
    { deleteBundle: boolean }
  >(
    DELETE_BUNDLE_MUTATION,
    { idempotencyKey, id },
    opts.token,
    opts.tenantSlug
  );

  return data?.deleteBundle ?? true;
}

/**
 * Pass the same `idempotencyKey` again when retrying an uncertain request.
 */
export async function addBundleItem(
  opts: GqlOpts,
  input: AddBundleItemInput,
  idempotencyKey = crypto.randomUUID()
): Promise<BundleItem> {
  if (!opts.token || !opts.tenantSlug) {
    throw new Error('Sign in again to add bundle item.');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    {
      idempotencyKey: string;
      bundleId: string;
      item: {
        productId: string;
        variantId?: string | null;
        quantity: number;
        isOptional: boolean;
        discountRate?: string | null;
        position?: number;
      };
    },
    { addBundleItem: BundleItem }
  >(
    ADD_BUNDLE_ITEM_MUTATION,
    {
      idempotencyKey,
      bundleId: input.bundleId,
      item: {
        productId: input.productId,
        variantId: input.variantId || null,
        quantity: input.quantity ?? 1,
        isOptional: Boolean(input.isOptional),
        discountRate: input.discountRate || null,
        // Omit the field to ask the owner service to append after the current maximum.
        ...(input.position === undefined ? {} : { position: input.position })
      }
    },
    opts.token,
    opts.tenantSlug
  );

  return data.addBundleItem;
}

/**
 * Pass the same `idempotencyKey` again when retrying an uncertain request.
 */
export async function removeBundleItem(
  opts: GqlOpts,
  bundleId: string,
  itemId: string,
  idempotencyKey = crypto.randomUUID()
): Promise<boolean> {
  if (!opts.token || !opts.tenantSlug) {
    throw new Error('Sign in again to remove bundle item.');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { idempotencyKey: string; bundleId: string; itemId: string },
    { removeBundleItem: boolean }
  >(
    REMOVE_BUNDLE_ITEM_MUTATION,
    { idempotencyKey, bundleId, itemId },
    opts.token,
    opts.tenantSlug
  );

  return data?.removeBundleItem ?? true;
}
