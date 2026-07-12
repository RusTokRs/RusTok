# Capability `rustok-ai`

`rustok-ai` is the RusToK capability crate for the AI host/orchestrator layer on top of `rustok-mcp`.

This crate is not a tenant-toggled module and is not part of the `Core` / `Optional` taxonomy.
Its task is to hold the layer between model provider and MCP tool surface, without extending `rustok-mcp`
to the role of model host.

## Purpose

- hold a provider-agnostic Rig engine contract for AI orchestration;
- provide a registry-driven Rig 0.39 runtime. `base_url` is a typed provider setting subject to
  server-owned egress policy; credentials are external `SecretRef` values, never plaintext;
- call MCP tools through a separate `McpClientAdapter`, rather than mixing provider logic with MCP server;
- store the chat/runtime model: sessions, messages, runs, tool traces, approval requests;
- provide a capability-owned canonical service layer for the persisted control plane.

## What is already implemented

### Provider/Runtime Layer

- `ProviderSlug` / `ProviderFeature` registry with typed setting and credential schemas;
- `InferenceEngine` backed by Rig 0.39;
- `RigAgentDriver` with persisted canonical history, tool policy, and approval boundaries;
- `ToolExecutionPolicy` with sensitive tool calls and approval boundary.
- `AiRouter` and direct-dispatch layer for first-party verticals without mandatory MCP hop.

### MCP integration

- `McpClientAdapter` as a separate layer on top of the RusToK MCP tool surface;
- current MVP wiring uses `rustok-mcp` and does not extend `rustok-mcp` with provider-specific responsibilities;
- Alloy/MCP tool traces and approval-gated execution are already part of the persisted chat flow.

### Persisted Control Plane and Host Composition

- tables:
  - `ai_provider_profiles`
  - `ai_tool_profiles`
  - `ai_chat_sessions`
  - `ai_chat_messages`
  - `ai_chat_runs`
  - `ai_tool_traces`
  - `ai_approval_requests`
- owner-owned GraphQL query/mutation/subscription surface in `crates/rustok-ai/src/graphql` for providers,
  tool profiles, sessions, traces and approvals;
- server-side orchestration service `AiManagementService`;
- `AiHostRuntime` is a host-neutral runtime contract; `rustok-ai` does not accept Loco
  `AppContext` and does not depend on the Loco crate;
- the target host integration is a generic manifest/runtime contribution contract. Removing
  existing direct AI construction from `apps/server` is a platform-owned prerequisite and is
  explicitly tracked as blocked in the implementation plan.
- Runtime observability now operates in two layers:
  - persisted `decision_trace` and run/session metadata in the control plane;
  - in-process `AiManagementService::metrics_snapshot()` and Prometheus module/span telemetry for router resolution and run outcomes.
- diagnostics snapshot now includes breakdown not only by provider/execution target, but also by
  task profile / resolved locale, so the operator can see routing and multilingual slices without going into raw traces.
- bounded streaming layer includes `AiRunStreamHub` in `rustok-ai`, GraphQL subscription
  `aiSessionEvents(sessionId)` in `rustok-ai` and live incremental output for operator chat /
  provider-backed text runs in both admin hosts for `OpenAI-compatible`, `Anthropic` and `Gemini`.
- direct verticals use the same streaming contract, so direct Alloy / content jobs do
  not lose the live delta/update surface compared to the runtime/MCP path.
- offline Rig protocol cassettes cover the normalized OpenAI-compatible, Anthropic,
  Gemini, cloud-auth, and deployment-local stream families; network protocol parsing
  remains inside Rig rather than reappearing as RusToK SSE code.
- in addition to live subscription, the server layer now holds a bounded recent-event cache; it is accessible
  via `AiManagementService::recent_stream_events(...)` and GraphQL query
  `aiRecentRunStreamEvents(sessionId?, limit?)` for diagnostics and session detail.
- diagnostics surface now also uses bounded recent run history from persisted control
  plane via `AiManagementService::list_recent_runs(...)` and GraphQL query
  `aiRecentRuns(limit?)` to show status/latency/provider/locale history without parsing raw traces.

### UI Packages

- major Leptos operator/admin UI package: `crates/rustok-ai/admin`;
- major Next.js operator/admin UI package: `apps/next-admin/packages/rustok-ai`;
- both UIs already support provider registry with editable `capabilities` and `usage_policy`;
- both UIs show execution metadata (`execution_mode`, `execution_path`) for session/run inspection;
- both UIs support direct job surfaces for `alloy_code`, `image_asset`, `product_copy` and `blog_draft`;
- `locale` field in admin UI is an optional override: empty value leaves the AI runtime
  to use the request locale chain (`request -> tenant default -> en`), rather than forcing `en`;
- both UIs now have focused diagnostics sub-surface for router/run observability:
  - Leptos host: `/ai/diagnostics`
  - Next host: `/dashboard/ai/diagnostics`
- both UIs now support live session stream card via `graphql-transport-ws` subscription
  `aiSessionEvents`, without replacing persisted session detail and trace view.
- both UIs now also show bounded recent stream history, even if the user opened
  diagnostics/session detail after the live stream has already ended.
- both UIs now show recent run history as a separate diagnostics slice on top of persisted
  `ai_chat_runs`, not just aggregate metrics snapshot.
- both hosts act only as composition root:
  - `apps/admin` mounts the Leptos package;
  - `apps/next-admin` mounts the npm package `@rustok/ai-admin`.
- browser-target verification for the Leptos package now includes a separate `hydrate` check, so that
  the WebSocket streaming path is tested not only on SSR.

### Opt-in live provider connectivity

Live provider probes are intentionally excluded from default test gates. An
operator may run the ignored test only with deployment-owned configuration:

```powershell
$env:RUSTOK_AI_LIVE_PROVIDER_CONFIGS_JSON = '[{...}]'
cargo test -p rustok-ai --features server -- --ignored probes_each_declared_live_provider_target
```

The JSON value is a non-empty array of `AiProviderConfig` values. A credential
reference must use resolver `env` and a key starting with `RUSTOK_AI_LIVE_`;
the test registers only that prefix. Endpoints, cloud identity, model settings,
and the list of targets remain operator/deployment configuration, never tenant
input. The test makes real provider requests and must not be enabled in shared
or default CI.

## Scope

### What stays in `rustok-ai`

- orchestration runtime;
- provider abstraction;
- direct first-party execution registry;
- chat/session/approval contracts;
- server-side management service;
- GraphQL query/mutation/subscription roots, DTO and permission checks;
- capability-owned large operator/admin UI packages.

### What stays in `rustok-mcp`

- MCP server transport/protocol boundary;
- tool surface and identity/policy/runtime binding;
- absence of provider-specific orchestration and model-host responsibilities.

### Host boundary

- `apps/server` must remain capability-neutral: it may consume generic module runtime and
  transport contributions, but it must not import AI-specific runtime, GraphQL, secret, or
  provider-policy types;
- the platform owner supplies the generic contribution mechanism; `rustok-ai` supplies the
  AI contribution through that mechanism once it exists.

## What is not yet implemented

- time-windowed diagnostics/trends on top of the current snapshot/history surface;
- persisted provider fallback/error analytics beyond the current in-process snapshot;
- additional provider families beyond those already implemented (`Anthropic`, `Gemini`, richer native adapters);
- remote MCP bootstrap beyond the current Rustok server wiring;
- separate marketplace/publish flow for AI artifacts.

## Related documents

- [README crate](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [README crate `rustok-mcp`](../../rustok-mcp/README.md)
- [Platform documentation map](../../../docs/index.md)
