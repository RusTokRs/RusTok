'use client';

import {
  IconBolt,
  IconPackage,
  IconRefresh,
  IconShieldLock,
  IconShoppingBag
} from '@tabler/icons-react';
import { usePathname, useRouter, useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';
import { startTransition, useEffect, useState } from 'react';
import { toast } from 'sonner';

import { useEnabledModulesActions } from '@/shared/hooks/use-enabled-modules';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import { Input } from '@/shared/ui/shadcn/input';
import { Progress } from '@/shared/ui/shadcn/progress';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger
} from '@/shared/ui/shadcn/tabs';

import {
  getActiveBuild,
  getActiveRelease,
  getBuildHistory,
  getMarketplaceModule,
  installModule,
  listMarketplaceModules,
  type BuildJob,
  type InstalledModule,
  type MarketplaceModule,
  type ModuleInfo,
  type ReleaseInfo,
  rollbackBuild,
  toggleModule,
  uninstallModule,
  upgradeModule
} from '../api';
import { ModuleDetailPanel } from './module-detail-panel';
import { ModuleCard } from './module-card';
import { ModuleUpdateCard } from './module-update-card';

interface ModulesListProps {
  adminSurface: 'leptos-admin' | 'next-admin';
  modules: ModuleInfo[];
  marketplaceModules: MarketplaceModule[];
  installedModules: InstalledModule[];
  activeBuild: BuildJob | null;
  activeRelease: ReleaseInfo | null;
  buildHistory: BuildJob[];
}

interface CatalogFilters {
  search: string;
  category: string;
  source: string;
  trustLevel: string;
  onlyCompatible: boolean;
  installedOnly: boolean;
}

const DEFAULT_FILTERS: CatalogFilters = {
  search: '',
  category: 'all',
  source: 'all',
  trustLevel: 'all',
  onlyCompatible: false,
  installedOnly: false
};

function upsertBuildJob(builds: BuildJob[], nextBuild: BuildJob): BuildJob[] {
  return [nextBuild, ...builds.filter((build) => build.id !== nextBuild.id)].slice(0, 10);
}

function humanizeLabel(value: string): string {
  return value
    .toLowerCase()
    .split('_')
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ');
}

function formatBuildSummary(build: BuildJob): string {
  return `${humanizeLabel(build.status)} / ${humanizeLabel(build.stage)} / ${build.progress}%`;
}

function formatTimestamp(value?: string | null): string {
  if (!value) {
    return 'Not available';
  }

  const timestamp = new Date(value);
  if (Number.isNaN(timestamp.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short'
  }).format(timestamp);
}

function catalogEntryToModuleInfo(module: MarketplaceModule): ModuleInfo {
  return {
    moduleSlug: module.slug,
    name: module.name,
    description: module.description,
    version: module.latestVersion,
    kind: module.kind,
    dependencies: module.dependencies,
    enabled: false,
    ownership: module.ownership,
    trustLevel: module.trustLevel,
    recommendedAdminSurfaces: module.recommendedAdminSurfaces,
    showcaseAdminSurfaces: module.showcaseAdminSurfaces
  };
}

function isBuildActive(build: BuildJob | null): boolean {
  if (!build) {
    return false;
  }

  return build.status === 'QUEUED' || build.status === 'RUNNING';
}

function normalizeCatalogFilters(filters: CatalogFilters): {
  search?: string;
  category?: string;
  source?: string;
  trustLevel?: string;
  onlyCompatible?: boolean;
  installedOnly?: boolean;
} {
  return {
    search: filters.search.trim() || undefined,
    category: filters.category !== 'all' ? filters.category : undefined,
    source: filters.source !== 'all' ? filters.source : undefined,
    trustLevel: filters.trustLevel !== 'all' ? filters.trustLevel : undefined,
    onlyCompatible: filters.onlyCompatible || undefined,
    installedOnly: filters.installedOnly || undefined
  };
}

export function ModulesList({
  adminSurface,
  modules: initialModules,
  marketplaceModules: initialMarketplaceModules,
  installedModules: initialInstalledModules,
  activeBuild: initialActiveBuild,
  activeRelease: initialActiveRelease,
  buildHistory: initialBuildHistory
}: ModulesListProps) {
  const [modules, setModules] = useState(initialModules);
  const [marketplaceCatalog, setMarketplaceCatalog] = useState(
    initialMarketplaceModules
  );
  const [installedModules, setInstalledModules] = useState(initialInstalledModules);
  const [activeBuild, setActiveBuild] = useState(initialActiveBuild);
  const [activeRelease, setActiveRelease] = useState(initialActiveRelease);
  const [buildHistory, setBuildHistory] = useState(initialBuildHistory);
  const [selectedModuleSlug, setSelectedModuleSlug] = useState<string | null>(null);
  const [selectedModuleDetail, setSelectedModuleDetail] =
    useState<MarketplaceModule | null>(null);
  const [moduleDetailLoading, setModuleDetailLoading] = useState(false);
  const [catalogFilterDraft, setCatalogFilterDraft] = useState(DEFAULT_FILTERS);
  const [appliedCatalogFilters, setAppliedCatalogFilters] = useState(DEFAULT_FILTERS);
  const [catalogRefreshing, setCatalogRefreshing] = useState(false);
  const [knownCategories, setKnownCategories] = useState(
    Array.from(
      new Set(
        initialMarketplaceModules
          .map((module) => module.category)
          .filter((category) => category.length > 0)
      )
    ).sort((left, right) => left.localeCompare(right))
  );
  const [knownSources, setKnownSources] = useState(
    Array.from(
      new Set(initialMarketplaceModules.map((module) => module.source).filter(Boolean))
    ).sort((left, right) => left.localeCompare(right))
  );
  const [loading, setLoading] = useState<string | null>(null);
  const [platformLoading, setPlatformLoading] = useState<string | null>(null);
  const [rollbackLoading, setRollbackLoading] = useState<string | null>(null);
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();
  const t = useTranslations('modules');
  const { setModuleEnabled } = useEnabledModulesActions();

  const coreModules = modules.filter((module) => module.kind === 'core');
  const optionalModules = modules.filter((module) => module.kind === 'optional');
  const installedSet = new Set(installedModules.map((module) => module.slug));
  const installedMap = new Map(installedModules.map((module) => [module.slug, module]));
  const catalogMap = new Map(marketplaceCatalog.map((module) => [module.slug, module]));
  const installedOptionalModules = optionalModules.filter((module) =>
    installedSet.has(module.moduleSlug)
  );
  const marketplaceModules = marketplaceCatalog.filter((module) => !installedSet.has(module.slug));
  const updateCandidates = marketplaceCatalog
    .map((module) => ({
      module,
      installedModule: installedMap.get(module.slug)
    }))
    .filter(
      (entry): entry is { module: MarketplaceModule; installedModule: InstalledModule } =>
        Boolean(
          entry.installedModule?.version &&
            entry.installedModule.version !== entry.module.latestVersion
        )
    );
  const visibleInstalledCount = coreModules.length + installedOptionalModules.length;
  const latestBuild = activeBuild ?? buildHistory[0] ?? null;
  const isShowcaseSurface = adminSurface === 'next-admin';

  useEffect(() => {
    const moduleFromUrl = searchParams.get('module');
    if (moduleFromUrl !== selectedModuleSlug) {
      setSelectedModuleSlug(moduleFromUrl);
      if (!moduleFromUrl) {
        setSelectedModuleDetail(null);
        setModuleDetailLoading(false);
      }
    }
  }, [searchParams, selectedModuleSlug]);

  useEffect(() => {
    if (!selectedModuleSlug) {
      return;
    }

    const catalogModule =
      marketplaceCatalog.find((module) => module.slug === selectedModuleSlug) ?? null;
    if (catalogModule) {
      setSelectedModuleDetail(catalogModule);
    }
  }, [marketplaceCatalog, selectedModuleSlug]);

  useEffect(() => {
    if (!selectedModuleSlug) {
      return;
    }

    let cancelled = false;

    const loadModuleDetail = async () => {
      setModuleDetailLoading(true);
      try {
        const detail = await getMarketplaceModule(selectedModuleSlug);
        if (!cancelled) {
          setSelectedModuleDetail(detail);
        }
      } catch (err) {
        if (!cancelled) {
          toast.error(
            err instanceof Error ? err.message : 'Failed to load module detail'
          );
        }
      } finally {
        if (!cancelled) {
          setModuleDetailLoading(false);
        }
      }
    };

    void loadModuleDetail();

    return () => {
      cancelled = true;
    };
  }, [selectedModuleSlug]);

  const refreshMarketplaceCatalog = async (
    nextFilters: CatalogFilters,
    silent = false
  ) => {
    if (!silent) {
      setCatalogRefreshing(true);
    }

    try {
      const normalized = normalizeCatalogFilters(nextFilters);
      const modules = await listMarketplaceModules(
        normalized.search,
        normalized.category,
        normalized.source,
        normalized.trustLevel,
        normalized.onlyCompatible,
        normalized.installedOnly
      );
      setMarketplaceCatalog(modules);
      setKnownCategories((prev) =>
        Array.from(
          new Set([
            ...prev,
            ...modules
              .map((module) => module.category)
              .filter((category) => category.length > 0)
          ])
        ).sort((left, right) => left.localeCompare(right))
      );
      setKnownSources((prev) =>
        Array.from(
          new Set([...prev, ...modules.map((module) => module.source).filter(Boolean)])
        ).sort((left, right) => left.localeCompare(right))
      );
    } finally {
      if (!silent) {
        setCatalogRefreshing(false);
      }
    }
  };

  const refreshOrchestrationState = async (filters = appliedCatalogFilters) => {
    try {
      const normalized = normalizeCatalogFilters(filters);
      const [nextActiveBuild, nextActiveRelease, nextBuildHistory, nextMarketplaceCatalog] =
        await Promise.all([
          getActiveBuild(),
          getActiveRelease(),
          getBuildHistory(10, 0),
          listMarketplaceModules(
            normalized.search,
            normalized.category,
            normalized.source,
            normalized.trustLevel,
            normalized.onlyCompatible,
            normalized.installedOnly
          )
        ]);
      setActiveBuild(nextActiveBuild);
      setActiveRelease(nextActiveRelease);
      setBuildHistory(nextBuildHistory);
      setMarketplaceCatalog(nextMarketplaceCatalog);
      setKnownCategories((prev) =>
        Array.from(
          new Set([
            ...prev,
            ...nextMarketplaceCatalog
              .map((module) => module.category)
              .filter((category) => category.length > 0)
          ])
        ).sort((left, right) => left.localeCompare(right))
      );
      setKnownSources((prev) =>
        Array.from(
          new Set([
            ...prev,
            ...nextMarketplaceCatalog.map((module) => module.source).filter(Boolean)
          ])
        ).sort((left, right) => left.localeCompare(right))
      );
    } catch {
      // Keep optimistic orchestration state if refresh fails.
    }
  };

  useEffect(() => {
    if (!isBuildActive(activeBuild)) {
      return undefined;
    }

    let cancelled = false;

    const refreshLiveState = async () => {
      try {
        const normalized = normalizeCatalogFilters(appliedCatalogFilters);
        const [nextActiveBuild, nextActiveRelease, nextBuildHistory, nextMarketplaceCatalog] =
          await Promise.all([
            getActiveBuild(),
            getActiveRelease(),
            getBuildHistory(10, 0),
            listMarketplaceModules(
              normalized.search,
              normalized.category,
              normalized.source,
              normalized.trustLevel,
              normalized.onlyCompatible,
              normalized.installedOnly
            )
          ]);
        if (cancelled) {
          return;
        }

        startTransition(() => {
          setActiveBuild(nextActiveBuild);
          setActiveRelease(nextActiveRelease);
          setBuildHistory(nextBuildHistory);
          setMarketplaceCatalog(nextMarketplaceCatalog);
        });
      } catch {
        // Keep the current snapshot until the next polling cycle succeeds.
      }
    };

    void refreshLiveState();
    const intervalId = window.setInterval(() => {
      void refreshLiveState();
    }, 5000);

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, [activeBuild?.id, activeBuild?.status, appliedCatalogFilters]);

  const upsertInstalledModule = (slug: string, version: string) => {
    const registryModule = modules.find((module) => module.moduleSlug === slug);
    const catalogModule = marketplaceCatalog.find((module) => module.slug === slug);

    setInstalledModules((prev) => {
      if (prev.some((module) => module.slug === slug)) {
        return prev.map((module) =>
          module.slug === slug ? { ...module, version } : module
        );
      }

      return [
        ...prev,
        {
          slug,
          source: catalogModule?.source ?? 'path',
          crateName: catalogModule?.crateName ?? registryModule?.moduleSlug ?? slug,
          version,
          required: false,
          dependencies: catalogModule?.dependencies ?? registryModule?.dependencies ?? []
        }
      ].sort((left, right) => left.slug.localeCompare(right.slug));
    });
  };

  const handleToggle = async (slug: string, enabled: boolean) => {
    setLoading(slug);
    try {
      const updated = await toggleModule(slug, enabled);
      setModules((prev) =>
        prev.map((module) =>
          module.moduleSlug === slug ? { ...module, enabled: updated.enabled } : module
        )
      );
      setModuleEnabled(slug, updated.enabled);
      toast.success(updated.enabled ? t('toast.enabled') : t('toast.disabled'));
      router.refresh();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t('error.load'));
    } finally {
      setLoading(null);
    }
  };

  const handleInstall = async (slug: string, version: string) => {
    setPlatformLoading(slug);
    try {
      const build = await installModule(slug, version);
      upsertInstalledModule(slug, version);
      setActiveBuild(build);
      setBuildHistory((prev) => upsertBuildJob(prev, build));
      toast.success(`Install queued for ${slug}`);
      await refreshOrchestrationState();
      router.refresh();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to queue install');
    } finally {
      setPlatformLoading(null);
    }
  };

  const handleUninstall = async (slug: string) => {
    setPlatformLoading(slug);
    try {
      const build = await uninstallModule(slug);
      setInstalledModules((prev) => prev.filter((item) => item.slug !== slug));
      setModules((prev) =>
        prev.map((module) =>
          module.moduleSlug === slug ? { ...module, enabled: false } : module
        )
      );
      setModuleEnabled(slug, false);
      setActiveBuild(build);
      setBuildHistory((prev) => upsertBuildJob(prev, build));
      toast.success(`Uninstall queued for ${slug}`);
      await refreshOrchestrationState();
      router.refresh();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to queue uninstall');
    } finally {
      setPlatformLoading(null);
    }
  };

  const handleUpgrade = async (slug: string, version: string) => {
    setPlatformLoading(slug);
    try {
      const build = await upgradeModule(slug, version);
      upsertInstalledModule(slug, version);
      setActiveBuild(build);
      setBuildHistory((prev) => upsertBuildJob(prev, build));
      toast.success(`Upgrade queued for ${slug}`);
      await refreshOrchestrationState();
      router.refresh();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to queue upgrade');
    } finally {
      setPlatformLoading(null);
    }
  };

  const handleRollback = async (buildId: string) => {
    setRollbackLoading(buildId);
    try {
      const restoredBuild = await rollbackBuild(buildId);
      setActiveBuild(null);
      setBuildHistory((prev) => upsertBuildJob(prev, restoredBuild));
      toast.success(`Rollback completed for ${buildId}`);
      await refreshOrchestrationState();
      router.refresh();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to rollback build');
    } finally {
      setRollbackLoading(null);
    }
  };

  const handleInspect = async (slug: string) => {
    setSelectedModuleDetail(catalogMap.get(slug) ?? null);
    const nextParams = new URLSearchParams(searchParams.toString());
    nextParams.set('module', slug);
    router.replace(`${pathname}?${nextParams.toString()}`, { scroll: false });
  };

  const handleCloseDetail = () => {
    const nextParams = new URLSearchParams(searchParams.toString());
    nextParams.delete('module');
    const query = nextParams.toString();
    router.replace(query ? `${pathname}?${query}` : pathname, { scroll: false });
  };

  const handleApplyFilters = async () => {
    try {
      setAppliedCatalogFilters(catalogFilterDraft);
      await refreshMarketplaceCatalog(catalogFilterDraft);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to refresh catalog');
    }
  };

  const handleResetFilters = async () => {
    setCatalogFilterDraft(DEFAULT_FILTERS);
    setAppliedCatalogFilters(DEFAULT_FILTERS);
    try {
      await refreshMarketplaceCatalog(DEFAULT_FILTERS);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to reset catalog');
    }
  };

  return (
    <div className='space-y-8'>
      <Card>
        <CardHeader className='pb-3'>
          <CardTitle className='text-sm font-medium'>Admin surface policy</CardTitle>
          <CardDescription>
            {isShowcaseSurface
              ? 'Next admin is a showcase surface. Dedicated module UI appears only where a module is explicitly marked with Next showcase support.'
              : 'Leptos admin is the canonical operator surface for module UI and ongoing module parity.'}
          </CardDescription>
        </CardHeader>
        <CardContent className='flex flex-wrap items-center gap-2 pt-0'>
          <Badge variant='secondary'>
            {isShowcaseSurface ? 'Current: Next showcase' : 'Current: Leptos canonical'}
          </Badge>
          <Badge variant='outline'>Primary modules target Leptos first</Badge>
        </CardContent>
      </Card>

      <div className='grid gap-4 xl:grid-cols-[minmax(0,1.4fr)_repeat(3,minmax(0,0.8fr))]'>
        <Card className='xl:col-span-2'>
          <CardHeader>
            <CardTitle className='flex items-center gap-2 text-base'>
              <IconBolt className='h-5 w-5' />
              Build orchestration
            </CardTitle>
            <CardDescription>
              Install, uninstall, and upgrade actions queue a shared rebuild for both admin stacks.
            </CardDescription>
          </CardHeader>
          <CardContent className='space-y-4'>
            {latestBuild ? (
              <>
                <div className='flex items-center justify-between gap-3'>
                  <div className='space-y-1'>
                    <div className='flex items-center gap-2'>
                      <Badge variant='outline'>{humanizeLabel(latestBuild.status)}</Badge>
                      <span className='text-muted-foreground text-xs'>
                        {humanizeLabel(latestBuild.stage)}
                      </span>
                    </div>
                    <p className='text-sm font-medium'>
                      {latestBuild.modulesDelta || latestBuild.reason || 'Platform module rebuild'}
                    </p>
                    <p className='text-muted-foreground text-xs'>
                      Updated {formatTimestamp(latestBuild.updatedAt)}
                    </p>
                    {activeRelease && (
                      <p className='text-muted-foreground text-xs'>
                        Active release {activeRelease.id} in {activeRelease.environment}
                      </p>
                    )}
                  </div>
                  <span className='text-sm font-semibold'>{latestBuild.progress}%</span>
                </div>
                <Progress value={latestBuild.progress} />
                <p className='text-muted-foreground text-xs'>
                  {activeBuild
                    ? 'Platform actions stay locked until the current build finishes.'
                    : 'No active build. The latest completed job is shown for context.'}
                </p>
                {isBuildActive(activeBuild) && (
                  <p className='text-muted-foreground text-xs'>
                    Live refresh is active every 5 seconds while this build is running.
                  </p>
                )}
              </>
            ) : (
              <p className='text-muted-foreground text-sm'>
                No platform builds yet. The first install, uninstall, or upgrade will queue one.
              </p>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader className='pb-3'>
            <CardTitle className='text-sm font-medium'>Installed</CardTitle>
          </CardHeader>
          <CardContent>
            <p className='text-3xl font-semibold'>{visibleInstalledCount}</p>
            <p className='text-muted-foreground mt-2 text-xs'>
              Core and optional modules visible to this admin workspace.
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className='pb-3'>
            <CardTitle className='text-sm font-medium'>Marketplace</CardTitle>
          </CardHeader>
          <CardContent>
            <p className='text-3xl font-semibold'>{marketplaceModules.length}</p>
            <p className='text-muted-foreground mt-2 text-xs'>
              Optional modules available to add into modules.toml.
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className='pb-3'>
            <CardTitle className='text-sm font-medium'>Updates</CardTitle>
          </CardHeader>
          <CardContent>
            <p className='text-3xl font-semibold'>{updateCandidates.length}</p>
            <p className='text-muted-foreground mt-2 text-xs'>
              Version-pinned modules that can be upgraded from this screen.
            </p>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader className='pb-3'>
          <CardTitle className='text-base'>Catalog filters</CardTitle>
          <CardDescription>
            Narrow Marketplace and Updates to the modules you actually want to review.
          </CardDescription>
        </CardHeader>
        <CardContent className='space-y-4'>
          <div className='grid gap-3 lg:grid-cols-[minmax(0,2fr)_repeat(3,minmax(0,1fr))_auto_auto]'>
            <Input
              value={catalogFilterDraft.search}
              onChange={(event) =>
                setCatalogFilterDraft((prev) => ({ ...prev, search: event.target.value }))
              }
              placeholder='Search by name, slug, or description'
            />
            <Select
              value={catalogFilterDraft.category}
              onValueChange={(value) =>
                setCatalogFilterDraft((prev) => ({ ...prev, category: value }))
              }
            >
              <SelectTrigger className='w-full'>
                <SelectValue placeholder='Category' />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value='all'>All categories</SelectItem>
                {knownCategories.map((category) => (
                  <SelectItem key={category} value={category}>
                    {humanizeLabel(category)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Select
              value={catalogFilterDraft.source}
              onValueChange={(value) =>
                setCatalogFilterDraft((prev) => ({ ...prev, source: value }))
              }
            >
              <SelectTrigger className='w-full'>
                <SelectValue placeholder='Source' />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value='all'>All sources</SelectItem>
                {knownSources.map((source) => (
                  <SelectItem key={source} value={source}>
                    {humanizeLabel(source)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Select
              value={catalogFilterDraft.trustLevel}
              onValueChange={(value) =>
                setCatalogFilterDraft((prev) => ({ ...prev, trustLevel: value }))
              }
            >
              <SelectTrigger className='w-full'>
                <SelectValue placeholder='Trust' />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value='all'>All trust levels</SelectItem>
                <SelectItem value='core'>Core</SelectItem>
                <SelectItem value='verified'>Verified</SelectItem>
                <SelectItem value='unverified'>Unverified</SelectItem>
                <SelectItem value='private'>Private</SelectItem>
              </SelectContent>
            </Select>
            <Button
              variant={catalogFilterDraft.onlyCompatible ? 'default' : 'outline'}
              type='button'
              onClick={() =>
                setCatalogFilterDraft((prev) => ({
                  ...prev,
                  onlyCompatible: !prev.onlyCompatible
                }))
              }
            >
              {catalogFilterDraft.onlyCompatible ? 'Compatible only' : 'Include risks'}
            </Button>
            <Button
              variant={catalogFilterDraft.installedOnly ? 'default' : 'outline'}
              type='button'
              onClick={() =>
                setCatalogFilterDraft((prev) => ({
                  ...prev,
                  installedOnly: !prev.installedOnly
                }))
              }
            >
              {catalogFilterDraft.installedOnly ? 'Installed only' : 'All install states'}
            </Button>
          </div>
          <div className='flex flex-wrap items-center justify-between gap-3'>
            <div className='flex flex-wrap items-center gap-2 text-xs'>
              <Badge variant='secondary'>
                {catalogRefreshing ? 'Refreshing catalog' : 'Catalog ready'}
              </Badge>
              {JSON.stringify(catalogFilterDraft) !== JSON.stringify(appliedCatalogFilters) && (
                <Badge variant='outline'>Pending changes</Badge>
              )}
              {appliedCatalogFilters.search && (
                <Badge variant='outline'>Search: {appliedCatalogFilters.search}</Badge>
              )}
              {appliedCatalogFilters.category !== 'all' && (
                <Badge variant='outline'>
                  Category: {humanizeLabel(appliedCatalogFilters.category)}
                </Badge>
              )}
              {appliedCatalogFilters.source !== 'all' && (
                <Badge variant='outline'>
                  Source: {humanizeLabel(appliedCatalogFilters.source)}
                </Badge>
              )}
              {appliedCatalogFilters.trustLevel !== 'all' && (
                <Badge variant='outline'>
                  Trust: {humanizeLabel(appliedCatalogFilters.trustLevel)}
                </Badge>
              )}
              {appliedCatalogFilters.onlyCompatible && (
                <Badge variant='outline'>Compatible only</Badge>
              )}
              {appliedCatalogFilters.installedOnly && (
                <Badge variant='outline'>Installed only</Badge>
              )}
            </div>
            <div className='flex items-center gap-2'>
              <Button
                variant='outline'
                type='button'
                disabled={catalogRefreshing}
                onClick={() => void handleResetFilters()}
              >
                Reset
              </Button>
              <Button
                type='button'
                disabled={catalogRefreshing}
                onClick={() => void handleApplyFilters()}
              >
                {catalogRefreshing ? 'Refreshing...' : 'Apply filters'}
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      {selectedModuleSlug && (
        <ModuleDetailPanel
          adminSurface={adminSurface}
          slug={selectedModuleSlug}
          module={selectedModuleDetail}
          loading={moduleDetailLoading}
          onClose={handleCloseDetail}
        />
      )}

      <Tabs defaultValue='installed' className='space-y-6'>
        <TabsList>
          <TabsTrigger value='installed'>Installed</TabsTrigger>
          <TabsTrigger value='marketplace'>Marketplace</TabsTrigger>
          <TabsTrigger value='updates'>Updates</TabsTrigger>
        </TabsList>

        <TabsContent value='installed' className='space-y-6'>
          <Card>
            <CardHeader>
              <CardTitle className='text-base'>Build history</CardTitle>
              <CardDescription>
                Recent rebuild jobs visible to both admin applications.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <div className='space-y-3'>
                {buildHistory.length > 0 ? (
                  buildHistory.map((build) => (
                    <div
                      key={build.id}
                      className='flex items-center justify-between gap-3 rounded-lg border px-3 py-2'
                    >
                      <div className='space-y-1'>
                        <p className='text-sm font-medium'>
                          {build.modulesDelta || build.reason || build.id}
                        </p>
                        <p className='text-muted-foreground text-xs'>
                          {formatBuildSummary(build)}
                        </p>
                        {build.errorMessage && (
                          <p className='text-xs text-destructive'>{build.errorMessage}</p>
                        )}
                        <div className='flex flex-wrap items-center gap-2 text-xs'>
                          {build.releaseId && (
                            <Badge variant='secondary'>Release {build.releaseId}</Badge>
                          )}
                          {activeRelease?.id === build.releaseId && (
                            <Badge variant='outline'>Active release</Badge>
                          )}
                          {build.logsUrl && (
                            <a
                              className='text-primary underline-offset-4 hover:underline'
                              href={build.logsUrl}
                              target='_blank'
                              rel='noreferrer'
                            >
                              Open logs
                            </a>
                          )}
                        </div>
                      </div>
                      <div className='space-y-2 text-right'>
                        <Badge variant='outline'>{humanizeLabel(build.status)}</Badge>
                        <p className='text-muted-foreground mt-1 text-xs'>
                          {formatTimestamp(build.createdAt)}
                        </p>
                        {activeRelease?.id === build.releaseId &&
                          activeRelease?.previousReleaseId && (
                            <button
                              type='button'
                              className='text-primary text-xs font-medium underline-offset-4 hover:underline disabled:no-underline disabled:opacity-50'
                              disabled={rollbackLoading === build.id || Boolean(activeBuild)}
                              onClick={() => handleRollback(build.id)}
                            >
                              {rollbackLoading === build.id ? 'Rolling back...' : 'Rollback'}
                            </button>
                          )}
                      </div>
                    </div>
                  ))
                ) : (
                  <p className='text-muted-foreground text-sm'>No builds yet.</p>
                )}
              </div>
            </CardContent>
          </Card>

          <div className='space-y-3'>
            <div className='flex items-center gap-2'>
              <IconShieldLock className='text-muted-foreground h-5 w-5' />
              <h3 className='text-lg font-semibold'>{t('section.core')}</h3>
              <Badge variant='secondary' className='text-xs'>
                {t('always_active')}
              </Badge>
            </div>
            <div className='grid gap-4 md:grid-cols-2 xl:grid-cols-3'>
              {coreModules.map((module) => (
                <ModuleCard
                  key={module.moduleSlug}
                  module={module}
                  catalogModule={catalogMap.get(module.moduleSlug) ?? null}
                  loading={loading === module.moduleSlug}
                  platformLoading={platformLoading === module.moduleSlug}
                  platformInstalled
                  platformBusy={Boolean(activeBuild)}
                  platformVersion={installedMap.get(module.moduleSlug)?.version}
                  recommendedVersion={module.version}
                  onInspect={handleInspect}
                />
              ))}
            </div>
          </div>

          <div className='space-y-3'>
            <div className='flex items-center gap-2'>
              <IconPackage className='text-muted-foreground h-5 w-5' />
              <h3 className='text-lg font-semibold'>{t('section.optional')}</h3>
              <Badge variant='outline' className='text-xs'>
                {installedOptionalModules.length} installed
              </Badge>
            </div>
            {installedOptionalModules.length > 0 ? (
              <div className='grid gap-4 md:grid-cols-2 xl:grid-cols-3'>
                {installedOptionalModules.map((module) => (
                  <ModuleCard
                    key={module.moduleSlug}
                    module={module}
                    catalogModule={catalogMap.get(module.moduleSlug) ?? null}
                    loading={loading === module.moduleSlug}
                    platformLoading={platformLoading === module.moduleSlug}
                    platformInstalled
                    platformBusy={Boolean(activeBuild)}
                    platformVersion={installedMap.get(module.moduleSlug)?.version}
                    recommendedVersion={module.version}
                    onToggle={handleToggle}
                    onInspect={handleInspect}
                    onUninstall={handleUninstall}
                  />
                ))}
              </div>
            ) : (
              <Card>
                <CardContent className='pt-6'>
                  <p className='text-muted-foreground text-sm'>
                    No optional modules are installed yet. Use the Marketplace tab to queue the first install.
                  </p>
                </CardContent>
              </Card>
            )}
          </div>
        </TabsContent>

        <TabsContent value='marketplace' className='space-y-6'>
          <Card>
            <CardHeader>
              <CardTitle className='flex items-center gap-2 text-base'>
                <IconShoppingBag className='h-5 w-5' />
                Catalog workspace
              </CardTitle>
              <CardDescription>
                Modules here are known to the platform registry but not yet present in modules.toml.
              </CardDescription>
            </CardHeader>
          </Card>

          {marketplaceModules.length > 0 ? (
            <div className='grid gap-4 md:grid-cols-2 xl:grid-cols-3'>
              {marketplaceModules.map((module) => (
                <ModuleCard
                  key={module.slug}
                  module={catalogEntryToModuleInfo(module)}
                  catalogModule={module}
                  loading={loading === module.slug}
                  platformLoading={platformLoading === module.slug}
                  platformInstalled={false}
                  platformBusy={Boolean(activeBuild)}
                  recommendedVersion={module.latestVersion}
                  onInspect={handleInspect}
                  onInstall={handleInstall}
                />
              ))}
            </div>
          ) : (
            <Card>
              <CardContent className='pt-6'>
                <p className='text-muted-foreground text-sm'>
                  All optional registry modules are already installed in the platform manifest.
                </p>
              </CardContent>
            </Card>
          )}
        </TabsContent>

        <TabsContent value='updates' className='space-y-6'>
          <Card>
            <CardHeader>
              <CardTitle className='flex items-center gap-2 text-base'>
                <IconRefresh className='h-5 w-5' />
                Versioned updates
              </CardTitle>
              <CardDescription>
                Only modules with an explicit installed version and a newer registry version appear here.
              </CardDescription>
            </CardHeader>
          </Card>

          {updateCandidates.length > 0 ? (
            <div className='grid gap-4 md:grid-cols-2 xl:grid-cols-3'>
              {updateCandidates.map(({ module, installedModule }) => (
                <ModuleUpdateCard
                  key={module.slug}
                  module={module}
                  installedModule={installedModule}
                  loading={platformLoading === module.slug}
                  platformBusy={Boolean(activeBuild)}
                  onInspect={handleInspect}
                  onUpgrade={handleUpgrade}
                />
              ))}
            </div>
          ) : (
            <Card>
              <CardContent className='space-y-2 pt-6'>
                <p className='text-sm font-medium'>No pinned module updates detected.</p>
                <p className='text-muted-foreground text-sm'>
                  Path-based local modules follow the current repository state and therefore do not show a separate version upgrade action.
                </p>
              </CardContent>
            </Card>
          )}
        </TabsContent>
      </Tabs>
    </div>
  );
}
