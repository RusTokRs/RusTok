'use client';

import React from 'react';

import { graphqlRequest as sharedGraphqlRequest } from '../../../src/shared/api/graphql';

type SearchAdminTab = 'overview' | 'playground' | 'analytics' | 'dictionaries';

export type SearchAdminPageProps = {
  token?: string | null;
  tenantSlug?: string | null;
  graphqlUrl?: string;
  initialTab?: SearchAdminTab;
  initialQuery?: string;
  laggingLimit?: number;
};

type SearchAdminBootstrap = {
  availableSearchEngines: Array<{
    kind: string;
    label: string;
    providedBy: string;
    enabled: boolean;
    defaultEngine: boolean;
  }>;
  searchSettingsPreview: {
    tenantId: string | null;
    activeEngine: string;
    fallbackEngine: string;
    config: string;
    updatedAt: string;
  };
  searchDiagnostics: {
    tenantId: string;
    totalDocuments: number;
    publicDocuments: number;
    contentDocuments: number;
    productDocuments: number;
    staleDocuments: number;
    missingDocuments: number;
    orphanedDocuments: number;
    newestIndexedAt: string | null;
    oldestIndexedAt: string | null;
    maxLagSeconds: number;
    state: string;
  };
};

type SearchPreviewPayload = {
  queryLogId: string | null;
  presetKey: string | null;
  total: number;
  tookMs: number;
  engine: string;
  rankingProfile: string;
  items: Array<{
    id: string;
    entityType: string;
    sourceModule: string;
    title: string;
    snippet: string | null;
    score: number;
    url: string | null;
    payload: string;
  }>;
  facets: Array<{
    name: string;
    buckets: Array<{ value: string; count: number }>;
  }>;
};

type SearchFilterPresetPayload = {
  key: string;
  label: string;
  entityTypes: string[];
  sourceModules: string[];
  statuses: string[];
  rankingProfile: string | null;
};

type LaggingSearchDocumentPayload = {
  documentKey: string;
  documentId: string;
  sourceModule: string;
  entityType: string;
  locale: string;
  status: string;
  title: string;
  updatedAt: string;
  indexedAt: string;
  lagSeconds: number;
};

type SearchConsistencyIssuePayload = {
  issueKind: string;
  documentKey: string;
  documentId: string;
  sourceModule: string;
  entityType: string;
  locale: string;
  status: string;
  title: string;
  updatedAt: string;
  indexedAt: string | null;
};

type SearchAnalyticsSummaryPayload = {
  windowDays: number;
  totalQueries: number;
  successfulQueries: number;
  zeroResultQueries: number;
  zeroResultRate: number;
  slowQueries: number;
  slowQueryRate: number;
  avgTookMs: number;
  avgResultsPerQuery: number;
  uniqueQueries: number;
  clickedQueries: number;
  totalClicks: number;
  clickThroughRate: number;
  abandonmentQueries: number;
  abandonmentRate: number;
  lastQueryAt: string | null;
};

type SearchAnalyticsQueryRowPayload = {
  query: string;
  hits: number;
  zeroResultHits: number;
  clicks: number;
  avgTookMs: number;
  avgResults: number;
  clickThroughRate: number;
  abandonmentRate: number;
  lastSeenAt: string;
};

type SearchAnalyticsInsightRowPayload = {
  query: string;
  hits: number;
  zeroResultHits: number;
  clicks: number;
  clickThroughRate: number;
  abandonmentRate: number;
  recommendation: string;
};

type SearchAnalyticsPayload = {
  summary: SearchAnalyticsSummaryPayload;
  topQueries: SearchAnalyticsQueryRowPayload[];
  zeroResultQueries: SearchAnalyticsQueryRowPayload[];
  slowQueries: SearchAnalyticsQueryRowPayload[];
  lowCtrQueries: SearchAnalyticsQueryRowPayload[];
  abandonmentQueries: SearchAnalyticsQueryRowPayload[];
  intelligenceCandidates: SearchAnalyticsInsightRowPayload[];
};

type SearchSynonymPayload = {
  id: string;
  term: string;
  synonyms: string[];
  updatedAt: string;
};

type SearchStopWordPayload = {
  id: string;
  value: string;
  updatedAt: string;
};

type SearchQueryRulePayload = {
  id: string;
  queryText: string;
  queryNormalized: string;
  ruleKind: string;
  documentId: string;
  entityType: string;
  sourceModule: string;
  title: string;
  pinnedPosition: number;
  updatedAt: string;
};

type SearchDictionarySnapshotPayload = {
  synonyms: SearchSynonymPayload[];
  stopWords: SearchStopWordPayload[];
  queryRules: SearchQueryRulePayload[];
};

const SEARCH_ADMIN_BOOTSTRAP_QUERY = `
  query SearchAdminBootstrap {
    availableSearchEngines { kind label providedBy enabled defaultEngine }
    searchSettingsPreview { tenantId activeEngine fallbackEngine config updatedAt }
    searchDiagnostics {
      tenantId totalDocuments publicDocuments contentDocuments productDocuments
      staleDocuments missingDocuments orphanedDocuments
      newestIndexedAt oldestIndexedAt maxLagSeconds state
    }
  }
`;

const SEARCH_PREVIEW_QUERY = `
  query SearchPreview($input: SearchPreviewInput!) {
    searchPreview(input: $input) {
      queryLogId presetKey total tookMs engine rankingProfile
      items { id entityType sourceModule title snippet score locale url payload }
      facets { name buckets { value count } }
    }
  }
`;

const SEARCH_FILTER_PRESETS_QUERY = `
  query SearchFilterPresets($input: SearchFilterPresetsInput!) {
    searchFilterPresets(input: $input) {
      key
      label
      entityTypes
      sourceModules
      statuses
      rankingProfile
    }
  }
`;

const SEARCH_LAGGING_DOCUMENTS_QUERY = `
  query SearchLaggingDocuments($limit: Int) {
    searchLaggingDocuments(limit: $limit) {
      documentKey documentId sourceModule entityType locale status isPublic title updatedAt indexedAt lagSeconds
    }
  }
`;

const SEARCH_CONSISTENCY_ISSUES_QUERY = `
  query SearchConsistencyIssues($limit: Int) {
    searchConsistencyIssues(limit: $limit) {
      issueKind documentKey documentId sourceModule entityType locale status title updatedAt indexedAt
    }
  }
`;

const SEARCH_ANALYTICS_QUERY = `
  query SearchAnalytics($days: Int, $limit: Int) {
    searchAnalytics(days: $days, limit: $limit) {
      summary {
        windowDays totalQueries successfulQueries zeroResultQueries
        zeroResultRate slowQueries slowQueryRate avgTookMs avgResultsPerQuery uniqueQueries
        clickedQueries totalClicks clickThroughRate abandonmentQueries
        abandonmentRate lastQueryAt
      }
      topQueries { query hits zeroResultHits clicks avgTookMs avgResults clickThroughRate abandonmentRate lastSeenAt }
      zeroResultQueries { query hits zeroResultHits clicks avgTookMs avgResults clickThroughRate abandonmentRate lastSeenAt }
      slowQueries { query hits zeroResultHits clicks avgTookMs avgResults clickThroughRate abandonmentRate lastSeenAt }
      lowCtrQueries { query hits zeroResultHits clicks avgTookMs avgResults clickThroughRate abandonmentRate lastSeenAt }
      abandonmentQueries { query hits zeroResultHits clicks avgTookMs avgResults clickThroughRate abandonmentRate lastSeenAt }
      intelligenceCandidates { query hits zeroResultHits clicks clickThroughRate abandonmentRate recommendation }
    }
  }
`;

const SEARCH_DICTIONARY_SNAPSHOT_QUERY = `
  query SearchDictionarySnapshot {
    searchDictionarySnapshot {
      synonyms { id term synonyms updatedAt }
      stopWords { id value updatedAt }
      queryRules {
        id queryText queryNormalized ruleKind documentId
        entityType sourceModule title pinnedPosition updatedAt
      }
    }
  }
`;

const TRIGGER_SEARCH_REBUILD_MUTATION = `
  mutation TriggerSearchRebuild($input: TriggerSearchRebuildInput!) {
    triggerSearchRebuild(input: $input) { success queued tenantId targetType targetId }
  }
`;

const TRACK_SEARCH_CLICK_MUTATION = `
  mutation TrackSearchClick($input: TrackSearchClickInput!) {
    trackSearchClick(input: $input) { success tracked }
  }
`;

const UPDATE_SEARCH_SETTINGS_MUTATION = `
  mutation UpdateSearchSettings($input: UpdateSearchSettingsInput!) {
    updateSearchSettings(input: $input) {
      success
      settings { tenantId activeEngine fallbackEngine config updatedAt }
    }
  }
`;

const UPSERT_SEARCH_SYNONYM_MUTATION = `
  mutation UpsertSearchSynonym($input: UpsertSearchSynonymInput!) {
    upsertSearchSynonym(input: $input) { success }
  }
`;

const DELETE_SEARCH_SYNONYM_MUTATION = `
  mutation DeleteSearchSynonym($input: DeleteSearchSynonymInput!) {
    deleteSearchSynonym(input: $input) { success }
  }
`;

const ADD_SEARCH_STOP_WORD_MUTATION = `
  mutation AddSearchStopWord($input: AddSearchStopWordInput!) {
    addSearchStopWord(input: $input) { success }
  }
`;

const DELETE_SEARCH_STOP_WORD_MUTATION = `
  mutation DeleteSearchStopWord($input: DeleteSearchStopWordInput!) {
    deleteSearchStopWord(input: $input) { success }
  }
`;

const UPSERT_SEARCH_PIN_RULE_MUTATION = `
  mutation UpsertSearchPinRule($input: UpsertSearchPinRuleInput!) {
    upsertSearchPinRule(input: $input) { success }
  }
`;

const DELETE_SEARCH_QUERY_RULE_MUTATION = `
  mutation DeleteSearchQueryRule($input: DeleteSearchQueryRuleInput!) {
    deleteSearchQueryRule(input: $input) { success }
  }
`;

const tabs: Array<{ key: SearchAdminTab; label: string }> = [
  { key: 'overview', label: 'Overview' },
  { key: 'playground', label: 'Playground' },
  { key: 'analytics', label: 'Analytics' },
  { key: 'dictionaries', label: 'Dictionaries' }
];

type SearchPreviewFiltersInput = {
  entityTypes: string[];
  sourceModules: string[];
  statuses: string[];
};

function parseCsv(value: string): string[] {
  return value
    .split(',')
    .map((segment) => segment.trim())
    .filter(Boolean);
}

function optionalText(value: string): string | undefined {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : undefined;
}

function prettyJsonString(value: string): string {
  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value;
  }
}

function parseJsonValue(value: string): unknown | null {
  try {
    return JSON.parse(value);
  } catch {
    return null;
  }
}

function extractRankingProfileValue(
  configText: string,
  surface: string
): string {
  const config = parseJsonValue(configText);
  const rankingProfiles =
    config &&
    typeof config === 'object' &&
    config !== null &&
    'ranking_profiles' in config
      ? (config as { ranking_profiles?: Record<string, unknown> })
          .ranking_profiles
      : undefined;
  const value =
    rankingProfiles && typeof rankingProfiles[surface] === 'string'
      ? (rankingProfiles[surface] as string)
      : undefined;

  if (value) return value;
  return surface === 'admin_global_search' ? 'exact' : 'balanced';
}

function extractSurfacePresetsJson(
  configText: string,
  surface: string
): string {
  const config = parseJsonValue(configText);
  const filterPresets =
    config &&
    typeof config === 'object' &&
    config !== null &&
    'filter_presets' in config
      ? (config as { filter_presets?: Record<string, unknown> }).filter_presets
      : undefined;
  const value = filterPresets?.[surface];
  if (Array.isArray(value)) {
    return JSON.stringify(value, null, 2);
  }
  return '[]';
}

function parseJsonArrayForEditor(label: string, value: string): unknown[] {
  let parsed: unknown;
  try {
    parsed = JSON.parse(value);
  } catch (error: unknown) {
    throw new Error(`${label} must be valid JSON: ${errorMessage(error)}`);
  }
  if (!Array.isArray(parsed)) {
    throw new Error(`${label} must be a JSON array.`);
  }
  return parsed;
}

function mergeRelevanceEditorConfig(input: {
  configText: string;
  rankingDefault: string;
  rankingPreview: string;
  rankingStorefront: string;
  rankingAdminGlobal: string;
  previewPresets: string;
  storefrontPresets: string;
}): string {
  const parsed = parseJsonValue(input.configText);
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    throw new Error('Settings config root must be a JSON object.');
  }

  const config = { ...(parsed as Record<string, unknown>) };
  config.ranking_profiles = {
    default: input.rankingDefault,
    search_preview: input.rankingPreview,
    storefront_search: input.rankingStorefront,
    admin_global_search: input.rankingAdminGlobal
  };
  config.filter_presets = {
    search_preview: parseJsonArrayForEditor(
      'Preview filter presets',
      input.previewPresets
    ),
    storefront_search: parseJsonArrayForEditor(
      'Storefront filter presets',
      input.storefrontPresets
    )
  };

  return JSON.stringify(config, null, 2);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'Unexpected error';
}

async function graphqlRequest<TData>(
  query: string,
  variables: unknown,
  opts: {
    token?: string | null;
    tenantSlug?: string | null;
    graphqlUrl?: string;
  }
): Promise<TData> {
  return sharedGraphqlRequest<unknown, TData>(
    query,
    variables,
    opts.token,
    opts.tenantSlug,
    { graphqlUrl: opts.graphqlUrl }
  );
}

export function SearchAdminPage({
  token = null,
  tenantSlug = null,
  graphqlUrl,
  initialTab = 'overview',
  initialQuery = '',
  laggingLimit = 25
}: SearchAdminPageProps = {}): React.JSX.Element {
  const [activeTab, setActiveTab] = React.useState<SearchAdminTab>(initialTab);
  const [bootstrap, setBootstrap] = React.useState<SearchAdminBootstrap | null>(
    null
  );
  const [bootstrapError, setBootstrapError] = React.useState<string | null>(
    null
  );
  const [bootstrapLoading, setBootstrapLoading] = React.useState(false);
  const [laggingDocuments, setLaggingDocuments] = React.useState<
    LaggingSearchDocumentPayload[]
  >([]);
  const [laggingError, setLaggingError] = React.useState<string | null>(null);
  const [laggingLoading, setLaggingLoading] = React.useState(false);
  const [consistencyIssues, setConsistencyIssues] = React.useState<
    SearchConsistencyIssuePayload[]
  >([]);
  const [consistencyError, setConsistencyError] = React.useState<string | null>(
    null
  );
  const [consistencyLoading, setConsistencyLoading] = React.useState(false);
  const [analytics, setAnalytics] =
    React.useState<SearchAnalyticsPayload | null>(null);
  const [analyticsError, setAnalyticsError] = React.useState<string | null>(
    null
  );
  const [analyticsLoading, setAnalyticsLoading] = React.useState(false);
  const [refreshNonce, setRefreshNonce] = React.useState(0);
  const [query, setQuery] = React.useState(initialQuery);
  const [entityTypes, setEntityTypes] = React.useState('');
  const [sourceModules, setSourceModules] = React.useState('');
  const [statuses, setStatuses] = React.useState('');
  const [rankingProfile, setRankingProfile] = React.useState('');
  const [presetKey, setPresetKey] = React.useState('');
  const [filterPresets, setFilterPresets] = React.useState<
    SearchFilterPresetPayload[]
  >([]);
  const [filterPresetsLoading, setFilterPresetsLoading] = React.useState(false);
  const [filterPresetsError, setFilterPresetsError] = React.useState<
    string | null
  >(null);
  const [preview, setPreview] = React.useState<SearchPreviewPayload | null>(
    null
  );
  const [previewError, setPreviewError] = React.useState<string | null>(null);
  const [previewBusy, setPreviewBusy] = React.useState(false);
  const [rebuildScope, setRebuildScope] = React.useState('search');
  const [rebuildTargetId, setRebuildTargetId] = React.useState('');
  const [rebuildBusy, setRebuildBusy] = React.useState(false);
  const [rebuildFeedback, setRebuildFeedback] = React.useState<string | null>(
    null
  );
  const [settingsActiveEngine, setSettingsActiveEngine] =
    React.useState('postgres');
  const [settingsFallbackEngine, setSettingsFallbackEngine] =
    React.useState('postgres');
  const [settingsConfig, setSettingsConfig] = React.useState('{}');
  const [rankingDefaultProfile, setRankingDefaultProfile] =
    React.useState('balanced');
  const [rankingPreviewProfile, setRankingPreviewProfile] =
    React.useState('balanced');
  const [rankingStorefrontProfile, setRankingStorefrontProfile] =
    React.useState('balanced');
  const [rankingAdminGlobalProfile, setRankingAdminGlobalProfile] =
    React.useState('exact');
  const [previewPresetsConfig, setPreviewPresetsConfig] = React.useState('[]');
  const [storefrontPresetsConfig, setStorefrontPresetsConfig] =
    React.useState('[]');
  const [settingsBusy, setSettingsBusy] = React.useState(false);
  const [settingsFeedback, setSettingsFeedback] = React.useState<string | null>(
    null
  );

  const runPreviewRequest = React.useEffectEvent(
    async (
      queryValue: string,
      filters: SearchPreviewFiltersInput,
      selectedRankingProfile: string,
      selectedPresetKey: string
    ) => {
      if (!token || !tenantSlug) {
        setPreviewError('Search preview requires token and tenant slug.');
        return;
      }

      setPreviewBusy(true);
      setPreviewError(null);

      try {
        const data = await graphqlRequest<{
          searchPreview: SearchPreviewPayload;
        }>(
          SEARCH_PREVIEW_QUERY,
          {
            input: {
              query: queryValue,
              limit: 12,
              offset: 0,
              rankingProfile: optionalText(selectedRankingProfile),
              presetKey: optionalText(selectedPresetKey),
              entityTypes: filters.entityTypes.length
                ? filters.entityTypes
                : undefined,
              sourceModules: filters.sourceModules.length
                ? filters.sourceModules
                : undefined,
              statuses: filters.statuses.length ? filters.statuses : undefined
            }
          },
          { token, tenantSlug, graphqlUrl }
        );

        setPreview(data.searchPreview);
      } catch (error: unknown) {
        setPreviewError(errorMessage(error));
      } finally {
        setPreviewBusy(false);
      }
    }
  );

  React.useEffect(() => {
    setQuery(initialQuery);
    if (initialQuery.trim().length > 0) {
      setActiveTab('playground');
      void runPreviewRequest(
        initialQuery,
        {
          entityTypes: [],
          sourceModules: [],
          statuses: []
        },
        rankingProfile,
        presetKey
      );
    }
  }, [initialQuery, presetKey, rankingProfile, runPreviewRequest]);

  React.useEffect(() => {
    let cancelled = false;
    if (!token || !tenantSlug) {
      setFilterPresets([]);
      setFilterPresetsError(null);
      setFilterPresetsLoading(false);
      return () => {
        cancelled = true;
      };
    }

    setFilterPresetsLoading(true);
    setFilterPresetsError(null);

    void graphqlRequest<{
      searchFilterPresets: SearchFilterPresetPayload[];
    }>(
      SEARCH_FILTER_PRESETS_QUERY,
      {
        input: {
          surface: 'search_preview'
        }
      },
      { token, tenantSlug, graphqlUrl }
    )
      .then((data) => {
        if (!cancelled) {
          setFilterPresets(data.searchFilterPresets);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setFilterPresetsError(errorMessage(error));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setFilterPresetsLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [token, tenantSlug, graphqlUrl, refreshNonce]);

  React.useEffect(() => {
    let cancelled = false;
    if (!token || !tenantSlug) {
      setBootstrap(null);
      setBootstrapError(null);
      setBootstrapLoading(false);
      return () => {
        cancelled = true;
      };
    }

    setBootstrapLoading(true);
    setBootstrapError(null);

    void graphqlRequest<SearchAdminBootstrap>(
      SEARCH_ADMIN_BOOTSTRAP_QUERY,
      undefined,
      { token, tenantSlug, graphqlUrl }
    )
      .then((data) => {
        if (!cancelled) {
          setBootstrap(data);
          setSettingsActiveEngine(data.searchSettingsPreview.activeEngine);
          setSettingsFallbackEngine(data.searchSettingsPreview.fallbackEngine);
          setSettingsConfig(
            prettyJsonString(data.searchSettingsPreview.config)
          );
          setRankingDefaultProfile(
            extractRankingProfileValue(
              data.searchSettingsPreview.config,
              'default'
            )
          );
          setRankingPreviewProfile(
            extractRankingProfileValue(
              data.searchSettingsPreview.config,
              'search_preview'
            )
          );
          setRankingStorefrontProfile(
            extractRankingProfileValue(
              data.searchSettingsPreview.config,
              'storefront_search'
            )
          );
          setRankingAdminGlobalProfile(
            extractRankingProfileValue(
              data.searchSettingsPreview.config,
              'admin_global_search'
            )
          );
          setPreviewPresetsConfig(
            extractSurfacePresetsJson(
              data.searchSettingsPreview.config,
              'search_preview'
            )
          );
          setStorefrontPresetsConfig(
            extractSurfacePresetsJson(
              data.searchSettingsPreview.config,
              'storefront_search'
            )
          );
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) setBootstrapError(errorMessage(error));
      })
      .finally(() => {
        if (!cancelled) setBootstrapLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [token, tenantSlug, graphqlUrl, refreshNonce]);

  React.useEffect(() => {
    let cancelled = false;
    if (!token || !tenantSlug) {
      setLaggingDocuments([]);
      setLaggingError(null);
      setLaggingLoading(false);
      return () => {
        cancelled = true;
      };
    }

    setLaggingLoading(true);
    setLaggingError(null);

    void graphqlRequest<{
      searchLaggingDocuments: LaggingSearchDocumentPayload[];
    }>(
      SEARCH_LAGGING_DOCUMENTS_QUERY,
      { limit: laggingLimit },
      { token, tenantSlug, graphqlUrl }
    )
      .then((data) => {
        if (!cancelled) setLaggingDocuments(data.searchLaggingDocuments);
      })
      .catch((error: unknown) => {
        if (!cancelled) setLaggingError(errorMessage(error));
      })
      .finally(() => {
        if (!cancelled) setLaggingLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [token, tenantSlug, graphqlUrl, laggingLimit, refreshNonce]);

  React.useEffect(() => {
    let cancelled = false;
    if (!token || !tenantSlug || activeTab !== 'analytics') {
      if (!token || !tenantSlug) {
        setConsistencyIssues([]);
        setConsistencyError(null);
        setConsistencyLoading(false);
      }
      return () => {
        cancelled = true;
      };
    }

    setConsistencyLoading(true);
    setConsistencyError(null);

    void graphqlRequest<{
      searchConsistencyIssues: SearchConsistencyIssuePayload[];
    }>(
      SEARCH_CONSISTENCY_ISSUES_QUERY,
      { limit: laggingLimit },
      { token, tenantSlug, graphqlUrl }
    )
      .then((data) => {
        if (!cancelled) setConsistencyIssues(data.searchConsistencyIssues);
      })
      .catch((error: unknown) => {
        if (!cancelled) setConsistencyError(errorMessage(error));
      })
      .finally(() => {
        if (!cancelled) setConsistencyLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [activeTab, token, tenantSlug, graphqlUrl, laggingLimit, refreshNonce]);

  React.useEffect(() => {
    let cancelled = false;
    if (!token || !tenantSlug || activeTab !== 'analytics') {
      if (!token || !tenantSlug) {
        setAnalytics(null);
        setAnalyticsError(null);
        setAnalyticsLoading(false);
      }
      return () => {
        cancelled = true;
      };
    }

    setAnalyticsLoading(true);
    setAnalyticsError(null);

    void graphqlRequest<{ searchAnalytics: SearchAnalyticsPayload }>(
      SEARCH_ANALYTICS_QUERY,
      { days: 7, limit: 10 },
      { token, tenantSlug, graphqlUrl }
    )
      .then((data) => {
        if (!cancelled) setAnalytics(data.searchAnalytics);
      })
      .catch((error: unknown) => {
        if (!cancelled) setAnalyticsError(errorMessage(error));
      })
      .finally(() => {
        if (!cancelled) setAnalyticsLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [activeTab, token, tenantSlug, graphqlUrl, refreshNonce]);

  async function runPreview(
    event: React.FormEvent<HTMLFormElement>
  ): Promise<void> {
    event.preventDefault();
    await runPreviewRequest(
      query,
      {
        entityTypes: parseCsv(entityTypes),
        sourceModules: parseCsv(sourceModules),
        statuses: parseCsv(statuses)
      },
      rankingProfile,
      presetKey
    );
  }

  async function queueRebuild(): Promise<void> {
    if (!token || !tenantSlug) {
      setRebuildFeedback('Scoped rebuild requires token and tenant slug.');
      return;
    }

    setRebuildBusy(true);
    setRebuildFeedback(null);

    try {
      const data = await graphqlRequest<{
        triggerSearchRebuild: { targetType: string; targetId: string | null };
      }>(
        TRIGGER_SEARCH_REBUILD_MUTATION,
        {
          input: {
            targetType: rebuildScope,
            targetId: optionalText(rebuildTargetId)
          }
        },
        { token, tenantSlug, graphqlUrl }
      );
      const payload = data.triggerSearchRebuild;
      const suffix = payload.targetId ? ` for target ${payload.targetId}` : '';
      setRebuildFeedback(
        `Rebuild queued for ${payload.targetType} scope${suffix}.`
      );
      setRefreshNonce((value) => value + 1);
    } catch (error: unknown) {
      setRebuildFeedback(
        `Failed to queue search rebuild: ${errorMessage(error)}`
      );
    } finally {
      setRebuildBusy(false);
    }
  }

  async function saveSettings(): Promise<void> {
    if (!token || !tenantSlug) {
      setSettingsFeedback('Saving settings requires token and tenant slug.');
      return;
    }

    let mergedConfig: string;
    try {
      mergedConfig = mergeRelevanceEditorConfig({
        configText: settingsConfig,
        rankingDefault: rankingDefaultProfile,
        rankingPreview: rankingPreviewProfile,
        rankingStorefront: rankingStorefrontProfile,
        rankingAdminGlobal: rankingAdminGlobalProfile,
        previewPresets: previewPresetsConfig,
        storefrontPresets: storefrontPresetsConfig
      });
    } catch (error: unknown) {
      setSettingsFeedback(errorMessage(error));
      return;
    }

    setSettingsBusy(true);
    setSettingsFeedback(null);

    try {
      const data = await graphqlRequest<{
        updateSearchSettings: {
          success: boolean;
          settings: SearchAdminBootstrap['searchSettingsPreview'];
        };
      }>(
        UPDATE_SEARCH_SETTINGS_MUTATION,
        {
          input: {
            activeEngine: settingsActiveEngine,
            fallbackEngine: settingsFallbackEngine,
            config: mergedConfig
          }
        },
        { token, tenantSlug, graphqlUrl }
      );
      const settings = data.updateSearchSettings.settings;
      setSettingsFeedback('Search settings saved.');
      setSettingsActiveEngine(settings.activeEngine);
      setSettingsFallbackEngine(settings.fallbackEngine);
      setSettingsConfig(prettyJsonString(settings.config));
      setRankingDefaultProfile(
        extractRankingProfileValue(settings.config, 'default')
      );
      setRankingPreviewProfile(
        extractRankingProfileValue(settings.config, 'search_preview')
      );
      setRankingStorefrontProfile(
        extractRankingProfileValue(settings.config, 'storefront_search')
      );
      setRankingAdminGlobalProfile(
        extractRankingProfileValue(settings.config, 'admin_global_search')
      );
      setPreviewPresetsConfig(
        extractSurfacePresetsJson(settings.config, 'search_preview')
      );
      setStorefrontPresetsConfig(
        extractSurfacePresetsJson(settings.config, 'storefront_search')
      );
      setRefreshNonce((value) => value + 1);
    } catch (error: unknown) {
      setSettingsFeedback(
        `Failed to save search settings: ${errorMessage(error)}`
      );
    } finally {
      setSettingsBusy(false);
    }
  }

  return (
    <section className='space-y-6 rounded-[24px] border border-zinc-300 bg-white p-6'>
      <header className='flex flex-wrap items-start justify-between gap-4'>
        <div className='space-y-3'>
          <div className='inline-flex rounded-full border border-zinc-300 px-3 py-1 text-[12px] tracking-[0.14em] text-zinc-500 uppercase'>
            search
          </div>
          <div>
            <h1 className='text-[28px] font-semibold text-zinc-900'>
              Search Control Plane
            </h1>
            <p className='mt-3 max-w-3xl text-sm text-zinc-600'>
              Module-owned Next admin surface for diagnostics, scoped rebuilds,
              and PostgreSQL FTS inspection.
            </p>
          </div>
        </div>
        <div className='flex flex-wrap gap-2'>
          {tabs.map((tab) => (
            <button
              key={tab.key}
              type='button'
              onClick={() => React.startTransition(() => setActiveTab(tab.key))}
              className={
                tab.key === activeTab
                  ? 'rounded-xl bg-teal-700 px-4 py-2 text-sm font-medium text-white'
                  : 'rounded-xl border border-zinc-300 px-4 py-2 text-sm font-medium text-zinc-900'
              }
            >
              {tab.label}
            </button>
          ))}
        </div>
      </header>
      {!token || !tenantSlug ? (
        <EmptyPanel
          title='Search admin package is ready for host wiring'
          body='Pass token and tenantSlug from the Next admin host to enable diagnostics, rebuilds, and FTS preview.'
        />
      ) : bootstrapLoading ? (
        <LoadingPanel label='Loading search control plane...' />
      ) : bootstrapError ? (
        <ErrorPanel
          message={`Failed to load search bootstrap data: ${bootstrapError}`}
        />
      ) : !bootstrap ? (
        <EmptyPanel
          title='No bootstrap payload'
          body='The GraphQL endpoint returned no usable search bootstrap data.'
        />
      ) : activeTab === 'playground' ? (
        <PlaygroundPanel
          query={query}
          presetKey={presetKey}
          filterPresets={filterPresets}
          filterPresetsLoading={filterPresetsLoading}
          filterPresetsError={filterPresetsError}
          rankingProfile={rankingProfile}
          entityTypes={entityTypes}
          sourceModules={sourceModules}
          statuses={statuses}
          preview={preview}
          previewBusy={previewBusy}
          previewError={previewError}
          token={token}
          tenantSlug={tenantSlug}
          graphqlUrl={graphqlUrl}
          onQueryChange={setQuery}
          onPresetKeyChange={setPresetKey}
          onRankingProfileChange={setRankingProfile}
          onEntityTypesChange={setEntityTypes}
          onSourceModulesChange={setSourceModules}
          onStatusesChange={setStatuses}
          onSubmit={(event) => void runPreview(event)}
        />
      ) : activeTab === 'analytics' ? (
        <AnalyticsPanel
          diagnostics={bootstrap.searchDiagnostics}
          analytics={analytics}
          analyticsError={analyticsError}
          analyticsLoading={analyticsLoading}
          laggingDocuments={laggingDocuments}
          laggingError={laggingError}
          laggingLoading={laggingLoading}
          consistencyIssues={consistencyIssues}
          consistencyError={consistencyError}
          consistencyLoading={consistencyLoading}
        />
      ) : activeTab === 'dictionaries' ? (
        <DictionariesPanel
          token={token}
          tenantSlug={tenantSlug}
          graphqlUrl={graphqlUrl}
        />
      ) : (
        <OverviewPanel
          bootstrap={bootstrap}
          settingsActiveEngine={settingsActiveEngine}
          settingsFallbackEngine={settingsFallbackEngine}
          settingsConfig={settingsConfig}
          rankingDefaultProfile={rankingDefaultProfile}
          rankingPreviewProfile={rankingPreviewProfile}
          rankingStorefrontProfile={rankingStorefrontProfile}
          rankingAdminGlobalProfile={rankingAdminGlobalProfile}
          previewPresetsConfig={previewPresetsConfig}
          storefrontPresetsConfig={storefrontPresetsConfig}
          settingsBusy={settingsBusy}
          settingsFeedback={settingsFeedback}
          rebuildScope={rebuildScope}
          rebuildTargetId={rebuildTargetId}
          rebuildBusy={rebuildBusy}
          rebuildFeedback={rebuildFeedback}
          onSettingsActiveEngineChange={setSettingsActiveEngine}
          onSettingsFallbackEngineChange={setSettingsFallbackEngine}
          onSettingsConfigChange={setSettingsConfig}
          onRankingDefaultProfileChange={setRankingDefaultProfile}
          onRankingPreviewProfileChange={setRankingPreviewProfile}
          onRankingStorefrontProfileChange={setRankingStorefrontProfile}
          onRankingAdminGlobalProfileChange={setRankingAdminGlobalProfile}
          onPreviewPresetsConfigChange={setPreviewPresetsConfig}
          onStorefrontPresetsConfigChange={setStorefrontPresetsConfig}
          onSaveSettings={() => void saveSettings()}
          onScopeChange={setRebuildScope}
          onTargetIdChange={setRebuildTargetId}
          onQueueRebuild={() => void queueRebuild()}
        />
      )}
    </section>
  );
}

function OverviewPanel(props: {
  bootstrap: SearchAdminBootstrap;
  settingsActiveEngine: string;
  settingsFallbackEngine: string;
  settingsConfig: string;
  rankingDefaultProfile: string;
  rankingPreviewProfile: string;
  rankingStorefrontProfile: string;
  rankingAdminGlobalProfile: string;
  previewPresetsConfig: string;
  storefrontPresetsConfig: string;
  settingsBusy: boolean;
  settingsFeedback: string | null;
  rebuildScope: string;
  rebuildTargetId: string;
  rebuildBusy: boolean;
  rebuildFeedback: string | null;
  onSettingsActiveEngineChange: (value: string) => void;
  onSettingsFallbackEngineChange: (value: string) => void;
  onSettingsConfigChange: (value: string) => void;
  onRankingDefaultProfileChange: (value: string) => void;
  onRankingPreviewProfileChange: (value: string) => void;
  onRankingStorefrontProfileChange: (value: string) => void;
  onRankingAdminGlobalProfileChange: (value: string) => void;
  onPreviewPresetsConfigChange: (value: string) => void;
  onStorefrontPresetsConfigChange: (value: string) => void;
  onSaveSettings: () => void;
  onScopeChange: (value: string) => void;
  onTargetIdChange: (value: string) => void;
  onQueueRebuild: () => void;
}): React.JSX.Element {
  const diagnostics = props.bootstrap.searchDiagnostics;
  return (
    <div className='space-y-6'>
      <div className='grid gap-4 md:grid-cols-2 xl:grid-cols-4'>
        <InfoCard
          title='Active engine'
          value={props.bootstrap.searchSettingsPreview.activeEngine}
          detail='Effective search settings for the current tenant.'
        />
        <InfoCard
          title='Fallback engine'
          value={props.bootstrap.searchSettingsPreview.fallbackEngine}
          detail='Used when an external engine is configured but unavailable.'
        />
        <InfoCard
          title='Available engines'
          value={String(props.bootstrap.availableSearchEngines.length)}
          detail='Only installed connectors appear in runtime selection.'
        />
        <InfoCard
          title='Updated at'
          value={props.bootstrap.searchSettingsPreview.updatedAt}
          detail='Timestamp of the effective settings record.'
        />
      </div>
      <div className='grid gap-4 md:grid-cols-2 xl:grid-cols-6 2xl:grid-cols-7'>
        <DiagnosticsCard diagnostics={diagnostics} />
        <InfoCard
          title='Documents'
          value={String(diagnostics.totalDocuments)}
          detail='Total search documents in rustok-search storage.'
        />
        <InfoCard
          title='Public docs'
          value={String(diagnostics.publicDocuments)}
          detail='Published documents visible to storefront search.'
        />
        <InfoCard
          title='Stale docs'
          value={String(diagnostics.staleDocuments)}
          detail='Documents where indexed_at lags behind source updated_at.'
        />
        <InfoCard
          title='Missing docs'
          value={String(diagnostics.missingDocuments)}
          detail='Source rows that should exist in search_documents but do not.'
        />
        <InfoCard
          title='Orphaned docs'
          value={String(diagnostics.orphanedDocuments)}
          detail='Search documents that no longer have a matching source row.'
        />
        <InfoCard
          title='Max lag'
          value={`${diagnostics.maxLagSeconds}s`}
          detail='Worst-case lag between source update and search projection.'
        />
      </div>
      <article className='rounded-3xl border border-zinc-200 bg-white p-6'>
        <h2 className='text-lg font-semibold text-zinc-900'>Engine Settings</h2>
        <p className='mt-2 text-sm text-zinc-600'>
          Save the effective search engine selection and JSON config for the
          current tenant. Only engines installed in the runtime appear here.
        </p>
        <div className='mt-5 grid gap-4 md:grid-cols-2'>
          <Field label='Active engine'>
            <select
              value={props.settingsActiveEngine}
              onChange={(event) =>
                props.onSettingsActiveEngineChange(event.target.value)
              }
              className='w-full rounded-xl border border-zinc-300 px-3 py-2 text-sm'
            >
              {props.bootstrap.availableSearchEngines.map((engine) => (
                <option key={`active-${engine.kind}`} value={engine.kind}>
                  {engine.label} ({engine.kind})
                </option>
              ))}
            </select>
          </Field>
          <Field label='Fallback engine'>
            <select
              value={props.settingsFallbackEngine}
              onChange={(event) =>
                props.onSettingsFallbackEngineChange(event.target.value)
              }
              className='w-full rounded-xl border border-zinc-300 px-3 py-2 text-sm'
            >
              {props.bootstrap.availableSearchEngines.map((engine) => (
                <option key={`fallback-${engine.kind}`} value={engine.kind}>
                  {engine.label} ({engine.kind})
                </option>
              ))}
            </select>
          </Field>
        </div>
        <div className='mt-6 rounded-2xl border border-zinc-200 bg-zinc-50 p-4'>
          <h3 className='text-base font-semibold text-zinc-900'>
            Relevance Settings
          </h3>
          <p className='mt-2 text-sm text-zinc-600'>
            Structured editor for ranking defaults and filter presets. These
            values are merged back into <code>search_settings.config</code> on
            save.
          </p>
          <div className='mt-4 grid gap-4 xl:grid-cols-2'>
            <Field label='Default ranking profile'>
              <select
                value={props.rankingDefaultProfile}
                onChange={(event) =>
                  props.onRankingDefaultProfileChange(event.target.value)
                }
                className='w-full rounded-xl border border-zinc-300 px-3 py-2 text-sm'
              >
                <option value='balanced'>balanced</option>
                <option value='exact'>exact</option>
                <option value='fresh'>fresh</option>
                <option value='catalog'>catalog</option>
                <option value='content'>content</option>
              </select>
            </Field>
            <Field label='Preview ranking profile'>
              <select
                value={props.rankingPreviewProfile}
                onChange={(event) =>
                  props.onRankingPreviewProfileChange(event.target.value)
                }
                className='w-full rounded-xl border border-zinc-300 px-3 py-2 text-sm'
              >
                <option value='balanced'>balanced</option>
                <option value='exact'>exact</option>
                <option value='fresh'>fresh</option>
                <option value='catalog'>catalog</option>
                <option value='content'>content</option>
              </select>
            </Field>
            <Field label='Storefront ranking profile'>
              <select
                value={props.rankingStorefrontProfile}
                onChange={(event) =>
                  props.onRankingStorefrontProfileChange(event.target.value)
                }
                className='w-full rounded-xl border border-zinc-300 px-3 py-2 text-sm'
              >
                <option value='balanced'>balanced</option>
                <option value='exact'>exact</option>
                <option value='fresh'>fresh</option>
                <option value='catalog'>catalog</option>
                <option value='content'>content</option>
              </select>
            </Field>
            <Field label='Admin global ranking profile'>
              <select
                value={props.rankingAdminGlobalProfile}
                onChange={(event) =>
                  props.onRankingAdminGlobalProfileChange(event.target.value)
                }
                className='w-full rounded-xl border border-zinc-300 px-3 py-2 text-sm'
              >
                <option value='balanced'>balanced</option>
                <option value='exact'>exact</option>
                <option value='fresh'>fresh</option>
                <option value='catalog'>catalog</option>
                <option value='content'>content</option>
              </select>
            </Field>
          </div>
          <div className='mt-4 grid gap-4 xl:grid-cols-2'>
            <Field label='Preview filter presets (JSON array)'>
              <textarea
                value={props.previewPresetsConfig}
                onChange={(event) =>
                  props.onPreviewPresetsConfigChange(event.target.value)
                }
                className='min-h-[12rem] w-full rounded-xl border border-zinc-300 px-3 py-2 font-mono text-sm'
              />
              <p className='mt-2 text-xs text-zinc-500'>
                Each item supports: key, label, entity_types, source_modules,
                statuses, ranking_profile.
              </p>
            </Field>
            <Field label='Storefront filter presets (JSON array)'>
              <textarea
                value={props.storefrontPresetsConfig}
                onChange={(event) =>
                  props.onStorefrontPresetsConfigChange(event.target.value)
                }
                className='min-h-[12rem] w-full rounded-xl border border-zinc-300 px-3 py-2 font-mono text-sm'
              />
              <p className='mt-2 text-xs text-zinc-500'>
                Presets drive public tabs and default filter scopes for
                storefront search.
              </p>
            </Field>
          </div>
        </div>
        <Field label='Engine config (JSON)'>
          <textarea
            value={props.settingsConfig}
            onChange={(event) =>
              props.onSettingsConfigChange(event.target.value)
            }
            className='mt-4 min-h-[14rem] w-full rounded-xl border border-zinc-300 px-3 py-2 font-mono text-sm'
          />
        </Field>
        {props.settingsFeedback ? (
          <div className='mt-4 rounded-2xl border border-zinc-200 bg-zinc-50 px-4 py-3 text-sm text-zinc-600'>
            {props.settingsFeedback}
          </div>
        ) : null}
        <div className='mt-4 flex justify-end'>
          <button
            type='button'
            onClick={props.onSaveSettings}
            disabled={props.settingsBusy}
            className='rounded-xl bg-teal-700 px-4 py-2 text-sm font-medium text-white disabled:opacity-50'
          >
            {props.settingsBusy ? 'Saving...' : 'Save Search Settings'}
          </button>
        </div>
      </article>
      <article className='rounded-3xl border border-zinc-200 bg-white p-6'>
        <h2 className='text-lg font-semibold text-zinc-900'>Scoped Rebuild</h2>
        <p className='mt-2 text-sm text-zinc-600'>
          Queue tenant-wide or scoped rebuilds. <code>content</code> and{' '}
          <code>product</code> rebuild the whole slice when target ID is empty,
          or a single entity when target ID is provided.
        </p>
        <div className='mt-5 grid gap-4 md:grid-cols-[14rem_minmax(0,1fr)_auto]'>
          <label className='space-y-2'>
            <span className='text-sm font-medium text-zinc-900'>Scope</span>
            <select
              value={props.rebuildScope}
              onChange={(event) => props.onScopeChange(event.target.value)}
              className='w-full rounded-xl border border-zinc-300 px-3 py-2 text-sm'
            >
              <option value='search'>search</option>
              <option value='content'>content</option>
              <option value='product'>product</option>
            </select>
          </label>
          <label className='space-y-2'>
            <span className='text-sm font-medium text-zinc-900'>
              Target ID (optional)
            </span>
            <input
              value={props.rebuildTargetId}
              onChange={(event) => props.onTargetIdChange(event.target.value)}
              placeholder='UUID for single node/product rebuild'
              className='w-full rounded-xl border border-zinc-300 px-3 py-2 text-sm'
            />
          </label>
          <div className='flex items-end'>
            <button
              type='button'
              onClick={props.onQueueRebuild}
              disabled={props.rebuildBusy}
              className='rounded-xl bg-teal-700 px-4 py-2 text-sm font-medium text-white disabled:opacity-50'
            >
              {props.rebuildBusy ? 'Queueing...' : 'Queue Rebuild'}
            </button>
          </div>
        </div>
        {props.rebuildFeedback ? (
          <div className='mt-4 rounded-2xl border border-zinc-200 bg-zinc-50 px-4 py-3 text-sm text-zinc-600'>
            {props.rebuildFeedback}
          </div>
        ) : null}
      </article>
    </div>
  );
}

function PlaygroundPanel(props: {
  query: string;
  presetKey: string;
  filterPresets: SearchFilterPresetPayload[];
  filterPresetsLoading: boolean;
  filterPresetsError: string | null;
  rankingProfile: string;
  entityTypes: string;
  sourceModules: string;
  statuses: string;
  preview: SearchPreviewPayload | null;
  previewBusy: boolean;
  previewError: string | null;
  token: string | null;
  tenantSlug: string | null;
  graphqlUrl?: string;
  onQueryChange: (value: string) => void;
  onPresetKeyChange: (value: string) => void;
  onRankingProfileChange: (value: string) => void;
  onEntityTypesChange: (value: string) => void;
  onSourceModulesChange: (value: string) => void;
  onStatusesChange: (value: string) => void;
  onSubmit: (event: React.FormEvent<HTMLFormElement>) => void;
}): React.JSX.Element {
  return (
    <div className='grid gap-6 xl:grid-cols-[minmax(0,22rem)_minmax(0,1fr)]'>
      <form
        className='space-y-4 rounded-3xl border border-zinc-200 bg-white p-6'
        onSubmit={props.onSubmit}
      >
        <h2 className='text-lg font-semibold text-zinc-900'>Search Preview</h2>
        <p className='text-sm text-zinc-600'>
          Runs the current PostgreSQL FTS preview path over rustok-search
          documents and lets you compare presets and ranking profiles.
        </p>
        <Field label='Query'>
          <input
            value={props.query}
            onChange={(event) => props.onQueryChange(event.target.value)}
            className='w-full rounded-xl border border-zinc-300 px-3 py-2 text-sm'
          />
        </Field>
        <Field label='Filter preset'>
          <select
            value={props.presetKey}
            onChange={(event) => props.onPresetKeyChange(event.target.value)}
            className='w-full rounded-xl border border-zinc-300 px-3 py-2 text-sm'
          >
            <option value=''>auto</option>
            {props.filterPresets.map((preset) => (
              <option key={preset.key} value={preset.key}>
                {preset.label} ({preset.key})
              </option>
            ))}
          </select>
          <p className='mt-2 text-xs text-zinc-500'>
            {props.filterPresetsLoading
              ? 'Loading tenant presets...'
              : props.filterPresetsError
                ? `Preset load failed: ${props.filterPresetsError}`
                : 'Presets come from search_settings.config.filter_presets.search_preview.'}
          </p>
        </Field>
        <Field label='Ranking profile'>
          <select
            value={props.rankingProfile}
            onChange={(event) =>
              props.onRankingProfileChange(event.target.value)
            }
            className='w-full rounded-xl border border-zinc-300 px-3 py-2 text-sm'
          >
            <option value=''>auto</option>
            <option value='balanced'>balanced</option>
            <option value='exact'>exact</option>
            <option value='fresh'>fresh</option>
            <option value='catalog'>catalog</option>
            <option value='content'>content</option>
          </select>
        </Field>
        <Field label='Entity types (CSV)'>
          <input
            value={props.entityTypes}
            onChange={(event) => props.onEntityTypesChange(event.target.value)}
            className='w-full rounded-xl border border-zinc-300 px-3 py-2 text-sm'
          />
        </Field>
        <Field label='Source modules (CSV)'>
          <input
            value={props.sourceModules}
            onChange={(event) =>
              props.onSourceModulesChange(event.target.value)
            }
            className='w-full rounded-xl border border-zinc-300 px-3 py-2 text-sm'
          />
        </Field>
        <Field label='Statuses (CSV)'>
          <input
            value={props.statuses}
            onChange={(event) => props.onStatusesChange(event.target.value)}
            className='w-full rounded-xl border border-zinc-300 px-3 py-2 text-sm'
          />
        </Field>
        {props.previewError ? (
          <ErrorPanel
            message={`Failed to run search preview: ${props.previewError}`}
          />
        ) : null}
        <button
          type='submit'
          disabled={props.previewBusy}
          className='w-full rounded-xl bg-teal-700 px-4 py-2 text-sm font-medium text-white disabled:opacity-50'
        >
          {props.previewBusy ? 'Running...' : 'Run FTS Preview'}
        </button>
      </form>
      {props.preview ? (
        <PreviewPanel
          payload={props.preview}
          token={props.token}
          tenantSlug={props.tenantSlug}
          graphqlUrl={props.graphqlUrl}
        />
      ) : (
        <EmptyPanel
          title='No preview results yet'
          body='Run a preview query to inspect FTS results, facets, and effective engine output.'
        />
      )}
    </div>
  );
}

function AnalyticsPanel(props: {
  diagnostics: SearchAdminBootstrap['searchDiagnostics'];
  analytics: SearchAnalyticsPayload | null;
  analyticsError: string | null;
  analyticsLoading: boolean;
  laggingDocuments: LaggingSearchDocumentPayload[];
  laggingError: string | null;
  laggingLoading: boolean;
  consistencyIssues: SearchConsistencyIssuePayload[];
  consistencyError: string | null;
  consistencyLoading: boolean;
}): React.JSX.Element {
  return (
    <div className='space-y-6'>
      <div className='grid gap-4 md:grid-cols-2 xl:grid-cols-6 2xl:grid-cols-7'>
        <DiagnosticsCard diagnostics={props.diagnostics} />
        <InfoCard
          title='Lagging docs'
          value={String(props.diagnostics.staleDocuments)}
          detail='Documents where projection timestamps are behind source updates.'
        />
        <InfoCard
          title='Missing docs'
          value={String(props.diagnostics.missingDocuments)}
          detail='Expected projection rows that are absent from search storage.'
        />
        <InfoCard
          title='Orphaned docs'
          value={String(props.diagnostics.orphanedDocuments)}
          detail='Projection rows without a matching content/product source row.'
        />
        <InfoCard
          title='Max lag'
          value={`${props.diagnostics.maxLagSeconds}s`}
          detail='Largest observed lag in seconds.'
        />
        <InfoCard
          title='Newest indexed'
          value={props.diagnostics.newestIndexedAt ?? 'not indexed yet'}
          detail='Most recent index write in rustok-search storage.'
        />
        <InfoCard
          title='Oldest indexed'
          value={props.diagnostics.oldestIndexedAt ?? 'not indexed yet'}
          detail='Oldest surviving indexed document timestamp.'
        />
      </div>
      <article className='rounded-3xl border border-zinc-200 bg-white p-6'>
        <h2 className='text-lg font-semibold text-zinc-900'>
          Search Analytics
        </h2>
        <p className='mt-2 text-sm text-zinc-600'>
          CTR, abandonment, zero-result, and slow-query analysis over the recent
          query log window.
        </p>
        <div className='mt-5'>
          {props.analyticsLoading ? (
            <LoadingPanel label='Loading search analytics...' />
          ) : props.analyticsError ? (
            <ErrorPanel
              message={`Failed to load search analytics: ${props.analyticsError}`}
            />
          ) : props.analytics ? (
            <AnalyticsSummary analytics={props.analytics} />
          ) : (
            <EmptyPanel
              title='No analytics snapshot'
              body='Search analytics will appear here after the first logged queries.'
            />
          )}
        </div>
      </article>
      <article className='rounded-3xl border border-zinc-200 bg-white p-6'>
        <h2 className='text-lg font-semibold text-zinc-900'>
          Lagging Documents
        </h2>
        <p className='mt-2 text-sm text-zinc-600'>
          Raw diagnostics for the most stale documents in search storage.
        </p>
        <div className='mt-5'>
          {props.laggingLoading ? (
            <LoadingPanel label='Loading lagging documents...' />
          ) : props.laggingError ? (
            <ErrorPanel
              message={`Failed to load lagging search diagnostics: ${props.laggingError}`}
            />
          ) : (
            <LaggingTable rows={props.laggingDocuments} />
          )}
        </div>
      </article>
      <article className='rounded-3xl border border-zinc-200 bg-white p-6'>
        <h2 className='text-lg font-semibold text-zinc-900'>
          Consistency Issues
        </h2>
        <p className='mt-2 text-sm text-zinc-600'>
          Missing projections and orphaned search documents compared to current
          content and product source state.
        </p>
        <div className='mt-5'>
          {props.consistencyLoading ? (
            <LoadingPanel label='Loading consistency issues...' />
          ) : props.consistencyError ? (
            <ErrorPanel
              message={`Failed to load search consistency diagnostics: ${props.consistencyError}`}
            />
          ) : (
            <ConsistencyTable rows={props.consistencyIssues} />
          )}
        </div>
      </article>
    </div>
  );
}

function DictionariesPanel(props: {
  token: string | null;
  tenantSlug: string | null;
  graphqlUrl?: string;
}): React.JSX.Element {
  const [snapshot, setSnapshot] =
    React.useState<SearchDictionarySnapshotPayload | null>(null);
  const [loading, setLoading] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [feedback, setFeedback] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState(false);
  const [refreshNonce, setRefreshNonce] = React.useState(0);
  const [synonymTerm, setSynonymTerm] = React.useState('');
  const [synonymValues, setSynonymValues] = React.useState('');
  const [stopWord, setStopWord] = React.useState('');
  const [pinQuery, setPinQuery] = React.useState('');
  const [pinDocumentId, setPinDocumentId] = React.useState('');
  const [pinPosition, setPinPosition] = React.useState('1');

  React.useEffect(() => {
    let cancelled = false;
    if (!props.token || !props.tenantSlug) {
      setSnapshot(null);
      setError(null);
      setLoading(false);
      return () => {
        cancelled = true;
      };
    }

    setLoading(true);
    setError(null);

    void graphqlRequest<{
      searchDictionarySnapshot: SearchDictionarySnapshotPayload;
    }>(SEARCH_DICTIONARY_SNAPSHOT_QUERY, undefined, {
      token: props.token,
      tenantSlug: props.tenantSlug,
      graphqlUrl: props.graphqlUrl
    })
      .then((data) => {
        if (!cancelled) setSnapshot(data.searchDictionarySnapshot);
      })
      .catch((requestError: unknown) => {
        if (!cancelled) setError(errorMessage(requestError));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [props.token, props.tenantSlug, props.graphqlUrl, refreshNonce]);

  async function runMutation<TData>(
    query: string,
    variables: unknown,
    successMessage: string
  ): Promise<void> {
    if (!props.token || !props.tenantSlug) {
      setFeedback('Dictionary actions require token and tenant slug.');
      return;
    }

    setBusy(true);
    setFeedback(null);

    try {
      await graphqlRequest<TData>(query, variables, {
        token: props.token,
        tenantSlug: props.tenantSlug,
        graphqlUrl: props.graphqlUrl
      });
      setFeedback(successMessage);
      setRefreshNonce((value) => value + 1);
    } catch (mutationError: unknown) {
      setFeedback(errorMessage(mutationError));
    } finally {
      setBusy(false);
    }
  }

  async function submitSynonym(
    event: React.FormEvent<HTMLFormElement>
  ): Promise<void> {
    event.preventDefault();
    await runMutation<{ upsertSearchSynonym: { success: boolean } }>(
      UPSERT_SEARCH_SYNONYM_MUTATION,
      {
        input: {
          term: synonymTerm,
          synonyms: parseCsv(synonymValues)
        }
      },
      'Synonym dictionary updated.'
    );
    setSynonymTerm('');
    setSynonymValues('');
  }

  async function submitStopWord(
    event: React.FormEvent<HTMLFormElement>
  ): Promise<void> {
    event.preventDefault();
    await runMutation<{ addSearchStopWord: { success: boolean } }>(
      ADD_SEARCH_STOP_WORD_MUTATION,
      {
        input: {
          value: stopWord
        }
      },
      'Stop-word dictionary updated.'
    );
    setStopWord('');
  }

  async function submitPinRule(
    event: React.FormEvent<HTMLFormElement>
  ): Promise<void> {
    event.preventDefault();
    const normalizedPosition = optionalText(pinPosition);
    if (
      normalizedPosition &&
      Number.isNaN(Number.parseInt(normalizedPosition, 10))
    ) {
      setFeedback('Pinned position must be a positive integer.');
      return;
    }
    await runMutation<{ upsertSearchPinRule: { success: boolean } }>(
      UPSERT_SEARCH_PIN_RULE_MUTATION,
      {
        input: {
          queryText: pinQuery,
          documentId: pinDocumentId,
          pinnedPosition: normalizedPosition
            ? Number.parseInt(normalizedPosition, 10)
            : 1
        }
      },
      'Pinned result rule updated.'
    );
    setPinQuery('');
    setPinDocumentId('');
    setPinPosition('1');
  }

  return (
    <div className='space-y-6'>
      <article className='rounded-3xl border border-zinc-200 bg-white p-6'>
        <h2 className='text-lg font-semibold text-zinc-900'>
          Search Dictionaries
        </h2>
        <p className='mt-2 text-sm text-zinc-600'>
          Tenant-owned stop words, synonyms, and exact-query pin rules. These
          dictionaries now apply to both admin preview and storefront search on
          the shared backend contract.
        </p>
        {feedback ? (
          <div className='mt-4 rounded-2xl border border-zinc-200 bg-zinc-50 px-4 py-3 text-sm text-zinc-600'>
            {feedback}
          </div>
        ) : null}
      </article>

      <div className='grid gap-6 xl:grid-cols-3'>
        <form
          className='space-y-4 rounded-3xl border border-zinc-200 bg-white p-6'
          onSubmit={(event) => void submitSynonym(event)}
        >
          <h3 className='text-base font-semibold text-zinc-900'>Synonyms</h3>
          <p className='text-sm text-zinc-600'>
            Expand exact tokens into equivalent search terms.
          </p>
          <Field label='Canonical term'>
            <input
              value={synonymTerm}
              onChange={(event) => setSynonymTerm(event.target.value)}
              className='w-full rounded-xl border border-zinc-300 px-3 py-2 text-sm'
            />
          </Field>
          <Field label='Synonyms (CSV)'>
            <input
              value={synonymValues}
              onChange={(event) => setSynonymValues(event.target.value)}
              className='w-full rounded-xl border border-zinc-300 px-3 py-2 text-sm'
            />
          </Field>
          <button
            type='submit'
            disabled={busy}
            className='w-full rounded-xl bg-teal-700 px-4 py-2 text-sm font-medium text-white disabled:opacity-50'
          >
            {busy ? 'Saving...' : 'Save Synonym Group'}
          </button>
        </form>

        <form
          className='space-y-4 rounded-3xl border border-zinc-200 bg-white p-6'
          onSubmit={(event) => void submitStopWord(event)}
        >
          <h3 className='text-base font-semibold text-zinc-900'>Stop Words</h3>
          <p className='text-sm text-zinc-600'>
            Remove low-signal tokens before FTS execution.
          </p>
          <Field label='Stop word'>
            <input
              value={stopWord}
              onChange={(event) => setStopWord(event.target.value)}
              className='w-full rounded-xl border border-zinc-300 px-3 py-2 text-sm'
            />
          </Field>
          <button
            type='submit'
            disabled={busy}
            className='w-full rounded-xl bg-teal-700 px-4 py-2 text-sm font-medium text-white disabled:opacity-50'
          >
            {busy ? 'Saving...' : 'Add Stop Word'}
          </button>
        </form>

        <form
          className='space-y-4 rounded-3xl border border-zinc-200 bg-white p-6'
          onSubmit={(event) => void submitPinRule(event)}
        >
          <h3 className='text-base font-semibold text-zinc-900'>
            Pinned Results
          </h3>
          <p className='text-sm text-zinc-600'>
            Pin an existing search document for an exact normalized query.
          </p>
          <Field label='Query text'>
            <input
              value={pinQuery}
              onChange={(event) => setPinQuery(event.target.value)}
              className='w-full rounded-xl border border-zinc-300 px-3 py-2 text-sm'
            />
          </Field>
          <Field label='Document ID'>
            <input
              value={pinDocumentId}
              onChange={(event) => setPinDocumentId(event.target.value)}
              className='w-full rounded-xl border border-zinc-300 px-3 py-2 text-sm'
            />
          </Field>
          <Field label='Pinned position'>
            <input
              type='number'
              min={1}
              value={pinPosition}
              onChange={(event) => setPinPosition(event.target.value)}
              className='w-full rounded-xl border border-zinc-300 px-3 py-2 text-sm'
            />
          </Field>
          <button
            type='submit'
            disabled={busy}
            className='w-full rounded-xl bg-teal-700 px-4 py-2 text-sm font-medium text-white disabled:opacity-50'
          >
            {busy ? 'Saving...' : 'Save Pin Rule'}
          </button>
        </form>
      </div>

      {loading ? (
        <LoadingPanel label='Loading search dictionaries...' />
      ) : error ? (
        <ErrorPanel message={`Failed to load search dictionaries: ${error}`} />
      ) : !snapshot ? (
        <EmptyPanel
          title='No dictionary snapshot'
          body='The GraphQL endpoint returned no dictionary data for the current tenant.'
        />
      ) : (
        <div className='space-y-6'>
          <DictionaryTable
            title='Synonym Groups'
            description='Each group expands all included terms as equivalent tokens.'
            emptyTitle='No synonym groups configured yet'
            emptyBody='Create the first synonym group to expand frequent query variants.'
            headers={['Term', 'Synonyms', 'Updated', 'Actions']}
            rows={snapshot.synonyms.map((row) => ({
              key: row.id,
              cells: [
                <div className='font-medium text-zinc-900' key='term'>
                  {row.term}
                </div>,
                <span key='synonyms'>{row.synonyms.join(', ')}</span>,
                <span key='updated'>{row.updatedAt}</span>,
                <button
                  key='actions'
                  type='button'
                  disabled={busy}
                  onClick={() =>
                    void runMutation<{
                      deleteSearchSynonym: { success: boolean };
                    }>(
                      DELETE_SEARCH_SYNONYM_MUTATION,
                      { input: { synonymId: row.id } },
                      'Synonym removed.'
                    )
                  }
                  className='rounded-xl border border-zinc-300 px-3 py-1 text-xs font-medium text-zinc-900 disabled:opacity-50'
                >
                  Delete
                </button>
              ]
            }))}
          />
          <DictionaryTable
            title='Stop Words'
            description='Terms removed from the effective FTS query.'
            emptyTitle='No stop words configured yet'
            emptyBody='Add stop words to strip low-signal tokens before full-text search.'
            headers={['Value', 'Updated', 'Actions']}
            rows={snapshot.stopWords.map((row) => ({
              key: row.id,
              cells: [
                <div className='font-medium text-zinc-900' key='value'>
                  {row.value}
                </div>,
                <span key='updated'>{row.updatedAt}</span>,
                <button
                  key='actions'
                  type='button'
                  disabled={busy}
                  onClick={() =>
                    void runMutation<{
                      deleteSearchStopWord: { success: boolean };
                    }>(
                      DELETE_SEARCH_STOP_WORD_MUTATION,
                      { input: { stopWordId: row.id } },
                      'Stop word removed.'
                    )
                  }
                  className='rounded-xl border border-zinc-300 px-3 py-1 text-xs font-medium text-zinc-900 disabled:opacity-50'
                >
                  Delete
                </button>
              ]
            }))}
          />
          <DictionaryTable
            title='Pinned Query Rules'
            description='Exact normalized queries that promote specific documents to chosen positions.'
            emptyTitle='No pinned query rules configured yet'
            emptyBody='Add the first pin rule to curate search results for an exact query.'
            headers={['Query', 'Target', 'Position', 'Updated', 'Actions']}
            rows={snapshot.queryRules.map((row) => ({
              key: row.id,
              cells: [
                <div key='query'>
                  <div className='font-medium text-zinc-900'>
                    {row.queryText}
                  </div>
                  <div className='mt-1 text-xs text-zinc-500'>
                    {row.queryNormalized}
                  </div>
                </div>,
                <div key='target'>
                  <div className='font-medium text-zinc-900'>{row.title}</div>
                  <div className='mt-1 text-xs text-zinc-500'>
                    {row.documentId} / {row.sourceModule} / {row.entityType}
                  </div>
                </div>,
                <span key='position'>{String(row.pinnedPosition)}</span>,
                <span key='updated'>{row.updatedAt}</span>,
                <button
                  key='actions'
                  type='button'
                  disabled={busy}
                  onClick={() =>
                    void runMutation<{
                      deleteSearchQueryRule: { success: boolean };
                    }>(
                      DELETE_SEARCH_QUERY_RULE_MUTATION,
                      { input: { queryRuleId: row.id } },
                      'Pinned rule removed.'
                    )
                  }
                  className='rounded-xl border border-zinc-300 px-3 py-1 text-xs font-medium text-zinc-900 disabled:opacity-50'
                >
                  Delete
                </button>
              ]
            }))}
          />
        </div>
      )}
    </div>
  );
}

function AnalyticsSummary({
  analytics
}: {
  analytics: SearchAnalyticsPayload;
}): React.JSX.Element {
  const summary = analytics.summary;

  return (
    <div className='space-y-6'>
      <div className='grid gap-4 md:grid-cols-2 xl:grid-cols-5'>
        <InfoCard
          title='Window'
          value={`${summary.windowDays}d`}
          detail='Rolling analytics lookback window.'
        />
        <InfoCard
          title='Queries'
          value={String(summary.totalQueries)}
          detail='All logged search queries in the current window.'
        />
        <InfoCard
          title='CTR'
          value={`${(summary.clickThroughRate * 100).toFixed(1)}%`}
          detail='Share of eligible successful queries that received at least one click.'
        />
        <InfoCard
          title='Abandonment'
          value={`${(summary.abandonmentRate * 100).toFixed(1)}%`}
          detail='Eligible successful queries with no tracked click.'
        />
        <InfoCard
          title='Zero-result rate'
          value={`${(summary.zeroResultRate * 100).toFixed(1)}%`}
          detail='Share of successful queries that returned no results.'
        />
      </div>
      <div className='grid gap-4 md:grid-cols-2 xl:grid-cols-5'>
        <InfoCard
          title='Avg latency'
          value={`${summary.avgTookMs.toFixed(1)} ms`}
          detail='Average PostgreSQL search execution time.'
        />
        <InfoCard
          title='Slow-query rate'
          value={`${(summary.slowQueryRate * 100).toFixed(1)}%`}
          detail='Share of successful queries at or above the current slow-query threshold.'
        />
        <InfoCard
          title='Total clicks'
          value={String(summary.totalClicks)}
          detail='All tracked result clicks in the current window.'
        />
        <InfoCard
          title='Abandoned queries'
          value={String(summary.abandonmentQueries)}
          detail='Successful queries older than the click evaluation window with no clicks.'
        />
        <InfoCard
          title='Unique queries'
          value={String(summary.uniqueQueries)}
          detail='Distinct normalized queries observed in the window.'
        />
      </div>
      <div className='grid gap-6 xl:grid-cols-2'>
        <article className='rounded-2xl border border-zinc-200 bg-zinc-50 p-4'>
          <h3 className='text-base font-semibold text-zinc-900'>Top Queries</h3>
          <p className='mt-2 text-sm text-zinc-600'>
            Most frequent successful queries across admin and storefront search.
          </p>
          <div className='mt-4'>
            <AnalyticsTable
              rows={analytics.topQueries}
              emptyTitle='No successful queries yet'
              emptyBody='Top queries will appear once search usage is recorded.'
            />
          </div>
        </article>
        <article className='rounded-2xl border border-zinc-200 bg-zinc-50 p-4'>
          <h3 className='text-base font-semibold text-zinc-900'>
            Zero-Result Queries
          </h3>
          <p className='mt-2 text-sm text-zinc-600'>
            Queries that repeatedly return nothing and likely need synonyms,
            redirects, or missing content fixes.
          </p>
          <div className='mt-4'>
            <AnalyticsTable
              rows={analytics.zeroResultQueries}
              emptyTitle='No zero-result queries'
              emptyBody='No empty-result queries were recorded in the current window.'
            />
          </div>
        </article>
      </div>
      <div className='grid gap-6 xl:grid-cols-2'>
        <article className='rounded-2xl border border-zinc-200 bg-zinc-50 p-4'>
          <h3 className='text-base font-semibold text-zinc-900'>
            Slow Queries
          </h3>
          <p className='mt-2 text-sm text-zinc-600'>
            Queries whose average execution time meets or exceeds the current
            slow-query threshold.
          </p>
          <div className='mt-4'>
            <AnalyticsTable
              rows={analytics.slowQueries}
              emptyTitle='No slow queries'
              emptyBody='No slow queries were detected in the current window.'
            />
          </div>
        </article>
        <article className='rounded-2xl border border-zinc-200 bg-zinc-50 p-4'>
          <h3 className='text-base font-semibold text-zinc-900'>
            Low CTR Queries
          </h3>
          <p className='mt-2 text-sm text-zinc-600'>
            Frequent queries whose result sets are not attracting clicks.
          </p>
          <div className='mt-4'>
            <AnalyticsTable
              rows={analytics.lowCtrQueries}
              emptyTitle='No low-CTR queries'
              emptyBody='No low-CTR queries were detected in the current window.'
            />
          </div>
        </article>
        <article className='rounded-2xl border border-zinc-200 bg-zinc-50 p-4'>
          <h3 className='text-base font-semibold text-zinc-900'>
            Abandonment Queries
          </h3>
          <p className='mt-2 text-sm text-zinc-600'>
            Successful queries that tend to end without any click.
          </p>
          <div className='mt-4'>
            <AnalyticsTable
              rows={analytics.abandonmentQueries}
              emptyTitle='No abandonment candidates'
              emptyBody='No abandoned high-volume queries were detected in the current window.'
            />
          </div>
        </article>
      </div>
      <article className='rounded-2xl border border-zinc-200 bg-zinc-50 p-4'>
        <h3 className='text-base font-semibold text-zinc-900'>
          Query Intelligence
        </h3>
        <p className='mt-2 text-sm text-zinc-600'>
          Queries that most likely need synonyms, redirects, pinning, or ranking
          adjustments.
        </p>
        <div className='mt-4'>
          <IntelligenceTable rows={analytics.intelligenceCandidates} />
        </div>
      </article>
    </div>
  );
}

function PreviewPanel({
  payload,
  token,
  tenantSlug,
  graphqlUrl
}: {
  payload: SearchPreviewPayload;
  token: string | null;
  tenantSlug: string | null;
  graphqlUrl?: string;
}): React.JSX.Element {
  return (
    <article className='rounded-3xl border border-zinc-200 bg-white p-6'>
      <h2 className='text-lg font-semibold text-zinc-900'>Preview Results</h2>
      <p className='mt-2 text-sm text-zinc-600'>
        {payload.total} results in {payload.tookMs} ms via {payload.engine} (
        {payload.rankingProfile})
        {payload.presetKey ? ` preset ${payload.presetKey}` : ''}
      </p>
      <div className='mt-5 grid gap-4 lg:grid-cols-3'>
        {payload.facets.map((facet) => (
          <article
            key={facet.name}
            className='rounded-2xl border border-zinc-200 bg-zinc-50 p-4'
          >
            <div className='text-sm font-semibold text-zinc-900 capitalize'>
              {facet.name.replaceAll('_', ' ')}
            </div>
            <div className='mt-3 flex flex-wrap gap-2'>
              {facet.buckets.map((bucket) => (
                <span
                  key={`${facet.name}-${bucket.value}`}
                  className='rounded-full border border-zinc-300 px-3 py-1 text-xs text-zinc-600'
                >
                  {bucket.value} ({bucket.count})
                </span>
              ))}
            </div>
          </article>
        ))}
      </div>
      <div className='mt-6 space-y-3'>
        {payload.items.map((item, index) => (
          <article
            key={item.id}
            className='rounded-2xl border border-zinc-200 bg-zinc-50 p-4'
          >
            <div className='flex flex-wrap gap-2 text-[11px] tracking-[0.16em] text-zinc-500 uppercase'>
              <span>{item.entityType}</span>
              <span>|</span>
              <span>{item.sourceModule}</span>
              <span>|</span>
              <span>score {item.score.toFixed(3)}</span>
            </div>
            <h3 className='mt-2 text-base font-semibold text-zinc-900'>
              {item.title}
            </h3>
            <p className='mt-2 text-sm text-zinc-600'>
              {item.snippet ?? 'No snippet returned.'}
            </p>
            {item.url ? (
              <a
                className='mt-4 inline-flex text-sm font-medium text-teal-700 hover:underline'
                href={item.url}
                onClick={(event) => {
                  if (!payload.queryLogId || !token || !tenantSlug) return;
                  event.preventDefault();
                  void graphqlRequest<{
                    trackSearchClick: { success: boolean };
                  }>(
                    TRACK_SEARCH_CLICK_MUTATION,
                    {
                      input: {
                        queryLogId: payload.queryLogId,
                        documentId: item.id,
                        position: index + 1,
                        href: item.url
                      }
                    },
                    { token, tenantSlug, graphqlUrl }
                  ).finally(() => {
                    window.location.href = item.url!;
                  });
                }}
              >
                Open result
              </a>
            ) : (
              <p className='mt-4 text-xs text-zinc-500'>
                No target URL is available for this result yet.
              </p>
            )}
          </article>
        ))}
      </div>
    </article>
  );
}

function DictionaryTable(props: {
  title: string;
  description: string;
  emptyTitle: string;
  emptyBody: string;
  headers: string[];
  rows: Array<{ key: string; cells: React.ReactNode[] }>;
}): React.JSX.Element {
  return (
    <article className='rounded-3xl border border-zinc-200 bg-white p-6'>
      <h3 className='text-base font-semibold text-zinc-900'>{props.title}</h3>
      <p className='mt-2 text-sm text-zinc-600'>{props.description}</p>
      <div className='mt-5'>
        {props.rows.length ? (
          <div className='overflow-hidden rounded-2xl border border-zinc-200'>
            <table className='w-full text-sm'>
              <thead className='border-b border-zinc-200 bg-zinc-50'>
                <tr>
                  {props.headers.map((header) => (
                    <Th key={header}>{header}</Th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {props.rows.map((row) => (
                  <tr key={row.key} className='border-t border-zinc-100'>
                    {row.cells.map((cell, index) => (
                      <Td key={`${row.key}-${index}`}>{cell}</Td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <EmptyPanel title={props.emptyTitle} body={props.emptyBody} />
        )}
      </div>
    </article>
  );
}

function LaggingTable({
  rows
}: {
  rows: LaggingSearchDocumentPayload[];
}): React.JSX.Element {
  if (!rows.length)
    return (
      <EmptyPanel
        title='Projection is caught up'
        body='No lagging documents detected. Search projection is currently up to date.'
      />
    );
  return (
    <div className='overflow-hidden rounded-2xl border border-zinc-200'>
      <table className='w-full text-sm'>
        <thead className='border-b border-zinc-200 bg-zinc-50'>
          <tr>
            <Th>Title</Th>
            <Th>Type</Th>
            <Th>Locale</Th>
            <Th>Lag</Th>
            <Th>Indexed</Th>
            <Th>Updated</Th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.documentKey} className='border-t border-zinc-100'>
              <Td>
                <div className='font-medium text-zinc-900'>{row.title}</div>
                <div className='mt-1 text-xs text-zinc-500'>
                  {row.documentKey}
                </div>
              </Td>
              <Td>{`${row.sourceModule}/${row.entityType} (${row.status})`}</Td>
              <Td>{row.locale}</Td>
              <Td>
                <span className='rounded-full border border-amber-200 bg-amber-50 px-2.5 py-0.5 text-xs font-semibold text-amber-700'>
                  {row.lagSeconds}s
                </span>
              </Td>
              <Td>{row.indexedAt}</Td>
              <Td>{row.updatedAt}</Td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function ConsistencyTable({
  rows
}: {
  rows: SearchConsistencyIssuePayload[];
}): React.JSX.Element {
  if (!rows.length) {
    return (
      <EmptyPanel
        title='Projection consistency is healthy'
        body='No missing or orphaned search documents detected for the current tenant.'
      />
    );
  }
  return (
    <div className='overflow-hidden rounded-2xl border border-zinc-200'>
      <table className='w-full text-sm'>
        <thead className='border-b border-zinc-200 bg-zinc-50'>
          <tr>
            <Th>Issue</Th>
            <Th>Title</Th>
            <Th>Type</Th>
            <Th>Locale</Th>
            <Th>Updated</Th>
            <Th>Indexed</Th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => {
            const badgeClass =
              row.issueKind === 'missing'
                ? 'border-rose-200 bg-rose-50 text-rose-700'
                : 'border-orange-200 bg-orange-50 text-orange-700';
            return (
              <tr
                key={`${row.issueKind}:${row.documentKey}`}
                className='border-t border-zinc-100'
              >
                <Td>
                  <span
                    className={`rounded-full border px-2.5 py-0.5 text-xs font-semibold ${badgeClass}`}
                  >
                    {row.issueKind}
                  </span>
                </Td>
                <Td>
                  <div className='font-medium text-zinc-900'>{row.title}</div>
                  <div className='mt-1 text-xs text-zinc-500'>
                    {row.documentKey}
                  </div>
                </Td>
                <Td>{`${row.sourceModule}/${row.entityType} (${row.status})`}</Td>
                <Td>{row.locale}</Td>
                <Td>{row.updatedAt}</Td>
                <Td>{row.indexedAt ?? 'not indexed'}</Td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function AnalyticsTable({
  rows,
  emptyTitle,
  emptyBody
}: {
  rows: SearchAnalyticsQueryRowPayload[];
  emptyTitle: string;
  emptyBody: string;
}): React.JSX.Element {
  if (!rows.length) {
    return <EmptyPanel title={emptyTitle} body={emptyBody} />;
  }

  return (
    <div className='overflow-hidden rounded-2xl border border-zinc-200'>
      <table className='w-full text-sm'>
        <thead className='border-b border-zinc-200 bg-white'>
          <tr>
            <Th>Query</Th>
            <Th>Hits</Th>
            <Th>Zero hits</Th>
            <Th>Clicks</Th>
            <Th>CTR</Th>
            <Th>Abandonment</Th>
            <Th>Avg latency</Th>
            <Th>Avg results</Th>
            <Th>Last seen</Th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr
              key={`${row.query}-${row.lastSeenAt}`}
              className='border-t border-zinc-100'
            >
              <Td>
                <div className='font-medium text-zinc-900'>{row.query}</div>
              </Td>
              <Td>{String(row.hits)}</Td>
              <Td>{String(row.zeroResultHits)}</Td>
              <Td>{String(row.clicks)}</Td>
              <Td>{`${(row.clickThroughRate * 100).toFixed(1)}%`}</Td>
              <Td>{`${(row.abandonmentRate * 100).toFixed(1)}%`}</Td>
              <Td>{`${row.avgTookMs.toFixed(1)} ms`}</Td>
              <Td>{row.avgResults.toFixed(1)}</Td>
              <Td>{row.lastSeenAt}</Td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function IntelligenceTable({
  rows
}: {
  rows: SearchAnalyticsInsightRowPayload[];
}): React.JSX.Element {
  if (!rows.length) {
    return (
      <EmptyPanel
        title='No query-intelligence candidates'
        body='No synonym, redirect, or ranking candidates surfaced in the current window.'
      />
    );
  }

  return (
    <div className='overflow-hidden rounded-2xl border border-zinc-200'>
      <table className='w-full text-sm'>
        <thead className='border-b border-zinc-200 bg-white'>
          <tr>
            <Th>Query</Th>
            <Th>Hits</Th>
            <Th>Zero hits</Th>
            <Th>Clicks</Th>
            <Th>CTR</Th>
            <Th>Recommendation</Th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr
              key={`${row.query}-${row.recommendation}`}
              className='border-t border-zinc-100'
            >
              <Td>{row.query}</Td>
              <Td>{String(row.hits)}</Td>
              <Td>{String(row.zeroResultHits)}</Td>
              <Td>{String(row.clicks)}</Td>
              <Td>{`${(row.clickThroughRate * 100).toFixed(1)}%`}</Td>
              <Td>{row.recommendation}</Td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function DiagnosticsCard({
  diagnostics
}: {
  diagnostics: SearchAdminBootstrap['searchDiagnostics'];
}): React.JSX.Element {
  const badgeClass =
    diagnostics.state === 'healthy'
      ? 'border-emerald-200 bg-emerald-50 text-emerald-700'
      : diagnostics.state === 'inconsistent'
        ? 'border-rose-200 bg-rose-50 text-rose-700'
        : diagnostics.state === 'lagging'
          ? 'border-amber-200 bg-amber-50 text-amber-700'
          : 'border-slate-200 bg-slate-50 text-slate-700';
  return (
    <article className='rounded-3xl border border-zinc-200 bg-zinc-50 p-5'>
      <div className='text-xs tracking-[0.2em] text-zinc-500 uppercase'>
        Index state
      </div>
      <div className='mt-3'>
        <span
          className={`rounded-full border px-3 py-1 text-xs font-semibold ${badgeClass}`}
        >
          {diagnostics.state}
        </span>
      </div>
      <p className='mt-3 text-sm text-zinc-600'>
        Newest indexed: {diagnostics.newestIndexedAt ?? 'not indexed yet'}
      </p>
    </article>
  );
}

function InfoCard({
  title,
  value,
  detail
}: {
  title: string;
  value: string;
  detail: string;
}): React.JSX.Element {
  return (
    <article className='rounded-3xl border border-zinc-200 bg-zinc-50 p-5'>
      <div className='text-xs tracking-[0.2em] text-zinc-500 uppercase'>
        {title}
      </div>
      <div className='mt-2 text-lg font-semibold text-zinc-900'>{value}</div>
      <p className='mt-2 text-sm text-zinc-600'>{detail}</p>
    </article>
  );
}

function Field({
  label,
  children
}: {
  label: string;
  children: React.ReactNode;
}): React.JSX.Element {
  return (
    <label className='block space-y-2'>
      <span className='text-sm font-medium text-zinc-900'>{label}</span>
      {children}
    </label>
  );
}

function Th({ children }: { children: React.ReactNode }): React.JSX.Element {
  return (
    <th className='px-4 py-3 text-left text-xs font-semibold tracking-[0.14em] text-zinc-500 uppercase'>
      {children}
    </th>
  );
}

function Td({ children }: { children: React.ReactNode }): React.JSX.Element {
  return (
    <td className='px-4 py-3 align-top text-xs text-zinc-600'>{children}</td>
  );
}

function LoadingPanel({ label }: { label: string }): React.JSX.Element {
  return (
    <div className='rounded-3xl border border-dashed border-zinc-300 bg-zinc-50 p-10 text-center text-sm text-zinc-500'>
      {label}
    </div>
  );
}

function ErrorPanel({ message }: { message: string }): React.JSX.Element {
  return (
    <div className='rounded-2xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700'>
      {message}
    </div>
  );
}

function EmptyPanel({
  title,
  body
}: {
  title: string;
  body: string;
}): React.JSX.Element {
  return (
    <article className='rounded-3xl border border-dashed border-zinc-300 bg-zinc-50 p-10 text-center'>
      <h2 className='text-xl font-semibold text-zinc-900'>{title}</h2>
      <p className='mx-auto mt-3 max-w-3xl text-sm text-zinc-600'>{body}</p>
    </article>
  );
}

export default SearchAdminPage;
