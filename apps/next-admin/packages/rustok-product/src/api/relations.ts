/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { graphqlRequest, type GqlOpts } from '@/lib/graphql';
import type {
  ProductRelation,
  ProductRelationType,
  AddProductRelationInput
} from './types';

export const FETCH_RELATIONS_QUERY = `
query ProductAdminProductRelations($productId: UUID!, $relationType: GqlRelationType) {
  productRelations(productId: $productId, relationType: $relationType) {
    id
    productId
    relatedProductId
    relationType
    position
    metadata
    createdAt
    updatedAt
  }
}
`;

export const ADD_RELATION_MUTATION = `
mutation ProductAdminAddRelation($input: AddProductRelationInput!) {
  addProductRelation(input: $input) {
    id
    productId
    relatedProductId
    relationType
    position
    metadata
    createdAt
    updatedAt
  }
}
`;

export const REMOVE_RELATION_MUTATION = `
mutation ProductAdminRemoveRelation($id: UUID!) {
  removeProductRelation(id: $id)
}
`;

export const REORDER_RELATIONS_MUTATION = `
mutation ProductAdminReorderRelations($productId: UUID!, $relationType: GqlRelationType!, $orderedRelationIds: [UUID!]!) {
  reorderProductRelations(productId: $productId, relationType: $relationType, orderedRelationIds: $orderedRelationIds) {
    id
    productId
    relatedProductId
    relationType
    position
    metadata
    createdAt
    updatedAt
  }
}
`;

export async function fetchProductRelations(
  opts: GqlOpts,
  productId: string,
  relationType?: ProductRelationType
): Promise<ProductRelation[]> {
  if (!opts.token || !opts.tenantSlug) {
    throw new Error('Unauthorized');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { productId: string; relationType?: ProductRelationType },
    { productRelations: ProductRelation[] }
  >(
    FETCH_RELATIONS_QUERY,
    {
      productId,
      relationType: relationType || undefined
    },
    opts.token,
    opts.tenantSlug
  );

  return data.productRelations || [];
}

export async function addProductRelation(
  opts: GqlOpts,
  input: AddProductRelationInput
): Promise<ProductRelation> {
  if (!opts.token || !opts.tenantSlug) {
    throw new Error('Unauthorized');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { input: AddProductRelationInput },
    { addProductRelation: ProductRelation }
  >(
    ADD_RELATION_MUTATION,
    { input },
    opts.token,
    opts.tenantSlug
  );

  return data.addProductRelation;
}

export async function removeProductRelation(
  opts: GqlOpts,
  id: string
): Promise<boolean> {
  if (!opts.token || !opts.tenantSlug) {
    throw new Error('Unauthorized');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { id: string },
    { removeProductRelation: boolean }
  >(
    REMOVE_RELATION_MUTATION,
    { id },
    opts.token,
    opts.tenantSlug
  );

  return data.removeProductRelation;
}

export async function reorderProductRelations(
  opts: GqlOpts,
  productId: string,
  relationType: ProductRelationType,
  orderedRelationIds: string[]
): Promise<ProductRelation[]> {
  if (!opts.token || !opts.tenantSlug) {
    throw new Error('Unauthorized');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    {
      productId: string;
      relationType: ProductRelationType;
      orderedRelationIds: string[];
    },
    { reorderProductRelations: ProductRelation[] }
  >(
    REORDER_RELATIONS_MUTATION,
    {
      productId,
      relationType,
      orderedRelationIds
    },
    opts.token,
    opts.tenantSlug
  );

  return data.reorderProductRelations || [];
}
