import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { EventsPage } from '@/features/events';
import { Suspense } from 'react';
import { getTranslations } from 'next-intl/server';

export async function generateMetadata() {
  const t = await getTranslations('events');
  return { title: `Dashboard: ${t('title')}` };
}

export default async function Page() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const t = await getTranslations('events');

  return (
    <PageContainer
      scrollable
      pageTitle={t('title')}
      pageDescription={t('subtitle')}
    >
      <Suspense fallback={<div className='h-64 animate-pulse rounded-xl bg-muted' />}>
        <EventsPage token={token} tenantSlug={tenantSlug} />
      </Suspense>
    </PageContainer>
  );
}
