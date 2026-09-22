import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { PostFormPage } from '../../../../../packages/blog/src';
import { Suspense } from 'react';

export const metadata = {
  title: 'Dashboard: New Post'
};

export default async function Page() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;

  return (
    <PageContainer scrollable pageTitle='Create Post'>
      <Suspense fallback={<div>Loading form...</div>}>
        <PostFormPage
          token={token}
          tenantSlug={tenantSlug}
          tenantId={tenantId}
        />
      </Suspense>
    </PageContainer>
  );
}
