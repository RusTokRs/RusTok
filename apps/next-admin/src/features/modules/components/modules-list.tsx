'use client';

import {
  IconBolt,
  IconLayoutGrid,
  IconPackage,
  IconRefresh,
  IconShieldLock,
  IconShoppingBag,
  IconTable
} from '@tabler/icons-react';
import { usePathname, useRouter, useSearchParams } from 'next/navigation';
import { useTranslations } from '@rustok/next-fluent';
import { useSession } from 'next-auth/react';
import {
  startTransition,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState
} from 'react';
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
import { Switch } from '@/shared/ui/shadcn/switch';
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger
} from '@/shared/ui/shadcn/tabs';
import {
  DataTableEmptyState,
  DataTableStatic
} from '@/widgets/data-table/data-table-static';
import type { DataTableStaticColumn } from '@/widgets/data-table/data-table-static';

import {
  getActiveBuild,
  getBuildHistory,
  getMarketplaceModule,
  getTransitionCheckpoint,
  installModule,
  listActiveModuleTransitions,
  listInstalledModules,
  listMarketplaceModules,
  listMarketplaceRegistryFreshness,
  listRetentionHolds,
  listTenantModules,
  type BuildJob,
  type GqlOpts,
  type InstalledModule,
  type MarketplaceModule,
  type MarketplaceRegistryFreshness,
  type ModuleInfo,
  type ModuleTransitionCheckpoint,
  type RetentionHold,
  toggleModule,
  uninstallModule,
  upgradeModule
} from '@/shared/api/modules';
import { ModuleDetailPanel } from './module-detail-panel';
import { ModuleCard } from './module-card';
import { ModuleUpdateCard } from './module-update-card';
import { TransitionControlCard } from './transition-control-card';
import { RecoveryWorkspace } from './recovery-workspace';
import { ModuleSettingsDialog } from './module-settings-dialog';
import { ForumModuleSettingsFields } from './module-settings-fields/forum';
import { BlogModuleSettingsFields } from './module-settings-fields/blog';

interface ModulesListProps {
  adminSurface: 'leptos-admin' | 'next-admin';
  modules: ModuleInfo[];
  marketplaceModules: MarketplaceModule[];
  marketplaceRegistryFreshness: MarketplaceRegistryFreshness[];
  installedModules: InstalledModule[];
  activeBuild: BuildJob | null;
  buildHistory: BuildJob[];
  loadErrors?: string[];
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
  return [
    nextBuild,
    ...builds.filter((build) => build.id !== nextBuild.id)
  ].slice(0, 10);
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

function formatRegistryLastSuccess(unixMs?: number | null): string {
  if (unixMs === null || unixMs === undefined) {
    return 'Never';
  }
  const value = new Date(unixMs);
  return Number.isNaN(value.getTime()) ? 'Never' : value.toISOString();
}

function registryStatusVariant(
  status: MarketplaceRegistryFreshness['status']
): 'default' | 'secondary' | 'destructive' {
  if (status === 'READY') return 'default';
  if (status === 'DEGRADED') return 'destructive';
  return 'secondary';
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
    lifecycleRevision: 0,
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
  marketplaceRegistryFreshness: initialMarketplaceRegistryFreshness,
  installedModules: initialInstalledModules,
  activeBuild: initialActiveBuild,
  buildHistory: initialBuildHistory,
  loadErrors = []
}: ModulesListProps) {
  const [modules, setModules] = useState(initialModules);
  const [marketplaceCatalog, setMarketplaceCatalog] = useState(
    initialMarketplaceModules
  );
  const [registryFreshness, setRegistryFreshness] = useState(
    initialMarketplaceRegistryFreshness
  );
  const [installedModules, setInstalledModules] = useState(
    initialInstalledModules
  );
  const [activeBuild, setActiveBuild] = useState(initialActiveBuild);
  const [buildHistory, setBuildHistory] = useState(initialBuildHistory);
  const [selectedModuleSlug, setSelectedModuleSlug] = useState<string | null>(
    null
  );
  const [selectedModuleDetail, setSelectedModuleDetail] =
    useState<MarketplaceModule | null>(null);
  const [moduleDetailLoading, setModuleDetailLoading] = useState(false);
  const [activeTransitions, setActiveTransitions] = useState<
    ModuleTransitionCheckpoint[]
  >([]);
  const [transitionError, setTransitionError] = useState<string | null>(null);
  const [transitionRefresh, setTransitionRefresh] = useState(0);
  const [transitionScope, setTransitionScope] = useState<GqlOpts | null>(null);
  const [retentionHolds, setRetentionHolds] = useState<RetentionHold[]>([]);
  const [retentionScope, setRetentionScope] = useState<GqlOpts | null>(null);
  const [retentionError, setRetentionError] = useState<string | null>(null);
  const settingsRequest = useRef(0);
  const orchestrationRequest = useRef(0);
  const [settingsDialog, setSettingsDialog] = useState<{
    open: boolean;
    slug: string;
    settings: string;
    revision: number;
    scope: GqlOpts | null;
  }>({ open: false, slug: '', settings: '{}', revision: 0, scope: null });
  const [catalogFilterDraft, setCatalogFilterDraft] = useState(DEFAULT_FILTERS);
  const [appliedCatalogFilters, setAppliedCatalogFilters] =
    useState(DEFAULT_FILTERS);
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
      new Set(
        initialMarketplaceModules.map((module) => module.source).filter(Boolean)
      )
    ).sort((left, right) => left.localeCompare(right))
  );
  const [loading, setLoading] = useState<string | null>(null);
  const [platformLoading, setPlatformLoading] = useState<string | null>(null);
  const [installedViewMode, setInstalledViewMode] = useState<'grid' | 'table'>(
    'grid'
  );
  const [moduleSort, setModuleSort] = useState<{
    columnId: string;
    direction: 'asc' | 'desc';
  } | null>({ columnId: 'module', direction: 'asc' });
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();
  const t = useTranslations('modules');
  const { data: session } = useSession();
  const apiOpts = useMemo(
    () => ({
      token: session?.user?.rustokToken ?? null,
      tenantSlug: session?.user?.tenantSlug ?? null,
      tenantId: session?.user?.tenantId ?? null
    }),
    [session?.user?.rustokToken, session?.user?.tenantSlug, session?.user?.tenantId]
  );
  useEffect(() => {
    return () => {
      settingsRequest.current += 1;
      orchestrationRequest.current += 1;
    };
  }, [apiOpts]);
  const openSettings = useCallback(
    async (slug: string) => {
      const request = ++settingsRequest.current;
      try {
        const tenantModules = await listTenantModules(apiOpts);
        if (request !== settingsRequest.current) return;
        const current = tenantModules.find(
          (module) => module.moduleSlug === slug
        );
        if (!current) throw new Error(`Settings for ${slug} are unavailable`);
        setSettingsDialog({
          open: true,
          slug,
          settings: current.settings,
          revision: current.revision,
          scope: apiOpts
        });
      } catch (error) {
        if (request === settingsRequest.current) {
          toast.error(
            error instanceof Error
              ? error.message
              : 'Failed to load module settings'
          );
        }
      }
    },
    [apiOpts]
  );
  const { setModuleEnabled } = useEnabledModulesActions();

  const coreModules = modules.filter((module) => module.kind === 'core');
  const optionalModules = modules.filter(
    (module) => module.kind === 'optional'
  );
  const installedSet = new Set(installedModules.map((module) => module.slug));
  const installedMap = new Map(
    installedModules.map((module) => [module.slug, module])
  );
  const catalogMap = new Map(
    marketplaceCatalog.map((module) => [module.slug, module])
  );
  const installedOptionalModules = optionalModules.filter((module) =>
    installedSet.has(module.moduleSlug)
  );
  const marketplaceModules = marketplaceCatalog.filter(
    (module) => !installedSet.has(module.slug)
  );
  const updateCandidates = marketplaceCatalog
    .map((module) => ({
      module,
      installedModule: installedMap.get(module.slug)
    }))
    .filter(
      (
        entry
      ): entry is {
        module: MarketplaceModule;
        installedModule: InstalledModule;
      } =>
        Boolean(
          entry.installedModule?.version &&
          entry.installedModule.version !== entry.module.latestVersion
        )
    );
  const visibleInstalledCount =
    coreModules.length + installedOptionalModules.length;
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
      marketplaceCatalog.find((module) => module.slug === selectedModuleSlug) ??
      null;
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
        const detail = await getMarketplaceModule(selectedModuleSlug, apiOpts);
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
  }, [apiOpts, selectedModuleSlug]);

  useEffect(() => {
    let cancelled = false;
    async function loadRetentionAndSettings() {
      try {
        const holds = await listRetentionHolds(apiOpts);
        if (!cancelled) {
          setRetentionHolds(holds);
          setRetentionError(null);
          setRetentionScope(apiOpts);
        }
      } catch (error) {
        if (!cancelled) {
          setRetentionHolds([]);
          setRetentionError(
            error instanceof Error
              ? error.message
              : 'Failed to load retention holds'
          );
          setRetentionScope(apiOpts);
        }
      }
    }
    void loadRetentionAndSettings();
    return () => {
      cancelled = true;
    };
  }, [apiOpts, transitionRefresh]);

  const opIdFromUrl =
    searchParams.get('op_id') ?? searchParams.get('operation_id');
  useEffect(() => {
    let cancelled = false;
    let inFlight = false;
    async function loadTransitions() {
      if (inFlight) return;
      inFlight = true;
      try {
        const activeList = await listActiveModuleTransitions(apiOpts);
        let selected: ModuleTransitionCheckpoint | null = null;
        let selectionError: string | null = null;
        if (opIdFromUrl) {
          try {
            selected = await getTransitionCheckpoint(opIdFromUrl, apiOpts);
            if (!selected)
              selectionError = 'The selected transition was not found.';
          } catch (error) {
            selectionError =
              error instanceof Error
                ? error.message
                : 'Failed to load selected transition';
          }
        }
        if (!cancelled) {
          setActiveTransitions(
            selected
              ? [
                  selected,
                  ...activeList.filter(
                    (checkpoint) =>
                      checkpoint.operationId !== selected?.operationId
                  )
                ]
              : activeList
          );
          setTransitionError(selectionError);
          setTransitionScope(apiOpts);
        }
      } catch (error) {
        if (!cancelled) {
          setActiveTransitions([]);
          setTransitionError(
            error instanceof Error
              ? error.message
              : 'Failed to load module transitions'
          );
          setTransitionScope(apiOpts);
        }
      } finally {
        inFlight = false;
      }
    }
    void loadTransitions();
    const intervalId = window.setInterval(() => {
      void loadTransitions();
    }, 5000);
    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, [opIdFromUrl, apiOpts, transitionRefresh]);

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
        normalized.installedOnly,
        apiOpts
      );
      setMarketplaceCatalog(modules);
      try {
        setRegistryFreshness(await listMarketplaceRegistryFreshness(apiOpts));
      } catch {
        // Read-only module users can refresh the public catalog without
        // receiving operator-only registry observations.
      }
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
          new Set([
            ...prev,
            ...modules.map((module) => module.source).filter(Boolean)
          ])
        ).sort((left, right) => left.localeCompare(right))
      );
    } finally {
      if (!silent) {
        setCatalogRefreshing(false);
      }
    }
  };

  const refreshOrchestrationState = async (filters = appliedCatalogFilters) => {
    const request = ++orchestrationRequest.current;
    try {
      const normalized = normalizeCatalogFilters(filters);
      const [
        nextActiveBuild,
        nextBuildHistory,
        nextInstalledModules,
        nextMarketplaceCatalog,
        nextRegistryFreshness
      ] = await Promise.all([
        getActiveBuild(apiOpts),
        getBuildHistory(10, 0, apiOpts),
        listInstalledModules(apiOpts),
        listMarketplaceModules(
          normalized.search,
          normalized.category,
          normalized.source,
          normalized.trustLevel,
          normalized.onlyCompatible,
          normalized.installedOnly,
          apiOpts
        ),
        listMarketplaceRegistryFreshness(apiOpts).catch(() => null)
      ]);
      if (request !== orchestrationRequest.current) return;
      setActiveBuild(nextActiveBuild);
      setBuildHistory(nextBuildHistory);
      setInstalledModules(nextInstalledModules);
      setMarketplaceCatalog(nextMarketplaceCatalog);
      if (nextRegistryFreshness) {
        setRegistryFreshness(nextRegistryFreshness);
      }
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
            ...nextMarketplaceCatalog
              .map((module) => module.source)
              .filter(Boolean)
          ])
        ).sort((left, right) => left.localeCompare(right))
      );
    } catch (error) {
      if (request === orchestrationRequest.current) {
        toast.error(
          error instanceof Error
            ? error.message
            : 'Failed to refresh module state'
        );
      }
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
        const [
          nextActiveBuild,
          nextBuildHistory,
          nextMarketplaceCatalog,
          nextRegistryFreshness
        ] = await Promise.all([
          getActiveBuild(apiOpts),
          getBuildHistory(10, 0, apiOpts),
          listMarketplaceModules(
            normalized.search,
            normalized.category,
            normalized.source,
            normalized.trustLevel,
            normalized.onlyCompatible,
            normalized.installedOnly,
            apiOpts
          ),
          listMarketplaceRegistryFreshness(apiOpts).catch(() => null)
        ]);
        if (cancelled) {
          return;
        }

        startTransition(() => {
          setActiveBuild(nextActiveBuild);
          setBuildHistory(nextBuildHistory);
          setMarketplaceCatalog(nextMarketplaceCatalog);
          if (nextRegistryFreshness) {
            setRegistryFreshness(nextRegistryFreshness);
          }
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
  }, [activeBuild?.id, activeBuild?.status, apiOpts, appliedCatalogFilters]);

  const handleToggle = async (slug: string, enabled: boolean) => {
    const module = modules.find((candidate) => candidate.moduleSlug === slug);
    if (!module) {
      toast.error(t('error.load'));
      return;
    }
    setLoading(slug);
    try {
      const idempotencyKey = crypto.randomUUID();
      const updated = await toggleModule(
        slug,
        enabled,
        module.lifecycleRevision,
        idempotencyKey,
        apiOpts
      );
      setModules((prev) =>
        prev.map((module) =>
          module.moduleSlug === slug
            ? {
                ...module,
                enabled: updated.enabled,
                lifecycleRevision: updated.lifecycleRevision
              }
            : module
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
      const build = await installModule(slug, version, apiOpts);
      setActiveBuild(build);
      setBuildHistory((prev) => upsertBuildJob(prev, build));
      toast.success(`Install queued for ${slug}`);
      await refreshOrchestrationState();
      router.refresh();
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : 'Failed to queue install'
      );
    } finally {
      setPlatformLoading(null);
    }
  };

  const handleUninstall = async (slug: string) => {
    setPlatformLoading(slug);
    try {
      const build = await uninstallModule(slug, apiOpts);
      setActiveBuild(build);
      setBuildHistory((prev) => upsertBuildJob(prev, build));
      toast.success(`Uninstall queued for ${slug}`);
      await refreshOrchestrationState();
      router.refresh();
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : 'Failed to queue uninstall'
      );
    } finally {
      setPlatformLoading(null);
    }
  };

  const handleUpgrade = async (slug: string, version: string) => {
    setPlatformLoading(slug);
    try {
      const build = await upgradeModule(slug, version, apiOpts);
      setActiveBuild(build);
      setBuildHistory((prev) => upsertBuildJob(prev, build));
      toast.success(`Upgrade queued for ${slug}`);
      await refreshOrchestrationState();
      router.refresh();
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : 'Failed to queue upgrade'
      );
    } finally {
      setPlatformLoading(null);
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
    router.replace(query ? `${pathname}?${query}` : pathname, {
      scroll: false
    });
  };

  const handleApplyFilters = async () => {
    try {
      setAppliedCatalogFilters(catalogFilterDraft);
      await refreshMarketplaceCatalog(catalogFilterDraft);
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : 'Failed to refresh catalog'
      );
    }
  };

  const handleResetFilters = async () => {
    setCatalogFilterDraft(DEFAULT_FILTERS);
    setAppliedCatalogFilters(DEFAULT_FILTERS);
    try {
      await refreshMarketplaceCatalog(DEFAULT_FILTERS);
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : 'Failed to reset catalog'
      );
    }
  };

  const handleModuleSort = (columnId: string) => {
    setModuleSort((prev) => {
      if (prev?.columnId === columnId) {
        return prev.direction === 'asc'
          ? { columnId, direction: 'desc' }
          : null;
      }
      return { columnId, direction: 'asc' };
    });
  };

  const allInstalledModules = useMemo(() => {
    const list = [...coreModules, ...installedOptionalModules];
    if (!moduleSort) return list;
    return [...list].sort((a, b) => {
      let cmp = 0;
      if (moduleSort.columnId === 'module') {
        cmp = a.name.localeCompare(b.name);
      } else if (moduleSort.columnId === 'kind') {
        cmp = a.kind.localeCompare(b.kind);
      } else if (moduleSort.columnId === 'version') {
        cmp = a.version.localeCompare(b.version);
      }
      return moduleSort.direction === 'asc' ? cmp : -cmp;
    });
  }, [coreModules, installedOptionalModules, moduleSort]);

  const buildHistoryColumns: DataTableStaticColumn<BuildJob>[] = useMemo(
    () => [
      {
        id: 'job',
        header: 'Job / Target',
        cell: (build) => (
          <div className='space-y-1 py-1'>
            <p className='text-sm font-medium'>
              {build.modulesDelta || build.reason || build.id}
            </p>
            <p className='text-muted-foreground text-xs'>
              {formatBuildSummary(build)}
            </p>
            {build.errorMessage && (
              <p className='text-destructive text-xs'>{build.errorMessage}</p>
            )}
          </div>
        )
      },
      {
        id: 'status',
        header: 'Status',
        cell: (build) => (
          <Badge
            variant={
              build.status === 'completed'
                ? 'secondary'
                : build.status === 'failed'
                  ? 'destructive'
                  : 'outline'
            }
          >
            {humanizeLabel(build.status)}
          </Badge>
        )
      },
      {
        id: 'stage',
        header: 'Stage & Progress',
        cell: (build) => (
          <div className='min-w-[120px] space-y-1 text-xs'>
            <div className='flex items-center justify-between gap-2'>
              <span className='text-muted-foreground'>
                {humanizeLabel(build.stage)}
              </span>
              <span className='font-mono font-medium'>{build.progress}%</span>
            </div>
            <Progress value={build.progress} className='h-1.5 w-full' />
          </div>
        )
      },
      {
        id: 'createdAt',
        header: 'Created At',
        cell: (build) => (
          <p className='text-muted-foreground text-xs whitespace-nowrap'>
            {formatTimestamp(build.createdAt)}
          </p>
        )
      },
      {
        id: 'logs',
        header: 'Logs',
        cell: (build) =>
          build.logsUrl ? (
            <a
              className='text-primary text-xs whitespace-nowrap underline-offset-4 hover:underline'
              href={build.logsUrl}
              target='_blank'
              rel='noreferrer'
            >
              Open logs
            </a>
          ) : (
            <span className='text-muted-foreground text-xs'>—</span>
          )
      }
    ],
    []
  );

  const installedModulesColumns: DataTableStaticColumn<ModuleInfo>[] = useMemo(
    () => [
      {
        id: 'module',
        header: 'Module',
        sortable: true,
        cell: (module) => (
          <div className='space-y-0.5 py-1'>
            <div className='flex items-center gap-2'>
              <span className='text-sm font-medium'>{module.name}</span>
              <code className='bg-muted text-muted-foreground rounded px-1.5 py-0.5 font-mono text-[11px]'>
                {module.moduleSlug}
              </code>
            </div>
            <p className='text-muted-foreground line-clamp-1 text-xs'>
              {module.description}
            </p>
          </div>
        )
      },
      {
        id: 'kind',
        header: 'Type',
        sortable: true,
        cell: (module) => (
          <Badge
            variant={module.kind === 'core' ? 'secondary' : 'outline'}
            className='text-xs whitespace-nowrap'
          >
            {module.kind === 'core' ? t('badge.core') : 'Optional'}
          </Badge>
        )
      },
      {
        id: 'version',
        header: 'Version',
        sortable: true,
        cell: (module) => {
          const installed = installedMap.get(module.moduleSlug);
          const hasUpdate = Boolean(
            installed?.version &&
            module.version &&
            installed.version !== module.version
          );
          return (
            <div className='space-y-0.5 text-xs whitespace-nowrap'>
              <span className='font-mono'>
                v{installed?.version ?? module.version}
              </span>
              {hasUpdate && (
                <div>
                  <Badge
                    variant='outline'
                    className='border-amber-300 text-[10px] text-amber-600'
                  >
                    Update v{module.version}
                  </Badge>
                </div>
              )}
            </div>
          );
        }
      },
      {
        id: 'status',
        header: 'Status',
        cell: (module) => {
          const isCore = module.kind === 'core';
          const isInstalled = isCore || installedSet.has(module.moduleSlug);
          const tenantEnabled = isCore || (isInstalled && module.enabled);
          if (isCore) {
            return (
              <Badge variant='secondary' className='text-xs whitespace-nowrap'>
                {t('always_active')}
              </Badge>
            );
          }
          return (
            <div className='flex items-center gap-2'>
              <Switch
                checked={tenantEnabled}
                disabled={
                  loading === module.moduleSlug ||
                  platformLoading === module.moduleSlug ||
                  !isInstalled
                }
                onCheckedChange={(checked) =>
                  handleToggle(module.moduleSlug, checked)
                }
              />
              <span className='text-muted-foreground text-xs'>
                {tenantEnabled ? t('enabled') : t('disabled')}
              </span>
            </div>
          );
        }
      },
      {
        id: 'actions',
        header: 'Actions',
        cell: (module) => {
          const isCore = module.kind === 'core';
          return (
            <div className='flex items-center justify-end gap-1.5 whitespace-nowrap'>
              <Button
                variant='ghost'
                size='sm'
                className='h-8 px-2 text-xs'
                onClick={() => handleInspect(module.moduleSlug)}
              >
                Details
              </Button>
              <Button
                variant='outline'
                size='sm'
                className='h-8 px-2 text-xs'
                onClick={() => void openSettings(module.moduleSlug)}
              >
                Settings
              </Button>
              {!isCore && (
                <Button
                  variant='outline'
                  size='sm'
                  className='text-destructive hover:text-destructive h-8 px-2 text-xs'
                  disabled={
                    Boolean(activeBuild) ||
                    platformLoading === module.moduleSlug
                  }
                  onClick={() => handleUninstall(module.moduleSlug)}
                >
                  {platformLoading === module.moduleSlug
                    ? 'Queueing...'
                    : 'Uninstall'}
                </Button>
              )}
            </div>
          );
        }
      }
    ],
    [
      activeBuild,
      handleInspect,
      handleToggle,
      handleUninstall,
      installedMap,
      installedSet,
      loading,
      platformLoading,
      t,
      openSettings
    ]
  );

  return (
    <div className='space-y-8'>
      {loadErrors.length > 0 && (
        <Card className='border-destructive/30 bg-destructive/5'>
          <CardHeader>
            <CardTitle className='text-base'>
              Module data is partially unavailable
            </CardTitle>
            <CardDescription>
              The admin shell remains usable, but some module registry calls
              failed.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <ul className='text-muted-foreground list-disc space-y-1 pl-5 text-sm'>
              {loadErrors.map((error) => (
                <li key={error}>{error}</li>
              ))}
            </ul>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader className='pb-3'>
          <CardTitle className='text-sm font-medium'>
            Admin surface policy
          </CardTitle>
          <CardDescription>
            {isShowcaseSurface
              ? 'Next admin is a showcase surface. Dedicated module UI appears only where a module is explicitly marked with Next showcase support.'
              : 'Leptos admin is the canonical operator surface for module UI and ongoing module parity.'}
          </CardDescription>
        </CardHeader>
        <CardContent className='flex flex-wrap items-center gap-2 pt-0'>
          <Badge variant='secondary'>
            {isShowcaseSurface
              ? 'Current: Next showcase'
              : 'Current: Leptos canonical'}
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
              Install, uninstall, and upgrade actions queue a shared rebuild for
              both admin stacks.
            </CardDescription>
          </CardHeader>
          <CardContent className='space-y-4'>
            {latestBuild ? (
              <>
                <div className='flex items-center justify-between gap-3'>
                  <div className='space-y-1'>
                    <div className='flex items-center gap-2'>
                      <Badge variant='outline'>
                        {humanizeLabel(latestBuild.status)}
                      </Badge>
                      <span className='text-muted-foreground text-xs'>
                        {humanizeLabel(latestBuild.stage)}
                      </span>
                    </div>
                    <p className='text-sm font-medium'>
                      {latestBuild.modulesDelta ||
                        latestBuild.reason ||
                        'Platform module rebuild'}
                    </p>
                    <p className='text-muted-foreground text-xs'>
                      Updated {formatTimestamp(latestBuild.updatedAt)}
                    </p>
                  </div>
                  <span className='text-sm font-semibold'>
                    {latestBuild.progress}%
                  </span>
                </div>
                <Progress value={latestBuild.progress} />
                <p className='text-muted-foreground text-xs'>
                  {activeBuild
                    ? 'Platform actions stay locked until the current build finishes.'
                    : 'No active build. The latest completed job is shown for context.'}
                </p>
                {isBuildActive(activeBuild) && (
                  <p className='text-muted-foreground text-xs'>
                    Live refresh is active every 5 seconds while this build is
                    running.
                  </p>
                )}
              </>
            ) : (
              <p className='text-muted-foreground text-sm'>
                No platform builds yet. The first install, uninstall, or upgrade
                will queue one.
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
            <p className='text-3xl font-semibold'>
              {marketplaceModules.length}
            </p>
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
            Narrow Marketplace and Updates to the modules you actually want to
            review.
          </CardDescription>
        </CardHeader>
        <CardContent className='space-y-4'>
          <div className='grid gap-3 lg:grid-cols-[minmax(0,2fr)_repeat(3,minmax(0,1fr))_auto_auto]'>
            <Input
              value={catalogFilterDraft.search}
              onChange={(event) =>
                setCatalogFilterDraft((prev) => ({
                  ...prev,
                  search: event.target.value
                }))
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
                setCatalogFilterDraft((prev) => ({
                  ...prev,
                  trustLevel: value
                }))
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
              variant={
                catalogFilterDraft.onlyCompatible ? 'default' : 'outline'
              }
              type='button'
              onClick={() =>
                setCatalogFilterDraft((prev) => ({
                  ...prev,
                  onlyCompatible: !prev.onlyCompatible
                }))
              }
            >
              {catalogFilterDraft.onlyCompatible
                ? 'Compatible only'
                : 'Include risks'}
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
              {catalogFilterDraft.installedOnly
                ? 'Installed only'
                : 'All install states'}
            </Button>
          </div>
          <div className='flex flex-wrap items-center justify-between gap-3'>
            <div className='flex flex-wrap items-center gap-2 text-xs'>
              <Badge variant='secondary'>
                {catalogRefreshing ? 'Refreshing catalog' : 'Catalog ready'}
              </Badge>
              {JSON.stringify(catalogFilterDraft) !==
                JSON.stringify(appliedCatalogFilters) && (
                <Badge variant='outline'>Pending changes</Badge>
              )}
              {appliedCatalogFilters.search && (
                <Badge variant='outline'>
                  Search: {appliedCatalogFilters.search}
                </Badge>
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
          module={
            selectedModuleDetail?.slug === selectedModuleSlug &&
            searchParams.get('module') === selectedModuleSlug
              ? selectedModuleDetail
              : null
          }
          loading={moduleDetailLoading}
          onClose={handleCloseDetail}
          apiOpts={apiOpts}
          onRefresh={refreshOrchestrationState}
        />
      )}

      {settingsDialog.open && settingsDialog.scope === apiOpts && (
        <ModuleSettingsDialog
          moduleSlug={settingsDialog.slug}
          initialSettings={settingsDialog.settings}
          expectedRevision={settingsDialog.revision}
          open={settingsDialog.open}
          onOpenChange={(open) =>
            setSettingsDialog((prev) => ({ ...prev, open }))
          }
          onSaved={(slug, _newSettings, newRevision) => {
            setModules((prev) =>
              prev.map((m) =>
                m.moduleSlug === slug
                  ? { ...m, lifecycleRevision: newRevision }
                  : m
              )
            );
          }}
          apiOpts={apiOpts}
          settingsFields={
            settingsDialog.slug === 'forum'
              ? ({ settingsText, onSettingsTextChange, disabled }) => (
                  <ForumModuleSettingsFields
                    settingsText={settingsText}
                    onSettingsTextChange={onSettingsTextChange}
                    disabled={disabled}
                  />
                )
              : settingsDialog.slug === 'blog'
                ? ({ settingsText, onSettingsTextChange, disabled }) => (
                    <BlogModuleSettingsFields
                      settingsText={settingsText}
                      onSettingsTextChange={onSettingsTextChange}
                      disabled={disabled}
                    />
                  )
                : undefined
          }
        />
      )}

      <Tabs defaultValue='installed' className='space-y-6'>
        <TabsList>
          <TabsTrigger value='installed'>Installed</TabsTrigger>
          <TabsTrigger value='marketplace'>Marketplace</TabsTrigger>
          <TabsTrigger value='updates'>Updates</TabsTrigger>
        </TabsList>

        <TabsContent value='installed' className='space-y-6'>
          <RecoveryWorkspace
            key={`${session?.user?.id ?? ''}:${session?.user?.tenantId ?? ''}`}
            apiOpts={apiOpts}
            onModuleUpdated={(updated) => {
              setModules((current) => current.map((module) => module.moduleSlug === updated.moduleSlug
                ? { ...module, enabled: updated.enabled, lifecycleRevision: updated.revision }
                : module));
              setModuleEnabled(updated.moduleSlug, updated.enabled);
            }}
            onRefresh={() => { void refreshOrchestrationState(); router.refresh(); }}
          />
          <Card>
            <CardHeader>
              <CardTitle className='text-base'>Build history</CardTitle>
              <CardDescription>
                Recent rebuild jobs visible to both admin applications.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <DataTableStatic
                rows={buildHistory}
                columns={buildHistoryColumns}
                getRowKey={(build: BuildJob) => build.id}
                density='compact'
                emptyState={<DataTableEmptyState message='No builds yet.' />}
              />
            </CardContent>
          </Card>

          <div className='flex items-center justify-between gap-4'>
            <div className='flex items-center gap-2'>
              <h2 className='text-lg font-semibold'>Installed Modules</h2>
              <Badge variant='outline' className='text-xs'>
                {coreModules.length + installedOptionalModules.length} total
              </Badge>
            </div>
            <div className='bg-muted/40 flex items-center gap-1 rounded-lg border p-0.5'>
              <Button
                variant={installedViewMode === 'grid' ? 'secondary' : 'ghost'}
                size='sm'
                className='h-7 gap-1.5 px-2.5 text-xs'
                onClick={() => setInstalledViewMode('grid')}
              >
                <IconLayoutGrid className='h-3.5 w-3.5' />
                <span>Grid</span>
              </Button>
              <Button
                variant={installedViewMode === 'table' ? 'secondary' : 'ghost'}
                size='sm'
                className='h-7 gap-1.5 px-2.5 text-xs'
                onClick={() => setInstalledViewMode('table')}
              >
                <IconTable className='h-3.5 w-3.5' />
                <span>Table</span>
              </Button>
            </div>
          </div>

          {installedViewMode === 'table' ? (
            <Card>
              <CardContent className='pt-6'>
                <DataTableStatic
                  rows={allInstalledModules}
                  columns={installedModulesColumns}
                  getRowKey={(module: ModuleInfo) => module.moduleSlug}
                  density='compact'
                  sort={moduleSort}
                  onSort={handleModuleSort}
                  emptyState={
                    <DataTableEmptyState message='No modules installed.' />
                  }
                />
              </CardContent>
            </Card>
          ) : (
            <>
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
                      platformVersion={
                        installedMap.get(module.moduleSlug)?.version
                      }
                      recommendedVersion={module.version}
                      onInspect={handleInspect}
                      onConfigureSettings={(slug) => void openSettings(slug)}
                    />
                  ))}
                </div>
              </div>

              <div className='space-y-3'>
                <div className='flex items-center gap-2'>
                  <IconPackage className='text-muted-foreground h-5 w-5' />
                  <h3 className='text-lg font-semibold'>
                    {t('section.optional')}
                  </h3>
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
                        catalogModule={
                          catalogMap.get(module.moduleSlug) ?? null
                        }
                        loading={loading === module.moduleSlug}
                        platformLoading={platformLoading === module.moduleSlug}
                        platformInstalled
                        platformBusy={Boolean(activeBuild)}
                        platformVersion={
                          installedMap.get(module.moduleSlug)?.version
                        }
                        recommendedVersion={module.version}
                        onToggle={handleToggle}
                        onInspect={handleInspect}
                        onUninstall={handleUninstall}
                        onConfigureSettings={(slug) => void openSettings(slug)}
                      />
                    ))}
                  </div>
                ) : (
                  <Card>
                    <CardContent className='pt-6'>
                      <p className='text-muted-foreground text-sm'>
                        No optional modules are installed yet. Use the
                        Marketplace tab to queue the first install.
                      </p>
                    </CardContent>
                  </Card>
                )}
              </div>
            </>
          )}
        </TabsContent>

        <TabsContent value='marketplace' className='space-y-6'>
          <Card>
            <CardHeader>
              <CardTitle className='flex items-center gap-2 text-base'>
                <IconShoppingBag className='h-5 w-5' />
                Catalog workspace
              </CardTitle>
              <CardDescription>
                Modules here are known to the platform registry but not yet
                present in modules.toml.
              </CardDescription>
            </CardHeader>
          </Card>

          {registryFreshness.length > 0 && (
            <Card>
              <CardHeader>
                <CardTitle className='text-base'>
                  Federated registry freshness
                </CardTitle>
                <CardDescription>
                  Current status and last successful catalog access for every
                  configured registry.
                </CardDescription>
              </CardHeader>
              <CardContent className='grid gap-3 md:grid-cols-2 xl:grid-cols-3'>
                {registryFreshness.map((registry) => (
                  <div
                    key={registry.registryId}
                    className='rounded-lg border p-4'
                  >
                    <div className='flex items-center justify-between gap-3'>
                      <span className='font-mono text-sm font-medium'>
                        {registry.registryId}
                      </span>
                      <Badge variant={registryStatusVariant(registry.status)}>
                        {humanizeLabel(registry.status)}
                      </Badge>
                    </div>
                    <dl className='text-muted-foreground mt-3 space-y-2 text-xs'>
                      <div className='flex items-center justify-between gap-3'>
                        <dt>Last success</dt>
                        <dd className='text-foreground font-mono'>
                          {formatRegistryLastSuccess(
                            registry.lastSuccessUnixMs
                          )}
                        </dd>
                      </div>
                      <div className='flex items-center justify-between gap-3'>
                        <dt>Consecutive failures</dt>
                        <dd className='text-foreground font-mono'>
                          {registry.consecutiveFailures}
                        </dd>
                      </div>
                    </dl>
                  </div>
                ))}
              </CardContent>
            </Card>
          )}

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
                  All optional registry modules are already installed in the
                  platform manifest.
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
                Only modules with an explicit installed version and a newer
                registry version appear here.
              </CardDescription>
            </CardHeader>
          </Card>

          {transitionScope === apiOpts && transitionError && (
            <p role='alert' className='text-destructive text-sm'>
              {transitionError}
            </p>
          )}
          {retentionScope === apiOpts && retentionError && (
            <p role='alert' className='text-destructive text-sm'>
              {retentionError}
            </p>
          )}
          {transitionScope === apiOpts &&
            activeTransitions.map((checkpoint) => (
              <TransitionControlCard
                key={checkpoint.operationId}
                checkpoint={checkpoint}
                retentionHolds={
                  retentionScope === apiOpts && !retentionError
                    ? retentionHolds
                    : undefined
                }
                onRefresh={() => setTransitionRefresh((value) => value + 1)}
                apiOpts={apiOpts}
              />
            ))}

          {transitionScope === apiOpts &&
            activeTransitions.length === 0 &&
            retentionScope === apiOpts &&
            retentionHolds.length > 0 && (
              <Card>
                <CardHeader className='pb-3'>
                  <CardTitle className='text-muted-foreground text-xs font-semibold tracking-wider uppercase'>
                    Active Platform Retention Holds ({retentionHolds.length})
                  </CardTitle>
                </CardHeader>
                <CardContent className='space-y-2 font-mono text-xs'>
                  {retentionHolds.map((hold) => (
                    <div
                      key={hold.holdId}
                      className='flex items-center justify-between border-b pb-1 last:border-b-0'
                    >
                      <span className='text-foreground'>
                        {hold.targetType}: {hold.targetIdentity}
                      </span>
                      <Badge variant='secondary'>{hold.kind}</Badge>
                    </div>
                  ))}
                </CardContent>
              </Card>
            )}

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
                <p className='text-sm font-medium'>
                  No pinned module updates detected.
                </p>
                <p className='text-muted-foreground text-sm'>
                  Path-based local modules follow the current repository state
                  and therefore do not show a separate version upgrade action.
                </p>
              </CardContent>
            </Card>
          )}
        </TabsContent>
      </Tabs>
    </div>
  );
}
