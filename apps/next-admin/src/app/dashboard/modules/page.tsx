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
} from '@/shared/api/modules';
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

  const loadErrors: string[] = [];

  async function safeLoad<T>(
    label: string,
    loader: () => Promise<T>,
    fallback: T
  ): Promise<T> {
    try {
      return await loader();
    } catch (error) {
      loadErrors.push(
        `${label}: ${error instanceof Error ? error.message : 'request failed'}`
      );
      return fallback;
    }
  }

  const [
    modulesData,
    marketplaceModules,
    installedModules,
    activeBuild,
    activeRelease,
    buildHistory
  ] = await Promise.all([
    safeLoad('module registry', () => listModules(opts), { modules: [] }),
    safeLoad(
      'marketplace modules',
      () =>
        listMarketplaceModules(
          undefined,
          undefined,
          undefined,
          undefined,
          undefined,
          undefined,
          opts
        ),
      []
    ),
    safeLoad('installed modules', () => listInstalledModules(opts), []),
    safeLoad('active build', () => getActiveBuild(opts), null),
    safeLoad('active release', () => getActiveRelease(opts), null),
    safeLoad('build history', () => getBuildHistory(10, 0, opts), [])
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
      loadErrors={loadErrors}
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
