import { Suspense } from 'react';

import { auth } from '@/auth';
import { AiAdminClient } from '@/modules/ai-admin-client';
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
        <AiAdminClient
          token={token}
          tenantSlug={tenantSlug}
          section='diagnostics'
        />
      </Suspense>
    </PageContainer>
  );
}
