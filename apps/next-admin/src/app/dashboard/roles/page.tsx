import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { DataTableSkeleton } from '@/widgets/data-table';
import { RolesPage } from '@rustok/rbac-admin';
import { Suspense } from 'react';
import { redirect } from 'next/navigation';

export const metadata = {
  title: 'Dashboard: Roles & Permissions'
};

export default async function Page() {
  const session = await auth();
  // UX gate only (the server remains the authorization source of truth):
  // aligned with the nav item access (`admin` minimum in the role hierarchy),
  // so manager/customer sessions are not shown a page whose API calls the
  // server would deny for lack of settings/users read.
  const role = session?.user?.role?.toUpperCase();
  if (role !== 'ADMIN' && role !== 'SUPER_ADMIN') {
    redirect('/dashboard');
  }

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
