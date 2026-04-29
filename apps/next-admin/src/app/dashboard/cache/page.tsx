import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { CachePage } from '../../../../packages/cache/src';
import { Suspense } from 'react';

export const metadata = {
  title: 'Dashboard: Cache'
};

export default async function Page() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;

  return (
    <PageContainer
      scrollable
      pageTitle='Cache'
      pageDescription='Cache backend health and configuration'
    >
      <Suspense
        fallback={<div className='bg-muted h-40 animate-pulse rounded-xl' />}
      >
        <CachePage token={token} tenantSlug={tenantSlug} />
      </Suspense>
    </PageContainer>
  );
}
