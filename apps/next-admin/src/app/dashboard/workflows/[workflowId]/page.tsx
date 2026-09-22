import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { WorkflowDetailPage } from '../../../../../packages/workflow/src';
import { Suspense } from 'react';

export const metadata = {
  title: 'Dashboard: Workflow Detail'
};

interface PageProps {
  params: Promise<{ workflowId: string }>;
}

export default async function Page({ params }: PageProps) {
  const { workflowId } = await params;
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;

  return (
    <PageContainer scrollable pageTitle='Workflow'>
      <Suspense fallback={<div>Loading workflow...</div>}>
        <WorkflowDetailPage
          id={workflowId}
          token={token}
          tenantSlug={tenantSlug}
          tenantId={tenantId}
        />
      </Suspense>
    </PageContainer>
  );
}
