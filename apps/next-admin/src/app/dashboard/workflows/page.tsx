import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { WorkflowsPage } from '@/features/workflow';
import { Suspense } from 'react';

export const metadata = {
  title: 'Dashboard: Workflows'
};

export default async function Page() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;

  return (
    <PageContainer scrollable pageTitle="Workflows" pageDescription="Manage automated workflows">
      <Suspense fallback={<div>Loading workflows...</div>}>
        <WorkflowsPage token={token} tenantSlug={tenantSlug} tenantId={tenantId} />
      </Suspense>
    </PageContainer>
  );
}
