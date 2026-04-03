import { Suspense } from 'react';

import { AiAdminPage } from '@rustok/ai-admin';

import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';

export const metadata = {
  title: 'Dashboard: AI'
};

export default async function Page() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;

  return (
    <PageContainer
      scrollable
      pageTitle='AI'
      pageDescription='Manage providers, tool policies, operator chat sessions and approval gates'
    >
      <Suspense fallback={<div>Loading AI control plane...</div>}>
        <AiAdminPage token={token} tenantSlug={tenantSlug} />
      </Suspense>
    </PageContainer>
  );
}
