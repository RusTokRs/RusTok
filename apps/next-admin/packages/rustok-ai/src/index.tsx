'use client';

import React from 'react';

import { graphqlRequest as sharedGraphqlRequest } from '../../../src/shared/api/graphql';

type AiAdminPageProps = {
  token?: string | null;
  tenantSlug?: string | null;
  graphqlUrl?: string;
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
};

const BOOTSTRAP_QUERY = `
  query AiBootstrap {
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
    aiChatSessions { id title providerProfileId taskProfileId toolProfileId executionMode status latestRunStatus pendingApprovals }
  }
`;

const SESSION_QUERY = `
  query AiSession($id: UUID!) {
    aiChatSession(id: $id) {
      session { id title providerProfileId taskProfileId toolProfileId executionMode status latestRunStatus pendingApprovals }
      providerProfile {
        id slug displayName providerKind baseUrl model temperature maxTokens hasSecret isActive capabilities
        usagePolicy { allowedTaskProfiles deniedTaskProfiles restrictedRoleSlugs }
      }
      taskProfile { id slug displayName description targetCapability systemPrompt allowedProviderProfileIds preferredProviderProfileIds fallbackStrategy toolProfileId defaultExecutionMode isActive }
      toolProfile { id slug displayName description allowedTools deniedTools sensitiveTools isActive }
      messages { id role content }
      runs { id taskProfileId status model executionMode executionPath errorMessage decisionTrace }
      toolTraces { toolName status durationMs }
      approvals { id toolName reason status }
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

export function AiAdminPage(props: AiAdminPageProps) {
  const [providers, setProviders] = React.useState<Provider[]>([]);
  const [taskProfiles, setTaskProfiles] = React.useState<TaskProfile[]>([]);
  const [toolProfiles, setToolProfiles] = React.useState<ToolProfile[]>([]);
  const [sessions, setSessions] = React.useState<SessionSummary[]>([]);
  const [selectedSession, setSelectedSession] = React.useState<string | null>(null);
  const [detail, setDetail] = React.useState<SessionDetail | null>(null);
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
    capabilities: 'TEXT_GENERATION,STRUCTURED_GENERATION,CODE_GENERATION',
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
    initialMessage: ''
  });

  const [reply, setReply] = React.useState('');

  const loadBootstrap = React.useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await gql<{
        aiProviderProfiles: Provider[];
        aiTaskProfiles: TaskProfile[];
        aiToolProfiles: ToolProfile[];
        aiChatSessions: SessionSummary[];
      }>(BOOTSTRAP_QUERY, {} as Record<string, never>, props);
      setProviders(data.aiProviderProfiles);
      setTaskProfiles(data.aiTaskProfiles);
      setToolProfiles(data.aiToolProfiles);
      setSessions(data.aiChatSessions);
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
      capabilities: 'TEXT_GENERATION,STRUCTURED_GENERATION,CODE_GENERATION',
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
        const data = await gql<{ aiChatSession: SessionDetail | null }, { id: string }>(
          SESSION_QUERY,
          { id: sessionId },
          props
        );
        setDetail(data.aiChatSession);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load session');
      }
    },
    [props]
  );

  React.useEffect(() => {
    void loadBootstrap();
  }, [loadBootstrap]);

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
        <div className='grid gap-6 xl:grid-cols-[1.1fr_1fr_1.5fr]'>
          <div className='space-y-6'>
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
          </div>

          <div className='space-y-6'>
            <Card title='New Session'>
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
            </Card>

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
}) {
  return (
    <label className='block space-y-1'>
      <span className='text-sm text-muted-foreground'>{props.label}</span>
      <input
        className='w-full rounded-lg border border-input bg-background px-3 py-2 text-sm'
        onChange={(event) => props.onChange(event.target.value)}
        value={props.value}
      />
    </label>
  );
}

function splitCsv(value: string): string[] {
  return value
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
}
