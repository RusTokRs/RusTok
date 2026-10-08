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
  fetchBundles,
  BundlesPage,
  type BundleListResponse
} from '@rustok/product-admin';
import { createBundleAction, deleteBundleAction } from '../actions';

export const metadata = {
  title: 'RusTok Admin: Product Bundles'
};

type PageProps = {
  searchParams: Promise<{
    search?: string;
    status?: string;
    bundleType?: string;
    page?: string;
  }>;
};

export default async function ProductBundlesRoute({ searchParams }: PageProps) {
  const { search, status, bundleType, page } = await searchParams;
  const session = await auth();

  const opts = {
    graphql: graphqlRequest,
    token: session?.user?.rustokToken ?? null,
    tenantSlug: session?.user?.tenantSlug ?? null,
    tenantId: session?.user?.tenantId ?? null
  };

  const currentPage = parseInt(page || '1', 10) || 1;
  const perPage = 20;

  let bundlesData: BundleListResponse = {
    items: [],
    total: 0,
    page: 1,
    perPage
  };
  try {
    bundlesData = await fetchBundles(
      opts,
      {
        search: search || undefined,
        status: status || undefined,
        bundleType: bundleType || undefined,
        page: currentPage,
        perPage
      },
      'en'
    );
  } catch (err) {
    console.error('Failed to load bundles for admin directory:', err);
  }

  return (
    <PageContainer
      pageTitle='Product Bundles & Kits'
      pageDescription='Manage fixed product bundles, configurable kits, and package discounts across the catalog.'
    >
      <BundlesPage
        initialBundles={bundlesData.items}
        total={bundlesData.total}
        page={bundlesData.page}
        perPage={bundlesData.perPage}
        onCreateBundle={async (input) => {
          'use server';
          await createBundleAction(input);
        }}
        onDeleteBundle={async (id) => {
          'use server';
          await deleteBundleAction(id);
        }}
      />
    </PageContainer>
  );
}
