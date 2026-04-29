import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { EmailSettingsPage } from '../../../../packages/email/src';
import { Suspense } from 'react';

export const metadata = {
  title: 'Dashboard: Email Settings'
};

export default async function Page() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;

  return (
    <PageContainer
      scrollable
      pageTitle='Email Settings'
      pageDescription='Configure email delivery and SMTP settings'
    >
      <Suspense
        fallback={<div className='bg-muted h-64 animate-pulse rounded-xl' />}
      >
        <EmailSettingsPage token={token} tenantSlug={tenantSlug} />
      </Suspense>
    </PageContainer>
  );
}
