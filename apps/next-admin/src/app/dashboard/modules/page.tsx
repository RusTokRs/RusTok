import { Suspense } from 'react';
import { getTranslations } from 'next-intl/server';

import { auth } from '@/auth';
import {
  getActiveRelease,
  getActiveBuild,
  getBuildHistory,
  listInstalledModules,
  listMarketplaceModules,
  listModules
} from '@/features/modules/api';
import { ModulesList } from '@/features/modules/components/modules-list';
import { PageContainer } from '@/widgets/app-shell';

export const metadata = {
  title: 'Dashboard: Modules'
};

async function ModulesContent() {
  const session = await auth();
  const token = session?.user?.rustokToken;
  const tenantSlug = session?.user?.tenantSlug;
  const opts = { token, tenantSlug };

  const [modulesData, marketplaceModules, installedModules, activeBuild, activeRelease, buildHistory] = await Promise.all([
    listModules(opts),
    listMarketplaceModules(
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      opts
    ),
    listInstalledModules(opts),
    getActiveBuild(opts),
    getActiveRelease(opts),
    getBuildHistory(10, 0, opts)
  ]);

  return (
    <ModulesList
      adminSurface='next-admin'
      modules={modulesData.modules}
      marketplaceModules={marketplaceModules}
      installedModules={installedModules}
      activeBuild={activeBuild}
      activeRelease={activeRelease}
      buildHistory={buildHistory}
    />
  );
}

export default async function Page() {
  const t = await getTranslations('modules');
  return (
    <PageContainer
      scrollable
      pageTitle={t('title')}
      pageDescription={t('subtitle')}
    >
      <Suspense fallback={<div>Loading modules...</div>}>
        <ModulesContent />
      </Suspense>
    </PageContainer>
  );
}
