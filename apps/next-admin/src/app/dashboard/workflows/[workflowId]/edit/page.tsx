import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { WorkflowFormPage, getWorkflow } from '@rustok/workflow-admin';
import { Suspense } from 'react';

export const metadata = {
  title: 'Dashboard: Edit Workflow'
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

  const opts = { token, tenantSlug, tenantId };
  const workflow = await getWorkflow(workflowId, opts);

  return (
    <PageContainer scrollable pageTitle="Edit Workflow">
      <Suspense fallback={<div>Loading...</div>}>
        <WorkflowFormPage workflow={workflow ?? undefined} opts={opts} />
      </Suspense>
    </PageContainer>
  );
}
