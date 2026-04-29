import { Suspense } from 'react';

import { AiAdminPage } from '@rustok/ai-admin';

import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';

export const metadata = {
  title: 'Dashboard: AI Diagnostics'
};

export default async function Page() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;

  return (
    <PageContainer
      scrollable
      pageTitle='AI Diagnostics'
      pageDescription='Inspect router decisions, run health, execution targets and AI runtime diagnostics'
    >
      <Suspense fallback={<div>Loading AI diagnostics...</div>}>
        <AiAdminPage
          token={token}
          tenantSlug={tenantSlug}
          section='diagnostics'
        />
      </Suspense>
    </PageContainer>
  );
}
