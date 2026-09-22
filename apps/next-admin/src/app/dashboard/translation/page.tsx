import { Suspense } from 'react';

import { auth } from '@/auth';
import { TranslationAdminClient } from '@/modules/translation-admin-client';
import { PageContainer } from '@/widgets/app-shell';

export const metadata = {
  title: 'Dashboard: Translation'
};

export default async function TranslationPage() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;

  return (
    <PageContainer
      scrollable
      pageTitle='Translation'
      pageDescription='Manage exact-locale coverage, policy, inventory, and reviewed translation workflow'
    >
      <Suspense fallback={<div>Loading Translation control plane...</div>}>
        <TranslationAdminClient token={token} tenantSlug={tenantSlug} />
      </Suspense>
    </PageContainer>
  );
}
