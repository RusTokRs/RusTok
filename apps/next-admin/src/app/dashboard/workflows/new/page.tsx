import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { WorkflowFormPage } from '../../../../../packages/workflow/src';
import { Suspense } from 'react';

export const metadata = {
  title: 'Dashboard: New Workflow'
};

export default async function Page() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;

  return (
    <PageContainer scrollable pageTitle='New Workflow'>
      <Suspense fallback={<div>Loading...</div>}>
        <WorkflowFormPage opts={{ token, tenantSlug, tenantId }} />
      </Suspense>
    </PageContainer>
  );
}
