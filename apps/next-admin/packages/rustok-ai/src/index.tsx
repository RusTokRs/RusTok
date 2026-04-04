'use client';

import React from 'react';

import { graphqlRequest as sharedGraphqlRequest } from '../../../src/shared/api/graphql';

type AiAdminPageProps = {
  token?: string | null;
  tenantSlug?: string | null;
  graphqlUrl?: string;
  section?: 'overview' | 'diagnostics';
};

type Provider = {
  id: string;
  slug: string;
  displayName: string;
  providerKind: string;
  baseUrl: string;
  model: string;
  temperature?: number | null;
  maxTokens?: number | null;
  hasSecret: boolean;
  isActive: boolean;
  capabilities: string[];
  usagePolicy: {
    allowedTaskProfiles: string[];
    deniedTaskProfiles: string[];
    restrictedRoleSlugs: string[];
  };
};

type TaskProfile = {
  id: string;
  slug: string;
  displayName: string;
  description?: string | null;
  targetCapability: string;
  systemPrompt?: string | null;
  allowedProviderProfileIds: string[];
  preferredProviderProfileIds: string[];
  fallbackStrategy: string;
  toolProfileId?: string | null;
  defaultExecutionMode: string;
  isActive: boolean;
};

type ToolProfile = {
  id: string;
  slug: string;
  displayName: string;
  description?: string | null;
  allowedTools: string[];
  deniedTools: string[];
  sensitiveTools: string[];
  isActive: boolean;
};

type SessionSummary = {
  id: string;
  title: string;
  providerProfileId: string;
  taskProfileId?: string | null;
  toolProfileId?: string | null;
  executionMode: string;
  requestedLocale?: string | null;
  resolvedLocale: string;
  status: string;
  latestRunStatus?: string | null;
  pendingApprovals: number;
};

type SessionDetail = {
  session: SessionSummary;
  providerProfile: Provider;
  taskProfile?: TaskProfile | null;
  toolProfile?: ToolProfile | null;
  messages: Array<{
    id: string;
    role: string;
    content?: string | null;
  }>;
  runs: Array<{
    id: string;
    taskProfileId?: string | null;
    status: string;
    model: string;
    executionMode: string;
    executionPath: string;
    requestedLocale?: string | null;
    resolvedLocale: string;
    errorMessage?: string | null;
    decisionTrace: string;
  }>;
  toolTraces: Array<{
    toolName: string;
    status: string;
    durationMs: number;
  }>;
  approvals: Array<{
    id: string;
    toolName: string;
    reason?: string | null;
    status: string;
  }>;
  recentStreamEvents: RunStreamEvent[];
};

type MetricBucket = {
  label: string;
  total: number;
};

type RuntimeMetrics = {
  routerResolutionsTotal: number;
  routerOverridesTotal: number;
  selectedAutoTotal: number;
  selectedDirectTotal: number;
  selectedMcpTotal: number;
  completedRunsTotal: number;
  failedRunsTotal: number;
  waitingApprovalRunsTotal: number;
  localeFallbackTotal: number;
  runLatencyMsTotal: number;
  runLatencySamples: number;
  providerKindTotals: MetricBucket[];
  executionTargetTotals: MetricBucket[];
  taskProfileTotals: MetricBucket[];
  resolvedLocaleTotals: MetricBucket[];
};

type RecentRun = {
  id: string;
  sessionId: string;
  sessionTitle: string;
  providerProfileId: string;
  providerDisplayName: string;
  providerKind: string;
  taskProfileId?: string | null;
  taskProfileSlug?: string | null;
  status: string;
  model: string;
  executionMode: string;
  executionPath: string;
  executionTarget?: string | null;
  requestedLocale?: string | null;
  resolvedLocale: string;
  errorMessage?: string | null;
  startedAt: string;
  completedAt?: string | null;
  updatedAt: string;
  durationMs: number;
};

type RunStreamEvent = {
  sessionId: string;
  runId: string;
  eventKind: 'STARTED' | 'DELTA' | 'COMPLETED' | 'FAILED' | 'WAITING_APPROVAL';
  contentDelta?: string | null;
  accumulatedContent?: string | null;
  errorMessage?: string | null;
  createdAt: string;
};

const BOOTSTRAP_QUERY = `
  query AiBootstrap {
    aiRuntimeMetrics {
      routerResolutionsTotal
      routerOverridesTotal
      selectedAutoTotal
      selectedDirectTotal
      selectedMcpTotal
      completedRunsTotal
      failedRunsTotal
      waitingApprovalRunsTotal
      localeFallbackTotal
      runLatencyMsTotal
      runLatencySamples
      providerKindTotals { label total }
      executionTargetTotals { label total }
      taskProfileTotals { label total }
      resolvedLocaleTotals { label total }
    }
    aiProviderProfiles {
      id
      slug
      displayName
      providerKind
      baseUrl
      model
      temperature
      maxTokens
      hasSecret
      isActive
      capabilities
      usagePolicy { allowedTaskProfiles deniedTaskProfiles restrictedRoleSlugs }
    }
    aiTaskProfiles { id slug displayName description targetCapability systemPrompt allowedProviderProfileIds preferredProviderProfileIds fallbackStrategy toolProfileId defaultExecutionMode isActive }
    aiToolProfiles { id slug displayName description allowedTools deniedTools sensitiveTools isActive }
    aiChatSessions { id title providerProfileId taskProfileId toolProfileId executionMode requestedLocale resolvedLocale status latestRunStatus pendingApprovals }
    aiRecentRuns(limit: 20) {
      id
      sessionId
      sessionTitle
      providerProfileId
      providerDisplayName
      providerKind
      taskProfileId
      taskProfileSlug
      status
      model
      executionMode
      executionPath
      executionTarget
      requestedLocale
      resolvedLocale
      errorMessage
      startedAt
      completedAt
      updatedAt
      durationMs
    }
    aiRecentRunStreamEvents(limit: 20) {
      sessionId
      runId
      eventKind
      contentDelta
      accumulatedContent
      errorMessage
      createdAt
    }
  }
`;

const SESSION_QUERY = `
  query AiSession($id: UUID!) {
    aiChatSession(id: $id) {
      session { id title providerProfileId taskProfileId toolProfileId executionMode requestedLocale resolvedLocale status latestRunStatus pendingApprovals }
      providerProfile {
        id slug displayName providerKind baseUrl model temperature maxTokens hasSecret isActive capabilities
        usagePolicy { allowedTaskProfiles deniedTaskProfiles restrictedRoleSlugs }
      }
      taskProfile { id slug displayName description targetCapability systemPrompt allowedProviderProfileIds preferredProviderProfileIds fallbackStrategy toolProfileId defaultExecutionMode isActive }
      toolProfile { id slug displayName description allowedTools deniedTools sensitiveTools isActive }
      messages { id role content }
      runs { id taskProfileId status model executionMode executionPath requestedLocale resolvedLocale errorMessage decisionTrace }
      toolTraces { toolName status durationMs }
      approvals { id toolName reason status }
    }
    aiRecentRunStreamEvents(sessionId: $id, limit: 20) {
      sessionId
      runId
      eventKind
      contentDelta
      accumulatedContent
      errorMessage
      createdAt
    }
  }
`;

const AI_SESSION_EVENTS_SUBSCRIPTION = `
  subscription AiSessionEvents($sessionId: UUID!) {
    aiSessionEvents(sessionId: $sessionId) {
      sessionId
      runId
      eventKind
      contentDelta
      accumulatedContent
      errorMessage
      createdAt
    }
  }
`;

const CREATE_PROVIDER_MUTATION = `
  mutation CreateAiProviderProfile($input: CreateAiProviderProfileInputGql!) {
    createAiProviderProfile(input: $input) {
      id slug displayName providerKind baseUrl model temperature maxTokens hasSecret isActive capabilities
      usagePolicy { allowedTaskProfiles deniedTaskProfiles restrictedRoleSlugs }
    }
  }
`;

const TEST_PROVIDER_MUTATION = `
  mutation TestAiProviderProfile($id: UUID!) {
    testAiProviderProfile(id: $id) { ok provider model latencyMs message }
  }
`;

const UPDATE_PROVIDER_MUTATION = `
  mutation UpdateAiProviderProfile($id: UUID!, $input: UpdateAiProviderProfileInputGql!) {
    updateAiProviderProfile(id: $id, input: $input) {
      id slug displayName providerKind baseUrl model temperature maxTokens hasSecret isActive capabilities
      usagePolicy { allowedTaskProfiles deniedTaskProfiles restrictedRoleSlugs }
    }
  }
`;

const DEACTIVATE_PROVIDER_MUTATION = `
  mutation DeactivateAiProviderProfile($id: UUID!) {
    deactivateAiProviderProfile(id: $id) {
      id slug displayName providerKind baseUrl model temperature maxTokens hasSecret isActive capabilities
      usagePolicy { allowedTaskProfiles deniedTaskProfiles restrictedRoleSlugs }
    }
  }
`;

const CREATE_TOOL_PROFILE_MUTATION = `
  mutation CreateAiToolProfile($input: CreateAiToolProfileInputGql!) {
    createAiToolProfile(input: $input) { id slug displayName description allowedTools deniedTools sensitiveTools isActive }
  }
`;

const CREATE_TASK_PROFILE_MUTATION = `
  mutation CreateAiTaskProfile($input: CreateAiTaskProfileInputGql!) {
    createAiTaskProfile(input: $input) {
      id slug displayName description targetCapability systemPrompt allowedProviderProfileIds preferredProviderProfileIds fallbackStrategy toolProfileId defaultExecutionMode isActive
    }
  }
`;

const UPDATE_TASK_PROFILE_MUTATION = `
  mutation UpdateAiTaskProfile($id: UUID!, $input: UpdateAiTaskProfileInputGql!) {
    updateAiTaskProfile(id: $id, input: $input) {
      id slug displayName description targetCapability systemPrompt allowedProviderProfileIds preferredProviderProfileIds fallbackStrategy toolProfileId defaultExecutionMode isActive
    }
  }
`;

const UPDATE_TOOL_PROFILE_MUTATION = `
  mutation UpdateAiToolProfile($id: UUID!, $input: UpdateAiToolProfileInputGql!) {
    updateAiToolProfile(id: $id, input: $input) {
      id slug displayName description allowedTools deniedTools sensitiveTools isActive
    }
  }
`;

const START_SESSION_MUTATION = `
  mutation StartAiChatSession($input: StartAiChatSessionInputGql!) {
    startAiChatSession(input: $input) {
      session { session { id title status latestRunStatus pendingApprovals } }
      run { id status }
    }
  }
`;

const RUN_TASK_JOB_MUTATION = `
  mutation RunAiTaskJob($input: RunAiTaskJobInputGql!) {
    runAiTaskJob(input: $input) {
      session { session { id title status latestRunStatus pendingApprovals } }
      run { id status }
    }
  }
`;

const SEND_MESSAGE_MUTATION = `
  mutation SendAiChatMessage($sessionId: UUID!, $content: String!) {
    sendAiChatMessage(sessionId: $sessionId, content: $content) {
      session { session { id title status latestRunStatus pendingApprovals } }
      run { id status }
    }
  }
`;

const RESUME_APPROVAL_MUTATION = `
  mutation ResumeAiApproval($approvalId: UUID!, $input: ResumeAiApprovalInputGql!) {
    resumeAiApproval(approvalId: $approvalId, input: $input) {
      session { session { id title status latestRunStatus pendingApprovals } }
      run { id status }
    }
  }
`;

async function gql<TData, TVars = Record<string, never>>(
  query: string,
  variables: TVars,
  props: AiAdminPageProps
): Promise<TData> {
  return sharedGraphqlRequest<TVars, TData>(
    query,
    variables,
    props.token,
    props.tenantSlug,
    { graphqlUrl: props.graphqlUrl }
  );
}

function resolveGraphqlWsUrl(explicit?: string): string {
  const graphqlUrl =
    explicit ??
    (process.env.NEXT_PUBLIC_API_URL
      ? `${process.env.NEXT_PUBLIC_API_URL}/api/graphql`
      : 'http://localhost:5150/api/graphql');
  if (graphqlUrl.startsWith('https://')) {
    return `${graphqlUrl.replace('https://', 'wss://')}/ws`;
  }
  if (graphqlUrl.startsWith('http://')) {
    return `${graphqlUrl.replace('http://', 'ws://')}/ws`;
  }
  return `${graphqlUrl}/ws`;
}

function getClientLocale(): string | undefined {
  if (typeof document === 'undefined') return undefined;
  const match = document.cookie.match(/rustok-admin-locale=([^;]+)/);
  return match?.[1];
}

export function AiAdminPage(props: AiAdminPageProps) {
  const diagnosticsOnly = props.section === 'diagnostics';
  const [runtimeMetrics, setRuntimeMetrics] = React.useState<RuntimeMetrics | null>(null);
  const [providers, setProviders] = React.useState<Provider[]>([]);
  const [taskProfiles, setTaskProfiles] = React.useState<TaskProfile[]>([]);
  const [toolProfiles, setToolProfiles] = React.useState<ToolProfile[]>([]);
  const [sessions, setSessions] = React.useState<SessionSummary[]>([]);
  const [recentRuns, setRecentRuns] = React.useState<RecentRun[]>([]);
  const [recentStreamEvents, setRecentStreamEvents] = React.useState<RunStreamEvent[]>([]);
  const [selectedSession, setSelectedSession] = React.useState<string | null>(null);
  const [detail, setDetail] = React.useState<SessionDetail | null>(null);
  const [liveStream, setLiveStream] = React.useState<{
    runId: string;
    status: string;
    content: string;
    errorMessage?: string | null;
    connected: boolean;
  } | null>(null);
  const [loading, setLoading] = React.useState(true);
  const [error, setError] = React.useState<string | null>(null);
  const [feedback, setFeedback] = React.useState<string | null>(null);

  const [providerForm, setProviderForm] = React.useState({
    id: '',
    slug: '',
    displayName: '',
    providerKind: 'OPEN_AI_COMPATIBLE',
    baseUrl: 'http://localhost:11434',
    model: 'gpt-4.1-mini',
    apiKeySecret: '',
    temperature: '0.2',
    maxTokens: '1024',
    capabilities: 'TEXT_GENERATION,STRUCTURED_GENERATION,IMAGE_GENERATION,CODE_GENERATION',
    allowedTaskProfiles: '',
    deniedTaskProfiles: '',
    restrictedRoleSlugs: '',
    isActive: true
  });

  const [toolForm, setToolForm] = React.useState({
    id: '',
    slug: '',
    displayName: '',
    description: '',
    allowedTools:
      'list_modules,query_modules,module_details,mcp_health,mcp_whoami',
    deniedTools: '',
    sensitiveTools:
      'alloy_create_script,alloy_update_script,alloy_delete_script,alloy_apply_module_scaffold',
    isActive: true
  });

  const [taskForm, setTaskForm] = React.useState({
    id: '',
    slug: '',
    displayName: '',
    description: '',
    targetCapability: 'TEXT_GENERATION',
    systemPrompt: '',
    allowedProviderProfileIds: '',
    preferredProviderProfileIds: '',
    defaultExecutionMode: 'AUTO',
    isActive: true
  });

  const [sessionForm, setSessionForm] = React.useState({
    title: '',
    providerProfileId: '',
    taskProfileId: '',
    toolProfileId: '',
    locale: '',
    initialMessage: ''
  });

  const [alloyForm, setAlloyForm] = React.useState({
    title: 'Alloy Assist',
    locale: '',
    operation: 'list_scripts',
    scriptId: '',
    scriptName: '',
    scriptSource: '',
    runtimePayloadJson: '',
    assistantPrompt: ''
  });

  const [imageForm, setImageForm] = React.useState({
    title: 'Media Image',
    locale: '',
    prompt: '',
    negativePrompt: '',
    fileName: '',
    mediaTitle: '',
    altText: '',
    caption: '',
    size: '1024x1024',
    assistantPrompt: ''
  });
  const [productForm, setProductForm] = React.useState({
    title: 'Product Copy',
    locale: '',
    productId: '',
    sourceLocale: '',
    sourceTitle: '',
    sourceDescription: '',
    sourceMetaTitle: '',
    sourceMetaDescription: '',
    copyInstructions: '',
    assistantPrompt: ''
  });
  const [blogForm, setBlogForm] = React.useState({
    title: 'Blog Draft',
    locale: '',
    postId: '',
    sourceLocale: '',
    sourceTitle: '',
    sourceBody: '',
    sourceExcerpt: '',
    sourceSeoTitle: '',
    sourceSeoDescription: '',
    tags: '',
    categoryId: '',
    featuredImageUrl: '',
    copyInstructions: '',
    assistantPrompt: ''
  });

  const [reply, setReply] = React.useState('');

  const loadBootstrap = React.useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await gql<{
        aiRuntimeMetrics: RuntimeMetrics;
        aiProviderProfiles: Provider[];
        aiTaskProfiles: TaskProfile[];
        aiToolProfiles: ToolProfile[];
        aiChatSessions: SessionSummary[];
        aiRecentRuns: RecentRun[];
        aiRecentRunStreamEvents: RunStreamEvent[];
      }>(BOOTSTRAP_QUERY, {} as Record<string, never>, props);
      setRuntimeMetrics(data.aiRuntimeMetrics);
      setProviders(data.aiProviderProfiles);
      setTaskProfiles(data.aiTaskProfiles);
      setToolProfiles(data.aiToolProfiles);
      setSessions(data.aiChatSessions);
      setRecentRuns(data.aiRecentRuns);
      setRecentStreamEvents(data.aiRecentRunStreamEvents);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load AI bootstrap');
    } finally {
      setLoading(false);
    }
  }, [props]);

  const resetProviderForm = React.useCallback(() => {
    setProviderForm({
      id: '',
      slug: '',
      displayName: '',
      providerKind: 'OPEN_AI_COMPATIBLE',
      baseUrl: 'http://localhost:11434',
      model: 'gpt-4.1-mini',
      apiKeySecret: '',
      temperature: '0.2',
      maxTokens: '1024',
      capabilities: 'TEXT_GENERATION,STRUCTURED_GENERATION,IMAGE_GENERATION,CODE_GENERATION',
      allowedTaskProfiles: '',
      deniedTaskProfiles: '',
      restrictedRoleSlugs: '',
      isActive: true
    });
  }, []);

  const resetToolForm = React.useCallback(() => {
    setToolForm({
      id: '',
      slug: '',
      displayName: '',
      description: '',
      allowedTools: 'list_modules,query_modules,module_details,mcp_health,mcp_whoami',
      deniedTools: '',
      sensitiveTools:
        'alloy_create_script,alloy_update_script,alloy_delete_script,alloy_apply_module_scaffold',
      isActive: true
    });
  }, []);

  const resetTaskForm = React.useCallback(() => {
    setTaskForm({
      id: '',
      slug: '',
      displayName: '',
      description: '',
      targetCapability: 'TEXT_GENERATION',
      systemPrompt: '',
      allowedProviderProfileIds: '',
      preferredProviderProfileIds: '',
      defaultExecutionMode: 'AUTO',
      isActive: true
    });
  }, []);

  const loadSession = React.useCallback(
    async (sessionId: string) => {
      setSelectedSession(sessionId);
      try {
        const data = await gql<
          { aiChatSession: Omit<SessionDetail, 'recentStreamEvents'> | null; aiRecentRunStreamEvents: RunStreamEvent[] },
          { id: string }
        >(
          SESSION_QUERY,
          { id: sessionId },
          props
        );
        setDetail(
          data.aiChatSession
            ? {
                ...data.aiChatSession,
                recentStreamEvents: data.aiRecentRunStreamEvents
              }
            : null
        );
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load session');
      }
    },
    [props]
  );

  React.useEffect(() => {
    void loadBootstrap();
  }, [loadBootstrap]);

  React.useEffect(() => {
    if (!selectedSession || typeof WebSocket === 'undefined') {
      setLiveStream(null);
      return;
    }

    const ws = new WebSocket(resolveGraphqlWsUrl(props.graphqlUrl), 'graphql-transport-ws');
    const subscriptionId = `ai-session-${selectedSession}`;
    let subscribed = false;

    ws.onopen = () => {
      ws.send(
        JSON.stringify({
          type: 'connection_init',
          payload: {
            token: props.token ?? undefined,
            tenantSlug: props.tenantSlug ?? undefined,
            locale: getClientLocale()
          }
        })
      );
    };

    ws.onmessage = (event) => {
      const payload = JSON.parse(String(event.data)) as {
        type: string;
        payload?: {
          data?: { aiSessionEvents?: RunStreamEvent };
          errors?: Array<{ message: string }>;
        };
      };

      if (payload.type === 'connection_ack' && !subscribed) {
        subscribed = true;
        ws.send(
          JSON.stringify({
            id: subscriptionId,
            type: 'subscribe',
            payload: {
              query: AI_SESSION_EVENTS_SUBSCRIPTION,
              variables: { sessionId: selectedSession }
            }
          })
        );
        return;
      }

      const streamEvent = payload.payload?.data?.aiSessionEvents;
      if (payload.type === 'next' && streamEvent) {
        setLiveStream((current) => ({
          runId: streamEvent.runId,
          status: streamEvent.eventKind.toLowerCase(),
          content: streamEvent.accumulatedContent ?? current?.content ?? '',
          errorMessage: streamEvent.errorMessage,
          connected: true
        }));
        if (
          streamEvent.eventKind === 'COMPLETED' ||
          streamEvent.eventKind === 'FAILED' ||
          streamEvent.eventKind === 'WAITING_APPROVAL'
        ) {
          void loadSession(selectedSession);
        }
        return;
      }

      if (payload.type === 'error' || payload.payload?.errors?.length) {
        setLiveStream((current) =>
          current
            ? {
                ...current,
                connected: false,
                status: 'failed',
                errorMessage:
                  payload.payload?.errors?.[0]?.message ?? 'AI stream subscription failed'
              }
            : current
        );
      }
    };

    ws.onerror = () => {
      setLiveStream((current) => (current ? { ...current, connected: false } : current));
    };

    ws.onclose = () => {
      setLiveStream((current) => (current ? { ...current, connected: false } : current));
    };

    return () => {
      ws.close();
      setLiveStream(null);
    };
  }, [loadSession, props.graphqlUrl, props.tenantSlug, props.token, selectedSession]);

  return (
    <div className='space-y-6'>
      <header className='rounded-2xl border border-border bg-card p-6 shadow-sm'>
        <div className='space-y-2'>
          <span className='inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground'>
            capability
          </span>
          <h1 className='text-2xl font-semibold text-card-foreground'>AI Control Plane</h1>
          <p className='max-w-3xl text-sm text-muted-foreground'>
            Provider profiles, tool profiles, operator chat sessions, tool traces and approval gates.
          </p>
        </div>
        <div className='mt-4 flex flex-wrap gap-2 text-sm'>
          <a
            className={
              diagnosticsOnly
                ? 'rounded-full border border-border px-3 py-1.5 text-muted-foreground'
                : 'rounded-full border border-primary bg-primary/10 px-3 py-1.5 font-medium text-primary'
            }
            href='/dashboard/ai'
          >
            Overview
          </a>
          <a
            className={
              diagnosticsOnly
                ? 'rounded-full border border-primary bg-primary/10 px-3 py-1.5 font-medium text-primary'
                : 'rounded-full border border-border px-3 py-1.5 text-muted-foreground'
            }
            href='/dashboard/ai/diagnostics'
          >
            Diagnostics
          </a>
        </div>
      </header>

      {feedback ? (
        <div className='rounded-lg border border-emerald-300 bg-emerald-50 px-4 py-3 text-sm text-emerald-700'>
          {feedback}
        </div>
      ) : null}
      {error ? (
        <div className='rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive'>
          {error}
        </div>
      ) : null}

      {loading ? (
        <div className='h-32 animate-pulse rounded-2xl bg-muted' />
      ) : (
        <div className={diagnosticsOnly ? 'grid gap-6 xl:grid-cols-[1fr_1.5fr]' : 'grid gap-6 xl:grid-cols-[1.1fr_1fr_1.5fr]'}>
          {!diagnosticsOnly ? <div className='space-y-6'>
            <Card title='Providers'>
              <form
                className='space-y-3'
                onSubmit={async (event) => {
                  event.preventDefault();
                  setError(null);
                  const created = await gql<
                    { createAiProviderProfile: Provider },
                    { input: Record<string, unknown> }
                  >(
                    CREATE_PROVIDER_MUTATION,
                    {
                      input: {
                        slug: providerForm.slug,
                        displayName: providerForm.displayName,
                        providerKind: providerForm.providerKind,
                        baseUrl: providerForm.baseUrl,
                        model: providerForm.model,
                        apiKeySecret: providerForm.apiKeySecret || null,
                        temperature: Number(providerForm.temperature),
                        maxTokens: Number(providerForm.maxTokens),
                        capabilities: splitCsv(providerForm.capabilities),
                        usagePolicy: {
                          allowedTaskProfiles: splitCsv(providerForm.allowedTaskProfiles),
                          deniedTaskProfiles: splitCsv(providerForm.deniedTaskProfiles),
                          restrictedRoleSlugs: splitCsv(providerForm.restrictedRoleSlugs)
                        },
                        metadata: '{}'
                      }
                    },
                    props
                  ).catch((err: Error) => {
                    setError(err.message);
                    return null;
                  });
                  if (!created) return;
                  setFeedback(`Provider \`${created.createAiProviderProfile.slug}\` created.`);
                  setSessionForm((current) => ({
                    ...current,
                    providerProfileId: created.createAiProviderProfile.id
                  }));
                  resetProviderForm();
                  await loadBootstrap();
                }}
              >
                <Input label='Slug' value={providerForm.slug} onChange={(slug) => setProviderForm((current) => ({ ...current, slug }))} />
                <Input label='Display name' value={providerForm.displayName} onChange={(displayName) => setProviderForm((current) => ({ ...current, displayName }))} />
                <Input label='Provider kind' value={providerForm.providerKind} onChange={(providerKind) => setProviderForm((current) => ({ ...current, providerKind }))} />
                <Input label='Base URL' value={providerForm.baseUrl} onChange={(baseUrl) => setProviderForm((current) => ({ ...current, baseUrl }))} />
                <Input label='Model' value={providerForm.model} onChange={(model) => setProviderForm((current) => ({ ...current, model }))} />
                <Input label='API key' value={providerForm.apiKeySecret} onChange={(apiKeySecret) => setProviderForm((current) => ({ ...current, apiKeySecret }))} />
                <Input label='Temperature' value={providerForm.temperature} onChange={(temperature) => setProviderForm((current) => ({ ...current, temperature }))} />
                <Input label='Max tokens' value={providerForm.maxTokens} onChange={(maxTokens) => setProviderForm((current) => ({ ...current, maxTokens }))} />
                <Input label='Capabilities (csv)' value={providerForm.capabilities} onChange={(capabilities) => setProviderForm((current) => ({ ...current, capabilities }))} />
                <Input label='Allowed tasks (csv)' value={providerForm.allowedTaskProfiles} onChange={(allowedTaskProfiles) => setProviderForm((current) => ({ ...current, allowedTaskProfiles }))} />
                <Input label='Denied tasks (csv)' value={providerForm.deniedTaskProfiles} onChange={(deniedTaskProfiles) => setProviderForm((current) => ({ ...current, deniedTaskProfiles }))} />
                <Input label='Restricted roles (csv)' value={providerForm.restrictedRoleSlugs} onChange={(restrictedRoleSlugs) => setProviderForm((current) => ({ ...current, restrictedRoleSlugs }))} />
                <label className='flex items-center gap-2 text-sm text-muted-foreground'>
                  <input
                    checked={providerForm.isActive}
                    onChange={(event) =>
                      setProviderForm((current) => ({
                        ...current,
                        isActive: event.target.checked
                      }))
                    }
                    type='checkbox'
                  />
                  Active
                </label>
                <div className='flex flex-wrap gap-2'>
                  <button className='rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground' type='submit'>
                    Create provider
                  </button>
                  <button
                    className='rounded-lg border border-border px-4 py-2 text-sm font-medium'
                    onClick={async () => {
                      if (!providerForm.id) {
                        setError('Select a provider before updating it.');
                        return;
                      }
                      const updated = await gql<
                        { updateAiProviderProfile: Provider },
                        { id: string; input: Record<string, unknown> }
                      >(
                        UPDATE_PROVIDER_MUTATION,
                        {
                          id: providerForm.id,
                          input: {
                            displayName: providerForm.displayName,
                            baseUrl: providerForm.baseUrl,
                            model: providerForm.model,
                            temperature: Number(providerForm.temperature),
                            maxTokens: Number(providerForm.maxTokens),
                            capabilities: splitCsv(providerForm.capabilities),
                            usagePolicy: {
                              allowedTaskProfiles: splitCsv(providerForm.allowedTaskProfiles),
                              deniedTaskProfiles: splitCsv(providerForm.deniedTaskProfiles),
                              restrictedRoleSlugs: splitCsv(providerForm.restrictedRoleSlugs)
                            },
                            metadata: '{}',
                            isActive: providerForm.isActive
                          }
                        },
                        props
                      ).catch((err: Error) => {
                        setError(err.message);
                        return null;
                      });
                      if (!updated) return;
                      setFeedback(`Provider \`${updated.updateAiProviderProfile.slug}\` updated.`);
                      await loadBootstrap();
                    }}
                    type='button'
                  >
                    Update selected
                  </button>
                  <button
                    className='rounded-lg border border-border px-4 py-2 text-sm font-medium'
                    onClick={async () => {
                      if (!providerForm.id) {
                        setError('Select a provider before testing it.');
                        return;
                      }
                      const result = await gql<
                        { testAiProviderProfile: { message: string } },
                        { id: string }
                      >(TEST_PROVIDER_MUTATION, { id: providerForm.id }, props).catch((err: Error) => {
                        setError(err.message);
                        return null;
                      });
                      if (result) setFeedback(result.testAiProviderProfile.message);
                    }}
                    type='button'
                  >
                    Test selected
                  </button>
                  <button
                    className='rounded-lg border border-destructive/40 px-4 py-2 text-sm font-medium text-destructive'
                    onClick={async () => {
                      if (!providerForm.id) {
                        setError('Select a provider before deactivating it.');
                        return;
                      }
                      const deactivated = await gql<
                        { deactivateAiProviderProfile: Provider },
                        { id: string }
                      >(DEACTIVATE_PROVIDER_MUTATION, { id: providerForm.id }, props).catch((err: Error) => {
                        setError(err.message);
                        return null;
                      });
                      if (!deactivated) return;
                      setFeedback(
                        `Provider \`${deactivated.deactivateAiProviderProfile.slug}\` deactivated.`
                      );
                      setProviderForm((current) => ({ ...current, isActive: false }));
                      await loadBootstrap();
                    }}
                    type='button'
                  >
                    Deactivate
                  </button>
                  <button
                    className='rounded-lg border border-border px-4 py-2 text-sm font-medium'
                    onClick={() => resetProviderForm()}
                    type='button'
                  >
                    Reset
                  </button>
                </div>
              </form>
              <div className='mt-4 space-y-2'>
                {providers.map((provider) => (
                  <button
                    key={provider.id}
                    className='w-full rounded-lg border border-border px-3 py-3 text-left text-sm hover:bg-muted'
                    onClick={() => {
                      setSessionForm((current) => ({ ...current, providerProfileId: provider.id }));
                      setProviderForm({
                        id: provider.id,
                        slug: provider.slug,
                        displayName: provider.displayName,
                        providerKind: provider.providerKind,
                        baseUrl: provider.baseUrl,
                        model: provider.model,
                        apiKeySecret: '',
                        temperature:
                          provider.temperature !== null && provider.temperature !== undefined
                            ? String(provider.temperature)
                            : '',
                        maxTokens:
                          provider.maxTokens !== null && provider.maxTokens !== undefined
                            ? String(provider.maxTokens)
                            : '',
                        capabilities: provider.capabilities.join(','),
                        allowedTaskProfiles:
                          provider.usagePolicy.allowedTaskProfiles.join(','),
                        deniedTaskProfiles:
                          provider.usagePolicy.deniedTaskProfiles.join(','),
                        restrictedRoleSlugs:
                          provider.usagePolicy.restrictedRoleSlugs.join(','),
                        isActive: provider.isActive
                      });
                    }}
                    type='button'
                  >
                    <div className='font-medium'>{provider.displayName}</div>
                    <div className='text-muted-foreground'>
                      {provider.providerKind} · {provider.model} · {provider.capabilities.length} capabilities · {provider.isActive ? 'active' : 'inactive'}
                    </div>
                  </button>
                ))}
              </div>
            </Card>

            <Card title='Tool Profiles'>
              <form
                className='space-y-3'
                onSubmit={async (event) => {
                  event.preventDefault();
                  const created = await gql<
                    { createAiToolProfile: ToolProfile },
                    { input: Record<string, unknown> }
                  >(
                    CREATE_TOOL_PROFILE_MUTATION,
                    {
                      input: {
                        slug: toolForm.slug,
                        displayName: toolForm.displayName,
                        description: toolForm.description || null,
                        allowedTools: splitCsv(toolForm.allowedTools),
                        deniedTools: splitCsv(toolForm.deniedTools),
                        sensitiveTools: splitCsv(toolForm.sensitiveTools),
                        metadata: '{}'
                      }
                    },
                    props
                  ).catch((err: Error) => {
                    setError(err.message);
                    return null;
                  });
                  if (!created) return;
                  setFeedback(`Tool profile \`${created.createAiToolProfile.slug}\` created.`);
                  setSessionForm((current) => ({
                    ...current,
                    toolProfileId: created.createAiToolProfile.id
                  }));
                  resetToolForm();
                  await loadBootstrap();
                }}
              >
                <Input label='Slug' value={toolForm.slug} onChange={(slug) => setToolForm((current) => ({ ...current, slug }))} />
                <Input label='Display name' value={toolForm.displayName} onChange={(displayName) => setToolForm((current) => ({ ...current, displayName }))} />
                <Input label='Description' value={toolForm.description} onChange={(description) => setToolForm((current) => ({ ...current, description }))} />
                <Input label='Allowed tools (csv)' value={toolForm.allowedTools} onChange={(allowedTools) => setToolForm((current) => ({ ...current, allowedTools }))} />
                <Input label='Denied tools (csv)' value={toolForm.deniedTools} onChange={(deniedTools) => setToolForm((current) => ({ ...current, deniedTools }))} />
                <Input label='Sensitive tools (csv)' value={toolForm.sensitiveTools} onChange={(sensitiveTools) => setToolForm((current) => ({ ...current, sensitiveTools }))} />
                <label className='flex items-center gap-2 text-sm text-muted-foreground'>
                  <input
                    checked={toolForm.isActive}
                    onChange={(event) =>
                      setToolForm((current) => ({
                        ...current,
                        isActive: event.target.checked
                      }))
                    }
                    type='checkbox'
                  />
                  Active
                </label>
                <div className='flex flex-wrap gap-2'>
                  <button className='rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground' type='submit'>
                    Create tool profile
                  </button>
                  <button
                    className='rounded-lg border border-border px-4 py-2 text-sm font-medium'
                    onClick={async () => {
                      if (!toolForm.id) {
                        setError('Select a tool profile before updating it.');
                        return;
                      }
                      const updated = await gql<
                        { updateAiToolProfile: ToolProfile },
                        { id: string; input: Record<string, unknown> }
                      >(
                        UPDATE_TOOL_PROFILE_MUTATION,
                        {
                          id: toolForm.id,
                          input: {
                            displayName: toolForm.displayName,
                            description: toolForm.description || null,
                            allowedTools: splitCsv(toolForm.allowedTools),
                            deniedTools: splitCsv(toolForm.deniedTools),
                            sensitiveTools: splitCsv(toolForm.sensitiveTools),
                            metadata: '{}',
                            isActive: toolForm.isActive
                          }
                        },
                        props
                      ).catch((err: Error) => {
                        setError(err.message);
                        return null;
                      });
                      if (!updated) return;
                      setFeedback(`Tool profile \`${updated.updateAiToolProfile.slug}\` updated.`);
                      await loadBootstrap();
                    }}
                    type='button'
                  >
                    Update selected
                  </button>
                  <button
                    className='rounded-lg border border-border px-4 py-2 text-sm font-medium'
                    onClick={() => resetToolForm()}
                    type='button'
                  >
                    Reset
                  </button>
                </div>
              </form>
              <div className='mt-4 space-y-2'>
                {toolProfiles.map((profile) => (
                  <button
                    key={profile.id}
                    className='w-full rounded-lg border border-border px-3 py-3 text-left text-sm hover:bg-muted'
                    onClick={() => {
                      setSessionForm((current) => ({ ...current, toolProfileId: profile.id }));
                      setToolForm({
                        id: profile.id,
                        slug: profile.slug,
                        displayName: profile.displayName,
                        description: profile.description ?? '',
                        allowedTools: profile.allowedTools.join(','),
                        deniedTools: profile.deniedTools.join(','),
                        sensitiveTools: profile.sensitiveTools.join(','),
                        isActive: profile.isActive
                      });
                    }}
                    type='button'
                  >
                    <div className='font-medium'>{profile.displayName}</div>
                    <div className='text-muted-foreground'>
                      allowed: {profile.allowedTools.length} · sensitive: {profile.sensitiveTools.length} · {profile.isActive ? 'active' : 'inactive'}
                    </div>
                  </button>
                ))}
              </div>
            </Card>

            <Card title='Task Profiles'>
              <form
                className='space-y-3'
                onSubmit={async (event) => {
                  event.preventDefault();
                  const created = await gql<
                    { createAiTaskProfile: TaskProfile },
                    { input: Record<string, unknown> }
                  >(
                    CREATE_TASK_PROFILE_MUTATION,
                    {
                      input: {
                        slug: taskForm.slug,
                        displayName: taskForm.displayName,
                        description: taskForm.description || null,
                        targetCapability: taskForm.targetCapability,
                        systemPrompt: taskForm.systemPrompt || null,
                        allowedProviderProfileIds: splitCsv(taskForm.allowedProviderProfileIds),
                        preferredProviderProfileIds: splitCsv(taskForm.preferredProviderProfileIds),
                        fallbackStrategy: 'ordered',
                        toolProfileId: sessionForm.toolProfileId || null,
                        defaultExecutionMode: taskForm.defaultExecutionMode,
                        metadata: '{}'
                      }
                    },
                    props
                  ).catch((err: Error) => {
                    setError(err.message);
                    return null;
                  });
                  if (!created) return;
                  setFeedback(`Task profile \`${created.createAiTaskProfile.slug}\` created.`);
                  setSessionForm((current) => ({
                    ...current,
                    taskProfileId: created.createAiTaskProfile.id
                  }));
                  resetTaskForm();
                  await loadBootstrap();
                }}
              >
                <Input label='Slug' value={taskForm.slug} onChange={(slug) => setTaskForm((current) => ({ ...current, slug }))} />
                <Input label='Display name' value={taskForm.displayName} onChange={(displayName) => setTaskForm((current) => ({ ...current, displayName }))} />
                <Input label='Description' value={taskForm.description} onChange={(description) => setTaskForm((current) => ({ ...current, description }))} />
                <Input label='Capability' value={taskForm.targetCapability} onChange={(targetCapability) => setTaskForm((current) => ({ ...current, targetCapability }))} />
                <Input label='System prompt' value={taskForm.systemPrompt} onChange={(systemPrompt) => setTaskForm((current) => ({ ...current, systemPrompt }))} />
                <Input label='Allowed providers (csv)' value={taskForm.allowedProviderProfileIds} onChange={(allowedProviderProfileIds) => setTaskForm((current) => ({ ...current, allowedProviderProfileIds }))} />
                <Input label='Preferred providers (csv)' value={taskForm.preferredProviderProfileIds} onChange={(preferredProviderProfileIds) => setTaskForm((current) => ({ ...current, preferredProviderProfileIds }))} />
                <Input label='Execution mode' value={taskForm.defaultExecutionMode} onChange={(defaultExecutionMode) => setTaskForm((current) => ({ ...current, defaultExecutionMode }))} />
                <label className='flex items-center gap-2 text-sm text-muted-foreground'>
                  <input
                    checked={taskForm.isActive}
                    onChange={(event) =>
                      setTaskForm((current) => ({
                        ...current,
                        isActive: event.target.checked
                      }))
                    }
                    type='checkbox'
                  />
                  Active
                </label>
                <div className='flex flex-wrap gap-2'>
                  <button className='rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground' type='submit'>
                    Create task profile
                  </button>
                  <button
                    className='rounded-lg border border-border px-4 py-2 text-sm font-medium'
                    onClick={async () => {
                      if (!taskForm.id) {
                        setError('Select a task profile before updating it.');
                        return;
                      }
                      const updated = await gql<
                        { updateAiTaskProfile: TaskProfile },
                        { id: string; input: Record<string, unknown> }
                      >(
                        UPDATE_TASK_PROFILE_MUTATION,
                        {
                          id: taskForm.id,
                          input: {
                            displayName: taskForm.displayName,
                            description: taskForm.description || null,
                            targetCapability: taskForm.targetCapability,
                            systemPrompt: taskForm.systemPrompt || null,
                            allowedProviderProfileIds: splitCsv(taskForm.allowedProviderProfileIds),
                            preferredProviderProfileIds: splitCsv(taskForm.preferredProviderProfileIds),
                            fallbackStrategy: 'ordered',
                            toolProfileId: sessionForm.toolProfileId || null,
                            defaultExecutionMode: taskForm.defaultExecutionMode,
                            isActive: taskForm.isActive,
                            metadata: '{}'
                          }
                        },
                        props
                      ).catch((err: Error) => {
                        setError(err.message);
                        return null;
                      });
                      if (!updated) return;
                      setFeedback(`Task profile \`${updated.updateAiTaskProfile.slug}\` updated.`);
                      await loadBootstrap();
                    }}
                    type='button'
                  >
                    Update selected
                  </button>
                  <button
                    className='rounded-lg border border-border px-4 py-2 text-sm font-medium'
                    onClick={() => resetTaskForm()}
                    type='button'
                  >
                    Reset
                  </button>
                </div>
              </form>
              <div className='mt-4 space-y-2'>
                {taskProfiles.map((profile) => (
                  <button
                    key={profile.id}
                    className='w-full rounded-lg border border-border px-3 py-3 text-left text-sm hover:bg-muted'
                    onClick={() => {
                      setSessionForm((current) => ({ ...current, taskProfileId: profile.id }));
                      setTaskForm({
                        id: profile.id,
                        slug: profile.slug,
                        displayName: profile.displayName,
                        description: profile.description ?? '',
                        targetCapability: profile.targetCapability,
                        systemPrompt: profile.systemPrompt ?? '',
                        allowedProviderProfileIds: profile.allowedProviderProfileIds.join(','),
                        preferredProviderProfileIds: profile.preferredProviderProfileIds.join(','),
                        defaultExecutionMode: profile.defaultExecutionMode,
                        isActive: profile.isActive
                      });
                      if (profile.toolProfileId) {
                        setSessionForm((current) => ({
                          ...current,
                          toolProfileId: profile.toolProfileId ?? ''
                        }));
                      }
                    }}
                    type='button'
                  >
                    <div className='font-medium'>{profile.displayName}</div>
                    <div className='text-muted-foreground'>
                      {profile.targetCapability} · {profile.defaultExecutionMode} · {profile.isActive ? 'active' : 'inactive'}
                    </div>
                  </button>
                ))}
              </div>
            </Card>
          </div> : null}

          <div className='space-y-6'>
            <Card title='Diagnostics'>
              <div className='grid gap-3 sm:grid-cols-2'>
                <InfoItem label='Router resolutions' value={String(runtimeMetrics?.routerResolutionsTotal ?? 0)} />
                <InfoItem label='Overrides' value={String(runtimeMetrics?.routerOverridesTotal ?? 0)} />
                <InfoItem label='Completed runs' value={String(runtimeMetrics?.completedRunsTotal ?? 0)} />
                <InfoItem label='Failed runs' value={String(runtimeMetrics?.failedRunsTotal ?? 0)} />
                <InfoItem label='Waiting approval' value={String(runtimeMetrics?.waitingApprovalRunsTotal ?? 0)} />
                <InfoItem label='Locale fallbacks' value={String(runtimeMetrics?.localeFallbackTotal ?? 0)} />
                <InfoItem label='Direct selected' value={String(runtimeMetrics?.selectedDirectTotal ?? 0)} />
                <InfoItem label='MCP selected' value={String(runtimeMetrics?.selectedMcpTotal ?? 0)} />
              </div>
              <div className='mt-4 space-y-3 text-sm text-muted-foreground'>
                <div>
                  Average run latency:{' '}
                  {runtimeMetrics && runtimeMetrics.runLatencySamples > 0
                    ? Math.floor(runtimeMetrics.runLatencyMsTotal / runtimeMetrics.runLatencySamples)
                    : 0}{' '}
                  ms
                </div>
                <div>
                  <div className='font-medium text-foreground'>Provider buckets</div>
                  <div>{bucketSummary(runtimeMetrics?.providerKindTotals ?? [])}</div>
                </div>
                <div>
                  <div className='font-medium text-foreground'>Execution targets</div>
                  <div>{bucketSummary(runtimeMetrics?.executionTargetTotals ?? [])}</div>
                </div>
                <div>
                  <div className='font-medium text-foreground'>Task profiles</div>
                  <div>{bucketSummary(runtimeMetrics?.taskProfileTotals ?? [])}</div>
                </div>
                <div>
                  <div className='font-medium text-foreground'>Resolved locales</div>
                  <div>{bucketSummary(runtimeMetrics?.resolvedLocaleTotals ?? [])}</div>
                </div>
                <div>
                  <div className='font-medium text-foreground'>Recent runs</div>
                  <div>{formatRecentRunSummary(recentRuns)}</div>
                </div>
                <div className='space-y-2'>
                  {recentRuns.slice(0, 8).map((run) => (
                    <div key={run.id} className='rounded-lg border border-border px-3 py-2'>
                      <div className='font-medium text-foreground'>
                        {run.sessionTitle} В· {run.status} В· {run.durationMs} ms
                      </div>
                      <div>
                        {run.providerDisplayName} В· {run.executionTarget ?? run.executionPath} В·{' '}
                        {run.requestedLocale ?? 'auto'} -&gt; {run.resolvedLocale}
                      </div>
                      <div className='text-xs text-muted-foreground'>
                        {new Date(run.startedAt).toLocaleString()}
                        {run.taskProfileSlug ? ` В· task ${run.taskProfileSlug}` : ''}
                      </div>
                      {run.errorMessage ? (
                        <div className='mt-1 text-rose-600'>{run.errorMessage}</div>
                      ) : null}
                    </div>
                  ))}
                </div>
                <div>
                  <div className='font-medium text-foreground'>Recent stream events</div>
                  <div>{recentStreamEvents.length === 0 ? 'No recent events yet.' : `${recentStreamEvents.length} cached event(s)`}</div>
                </div>
              </div>
            </Card>

            {!diagnosticsOnly ? <Card title='Blog Draft'>
              <form
                className='space-y-3'
                onSubmit={async (event) => {
                  event.preventDefault();
                  if (!sessionForm.taskProfileId) {
                    setError('Select the `blog_draft` task profile before generating blog draft content.');
                    return;
                  }
                  const taskInputJson = JSON.stringify({
                    post_id: blogForm.postId || null,
                    source_locale: blogForm.sourceLocale || null,
                    source_title: blogForm.sourceTitle || null,
                    source_body: blogForm.sourceBody || null,
                    source_excerpt: blogForm.sourceExcerpt || null,
                    source_seo_title: blogForm.sourceSeoTitle || null,
                    source_seo_description: blogForm.sourceSeoDescription || null,
                    tags: splitCsv(blogForm.tags),
                    category_id: blogForm.categoryId || null,
                    featured_image_url: blogForm.featuredImageUrl || null,
                    copy_instructions: blogForm.copyInstructions || null,
                    assistant_prompt: blogForm.assistantPrompt || null
                  });
                  const started = await gql<
                    { runAiTaskJob: { session: { session: { id: string; title: string } } } },
                    { input: Record<string, unknown> }
                  >(
                    RUN_TASK_JOB_MUTATION,
                    {
                      input: {
                        title: blogForm.title,
                        providerProfileId: sessionForm.providerProfileId || null,
                        taskProfileId: sessionForm.taskProfileId,
                        executionMode: 'DIRECT',
                        locale: blogForm.locale || null,
                        taskInputJson,
                        metadata: '{}'
                      }
                    },
                    props
                  ).catch((err: Error) => {
                    setError(err.message);
                    return null;
                  });
                  if (!started) return;
                  const id = started.runAiTaskJob.session.session.id;
                  setFeedback(`Blog draft job \`${started.runAiTaskJob.session.session.title}\` completed.`);
                  await loadBootstrap();
                  await loadSession(id);
                }}
              >
                <Input label='Job title' value={blogForm.title} onChange={(title) => setBlogForm((current) => ({ ...current, title }))} />
                <Input
                  label='Locale'
                  placeholder='auto (request locale -> tenant default -> en)'
                  value={blogForm.locale}
                  onChange={(locale) => setBlogForm((current) => ({ ...current, locale }))}
                />
                <Input label='Existing post id' value={blogForm.postId} onChange={(postId) => setBlogForm((current) => ({ ...current, postId }))} />
                <Input label='Source locale' value={blogForm.sourceLocale} onChange={(sourceLocale) => setBlogForm((current) => ({ ...current, sourceLocale }))} />
                <Input label='Source title override' value={blogForm.sourceTitle} onChange={(sourceTitle) => setBlogForm((current) => ({ ...current, sourceTitle }))} />
                <Input label='Source body override' value={blogForm.sourceBody} onChange={(sourceBody) => setBlogForm((current) => ({ ...current, sourceBody }))} />
                <Input label='Source excerpt override' value={blogForm.sourceExcerpt} onChange={(sourceExcerpt) => setBlogForm((current) => ({ ...current, sourceExcerpt }))} />
                <Input label='Source SEO title override' value={blogForm.sourceSeoTitle} onChange={(sourceSeoTitle) => setBlogForm((current) => ({ ...current, sourceSeoTitle }))} />
                <Input label='Source SEO description override' value={blogForm.sourceSeoDescription} onChange={(sourceSeoDescription) => setBlogForm((current) => ({ ...current, sourceSeoDescription }))} />
                <Input label='Tags (csv)' value={blogForm.tags} onChange={(tags) => setBlogForm((current) => ({ ...current, tags }))} />
                <Input label='Category id' value={blogForm.categoryId} onChange={(categoryId) => setBlogForm((current) => ({ ...current, categoryId }))} />
                <Input label='Featured image URL' value={blogForm.featuredImageUrl} onChange={(featuredImageUrl) => setBlogForm((current) => ({ ...current, featuredImageUrl }))} />
                <Input label='Copy instructions' value={blogForm.copyInstructions} onChange={(copyInstructions) => setBlogForm((current) => ({ ...current, copyInstructions }))} />
                <Input label='Assistant prompt' value={blogForm.assistantPrompt} onChange={(assistantPrompt) => setBlogForm((current) => ({ ...current, assistantPrompt }))} />
                <div className='rounded-lg border border-border px-3 py-2 text-sm text-muted-foreground'>
                  Provider: {sessionForm.providerProfileId || 'optional'}
                  <br />
                  Task profile: {sessionForm.taskProfileId || 'select blog_draft'}
                  <br />
                  Mode: direct
                </div>
                <button className='rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground' type='submit'>
                  Generate blog draft
                </button>
              </form>
            </Card> : null}

            {!diagnosticsOnly ? <Card title='Product Copy'>
              <form
                className='space-y-3'
                onSubmit={async (event) => {
                  event.preventDefault();
                  if (!sessionForm.taskProfileId) {
                    setError('Select the `product_copy` task profile before generating localized product copy.');
                    return;
                  }
                  let normalizedProductId = '';
                  try {
                    normalizedProductId = productForm.productId.trim();
                    if (!normalizedProductId) {
                      throw new Error('Product id is required.');
                    }
                  } catch (err) {
                    setError(err instanceof Error ? err.message : 'Product id is required.');
                    return;
                  }
                  const taskInputJson = JSON.stringify({
                    product_id: normalizedProductId,
                    source_locale: productForm.sourceLocale || null,
                    source_title: productForm.sourceTitle || null,
                    source_description: productForm.sourceDescription || null,
                    source_meta_title: productForm.sourceMetaTitle || null,
                    source_meta_description: productForm.sourceMetaDescription || null,
                    copy_instructions: productForm.copyInstructions || null,
                    assistant_prompt: productForm.assistantPrompt || null
                  });
                  const started = await gql<
                    { runAiTaskJob: { session: { session: { id: string; title: string } } } },
                    { input: Record<string, unknown> }
                  >(
                    RUN_TASK_JOB_MUTATION,
                    {
                      input: {
                        title: productForm.title,
                        providerProfileId: sessionForm.providerProfileId || null,
                        taskProfileId: sessionForm.taskProfileId,
                        executionMode: 'DIRECT',
                        locale: productForm.locale || null,
                        taskInputJson,
                        metadata: '{}'
                      }
                    },
                    props
                  ).catch((err: Error) => {
                    setError(err.message);
                    return null;
                  });
                  if (!started) return;
                  const id = started.runAiTaskJob.session.session.id;
                  setFeedback(`Product copy job \`${started.runAiTaskJob.session.session.title}\` completed.`);
                  await loadBootstrap();
                  await loadSession(id);
                }}
              >
                <Input label='Job title' value={productForm.title} onChange={(title) => setProductForm((current) => ({ ...current, title }))} />
                <Input
                  label='Locale'
                  placeholder='auto (request locale -> tenant default -> en)'
                  value={productForm.locale}
                  onChange={(locale) => setProductForm((current) => ({ ...current, locale }))}
                />
                <Input label='Product id' value={productForm.productId} onChange={(productId) => setProductForm((current) => ({ ...current, productId }))} />
                <Input label='Source locale' value={productForm.sourceLocale} onChange={(sourceLocale) => setProductForm((current) => ({ ...current, sourceLocale }))} />
                <Input label='Source title override' value={productForm.sourceTitle} onChange={(sourceTitle) => setProductForm((current) => ({ ...current, sourceTitle }))} />
                <Input label='Source description override' value={productForm.sourceDescription} onChange={(sourceDescription) => setProductForm((current) => ({ ...current, sourceDescription }))} />
                <Input label='Source meta title override' value={productForm.sourceMetaTitle} onChange={(sourceMetaTitle) => setProductForm((current) => ({ ...current, sourceMetaTitle }))} />
                <Input label='Source meta description override' value={productForm.sourceMetaDescription} onChange={(sourceMetaDescription) => setProductForm((current) => ({ ...current, sourceMetaDescription }))} />
                <Input label='Copy instructions' value={productForm.copyInstructions} onChange={(copyInstructions) => setProductForm((current) => ({ ...current, copyInstructions }))} />
                <Input label='Assistant prompt' value={productForm.assistantPrompt} onChange={(assistantPrompt) => setProductForm((current) => ({ ...current, assistantPrompt }))} />
                <div className='rounded-lg border border-border px-3 py-2 text-sm text-muted-foreground'>
                  Provider: {sessionForm.providerProfileId || 'optional'}
                  <br />
                  Task profile: {sessionForm.taskProfileId || 'select product_copy'}
                  <br />
                  Mode: direct
                </div>
                <button className='rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground' type='submit'>
                  Generate product copy
                </button>
              </form>
            </Card> : null}

            {!diagnosticsOnly ? <Card title='Media Image'>
              <form
                className='space-y-3'
                onSubmit={async (event) => {
                  event.preventDefault();
                  if (!sessionForm.taskProfileId) {
                    setError('Select the `image_asset` task profile before generating a media image.');
                    return;
                  }
                  const taskInputJson = JSON.stringify({
                    prompt: imageForm.prompt,
                    negative_prompt: imageForm.negativePrompt || null,
                    title: imageForm.mediaTitle || null,
                    alt_text: imageForm.altText || null,
                    caption: imageForm.caption || null,
                    file_name: imageForm.fileName || null,
                    size: imageForm.size || null,
                    assistant_prompt: imageForm.assistantPrompt || null
                  });
                  const started = await gql<
                    { runAiTaskJob: { session: { session: { id: string; title: string } } } },
                    { input: Record<string, unknown> }
                  >(
                    RUN_TASK_JOB_MUTATION,
                    {
                      input: {
                        title: imageForm.title,
                        providerProfileId: sessionForm.providerProfileId || null,
                        taskProfileId: sessionForm.taskProfileId,
                        executionMode: 'DIRECT',
                        locale: imageForm.locale || null,
                        taskInputJson,
                        metadata: '{}'
                      }
                    },
                    props
                  ).catch((err: Error) => {
                    setError(err.message);
                    return null;
                  });
                  if (!started) return;
                  const id = started.runAiTaskJob.session.session.id;
                  setFeedback(`Image job \`${started.runAiTaskJob.session.session.title}\` completed.`);
                  await loadBootstrap();
                  await loadSession(id);
                }}
              >
                <Input label='Job title' value={imageForm.title} onChange={(title) => setImageForm((current) => ({ ...current, title }))} />
                <Input
                  label='Locale'
                  placeholder='auto (request locale -> tenant default -> en)'
                  value={imageForm.locale}
                  onChange={(locale) => setImageForm((current) => ({ ...current, locale }))}
                />
                <Input label='Prompt' value={imageForm.prompt} onChange={(prompt) => setImageForm((current) => ({ ...current, prompt }))} />
                <Input label='Negative prompt' value={imageForm.negativePrompt} onChange={(negativePrompt) => setImageForm((current) => ({ ...current, negativePrompt }))} />
                <Input label='File name' value={imageForm.fileName} onChange={(fileName) => setImageForm((current) => ({ ...current, fileName }))} />
                <Input label='Media title' value={imageForm.mediaTitle} onChange={(mediaTitle) => setImageForm((current) => ({ ...current, mediaTitle }))} />
                <Input label='Alt text' value={imageForm.altText} onChange={(altText) => setImageForm((current) => ({ ...current, altText }))} />
                <Input label='Caption' value={imageForm.caption} onChange={(caption) => setImageForm((current) => ({ ...current, caption }))} />
                <Input label='Size' value={imageForm.size} onChange={(size) => setImageForm((current) => ({ ...current, size }))} />
                <Input label='Assistant prompt' value={imageForm.assistantPrompt} onChange={(assistantPrompt) => setImageForm((current) => ({ ...current, assistantPrompt }))} />
                <div className='rounded-lg border border-border px-3 py-2 text-sm text-muted-foreground'>
                  Provider: {sessionForm.providerProfileId || 'optional'}
                  <br />
                  Task profile: {sessionForm.taskProfileId || 'select image_asset'}
                  <br />
                  Mode: direct
                </div>
                <button className='rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground' type='submit'>
                  Generate media image
                </button>
              </form>
            </Card> : null}

            {!diagnosticsOnly ? <Card title='Alloy Assist'>
              <form
                className='space-y-3'
                onSubmit={async (event) => {
                  event.preventDefault();
                  if (!sessionForm.taskProfileId) {
                    setError('Select the `alloy_code` task profile before running Alloy Assist.');
                    return;
                  }
                  const taskInputJson = JSON.stringify({
                    operation: alloyForm.operation,
                    script_id: alloyForm.scriptId || null,
                    script_name: alloyForm.scriptName || null,
                    script_source: alloyForm.scriptSource || null,
                    runtime_payload_json: alloyForm.runtimePayloadJson || null,
                    assistant_prompt: alloyForm.assistantPrompt || null
                  });
                  const started = await gql<
                    { runAiTaskJob: { session: { session: { id: string; title: string } } } },
                    { input: Record<string, unknown> }
                  >(
                    RUN_TASK_JOB_MUTATION,
                    {
                      input: {
                        title: alloyForm.title,
                        providerProfileId: sessionForm.providerProfileId || null,
                        taskProfileId: sessionForm.taskProfileId,
                        executionMode: 'DIRECT',
                        locale: alloyForm.locale || null,
                        taskInputJson,
                        metadata: '{}'
                      }
                    },
                    props
                  ).catch((err: Error) => {
                    setError(err.message);
                    return null;
                  });
                  if (!started) return;
                  const id = started.runAiTaskJob.session.session.id;
                  setFeedback(`Alloy job \`${started.runAiTaskJob.session.session.title}\` completed.`);
                  await loadBootstrap();
                  await loadSession(id);
                }}
              >
                <Input label='Job title' value={alloyForm.title} onChange={(title) => setAlloyForm((current) => ({ ...current, title }))} />
                <Input
                  label='Locale'
                  placeholder='auto (request locale -> tenant default -> en)'
                  value={alloyForm.locale}
                  onChange={(locale) => setAlloyForm((current) => ({ ...current, locale }))}
                />
                <Input label='Operation' value={alloyForm.operation} onChange={(operation) => setAlloyForm((current) => ({ ...current, operation }))} />
                <Input label='Script id' value={alloyForm.scriptId} onChange={(scriptId) => setAlloyForm((current) => ({ ...current, scriptId }))} />
                <Input label='Script name' value={alloyForm.scriptName} onChange={(scriptName) => setAlloyForm((current) => ({ ...current, scriptName }))} />
                <Input label='Assistant prompt' value={alloyForm.assistantPrompt} onChange={(assistantPrompt) => setAlloyForm((current) => ({ ...current, assistantPrompt }))} />
                <Input label='Script source' value={alloyForm.scriptSource} onChange={(scriptSource) => setAlloyForm((current) => ({ ...current, scriptSource }))} />
                <Input label='Runtime payload JSON' value={alloyForm.runtimePayloadJson} onChange={(runtimePayloadJson) => setAlloyForm((current) => ({ ...current, runtimePayloadJson }))} />
                <div className='rounded-lg border border-border px-3 py-2 text-sm text-muted-foreground'>
                  Provider: {sessionForm.providerProfileId || 'optional'}
                  <br />
                  Task profile: {sessionForm.taskProfileId || 'select alloy_code'}
                  <br />
                  Mode: direct
                </div>
                <button className='rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground' type='submit'>
                  Run Alloy job
                </button>
              </form>
            </Card> : null}

            {!diagnosticsOnly ? <Card title='New Session'>
              <form
                className='space-y-3'
                onSubmit={async (event) => {
                  event.preventDefault();
                  const started = await gql<
                    { startAiChatSession: { session: { session: { id: string; title: string } } } },
                    { input: Record<string, unknown> }
                  >(
                    START_SESSION_MUTATION,
                    {
                      input: {
                        title: sessionForm.title,
                        providerProfileId: sessionForm.providerProfileId || null,
                        taskProfileId: sessionForm.taskProfileId || null,
                        toolProfileId: sessionForm.toolProfileId || null,
                        locale: sessionForm.locale || null,
                        initialMessage: sessionForm.initialMessage || null,
                        metadata: '{}'
                      }
                    },
                    props
                  ).catch((err: Error) => {
                    setError(err.message);
                    return null;
                  });
                  if (!started) return;
                  const id = started.startAiChatSession.session.session.id;
                  setFeedback(`Session \`${started.startAiChatSession.session.session.title}\` started.`);
                  await loadBootstrap();
                  await loadSession(id);
                }}
              >
                <Input label='Title' value={sessionForm.title} onChange={(title) => setSessionForm((current) => ({ ...current, title }))} />
                <Input
                  label='Locale'
                  placeholder='auto (request locale -> tenant default -> en)'
                  value={sessionForm.locale}
                  onChange={(locale) => setSessionForm((current) => ({ ...current, locale }))}
                />
                <Input label='Initial message' value={sessionForm.initialMessage} onChange={(initialMessage) => setSessionForm((current) => ({ ...current, initialMessage }))} />
                <div className='rounded-lg border border-border px-3 py-2 text-sm text-muted-foreground'>
                  Provider: {sessionForm.providerProfileId || 'not selected'}
                  <br />
                  Task profile: {sessionForm.taskProfileId || 'optional'}
                  <br />
                  Tool profile: {sessionForm.toolProfileId || 'optional'}
                </div>
                <button className='rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground' type='submit'>
                  Start session
                </button>
              </form>
            </Card> : null}

            <Card title='Sessions'>
              <div className='space-y-2'>
                {sessions.map((session) => (
                  <button
                    key={session.id}
                    className='w-full rounded-lg border border-border px-3 py-3 text-left text-sm hover:bg-muted'
                    onClick={() => void loadSession(session.id)}
                    type='button'
                  >
                    <div className='font-medium'>{session.title}</div>
                    <div className='text-muted-foreground'>
                      status: {session.status} · mode: {session.executionMode} · latest: {session.latestRunStatus ?? 'idle'} · approvals: {session.pendingApprovals}
                    </div>
                  </button>
                ))}
              </div>
            </Card>
          </div>

          <Card title='Operator Chat'>
            {detail ? (
              <div className='space-y-5'>
                <div className='rounded-lg border border-border px-3 py-3 text-sm'>
                  <div className='font-medium'>{detail.session.title}</div>
                  <div className='text-muted-foreground'>
                    locale: {detail.session.requestedLocale ?? 'auto'} -&gt; {detail.session.resolvedLocale}
                  </div>
                  <div className='text-muted-foreground'>
                    provider: {detail.providerProfile.displayName} · model: {detail.providerProfile.model} · mode: {detail.session.executionMode}
                  </div>
                </div>

                <div className='max-h-[360px] space-y-3 overflow-y-auto rounded-xl border border-border p-3'>
                  {detail.messages.map((message) => (
                    <div key={message.id} className='rounded-lg border border-border px-3 py-3 text-sm'>
                      <div className='mb-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground'>
                        {message.role}
                      </div>
                      <div>{message.content ?? '(no textual content)'}</div>
                    </div>
                  ))}
                </div>

                {liveStream ? (
                  <div className='rounded-xl border border-primary/30 bg-primary/5 px-4 py-3 text-sm'>
                    <div className='flex items-center justify-between gap-3'>
                      <div className='font-medium text-foreground'>Live stream</div>
                      <div className='text-xs text-muted-foreground'>
                        {liveStream.connected ? 'connected' : 'disconnected'} · {liveStream.status}
                      </div>
                    </div>
                    <div className='mt-3 whitespace-pre-wrap text-foreground'>
                      {liveStream.content || 'Waiting for assistant output…'}
                    </div>
                    {liveStream.errorMessage ? (
                      <div className='mt-2 text-rose-600'>{liveStream.errorMessage}</div>
                    ) : null}
                  </div>
                ) : null}

                {detail?.recentStreamEvents?.length ? (
                  <Card title='Recent Stream Events'>
                    <div className='space-y-2 text-sm'>
                      {detail.recentStreamEvents.slice(0, 10).map((event) => (
                        <div key={`${event.runId}-${event.createdAt}`} className='rounded-lg border border-border px-3 py-2'>
                          <div className='font-medium'>
                            {event.eventKind} · {event.runId}
                          </div>
                          <div className='text-muted-foreground'>{new Date(event.createdAt).toLocaleString()}</div>
                          {event.accumulatedContent || event.contentDelta ? (
                            <div className='mt-1 whitespace-pre-wrap'>
                              {event.accumulatedContent ?? event.contentDelta}
                            </div>
                          ) : null}
                          {event.errorMessage ? (
                            <div className='mt-1 text-rose-600'>{event.errorMessage}</div>
                          ) : null}
                        </div>
                      ))}
                    </div>
                  </Card>
                ) : null}

                <form
                  className='space-y-3'
                  onSubmit={async (event) => {
                    event.preventDefault();
                    if (!selectedSession || !reply.trim()) return;
                    const result = await gql<
                      { sendAiChatMessage: { session: { session: { id: string } } } },
                      { sessionId: string; content: string }
                    >(SEND_MESSAGE_MUTATION, { sessionId: selectedSession, content: reply }, props).catch((err: Error) => {
                      setError(err.message);
                      return null;
                    });
                    if (!result) return;
                    setReply('');
                    await loadBootstrap();
                    await loadSession(selectedSession);
                  }}
                >
                  <textarea
                    className='min-h-28 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm'
                    onChange={(event) => setReply(event.target.value)}
                    value={reply}
                  />
                  <button className='rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground' type='submit'>
                    Send
                  </button>
                </form>

                {detail.approvals.filter((approval) => approval.status === 'pending').length > 0 ? (
                  <div className='space-y-3'>
                    <div className='text-sm font-semibold'>Pending approvals</div>
                    {detail.approvals
                      .filter((approval) => approval.status === 'pending')
                      .map((approval) => (
                        <div key={approval.id} className='rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-900'>
                          <div className='font-medium'>{approval.toolName}</div>
                          <div className='mt-1'>{approval.reason ?? 'Operator approval required'}</div>
                          <div className='mt-3 flex gap-2'>
                            <button
                              className='rounded-md bg-amber-900 px-3 py-2 text-xs font-semibold text-white'
                              onClick={async () => {
                                await gql(
                                  RESUME_APPROVAL_MUTATION,
                                  { approvalId: approval.id, input: { approved: true, reason: null } },
                                  props
                                ).catch((err: Error) => setError(err.message));
                                await loadBootstrap();
                                if (selectedSession) await loadSession(selectedSession);
                              }}
                              type='button'
                            >
                              Approve
                            </button>
                            <button
                              className='rounded-md border border-amber-900 px-3 py-2 text-xs font-semibold text-amber-900'
                              onClick={async () => {
                                await gql(
                                  RESUME_APPROVAL_MUTATION,
                                  { approvalId: approval.id, input: { approved: false, reason: 'Rejected in Next.js admin UI' } },
                                  props
                                ).catch((err: Error) => setError(err.message));
                                await loadBootstrap();
                                if (selectedSession) await loadSession(selectedSession);
                              }}
                              type='button'
                            >
                              Reject
                            </button>
                          </div>
                        </div>
                      ))}
                  </div>
                ) : null}

                <div className='space-y-3'>
                  <div className='text-sm font-semibold'>Runs</div>
                  {detail.runs.map((run) => (
                    <div key={run.id} className='rounded-lg border border-border px-3 py-3 text-sm'>
                      <div className='font-medium'>{run.model}</div>
                      <div className='text-muted-foreground'>
                        locale: {run.requestedLocale ?? 'auto'} -&gt; {run.resolvedLocale}
                      </div>
                      <div className='text-muted-foreground'>
                        {run.status} · {run.executionMode} · path {run.executionPath}
                      </div>
                      {run.errorMessage ? (
                        <div className='mt-2 text-destructive'>{run.errorMessage}</div>
                      ) : null}
                    </div>
                  ))}
                </div>

                <div className='space-y-3'>
                  <div className='text-sm font-semibold'>Tool trace</div>
                  {detail.toolTraces.map((trace, index) => (
                    <div key={`${trace.toolName}-${index}`} className='rounded-lg border border-border px-3 py-3 text-sm'>
                      <div className='font-medium'>{trace.toolName}</div>
                      <div className='text-muted-foreground'>
                        {trace.status} · {trace.durationMs} ms
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            ) : (
              <div className='rounded-lg border border-dashed border-border px-4 py-8 text-sm text-muted-foreground'>
                Select a session to inspect chat history, traces and approvals.
              </div>
            )}
          </Card>
        </div>
      )}
    </div>
  );
}

function Card(props: { title: string; children: React.ReactNode }) {
  return (
    <section className='rounded-2xl border border-border bg-card p-6 shadow-sm'>
      <h2 className='mb-4 text-lg font-semibold text-card-foreground'>{props.title}</h2>
      {props.children}
    </section>
  );
}

function Input(props: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
}) {
  return (
    <label className='block space-y-1'>
      <span className='text-sm text-muted-foreground'>{props.label}</span>
      <input
        className='w-full rounded-lg border border-input bg-background px-3 py-2 text-sm'
        onChange={(event) => props.onChange(event.target.value)}
        placeholder={props.placeholder}
        value={props.value}
      />
    </label>
  );
}

function InfoItem(props: { label: string; value: string }) {
  return (
    <div className='rounded-lg border border-border px-3 py-3'>
      <div className='text-xs uppercase tracking-wide text-muted-foreground'>{props.label}</div>
      <div className='mt-1 text-lg font-semibold text-card-foreground'>{props.value}</div>
    </div>
  );
}

function bucketSummary(buckets: MetricBucket[]): string {
  if (buckets.length === 0) {
    return 'no data';
  }
  return buckets.map((bucket) => `${bucket.label}=${bucket.total}`).join(', ');
}

function formatRecentRunSummary(runs: RecentRun[]): string {
  if (runs.length === 0) {
    return 'No recent runs yet.';
  }
  const failed = runs.filter((run) => run.status === 'failed').length;
  const waiting = runs.filter((run) => run.status === 'waiting_approval').length;
  const avgLatency = Math.round(
    runs.reduce((total, run) => total + Math.max(run.durationMs, 0), 0) / runs.length
  );
  return `${runs.length} run(s), ${failed} failed, ${waiting} waiting approval, avg ${avgLatency} ms`;
}

function splitCsv(value: string): string[] {
  return value
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
}
