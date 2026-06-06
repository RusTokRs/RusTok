import { GraphqlError, graphqlRequest } from './graphql';

const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL ?? 'http://localhost:5150';

export type SeoTargetCapabilityKind =
  | 'AUTHORING'
  | 'ROUTING'
  | 'BULK'
  | 'SITEMAPS';

export type SeoBulkJobStatusValue =
  | 'queued'
  | 'running'
  | 'completed'
  | 'partial'
  | 'failed';

export type SeoDiagnosticSeverity = 'info' | 'warning' | 'error';

export interface SeoTargetCapabilities {
  authoring: boolean;
  routing: boolean;
  bulk: boolean;
  sitemaps: boolean;
}

export interface SeoTargetRegistryEntry {
  slug: string;
  displayName: string;
  ownerModuleSlug: string;
  capabilities: SeoTargetCapabilities;
}

export interface SeoSitemapFileRecord {
  id: string;
  path: string;
  urlCount: number;
  createdAt: string;
}

export interface SeoSitemapStatusRecord {
  enabled: boolean;
  latestJobId: string | null;
  status: SeoBulkJobStatusValue | null;
  fileCount: number;
  generatedAt: string | null;
  files: SeoSitemapFileRecord[];
}

export interface SeoSitemapJobRecord {
  id: string;
  status: SeoBulkJobStatusValue;
  fileCount: number;
  startedAt: string | null;
  completedAt: string | null;
  lastError: string | null;
  createdAt: string;
  updatedAt: string;
  files: SeoSitemapFileRecord[];
}

export interface SeoBulkArtifactRecord {
  id: string;
  jobId: string;
  kind: string;
  fileName: string;
  mimeType: string;
  createdAt: string;
}

export interface SeoBulkJobRecord {
  id: string;
  operationKind: string;
  status: SeoBulkJobStatusValue;
  targetKind: string;
  locale: string;
  matchedCount: number;
  processedCount: number;
  succeededCount: number;
  failedCount: number;
  artifactCount: number;
  publishAfterWrite: boolean;
  startedAt: string | null;
  completedAt: string | null;
  createdAt: string;
  updatedAt: string;
  lastError: string | null;
  artifacts: SeoBulkArtifactRecord[];
}

export interface SeoDiagnosticCountRecord {
  key: string;
  count: number;
}

export interface SeoDiagnosticIssueRecord {
  code: string;
  severity: SeoDiagnosticSeverity;
  targetKind: string;
  targetId: string;
  targetLabel: string;
  route: string;
  locale: string;
  message: string;
  canonicalUrl: string | null;
  source: string;
}

export interface SeoDiagnosticsSummaryRecord {
  locale: string;
  totalTargets: number;
  readinessScore: number;
  issueCount: number;
  errorCount: number;
  warningCount: number;
  generatedCount: number;
  explicitCount: number;
  fallbackCount: number;
  issueCountsByCode: SeoDiagnosticCountRecord[];
  issueCountsByTargetKind: SeoDiagnosticCountRecord[];
  issues: SeoDiagnosticIssueRecord[];
}

export type SeoIndexReplayMode =
  | 'not_started'
  | 'repair_only'
  | 'replay_requested'
  | 'replaying'
  | 'replay_completed';

export interface SeoIndexCursorRecord {
  targetType: string;
  initialCursorAt: string;
  highWaterMarkAt: string;
  lastRepairCursorAt: string | null;
  replayMode: SeoIndexReplayMode;
  replayRequestedAt: string | null;
  replayCompletedAt: string | null;
}

export interface SeoIndexDeliveryStatusRecord {
  targetType: string | null;
  pendingCount: number;
  sentCount: number;
  retryCount: number;
  failedCount: number;
  deadLetterCount: number;
  cursors: SeoIndexCursorRecord[];
}

export interface SeoIndexRepairReplayInput {
  targetType?: string | null;
  limit?: number;
  replayHistorical?: boolean;
}

export interface SeoIndexRepairReplayResultRecord {
  targetType: string | null;
  limit: number;
  replayMode: SeoIndexReplayMode;
  repairedCount: number;
  replayedCount: number;
  historicalEventsScanned: number;
  replayRunId: string | null;
}

function normalizeSeoIndexTargetType(value?: string | null): string | null {
  if (!value) {
    return null;
  }
  const normalized = value.trim().toLowerCase();
  if (normalized.length === 0) {
    return null;
  }
  if (normalized !== 'content' && normalized !== 'product') {
    throw new GraphqlError(
      'Index target type must be `content` or `product`',
      'BAD_USER_INPUT'
    );
  }
  return normalized;
}

export function formatSeoReplayErrorMessage(error: unknown): string {
  if (error instanceof GraphqlError) {
    if (error.code === 'BAD_USER_INPUT') {
      return `Invalid replay input: ${error.message}`;
    }
    if (error.code === 'PERMISSION_DENIED' || error.code === 'UNAUTHENTICATED') {
      return 'You do not have permission to run SEO replay operations.';
    }
    if (error.code === 'NOT_FOUND') {
      return 'Requested SEO resource was not found for this tenant.';
    }
    return error.message;
  }

  return error instanceof Error ? error.message : 'SEO replay request failed';
}

export interface SeoApiOptions {
  token?: string | null;
  tenantSlug?: string | null;
  graphqlUrl?: string;
  apiBaseUrl?: string;
  locale?: string | null;
  preferRest?: boolean;
}

interface GraphqlLikeErrorRecord {
  message?: string;
  extensions?: {
    code?: string;
  };
}

interface SeoRestErrorEnvelope {
  errors?: GraphqlLikeErrorRecord[];
  error?: {
    code?: string;
    message?: string;
  };
  message?: string;
}

interface SeoTargetsResponse {
  seoTargets: SeoTargetRegistryEntry[];
}

interface SeoTargetsVariables {
  capability?: SeoTargetCapabilityKind | null;
}

interface SeoDiagnosticsResponse {
  seoDiagnostics: SeoDiagnosticsSummaryRecord;
}

interface SeoDiagnosticsVariables {
  locale?: string | null;
}

interface SeoSitemapStatusResponse {
  seoSitemapStatus: SeoSitemapStatusRecord;
}

interface SeoSitemapJobsResponse {
  seoSitemapJobs: SeoSitemapJobRecord[];
}

interface SeoSitemapJobsVariables {
  limit?: number;
}

interface SeoSitemapJobResponse {
  seoSitemapJob: SeoSitemapJobRecord | null;
}

interface SeoSitemapJobVariables {
  jobId: string;
}

interface SeoBulkJobsResponse {
  seoBulkJobs: SeoBulkJobRecord[];
}

interface SeoBulkJobsVariables {
  limit?: number;
}

interface SeoBulkJobResponse {
  seoBulkJob: SeoBulkJobRecord | null;
}

interface SeoBulkJobVariables {
  jobId: string;
}

interface SeoIndexDeliveryStatusResponse {
  seoIndexDeliveryStatus: SeoIndexDeliveryStatusRecord;
}

interface SeoIndexDeliveryStatusVariables {
  targetType?: string | null;
}

interface SeoIndexRepairReplayResponse {
  runSeoIndexRepairReplay: SeoIndexRepairReplayResultRecord;
}

interface SeoIndexRepairReplayVariables {
  input: {
    targetType?: string | null;
    limit: number;
    replayHistorical: boolean;
  };
}

const SEO_TARGETS_QUERY = `
query SeoTargets($capability: SeoTargetCapabilityKind) {
  seoTargets(capability: $capability) {
    slug
    displayName
    ownerModuleSlug
    capabilities {
      authoring
      routing
      bulk
      sitemaps
    }
  }
}
`;

const SEO_DIAGNOSTICS_QUERY = `
query SeoDiagnostics($locale: String) {
  seoDiagnostics(locale: $locale) {
    locale
    totalTargets
    readinessScore
    issueCount
    errorCount
    warningCount
    generatedCount
    explicitCount
    fallbackCount
    issueCountsByCode {
      key
      count
    }
    issueCountsByTargetKind {
      key
      count
    }
    issues {
      code
      severity
      targetKind
      targetId
      targetLabel
      route
      locale
      message
      canonicalUrl
      source
    }
  }
}
`;

const SEO_SITEMAP_STATUS_QUERY = `
query SeoSitemapStatus {
  seoSitemapStatus {
    enabled
    latestJobId
    status
    fileCount
    generatedAt
    files {
      id
      path
      urlCount
      createdAt
    }
  }
}
`;

const SEO_SITEMAP_JOBS_QUERY = `
query SeoSitemapJobs($limit: Int) {
  seoSitemapJobs(limit: $limit) {
    id
    status
    fileCount
    startedAt
    completedAt
    lastError
    createdAt
    updatedAt
    files {
      id
      path
      urlCount
      createdAt
    }
  }
}
`;

const SEO_SITEMAP_JOB_QUERY = `
query SeoSitemapJob($jobId: UUID!) {
  seoSitemapJob(jobId: $jobId) {
    id
    status
    fileCount
    startedAt
    completedAt
    lastError
    createdAt
    updatedAt
    files {
      id
      path
      urlCount
      createdAt
    }
  }
}
`;

const SEO_BULK_JOBS_QUERY = `
query SeoBulkJobs($limit: Int) {
  seoBulkJobs(limit: $limit) {
    id
    operationKind
    status
    targetKind
    locale
    matchedCount
    processedCount
    succeededCount
    failedCount
    artifactCount
    publishAfterWrite
    startedAt
    completedAt
    createdAt
    updatedAt
    lastError
    artifacts {
      id
      jobId
      kind
      fileName
      mimeType
      createdAt
    }
  }
}
`;

const SEO_BULK_JOB_QUERY = `
query SeoBulkJob($jobId: UUID!) {
  seoBulkJob(jobId: $jobId) {
    id
    operationKind
    status
    targetKind
    locale
    matchedCount
    processedCount
    succeededCount
    failedCount
    artifactCount
    publishAfterWrite
    startedAt
    completedAt
    createdAt
    updatedAt
    lastError
    artifacts {
      id
      jobId
      kind
      fileName
      mimeType
      createdAt
    }
  }
}
`;

const SEO_INDEX_DELIVERY_STATUS_QUERY = `
query SeoIndexDeliveryStatus($targetType: String) {
  seoIndexDeliveryStatus(targetType: $targetType) {
    targetType
    pendingCount
    sentCount
    retryCount
    failedCount
    deadLetterCount
    cursors {
      targetType
      initialCursorAt
      highWaterMarkAt
      lastRepairCursorAt
      replayMode
      replayRequestedAt
      replayCompletedAt
    }
  }
}
`;

const RUN_SEO_INDEX_REPAIR_REPLAY_MUTATION = `
mutation RunSeoIndexRepairReplay($input: SeoIndexRepairReplayInput!) {
  runSeoIndexRepairReplay(input: $input) {
    targetType
    limit
    replayMode
    repairedCount
    replayedCount
    historicalEventsScanned
    replayRunId
  }
}
`;

function resolveApiBaseUrl(explicit?: string): string {
  return explicit ?? API_BASE_URL;
}

function toCamelKey(value: string): string {
  return value.replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase());
}

function camelize<T>(value: unknown): T {
  if (Array.isArray(value)) {
    return value.map((item) => camelize(item)) as T;
  }
  if (value && typeof value === 'object') {
    return Object.entries(value as Record<string, unknown>).reduce(
      (acc, [key, current]) => {
        acc[toCamelKey(key)] = camelize(current);
        return acc;
      },
      {} as Record<string, unknown>
    ) as T;
  }
  return value as T;
}

function buildSeoHeaders(options: SeoApiOptions): Record<string, string> {
  const headers: Record<string, string> = {
    Accept: 'application/json'
  };

  if (options.token) {
    headers['Authorization'] = `Bearer ${options.token}`;
  }
  if (options.tenantSlug) {
    headers['X-Tenant-Slug'] = options.tenantSlug;
  }
  if (options.locale) {
    headers['Accept-Language'] = options.locale;
  }

  return headers;
}

function statusCodeToGraphqlCode(status: number): string {
  if (status === 400) return 'BAD_USER_INPUT';
  if (status === 401) return 'UNAUTHENTICATED';
  if (status === 403) return 'PERMISSION_DENIED';
  if (status === 404) return 'NOT_FOUND';
  return 'HTTP_ERROR';
}

function parseSeoRestErrorPayload(payload: unknown): GraphqlLikeErrorRecord | null {
  if (!payload || typeof payload !== 'object') {
    return null;
  }

  const envelope = payload as SeoRestErrorEnvelope;
  const graphqlLike = envelope.errors?.[0];
  if (graphqlLike) {
    return graphqlLike;
  }

  if (envelope.error) {
    return {
      message: envelope.error.message,
      extensions: {
        code: envelope.error.code
      }
    };
  }

  if (envelope.message) {
    return {
      message: envelope.message
    };
  }

  return null;
}

async function buildSeoRestError(response: Response): Promise<GraphqlError> {
  let payload: unknown;

  try {
    payload = await response.json();
  } catch {
    payload = null;
  }

  const parsed = parseSeoRestErrorPayload(payload);
  const code = parsed?.extensions?.code ?? statusCodeToGraphqlCode(response.status);
  const normalizedMessage = parsed?.message?.trim();
  const message =
    normalizedMessage && normalizedMessage.length > 0
      ? normalizedMessage
      : `SEO REST request failed with ${response.status}`;

  return new GraphqlError(message, code);
}

async function fetchSeoRest<T>(
  path: string,
  options: SeoApiOptions,
  query?: Record<string, string | number | undefined | null>
): Promise<T> {
  const url = new URL(path, resolveApiBaseUrl(options.apiBaseUrl));
  if (query) {
    Object.entries(query).forEach(([key, value]) => {
      if (value !== undefined && value !== null && `${value}`.length > 0) {
        url.searchParams.set(key, `${value}`);
      }
    });
  }

  const response = await fetch(url.toString(), {
    method: 'GET',
    headers: buildSeoHeaders(options),
    cache: 'no-store'
  });

  if (!response.ok) {
    throw await buildSeoRestError(response);
  }

  const payload = await response.json();
  return camelize<T>(payload);
}

async function postSeoRest<T>(
  path: string,
  options: SeoApiOptions,
  body: unknown
): Promise<T> {
  const url = new URL(path, resolveApiBaseUrl(options.apiBaseUrl));
  const response = await fetch(url.toString(), {
    method: 'POST',
    headers: {
      ...buildSeoHeaders(options),
      'Content-Type': 'application/json'
    },
    body: JSON.stringify(body),
    cache: 'no-store'
  });

  if (!response.ok) {
    throw await buildSeoRestError(response);
  }

  const payload = await response.json();
  return camelize<T>(payload);
}

function shouldPreferRest(options: SeoApiOptions): boolean {
  return options.preferRest === true;
}

function shouldFallbackToGraphql(error: unknown): boolean {
  if (!(error instanceof GraphqlError)) {
    return true;
  }

  return error.code === 'NOT_FOUND' || error.code === 'HTTP_ERROR';
}

function recalculateDiagnosticCounts(
  issues: SeoDiagnosticIssueRecord[]
): {
  issueCountsByCode: SeoDiagnosticCountRecord[];
  issueCountsByTargetKind: SeoDiagnosticCountRecord[];
  issueCount: number;
  errorCount: number;
  warningCount: number;
} {
  const byCode = new Map<string, number>();
  const byKind = new Map<string, number>();
  let errorCount = 0;
  let warningCount = 0;

  issues.forEach((issue) => {
    byCode.set(issue.code, (byCode.get(issue.code) ?? 0) + 1);
    byKind.set(issue.targetKind, (byKind.get(issue.targetKind) ?? 0) + 1);
    if (issue.severity === 'error') errorCount += 1;
    if (issue.severity === 'warning') warningCount += 1;
  });

  const toSortedList = (source: Map<string, number>) =>
    [...source.entries()]
      .map(([key, count]) => ({ key, count }))
      .sort((left, right) => right.count - left.count || left.key.localeCompare(right.key));

  return {
    issueCountsByCode: toSortedList(byCode),
    issueCountsByTargetKind: toSortedList(byKind),
    issueCount: issues.length,
    errorCount,
    warningCount
  };
}

function applyDiagnosticsFilters(
  summary: SeoDiagnosticsSummaryRecord,
  filters: {
    severity?: SeoDiagnosticSeverity;
    code?: string;
    targetKind?: string;
    limit?: number;
  }
): SeoDiagnosticsSummaryRecord {
  const normalizedCode = filters.code?.trim().toLowerCase();
  const normalizedTargetKind = filters.targetKind?.trim();

  let issues = summary.issues.filter((issue) => {
    if (filters.severity && issue.severity !== filters.severity) {
      return false;
    }
    if (normalizedCode && issue.code.toLowerCase() !== normalizedCode) {
      return false;
    }
    if (normalizedTargetKind && issue.targetKind !== normalizedTargetKind) {
      return false;
    }
    return true;
  });

  if (filters.limit && filters.limit > 0) {
    issues = issues.slice(0, filters.limit);
  }

  const recalculated = recalculateDiagnosticCounts(issues);
  return {
    ...summary,
    issues,
    issueCount: recalculated.issueCount,
    errorCount: recalculated.errorCount,
    warningCount: recalculated.warningCount,
    issueCountsByCode: recalculated.issueCountsByCode,
    issueCountsByTargetKind: recalculated.issueCountsByTargetKind
  };
}

export async function fetchSeoTargets(
  options: SeoApiOptions & {
    capability?: SeoTargetCapabilityKind | null;
  } = {}
): Promise<SeoTargetRegistryEntry[]> {
  const variables =
    options.capability === undefined
      ? undefined
      : { capability: options.capability };

  if (shouldPreferRest(options)) {
    try {
      return await fetchSeoRest<SeoTargetRegistryEntry[]>(
        '/api/seo/targets',
        options,
        options.capability ? { capability: options.capability.toLowerCase() } : undefined
      );
    } catch (error) {
      if (!shouldFallbackToGraphql(error)) {
        throw error;
      }
      // REST parity can be rollout-gated; keep GraphQL fallback.
    }
  }

  const data = await graphqlRequest<SeoTargetsVariables, SeoTargetsResponse>(
    SEO_TARGETS_QUERY,
    variables,
    options.token,
    options.tenantSlug,
    { graphqlUrl: options.graphqlUrl }
  );

  return data.seoTargets;
}

export async function fetchSeoDiagnostics(
  options: SeoApiOptions & {
    locale?: string | null;
    severity?: SeoDiagnosticSeverity;
    code?: string;
    targetKind?: string;
    limit?: number;
  } = {}
): Promise<SeoDiagnosticsSummaryRecord> {
  if (shouldPreferRest(options)) {
    try {
      return await fetchSeoRest<SeoDiagnosticsSummaryRecord>('/api/seo/diagnostics', options, {
        locale: options.locale,
        severity: options.severity,
        code: options.code,
        target_kind: options.targetKind,
        limit: options.limit
      });
    } catch (error) {
      if (!shouldFallbackToGraphql(error)) {
        throw error;
      }
      // REST parity can be rollout-gated; keep GraphQL fallback.
    }
  }

  const variables = options.locale ? { locale: options.locale } : undefined;
  const data = await graphqlRequest<SeoDiagnosticsVariables, SeoDiagnosticsResponse>(
    SEO_DIAGNOSTICS_QUERY,
    variables,
    options.token,
    options.tenantSlug,
    { graphqlUrl: options.graphqlUrl }
  );

  return applyDiagnosticsFilters(data.seoDiagnostics, {
    severity: options.severity,
    code: options.code,
    targetKind: options.targetKind,
    limit: options.limit
  });
}

export async function fetchSeoSitemapStatus(
  options: SeoApiOptions = {}
): Promise<SeoSitemapStatusRecord> {
  if (shouldPreferRest(options)) {
    try {
      return await fetchSeoRest<SeoSitemapStatusRecord>('/api/seo/sitemaps/status', options);
    } catch (error) {
      if (!shouldFallbackToGraphql(error)) {
        throw error;
      }
      // REST parity can be rollout-gated; keep GraphQL fallback.
    }
  }

  const data = await graphqlRequest<undefined, SeoSitemapStatusResponse>(
    SEO_SITEMAP_STATUS_QUERY,
    undefined,
    options.token,
    options.tenantSlug,
    { graphqlUrl: options.graphqlUrl }
  );

  return data.seoSitemapStatus;
}

export async function fetchSeoSitemapJobs(
  options: SeoApiOptions & { limit?: number } = {}
): Promise<SeoSitemapJobRecord[]> {
  const limit = options.limit ?? 20;

  if (shouldPreferRest(options)) {
    try {
      return await fetchSeoRest<SeoSitemapJobRecord[]>('/api/seo/sitemaps/jobs', options, {
        limit
      });
    } catch (error) {
      if (!shouldFallbackToGraphql(error)) {
        throw error;
      }
      // REST parity can be rollout-gated; keep GraphQL fallback.
    }
  }

  const data = await graphqlRequest<SeoSitemapJobsVariables, SeoSitemapJobsResponse>(
    SEO_SITEMAP_JOBS_QUERY,
    { limit },
    options.token,
    options.tenantSlug,
    { graphqlUrl: options.graphqlUrl }
  );

  return data.seoSitemapJobs;
}

export async function fetchSeoSitemapJob(
  jobId: string,
  options: SeoApiOptions = {}
): Promise<SeoSitemapJobRecord | null> {
  if (shouldPreferRest(options)) {
    try {
      return await fetchSeoRest<SeoSitemapJobRecord>(
        `/api/seo/sitemaps/jobs/${jobId}`,
        options
      );
    } catch (error) {
      if (!shouldFallbackToGraphql(error)) {
        throw error;
      }
      // REST parity can be rollout-gated; keep GraphQL fallback.
    }
  }

  const data = await graphqlRequest<SeoSitemapJobVariables, SeoSitemapJobResponse>(
    SEO_SITEMAP_JOB_QUERY,
    { jobId },
    options.token,
    options.tenantSlug,
    { graphqlUrl: options.graphqlUrl }
  );

  return data.seoSitemapJob;
}

export async function fetchSeoBulkJobs(
  options: SeoApiOptions & { limit?: number; status?: SeoBulkJobStatusValue } = {}
): Promise<SeoBulkJobRecord[]> {
  const limit = options.limit ?? 20;

  if (shouldPreferRest(options)) {
    try {
      return await fetchSeoRest<SeoBulkJobRecord[]>('/api/seo/bulk/jobs', options, {
        limit,
        status: options.status
      });
    } catch (error) {
      if (!shouldFallbackToGraphql(error)) {
        throw error;
      }
      // REST parity can be rollout-gated; keep GraphQL fallback.
    }
  }

  const data = await graphqlRequest<SeoBulkJobsVariables, SeoBulkJobsResponse>(
    SEO_BULK_JOBS_QUERY,
    { limit },
    options.token,
    options.tenantSlug,
    { graphqlUrl: options.graphqlUrl }
  );

  return options.status
    ? data.seoBulkJobs.filter((job) => job.status === options.status)
    : data.seoBulkJobs;
}

export async function fetchSeoBulkJob(
  jobId: string,
  options: SeoApiOptions = {}
): Promise<SeoBulkJobRecord | null> {
  if (shouldPreferRest(options)) {
    try {
      return await fetchSeoRest<SeoBulkJobRecord>(`/api/seo/bulk/jobs/${jobId}`, options);
    } catch (error) {
      if (!shouldFallbackToGraphql(error)) {
        throw error;
      }
      // REST parity can be rollout-gated; keep GraphQL fallback.
    }
  }

  const data = await graphqlRequest<SeoBulkJobVariables, SeoBulkJobResponse>(
    SEO_BULK_JOB_QUERY,
    { jobId },
    options.token,
    options.tenantSlug,
    { graphqlUrl: options.graphqlUrl }
  );

  return data.seoBulkJob;
}

export async function fetchSeoIndexDeliveryStatus(
  options: SeoApiOptions & { targetType?: string | null } = {}
): Promise<SeoIndexDeliveryStatusRecord> {
  if (shouldPreferRest(options)) {
    try {
      return await fetchSeoRest<SeoIndexDeliveryStatusRecord>(
        '/api/seo/index/tracking',
        options,
        {
          target_type: options.targetType
        }
      );
    } catch (error) {
      if (!shouldFallbackToGraphql(error)) {
        throw error;
      }
      // REST parity can be rollout-gated; keep GraphQL fallback.
    }
  }

  const variables = options.targetType ? { targetType: options.targetType } : undefined;
  const data = await graphqlRequest<
    SeoIndexDeliveryStatusVariables,
    SeoIndexDeliveryStatusResponse
  >(
    SEO_INDEX_DELIVERY_STATUS_QUERY,
    variables,
    options.token,
    options.tenantSlug,
    { graphqlUrl: options.graphqlUrl }
  );

  return data.seoIndexDeliveryStatus;
}

export async function runSeoIndexRepairReplay(
  input: SeoIndexRepairReplayInput,
  options: SeoApiOptions = {}
): Promise<SeoIndexRepairReplayResultRecord> {
  const normalizedInput = {
    targetType: normalizeSeoIndexTargetType(input.targetType ?? null),
    limit: Math.max(1, Math.min(500, input.limit ?? 100)),
    replayHistorical: input.replayHistorical === true
  };

  if (shouldPreferRest(options)) {
    try {
      return await postSeoRest<SeoIndexRepairReplayResultRecord>(
        '/api/seo/index/repair-replay',
        options,
        {
          target_type: normalizedInput.targetType,
          limit: normalizedInput.limit,
          replay_historical: normalizedInput.replayHistorical
        }
      );
    } catch (error) {
      if (!shouldFallbackToGraphql(error)) {
        throw error;
      }
      // REST parity can be rollout-gated; keep GraphQL fallback.
    }
  }

  const data = await graphqlRequest<
    SeoIndexRepairReplayVariables,
    SeoIndexRepairReplayResponse
  >(
    RUN_SEO_INDEX_REPAIR_REPLAY_MUTATION,
    {
      input: normalizedInput
    },
    options.token,
    options.tenantSlug,
    { graphqlUrl: options.graphqlUrl }
  );

  return data.runSeoIndexRepairReplay;
}
