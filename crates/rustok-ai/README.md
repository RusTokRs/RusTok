# rustok-ai

## Purpose

`rustok-ai` is RusToK's AI host/orchestrator capability crate.

It sits above `rustok-mcp`, keeps model-provider orchestration out of `rustok-mcp`, and owns the
typed runtime contracts for provider profiles, task profiles, hybrid direct/MCP execution,
chat sessions, runs, traces, and approval-gated tool execution.

Current implementation includes:
- Rig 0.39 as the canonical inference engine, with registry-driven provider profiles and
  built-in Rig providers plus Bedrock, Vertex AI, Gemini gRPC, and optional FastEmbed
- persisted `ProviderSlug`, deployment-owned `ProviderTargetId`, external `SecretRef` credential
  references, and server-owned resolver/egress policies
- AI-task RBAC permissions consumed from `rustok-core` / `rustok-rbac`
- generic agent-principal, owner-contributed agent catalog, and workflow-stage
  contracts; effective permissions are the intersection of the initiating
  subject and agent principal
- closed agent taxonomy: `product`, `code`, `orchestrator`, and `review`; only
  owner modules may publish descriptors and workflows in those categories
- multilingual locale-aware session/run contracts with arbitrary BCP-47-style locale tags
- direct task-job execution for first-party verticals `alloy_code`, `image_asset`, `product_copy`,
  and `blog_draft`
- bounded live streaming for provider-backed chat/text runs across `OpenAI-compatible`,
  `Anthropic`, and `Gemini` through `aiSessionEvents` over the existing GraphQL WebSocket transport
- bounded cached recent stream-event history available for diagnostics and session inspection
  through `AiManagementService::recent_stream_events(...)` and the server-side
  `aiRecentRunStreamEvents` query
- bounded recent run history for diagnostics through `AiManagementService::list_recent_runs(...)`
  and the owner-owned `aiRecentRuns` query
- provider-neutral RAG contracts for source-addressable documents, deterministic bounded
  chunking, provider-owned ingestion publication, bounded embedding batches through the
  existing Rig entrypoint, lexical/structural retrieval, citations, and data-only context
  injection
- the owner-neutral `AiStructuredTaskPort` contract for bounded billable
  structured execution, health, durable execution/status identity, usage/cost
  evidence, polling, and cancellation. Every execution exposes a content-free
  request binding (owner/task identity, policy/schema/input/evidence digests,
  classification, and limits) so an owner adapter can verify durable recovery
  without retaining raw inputs. A content-free execution/attempt ledger,
  idempotent registration, leases, cancellation receipts, durable
  pre-registration cancellation intents keyed by owner/idempotency, recovery, tenant
  budget reservations, immutable provider price and egress-classification
  policies, and an exact registered-task catalog now back the implementation
  track. Every tenant provider policy declares a non-empty classification
  allowlist. Candidate selection, estimates/reservations, classification-aware
  health, and provider-slot acquisition enforce it; a denied classification
  fails before execution registration or provider egress, and the slot
  acquisition rechecks it against concurrent policy changes. Provider attempt
  slots, immutable per-attempt price snapshots, actual token/cost recording,
  slot release, accounting-aware expired-lease recovery, and an encrypted
  TTL-bound result handoff are also durable. Successful attempt evidence,
  result insertion, slot release, budget settlement, and terminal execution
  commit atomically; tenant-scoped replay authenticates identity and digests,
  supports retained-key rotation, and never re-bills after expiry. The private
  structured runtime now performs exact descriptor validation, deterministic
  ordered inference/fallback, typed failure mapping, deadline enforcement, and
  durable cancellation observation. Permission-checked GraphQL/native
  contracts now provision tenant budget and provider policies, while the
  result keyring remains deployment-owned. The existing AI scheduler adapter
  performs queued-cancellation recovery, expired-lease reconciliation, and
  expired-result cleanup before claims. The production server selects the
  optional distribution bridge, which publishes a Translation-owned lazy
  runtime factory without server capability imports. Composed missing-keyring
  evidence is optional and fail-closed. Deterministic composed runtime evidence
  covers ordered provider fallback, fail-closed JSON Schema enforcement,
  sanitized failure recording, exact attempt usage/cost settlement,
  request-conflict rejection, in-flight cancellation with reservation release,
  quota rejection before provider execution, and encrypted restart replay
  without another provider call or bill. Configuration-level provider
  unavailability also produces typed degraded health. Live external-provider
  failure/restart evidence remains an activation prerequisite for machine
  translation; an ignored operator-only structured-runtime probe is available
  for collecting that deployment evidence. A separate-process file-backed
  recovery test also proves abandoned-attempt reconciliation, reservation
  preservation, and reclaim with a new lease
- owner-owned GraphQL query, mutation, subscription, and DTO surfaces under `graphql`, with
  host-specific role lookup supplied through `AiGraphqlRoleSlugProviderHandle`
- host-neutral `AiHostRuntime` for GraphQL mutations, direct execution, and in-process MCP
  execution; the capability crate does not consume host-wide runtime context
- bounded runtime observability via `AiManagementService::metrics_snapshot()` plus Prometheus
  module/span telemetry for router decisions and direct/MCP run outcomes
- large operator/admin surfaces for both Leptos and Next.js hosts
- dedicated AI diagnostics sub-routes for both admin hosts (`/ai/diagnostics`, `/dashboard/ai/diagnostics`)

Rig is the sole inference path. The host boundary is composed through generic module runtime
extensions: `apps/server` transfers typed extension values and runs registered durable workers
without importing AI capability types. Remaining verification work is tracked in the module
implementation plan.

## Responsibilities

- Expose a provider-agnostic Rig engine centered on `InferenceEngine` and `RigAgentDriver`.
- Own generic agent principals and workflow orchestration contracts; domain
  modules retain their own agent descriptors and allowed operations.
- Keep provider descriptions, target-bound connection settings, credentials, and feature declarations
  in owner-controlled registries.
- Delegate streaming protocol handling, tool-call assembly, and structured output constraints to Rig.
- Orchestrate chat runs, direct-vs-MCP execution selection, MCP tool calls, and approval flows.
- Own task-profile-driven routing through `AiRouter` and typed execution decisions.
- Persist requested/resolved locale metadata on AI sessions and runs.
- Treat admin locale fields as optional overrides; when omitted, AI runtime falls back to the
  effective request locale first, then tenant default locale, then platform fallback.
- Support direct Alloy Script Assist jobs (`list_scripts`, `get_script`, `validate_script`, `run_script`)
  against the canonical multi-file Alloy workspace contract.
- Support direct media image generation jobs that persist assets through `rustok-media`.
- Support direct localized product-copy jobs that persist translations through `rustok-commerce` /
  `CatalogService`.
- Support direct blog-draft jobs that create or update localized drafts through `rustok-blog` /
  `PostService`.
- Provide the canonical capability-owned persisted control-plane service layer through a
  host-neutral runtime contribution.
- Publish in-process runtime observability snapshots for router and run health.
- Publish session-scoped live run events (`started`, `delta`, `completed`, `failed`,
  `waiting_approval`) for operator/admin surfaces.
- Keep a bounded recent-event cache so diagnostics and session detail surfaces can inspect
  the latest streaming history even outside an active WebSocket subscription.
- Expose recent persisted run summaries with status, latency, locale, provider, and execution
  target metadata for diagnostics/history views in both admin hosts.
- Expose diagnostics breakdowns for provider kind, execution target, task profile, and resolved
  locale buckets in shared admin surfaces.
- Enforce the AI-host boundary separately from the MCP server boundary owned by `rustok-mcp`.
- Consume RBAC permissions from `rustok-core`/`rustok-rbac` instead of owning authorization.

## Interactions

- Uses `rustok-mcp` as the MCP server/tool surface.
- Uses direct execution mode for first-party platform workflows and MCP execution mode for
  tool/agent boundaries.
- Direct first-party verticals currently include:
  `alloy_code` for Alloy Script Assist, `image_asset` for image generation + media persistence,
  `product_copy` for tenant-locale-bound commerce translation updates, and `blog_draft` for
  tenant-locale-bound blog draft creation/update.
- Uses a host-provided database connection for provider profiles, tool profiles, sessions, task
  profiles, runs, traces, and approvals.
- Owns AI GraphQL resolvers and native transport contributions; the future host consumes only
  their generic module contribution contract.
- Ships a large Leptos operator/admin UI package in `crates/rustok-ai/admin`.
- Ships a large Next.js operator/admin UI package through `apps/next-admin/packages/rustok-ai`.

## Entry points

- `ProviderSlug`, `ProviderFeature`, `provider_catalog()`
- `ProviderTargetId`, `AiProviderTargetCatalog` (`RUSTOK_AI_PROVIDER_TARGETS_JSON` deployment config)
- `RUSTOK_AI_SECRET_RESOLVERS_JSON` deployment config for named env, mounted-file, Vault,
  Kubernetes, AWS Secrets Manager, GCP Secret Manager, and Azure Key Vault resolvers
- `InferenceEngine`, `RigAgentDriver`, `inference_for_slug(...)`
- `embed(...)`, `rerank(...)`
- `AiRouter`
- `AiStructuredTaskPort`, `AiStructuredTaskRequest`,
  `AiStructuredTaskExecution`
- `RUSTOK_AI_STRUCTURED_RESULT_KEYRING_JSON` deployment config for the
  AES-256-GCM transient-result active key, retained rotation keys, and TTL
- `McpClientAdapter`
- `ToolExecutionPolicy`
- `ProviderProfile`, `TaskProfile`, `ExecutionMode`, `ExecutionOverride`
- `ChatSession`, `ChatMessage`, `ChatRun`
- `ToolTrace`
- `AgentPrincipal`, `AgentCatalog`, `AgentWorkflowDescriptor`
- `ApprovalRequest`, `ApprovalDecision`
- `AiManagementService` (`server` feature)
- `AiHostRuntime` (`server` feature)
- `graphql::{AiQuery, AiMutation, AiSubscription}` (`graphql` feature)
- `AiGraphqlRoleSlugProvider`, `AiGraphqlRoleSlugProviderHandle` (`graphql` feature)

## Docs

- [Module docs](./docs/README.md)
- Leptos admin UI package: [`./admin/README.md`](./admin/README.md)
- Platform docs map: [`../../docs/index.md`](../../docs/index.md)

## Deployment secret resolvers

`RUSTOK_AI_SECRET_RESOLVERS_JSON` is process-owned configuration. Each entry has a unique
`alias`, non-empty `key_prefixes`, and a `kind`: `env`, `mounted_file`, `vault`, `kubernetes`,
`aws_secrets_manager`, `gcp_secret_manager`, or `azure_key_vault`. Tenant profiles persist only
the alias/key `SecretRef`; they cannot supply endpoints, namespaces, cloud projects, or identity
settings. When this variable is absent, the deployment retains the safe legacy `env` resolver
with the `RUSTOK_AI_` prefix and can optionally enable the mounted-file resolver through
`RUSTOK_AI_SECRET_MOUNT_ROOT`.

| Kind | Deployment-only fields |
| --- | --- |
| `env` | `alias`, `key_prefixes` |
| `mounted_file` | `alias`, `root`, `key_prefixes` |
| `vault` | `alias`, `endpoint`, optional `namespace`, `kv_mount`, `key_prefixes`, and either `token_env`/`token_file` or the three Kubernetes auth fields (`kubernetes_role`, `kubernetes_auth_mount`, `kubernetes_token_path`) |
| `kubernetes` | `alias`, `namespace`, `key_prefixes`; in-cluster identity only |
| `aws_secrets_manager` | `alias`, `key_prefixes`; default AWS credential chain only |
| `gcp_secret_manager` | `alias`, `project`, `key_prefixes`; ADC/workload identity only |
| `azure_key_vault` | `alias`, HTTPS `endpoint`, `key_prefixes`; default Azure credential only |

The process rejects duplicate aliases, blank prefixes, ambiguous Vault auth, invalid endpoint
shapes, and invalid cloud coordinates before making a resolver available to tenant profiles.

`RUSTOK_AI_STRUCTURED_RESULT_KEYRING_JSON` is optional and fail-closed. When
absent, the billable structured-task runtime is not published. Its shape is:

```json
{
  "active_key_id": "v2",
  "retention_seconds": 86400,
  "keys": {
    "v1": {"resolver": "deployment_env", "key": "RUSTOK_AI_RESULT_KEY_V1"},
    "v2": {"resolver": "deployment_env", "key": "RUSTOK_AI_RESULT_KEY_V2"}
  }
}
```

Each referenced secret is standard Base64 for exactly 32 bytes. Keep retired
key references configured until every row encrypted with that key has passed
the retention window and scheduled cleanup.
