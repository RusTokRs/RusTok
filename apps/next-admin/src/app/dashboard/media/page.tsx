import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { MediaPage } from '@rustok/media-admin';
import { Suspense } from 'react';

export const metadata = {
  title: 'Dashboard: Media'
};

export default async function Page() {
  const session = await auth();
  return (
    <PageContainer scrollable pageTitle='Media' pageDescription='Media library'>
      <Suspense fallback={<p>Loading media…</p>}>
        <MediaPage
          token={session?.user?.rustokToken ?? null}
          tenantSlug={session?.user?.tenantSlug ?? null}
          tenantId={session?.user?.tenantId ?? null}
        />
      </Suspense>
    </PageContainer>
  );
}
