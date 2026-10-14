import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { FormsPage } from '@rustok/forms-admin';
import { Suspense } from 'react';

export const metadata = {
  title: 'Dashboard: Form submissions'
};

export default async function Page() {
  const session = await auth();
  return (
    <PageContainer scrollable pageTitle="Form submissions" pageDescription="Forms inbox">
      <Suspense fallback={<p>Loading submissions…</p>}>
        <FormsPage
          token={session?.user?.rustokToken ?? null}
          tenantSlug={session?.user?.tenantSlug ?? null}
          tenantId={session?.user?.tenantId ?? null}
        />
      </Suspense>
    </PageContainer>
  );
}
