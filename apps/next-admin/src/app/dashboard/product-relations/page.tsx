/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

import { auth } from '@/auth';
import { graphqlRequest } from '@/shared/api/graphql';
import { PageContainer } from '@/widgets/app-shell';
import {
  fetchProductRelations,
  ProductRelationsCard,
  type ProductRelation
} from '@rustok/product-admin';
import {
  addProductRelationAction,
  removeProductRelationAction,
  reorderProductRelationsAction
} from '../product/actions';

export const metadata = {
  title: 'RusTok Admin: Product Relations'
};

type PageProps = {
  searchParams: Promise<{
    productId?: string;
  }>;
};

export default async function ProductRelationsRoute({
  searchParams
}: PageProps) {
  const { productId } = await searchParams;
  const session = await auth();

  const opts = {
    graphql: graphqlRequest,
    token: session?.user?.rustokToken ?? null,
    tenantSlug: session?.user?.tenantSlug ?? null,
    tenantId: session?.user?.tenantId ?? null
  };

  let relations: ProductRelation[] = [];
  if (productId) {
    try {
      relations = await fetchProductRelations(opts, productId);
    } catch (err) {
      console.error('Failed to pre-load product relations:', err);
    }
  }

  return (
    <PageContainer
      pageTitle='Product Relations'
      pageDescription='Merchandising associations, cross-sells, up-sells, accessories, and alternatives.'
    >
      <div className='space-y-6'>
        {productId ? (
          <ProductRelationsCard
            productId={productId}
            relations={relations}
            onAddRelation={async (input) => {
              'use server';
              await addProductRelationAction(input);
            }}
            onRemoveRelation={async (id) => {
              'use server';
              await removeProductRelationAction(id, productId);
            }}
            onReorderRelations={async (pid, type, orderedIds) => {
              'use server';
              await reorderProductRelationsAction(pid, type, orderedIds);
            }}
          />
        ) : (
          <div className='border-border text-muted-foreground rounded-lg border p-8 text-center'>
            <h3 className='text-foreground text-lg font-medium'>
              Select a Product
            </h3>
            <p className='mt-2 text-sm'>
              Product relations are managed within each product aggregate. Open
              any product in the{' '}
              <a href='/dashboard/product' className='text-primary underline'>
                Product Catalog
              </a>{' '}
              to configure its cross-sells, up-sells, and merchandising links,
              or specify{' '}
              <code className='text-xs'>?productId=&lt;UUID&gt;</code> in the
              URL.
            </p>
          </div>
        )}
      </div>
    </PageContainer>
  );
}
