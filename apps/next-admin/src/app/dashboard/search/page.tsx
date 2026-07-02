import { Suspense } from 'react';
import { getLocale } from 'next-intl/server';

import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import {
  listCatalogAttributeSearchOptions,
  listCatalogCategorySearchOptions
} from '../../../../packages/rustok-product/src';
import { SearchAdminPage } from '../../../../packages/search/src';

export const metadata = {
  title: 'Dashboard: Search'
};

async function loadCatalogSearchOptions(opts: {
  token: string | null;
  tenantSlug: string | null;
  tenantId: string | null;
  locale: string;
}) {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    return { categoryOptions: [], attributeOptions: [] };
  }

  try {
    const [categoryOptions, attributeOptions] = await Promise.all([
      listCatalogCategorySearchOptions(opts, opts.locale),
      listCatalogAttributeSearchOptions(opts, opts.locale)
    ]);
    return { categoryOptions, attributeOptions };
  } catch {
    return { categoryOptions: [], attributeOptions: [] };
  }
}

type PageProps = {
  searchParams?: Promise<{
    q?: string;
  }>;
};

export default async function Page({ searchParams }: PageProps) {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;
  const locale = await getLocale();
  const { categoryOptions, attributeOptions } = await loadCatalogSearchOptions({
    token,
    tenantSlug,
    tenantId,
    locale
  });
  const resolvedSearchParams = (await searchParams) ?? {};
  const initialQuery =
    typeof resolvedSearchParams.q === 'string' ? resolvedSearchParams.q : '';

  return (
    <PageContainer
      scrollable
      pageTitle='Search'
      pageDescription='Inspect search diagnostics, queue rebuilds, and run PostgreSQL FTS previews'
    >
      <Suspense fallback={<div>Loading search control plane...</div>}>
        <SearchAdminPage
          token={token}
          tenantSlug={tenantSlug}
          initialQuery={initialQuery}
          initialTab={initialQuery ? 'playground' : 'overview'}
          categoryOptions={categoryOptions}
          attributeOptions={attributeOptions}
        />
      </Suspense>
    </PageContainer>
  );
}
