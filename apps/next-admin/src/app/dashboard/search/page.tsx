import { Suspense } from 'react';

import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { SearchAdminPage } from '../../../../packages/search/src';

export const metadata = {
  title: 'Dashboard: Search'
};

type PageProps = {
  searchParams?: Promise<{
    q?: string;
  }>;
};

export default async function Page({ searchParams }: PageProps) {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
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
        />
      </Suspense>
    </PageContainer>
  );
}
