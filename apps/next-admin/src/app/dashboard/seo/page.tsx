import { Suspense } from 'react';

import { auth } from '@/auth';
import { SeoOperatorPanel } from '@/features/seo/components/seo-operator-panel';
import { PageContainer } from '@/widgets/app-shell';

export const metadata = {
  title: 'Dashboard: SEO control plane'
};

export default async function Page() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;

  return (
    <PageContainer
      scrollable
      pageTitle='SEO Control Plane'
      pageDescription='Operator-facing SEO index tracking, repair, and replay actions.'
    >
      <Suspense
        fallback={<div className='bg-muted h-64 animate-pulse rounded-xl' />}
      >
        <SeoOperatorPanel token={token} tenantSlug={tenantSlug} />
      </Suspense>
    </PageContainer>
  );
}
