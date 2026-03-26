import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { DataTableSkeleton } from '@/widgets/data-table';
import { RolesPage } from '@/features/rbac';
import { Suspense } from 'react';

export const metadata = {
  title: 'Dashboard: Roles & Permissions'
};

export default async function Page() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;

  return (
    <PageContainer
      scrollable
      pageTitle='Roles & Permissions'
      pageDescription='Platform roles and their permission sets'
    >
      <Suspense fallback={<DataTableSkeleton columnCount={3} rowCount={4} />}>
        <RolesPage token={token} tenantSlug={tenantSlug} />
      </Suspense>
    </PageContainer>
  );
}
