'use client';

import React from 'react';

import { graphqlRequest as sharedGraphqlRequest } from '../../../src/shared/api/graphql';

export type McpAdminPageProps = {
  token?: string | null;
  tenantSlug?: string | null;
  graphqlUrl?: string;
};

type McpScaffoldDraft = {
  id: string;
  clientId?: string | null;
  slug: string;
  crateName: string;
  status: 'STAGED' | 'APPLIED' | string;
  requestJson: string;
  previewJson: string;
  workspaceRoot?: string | null;
  appliedAt?: string | null;
  createdAt: string;
  updatedAt: string;
};

type McpAuditEvent = {
  id: string;
  clientId?: string | null;
  actorId?: string | null;
  actorType?: string | null;
  action: string;
  outcome: string;
  toolName?: string | null;
  reason?: string | null;
  correlationId?: string | null;
  createdAt: string;
};

type McpClient = {
  id: string;
  slug: string;
  displayName: string;
  description?: string | null;
  actorType: string;
  isActive: boolean;
  lastUsedAt?: string | null;
  createdAt: string;
};

type McpPolicy = {
  allowedTools: string[];
  deniedTools: string[];
  grantedPermissions: string[];
  grantedScopes: string[];
  updatedAt: string;
};

type McpToken = {
  id: string;
  tokenName: string;
  tokenPreview: string;
  isActive: boolean;
  lastUsedAt?: string | null;
  expiresAt?: string | null;
  createdAt: string;
};

type McpClientDetails = {
  client: McpClient;
  policy?: McpPolicy | null;
  tokens: McpToken[];
};

const MCP_SCAFFOLD_DRAFTS_QUERY = `
  query McpScaffoldDrafts {
    mcpModuleScaffoldDrafts(limit: 20) {
      id
      clientId
      slug
      crateName
      status
      requestJson
      previewJson
      workspaceRoot
      appliedAt
      createdAt
      updatedAt
    }
  }
`;

const MCP_AUDIT_EVENTS_QUERY = `
  query McpAuditEvents {
    mcpAuditEvents(limit: 30) {
      id
      clientId
      actorId
      actorType
      action
      outcome
      toolName
      reason
      correlationId
      createdAt
    }
  }
`;

const MCP_CLIENTS_QUERY = `
  query McpClients {
    mcpClients(limit: 50) {
      id
      slug
      displayName
      description
      actorType
      isActive
      lastUsedAt
      createdAt
    }
  }
`;

const MCP_CLIENT_DETAILS_QUERY = `
  query McpClientDetails($id: UUID!) {
    mcpClient(id: $id) {
      client {
        id
        slug
        displayName
        description
        actorType
        isActive
        lastUsedAt
        createdAt
      }
      policy {
        allowedTools
        deniedTools
        grantedPermissions
        grantedScopes
        updatedAt
      }
      tokens {
        id
        tokenName
        tokenPreview
        isActive
        lastUsedAt
        expiresAt
        createdAt
      }
    }
  }
`;

const CREATE_MCP_CLIENT_MUTATION = `
  mutation CreateMcpClient($input: CreateMcpClientInput!) {
    createMcpClient(input: $input) {
      client { id slug displayName actorType isActive }
      token { id tokenName tokenPreview isActive }
      plaintextToken
    }
  }
`;

const ROTATE_MCP_TOKEN_MUTATION = `
  mutation RotateMcpToken($clientId: UUID!, $input: RotateMcpTokenInput!) {
    rotateMcpClientToken(clientId: $clientId, input: $input) {
      client { id }
      token { id tokenName tokenPreview isActive }
      plaintextToken
    }
  }
`;

const UPDATE_MCP_POLICY_MUTATION = `
  mutation UpdateMcpPolicy($clientId: UUID!, $input: UpdateMcpPolicyInput!) {
    updateMcpClientPolicy(clientId: $clientId, input: $input) { clientId }
  }
`;

const REVOKE_MCP_TOKEN_MUTATION = `
  mutation RevokeMcpToken($tokenId: UUID!, $reason: String) {
    revokeMcpToken(tokenId: $tokenId, reason: $reason)
  }
`;

const DEACTIVATE_MCP_CLIENT_MUTATION = `
  mutation DeactivateMcpClient($clientId: UUID!, $reason: String) {
    deactivateMcpClient(clientId: $clientId, reason: $reason)
  }
`;

const STAGE_MCP_SCAFFOLD_DRAFT_MUTATION = `
  mutation StageMcpScaffoldDraft($input: StageMcpModuleScaffoldDraftInput!) {
    stageMcpModuleScaffoldDraft(input: $input) {
      id
      clientId
      slug
      crateName
      status
      requestJson
      previewJson
      workspaceRoot
      appliedAt
      createdAt
      updatedAt
    }
  }
`;

const APPLY_MCP_SCAFFOLD_DRAFT_MUTATION = `
  mutation ApplyMcpScaffoldDraft(
    $draftId: UUID!
    $input: ApplyMcpModuleScaffoldDraftInput!
  ) {
    applyMcpModuleScaffoldDraft(draftId: $draftId, input: $input) {
      id
      clientId
      slug
      crateName
      status
      requestJson
      previewJson
      workspaceRoot
      appliedAt
      createdAt
      updatedAt
    }
  }
`;

async function gql<TData, TVars = Record<string, never>>(
  query: string,
  variables: TVars,
  props: McpAdminPageProps
): Promise<TData> {
  return sharedGraphqlRequest<TVars, TData>(
    query,
    variables,
    props.token,
    props.tenantSlug,
    { graphqlUrl: props.graphqlUrl }
  );
}

export function McpAdminPage(props: McpAdminPageProps): React.JSX.Element {
  const [drafts, setDrafts] = React.useState<McpScaffoldDraft[]>([]);
  const [auditEvents, setAuditEvents] = React.useState<McpAuditEvent[]>([]);
  const [clients, setClients] = React.useState<McpClient[]>([]);
  const [selectedClientId, setSelectedClientId] = React.useState('');
  const [clientDetails, setClientDetails] =
    React.useState<McpClientDetails | null>(null);
  const [selectedDraftId, setSelectedDraftId] = React.useState('');
  const [loading, setLoading] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [feedback, setFeedback] = React.useState<string | null>(null);
  const [plaintextToken, setPlaintextToken] = React.useState<string | null>(null);
  const [clientForm, setClientForm] = React.useState({
    slug: '',
    displayName: '',
    description: '',
    actorType: 'SERVICE_CLIENT',
    tokenName: 'primary'
  });
  const [policyForm, setPolicyForm] = React.useState({
    allowedTools: '',
    deniedTools: '',
    grantedPermissions: '',
    grantedScopes: ''
  });
  const [rotateTokenName, setRotateTokenName] = React.useState('rotated');
  const [revokeExistingTokens, setRevokeExistingTokens] = React.useState(true);
  const [managementReason, setManagementReason] = React.useState('');
  const [form, setForm] = React.useState({
    clientId: '',
    slug: '',
    name: '',
    description: '',
    dependencies: '',
    withGraphql: true,
    withRest: true,
    workspaceRoot: '',
    confirmApply: false
  });

  const loadDrafts = React.useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [draftData, auditData, clientData] = await Promise.all([
        gql<{ mcpModuleScaffoldDrafts: McpScaffoldDraft[] }>(
          MCP_SCAFFOLD_DRAFTS_QUERY,
          {} as Record<string, never>,
          props
        ),
        gql<{ mcpAuditEvents: McpAuditEvent[] }>(
          MCP_AUDIT_EVENTS_QUERY,
          {} as Record<string, never>,
          props
        ),
        gql<{ mcpClients: McpClient[] }>(
          MCP_CLIENTS_QUERY,
          {} as Record<string, never>,
          props
        )
      ]);
      setDrafts(draftData.mcpModuleScaffoldDrafts);
      setAuditEvents(auditData.mcpAuditEvents);
      setClients(clientData.mcpClients);
      setSelectedDraftId((current) =>
        current &&
        draftData.mcpModuleScaffoldDrafts.some((draft) => draft.id === current)
          ? current
          : draftData.mcpModuleScaffoldDrafts[0]?.id ?? ''
      );
      setSelectedClientId((current) =>
        current && clientData.mcpClients.some((client) => client.id === current)
          ? current
          : clientData.mcpClients[0]?.id ?? ''
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load MCP drafts');
    } finally {
      setLoading(false);
    }
  }, [props.graphqlUrl, props.tenantSlug, props.token]);

  React.useEffect(() => {
    void loadDrafts();
  }, [loadDrafts]);

  const loadClientDetails = React.useCallback(
    async (clientId: string) => {
      if (!clientId) {
        setClientDetails(null);
        return;
      }
      await gql<{ mcpClient: McpClientDetails | null }, { id: string }>(
        MCP_CLIENT_DETAILS_QUERY,
        { id: clientId },
        props
      ).then((data) => {
        setClientDetails(data.mcpClient);
        const policy = data.mcpClient?.policy;
        setPolicyForm({
          allowedTools: policy?.allowedTools.join(', ') ?? '',
          deniedTools: policy?.deniedTools.join(', ') ?? '',
          grantedPermissions: policy?.grantedPermissions.join(', ') ?? '',
          grantedScopes: policy?.grantedScopes.join(', ') ?? ''
        });
      });
    },
    [props.graphqlUrl, props.tenantSlug, props.token]
  );

  React.useEffect(() => {
    void loadClientDetails(selectedClientId).catch((err: Error) =>
      setError(err.message)
    );
  }, [loadClientDetails, selectedClientId]);

  const selectedDraft =
    drafts.find((draft) => draft.id === selectedDraftId) ?? null;

  return (
    <div className='space-y-6'>
      <header className='border-border bg-card rounded-lg border p-6 shadow-sm'>
        <p className='text-muted-foreground text-sm'>MCP control plane</p>
        <h1 className='text-foreground text-2xl font-semibold'>
          MCP administration
        </h1>
      </header>

      {error ? (
        <div className='border-destructive/30 bg-destructive/10 text-destructive rounded-lg border px-3 py-2 text-sm'>
          {error}
        </div>
      ) : null}
      {feedback ? (
        <div className='border-border bg-muted rounded-lg border px-3 py-2 text-sm'>
          {feedback}
        </div>
      ) : null}
      {plaintextToken ? (
        <aside className='border-border bg-muted flex flex-wrap items-center gap-3 rounded-lg border p-3 text-sm'>
          <strong>New token, shown once</strong>
          <code className='min-w-0 flex-1 break-all'>{plaintextToken}</code>
          <button
            className='border-border rounded-lg border px-3 py-1.5'
            onClick={() => setPlaintextToken(null)}
            type='button'
          >
            Dismiss
          </button>
        </aside>
      ) : null}

      <section className='grid gap-6 lg:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]'>
        <form
          className='border-border bg-card space-y-4 rounded-lg border p-4'
          onSubmit={async (event) => {
            event.preventDefault();
            setError(null);
            const staged = await gql<
              { stageMcpModuleScaffoldDraft: McpScaffoldDraft },
              { input: Record<string, unknown> }
            >(
              STAGE_MCP_SCAFFOLD_DRAFT_MUTATION,
              {
                input: {
                  clientId: form.clientId || null,
                  slug: form.slug,
                  name: form.name,
                  description: form.description,
                  dependencies: splitCsv(form.dependencies),
                  withGraphql: form.withGraphql,
                  withRest: form.withRest
                }
              },
              props
            ).catch((err: Error) => {
              setError(err.message);
              return null;
            });
            if (!staged) return;
            setFeedback(
              `Draft ${staged.stageMcpModuleScaffoldDraft.crateName} staged.`
            );
            setSelectedDraftId(staged.stageMcpModuleScaffoldDraft.id);
            await loadDrafts();
          }}
        >
          <h2 className='text-foreground text-base font-semibold'>
            Stage draft
          </h2>
          <Input
            label='Client id'
            placeholder='optional MCP client UUID'
            value={form.clientId}
            onChange={(clientId) => setForm((current) => ({ ...current, clientId }))}
          />
          <Input
            label='Slug'
            value={form.slug}
            onChange={(slug) => setForm((current) => ({ ...current, slug }))}
          />
          <Input
            label='Name'
            value={form.name}
            onChange={(name) => setForm((current) => ({ ...current, name }))}
          />
          <Input
            label='Description'
            value={form.description}
            onChange={(description) =>
              setForm((current) => ({ ...current, description }))
            }
          />
          <Input
            label='Dependencies'
            placeholder='comma-separated module slugs'
            value={form.dependencies}
            onChange={(dependencies) =>
              setForm((current) => ({ ...current, dependencies }))
            }
          />
          <div className='grid gap-2 sm:grid-cols-2'>
            <Checkbox
              label='GraphQL'
              checked={form.withGraphql}
              onChange={(withGraphql) =>
                setForm((current) => ({ ...current, withGraphql }))
              }
            />
            <Checkbox
              label='REST'
              checked={form.withRest}
              onChange={(withRest) =>
                setForm((current) => ({ ...current, withRest }))
              }
            />
          </div>
          <button
            className='bg-primary text-primary-foreground rounded-lg px-4 py-2 text-sm font-medium disabled:cursor-not-allowed disabled:opacity-60'
            disabled={loading}
            type='submit'
          >
            Stage draft
          </button>
        </form>

        <section className='border-border bg-card space-y-4 rounded-lg border p-4'>
          <div className='flex items-center justify-between gap-3'>
            <h2 className='text-foreground text-base font-semibold'>
              Review and apply
            </h2>
            <button
              className='border-border rounded-lg border px-3 py-1.5 text-sm'
              onClick={() => void loadDrafts()}
              type='button'
            >
              Refresh
            </button>
          </div>
          <select
            className='border-input bg-background w-full rounded-lg border px-3 py-2 text-sm'
            onChange={(event) => setSelectedDraftId(event.target.value)}
            value={selectedDraftId}
          >
            <option value=''>Select draft</option>
            {drafts.map((draft) => (
              <option key={draft.id} value={draft.id}>
                {draft.crateName} - {draft.status}
              </option>
            ))}
          </select>

          {selectedDraft ? (
            <div className='space-y-3'>
              <div className='border-border text-muted-foreground rounded-lg border px-3 py-2 text-sm'>
                Crate: {selectedDraft.crateName}
                <br />
                Slug: {selectedDraft.slug}
                <br />
                Status: {selectedDraft.status}
                <br />
                Updated: {selectedDraft.updatedAt}
              </div>
              <pre className='border-border bg-muted max-h-72 overflow-auto rounded-lg border p-3 text-xs'>
                {formatJsonForDisplay(selectedDraft.previewJson)}
              </pre>
              <Input
                label='Workspace root'
                value={form.workspaceRoot}
                onChange={(workspaceRoot) =>
                  setForm((current) => ({ ...current, workspaceRoot }))
                }
              />
              <Checkbox
                label='Confirm apply'
                checked={form.confirmApply}
                onChange={(confirmApply) =>
                  setForm((current) => ({ ...current, confirmApply }))
                }
              />
              <button
                className='bg-primary text-primary-foreground rounded-lg px-4 py-2 text-sm font-medium disabled:cursor-not-allowed disabled:opacity-60'
                disabled={
                  selectedDraft.status === 'APPLIED' ||
                  !form.confirmApply ||
                  !form.workspaceRoot
                }
                onClick={async () => {
                  setError(null);
                  const applied = await gql<
                    { applyMcpModuleScaffoldDraft: McpScaffoldDraft },
                    { draftId: string; input: Record<string, unknown> }
                  >(
                    APPLY_MCP_SCAFFOLD_DRAFT_MUTATION,
                    {
                      draftId: selectedDraft.id,
                      input: {
                        workspaceRoot: form.workspaceRoot,
                        confirm: form.confirmApply
                      }
                    },
                    props
                  ).catch((err: Error) => {
                    setError(err.message);
                    return null;
                  });
                  if (!applied) return;
                  setFeedback(
                    `Draft ${applied.applyMcpModuleScaffoldDraft.crateName} applied.`
                  );
                  await loadDrafts();
                }}
                type='button'
              >
                Apply draft
              </button>
            </div>
          ) : null}
        </section>
      </section>

      <section className='border-border bg-card space-y-4 rounded-lg border p-4'>
        <div className='flex flex-wrap items-center justify-between gap-3'>
          <h2 className='text-foreground text-base font-semibold'>MCP clients</h2>
          <div className='flex min-w-0 items-center gap-2'>
            <select
              className='border-input bg-background min-w-0 rounded-lg border px-3 py-2 text-sm'
              onChange={(event) => setSelectedClientId(event.target.value)}
              value={selectedClientId}
            >
              <option value=''>Select client</option>
              {clients.map((client) => (
                <option key={client.id} value={client.id}>
                  {client.displayName} - {client.isActive ? 'active' : 'inactive'}
                </option>
              ))}
            </select>
            <button
              className='border-border rounded-lg border px-3 py-2 text-sm'
              disabled={loading}
              onClick={() => void loadDrafts()}
              type='button'
            >
              Refresh
            </button>
          </div>
        </div>

        <form
          className='border-border grid gap-3 border-t pt-4 md:grid-cols-2'
          onSubmit={async (event) => {
            event.preventDefault();
            setError(null);
            const result = await gql<
              {
                createMcpClient: {
                  client: { id: string };
                  plaintextToken: string;
                };
              },
              { input: Record<string, unknown> }
            >(
              CREATE_MCP_CLIENT_MUTATION,
              {
                input: {
                  ...clientForm,
                  description: clientForm.description || null,
                  tokenName: clientForm.tokenName || null,
                  allowedTools: [],
                  deniedTools: [],
                  grantedPermissions: [],
                  grantedScopes: []
                }
              },
              props
            ).catch((err: Error) => {
              setError(err.message);
              return null;
            });
            if (!result) return;
            setPlaintextToken(result.createMcpClient.plaintextToken);
            setFeedback('MCP client created.');
            await loadDrafts();
            setSelectedClientId(result.createMcpClient.client.id);
          }}
        >
          <h3 className='font-medium md:col-span-2'>Create client</h3>
          <Input
            label='Slug'
            value={clientForm.slug}
            onChange={(slug) => setClientForm((current) => ({ ...current, slug }))}
          />
          <Input
            label='Display name'
            value={clientForm.displayName}
            onChange={(displayName) =>
              setClientForm((current) => ({ ...current, displayName }))
            }
          />
          <Input
            label='Description'
            value={clientForm.description}
            onChange={(description) =>
              setClientForm((current) => ({ ...current, description }))
            }
          />
          <label className='block space-y-1 text-sm'>
            <span className='text-muted-foreground'>Actor type</span>
            <select
              className='border-input bg-background w-full rounded-lg border px-3 py-2'
              onChange={(event) =>
                setClientForm((current) => ({
                  ...current,
                  actorType: event.target.value
                }))
              }
              value={clientForm.actorType}
            >
              <option value='SERVICE_CLIENT'>Service client</option>
              <option value='MODEL_AGENT'>Model agent</option>
              <option value='HUMAN_USER'>Human user</option>
            </select>
          </label>
          <Input
            label='Initial token name'
            value={clientForm.tokenName}
            onChange={(tokenName) =>
              setClientForm((current) => ({ ...current, tokenName }))
            }
          />
          <div className='flex items-end'>
            <button
              className='bg-primary text-primary-foreground rounded-lg px-4 py-2 text-sm font-medium'
              type='submit'
            >
              Create client
            </button>
          </div>
        </form>

        {clientDetails ? (
          <div className='grid gap-6 lg:grid-cols-2'>
            <div className='space-y-4'>
              <div>
                <h3 className='font-medium'>{clientDetails.client.displayName}</h3>
                <p className='text-muted-foreground text-sm'>
                  {clientDetails.client.slug} / {clientDetails.client.actorType} /{' '}
                  {clientDetails.client.isActive ? 'Active' : 'Inactive'}
                </p>
                {clientDetails.client.description ? (
                  <p className='mt-2 text-sm'>{clientDetails.client.description}</p>
                ) : null}
              </div>
              <div className='space-y-2'>
                <h3 className='font-medium'>Policy</h3>
                <Input
                  label='Allowed tools'
                  value={policyForm.allowedTools}
                  onChange={(allowedTools) =>
                    setPolicyForm((current) => ({ ...current, allowedTools }))
                  }
                />
                <Input
                  label='Denied tools'
                  value={policyForm.deniedTools}
                  onChange={(deniedTools) =>
                    setPolicyForm((current) => ({ ...current, deniedTools }))
                  }
                />
                <Input
                  label='Permissions'
                  value={policyForm.grantedPermissions}
                  onChange={(grantedPermissions) =>
                    setPolicyForm((current) => ({ ...current, grantedPermissions }))
                  }
                />
                <Input
                  label='Scopes'
                  value={policyForm.grantedScopes}
                  onChange={(grantedScopes) =>
                    setPolicyForm((current) => ({ ...current, grantedScopes }))
                  }
                />
                <button
                  className='bg-primary text-primary-foreground rounded-lg px-3 py-2 text-sm'
                  onClick={async () => {
                    await gql<
                      { updateMcpClientPolicy: { clientId: string } },
                      { clientId: string; input: Record<string, unknown> }
                    >(
                      UPDATE_MCP_POLICY_MUTATION,
                      {
                        clientId: selectedClientId,
                        input: {
                          allowedTools: splitCsv(policyForm.allowedTools),
                          deniedTools: splitCsv(policyForm.deniedTools),
                          grantedPermissions: splitCsv(
                            policyForm.grantedPermissions
                          ),
                          grantedScopes: splitCsv(policyForm.grantedScopes)
                        }
                      },
                      props
                    );
                    setFeedback('MCP policy updated.');
                    await Promise.all([
                      loadDrafts(),
                      loadClientDetails(selectedClientId)
                    ]);
                  }}
                  type='button'
                >
                  Update policy
                </button>
              </div>
            </div>
            <div className='space-y-2'>
              <h3 className='font-medium'>Tokens</h3>
              <Input
                label='New token name'
                value={rotateTokenName}
                onChange={setRotateTokenName}
              />
              <Checkbox
                label='Revoke existing tokens'
                checked={revokeExistingTokens}
                onChange={setRevokeExistingTokens}
              />
              <button
                className='bg-primary text-primary-foreground rounded-lg px-3 py-2 text-sm'
                onClick={async () => {
                  const result = await gql<
                    { rotateMcpClientToken: { plaintextToken: string } },
                    { clientId: string; input: Record<string, unknown> }
                  >(
                    ROTATE_MCP_TOKEN_MUTATION,
                    {
                      clientId: selectedClientId,
                      input: {
                        tokenName: rotateTokenName || null,
                        revokeExistingTokens
                      }
                    },
                    props
                  );
                  setPlaintextToken(result.rotateMcpClientToken.plaintextToken);
                  setFeedback('MCP token rotated.');
                  await Promise.all([
                    loadDrafts(),
                    loadClientDetails(selectedClientId)
                  ]);
                }}
                type='button'
              >
                Rotate token
              </button>
              {clientDetails.tokens.map((token) => (
                <div className='border-border border-t py-2 text-sm' key={token.id}>
                  <div className='flex items-center justify-between gap-3'>
                    <span className='font-medium'>{token.tokenName}</span>
                    <span className='text-muted-foreground'>
                      {token.isActive ? 'Active' : 'Inactive'}
                    </span>
                  </div>
                  <p className='font-mono text-xs'>{token.tokenPreview}</p>
                  {token.expiresAt ? (
                    <p className='text-muted-foreground text-xs'>
                      Expires: {token.expiresAt}
                    </p>
                  ) : null}
                  <button
                    className='border-border mt-2 rounded-lg border px-2 py-1 text-xs disabled:opacity-50'
                    disabled={!token.isActive}
                    onClick={async () => {
                      await gql<
                        { revokeMcpToken: boolean },
                        { tokenId: string; reason: string | null }
                      >(
                        REVOKE_MCP_TOKEN_MUTATION,
                        { tokenId: token.id, reason: managementReason || null },
                        props
                      );
                      setFeedback('MCP token revoked.');
                      await Promise.all([
                        loadDrafts(),
                        loadClientDetails(selectedClientId)
                      ]);
                    }}
                    type='button'
                  >
                    Revoke
                  </button>
                </div>
              ))}
              {clientDetails.tokens.length === 0 ? (
                <p className='text-muted-foreground text-sm'>No tokens.</p>
              ) : null}
            </div>
            <div className='space-y-2 lg:col-span-2'>
              <Input
                label='Management reason'
                value={managementReason}
                onChange={setManagementReason}
              />
              <button
                className='border-destructive text-destructive rounded-lg border px-3 py-2 text-sm disabled:opacity-50'
                disabled={!clientDetails.client.isActive}
                onClick={async () => {
                  await gql<
                    { deactivateMcpClient: boolean },
                    { clientId: string; reason: string | null }
                  >(
                    DEACTIVATE_MCP_CLIENT_MUTATION,
                    {
                      clientId: selectedClientId,
                      reason: managementReason || null
                    },
                    props
                  );
                  setFeedback('MCP client deactivated.');
                  await Promise.all([
                    loadDrafts(),
                    loadClientDetails(selectedClientId)
                  ]);
                }}
                type='button'
              >
                Deactivate client
              </button>
            </div>
          </div>
        ) : (
          <p className='text-muted-foreground text-sm'>No client selected.</p>
        )}
      </section>

      <section className='border-border bg-card space-y-4 rounded-lg border p-4'>
        <div className='flex items-center justify-between gap-3'>
          <h2 className='text-foreground text-base font-semibold'>Audit events</h2>
          <button
            className='border-border rounded-lg border px-3 py-1.5 text-sm'
            disabled={loading}
            onClick={() => void loadDrafts()}
            type='button'
          >
            Refresh
          </button>
        </div>
        <div className='overflow-x-auto'>
          <table className='w-full min-w-[720px] text-left text-sm'>
            <thead className='text-muted-foreground border-b'>
              <tr>
                <th className='px-2 py-2 font-medium'>Time</th>
                <th className='px-2 py-2 font-medium'>Action</th>
                <th className='px-2 py-2 font-medium'>Outcome</th>
                <th className='px-2 py-2 font-medium'>Tool</th>
                <th className='px-2 py-2 font-medium'>Actor</th>
                <th className='px-2 py-2 font-medium'>Reason</th>
              </tr>
            </thead>
            <tbody>
              {auditEvents.map((event) => (
                <tr className='border-border border-b last:border-0' key={event.id}>
                  <td className='whitespace-nowrap px-2 py-2'>{event.createdAt}</td>
                  <td className='px-2 py-2 font-medium'>{event.action}</td>
                  <td className='px-2 py-2'>{event.outcome}</td>
                  <td className='px-2 py-2'>{event.toolName ?? 'Control plane'}</td>
                  <td className='px-2 py-2'>{event.actorType ?? 'Unknown'}</td>
                  <td className='max-w-64 truncate px-2 py-2'>{event.reason ?? '-'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {auditEvents.length === 0 && !loading ? (
          <p className='text-muted-foreground text-sm'>No audit events.</p>
        ) : null}
      </section>
    </div>
  );
}

function Input({
  label,
  value,
  onChange,
  placeholder
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
}) {
  return (
    <label className='block space-y-1 text-sm'>
      <span className='text-muted-foreground'>{label}</span>
      <input
        className='border-input bg-background w-full rounded-lg border px-3 py-2'
        placeholder={placeholder}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}

function Checkbox({
  label,
  checked,
  onChange
}: {
  label: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <label className='border-border flex items-center gap-2 rounded-lg border px-3 py-2 text-sm'>
      <input
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
        type='checkbox'
      />
      {label}
    </label>
  );
}

function splitCsv(value: string): string[] {
  return value
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
}

function formatJsonForDisplay(value: string): string {
  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value;
  }
}

export default McpAdminPage;
