# rustok-ai

`rustok-ai` is RusToK's AI host/orchestrator capability crate.

It sits above `rustok-mcp`, keeps model-provider orchestration out of `rustok-mcp`, and owns the
typed runtime contracts for provider profiles, task profiles, hybrid direct/MCP execution,
chat sessions, runs, traces, and approval-gated tool execution.

Current implementation includes:
- multiprovider routing for `OpenAI-compatible`, `Anthropic`, and `Gemini`
- AI-task RBAC permissions consumed from `rustok-core` / `rustok-rbac`
- multilingual locale-aware session/run contracts with arbitrary BCP-47-style locale tags
- direct task-job execution for first-party verticals `alloy_code`, `image_asset`, `product_copy`,
  and `blog_draft`
- bounded live streaming for provider-backed chat/text runs across `OpenAI-compatible`,
  `Anthropic`, and `Gemini` through `aiSessionEvents` over the existing GraphQL WebSocket transport
- bounded cached recent stream-event history available for diagnostics and session inspection
  through `AiManagementService::recent_stream_events(...)` and the server-side
  `aiRecentRunStreamEvents` query
- bounded recent run history for diagnostics through `AiManagementService::list_recent_runs(...)`
  and the server-side `aiRecentRuns` query
- bounded runtime observability via `AiManagementService::metrics_snapshot()` plus Prometheus
  module/span telemetry for router decisions and direct/MCP run outcomes
- shared admin surfaces for Leptos and Next.js hosts
- dedicated AI diagnostics sub-routes for both admin hosts (`/ai/diagnostics`, `/dashboard/ai/diagnostics`)

The current implementation is sufficient to treat `rustok-ai` as MVP-complete for the initial
RusToK AI host/orchestrator scope. Remaining work is post-MVP depth, not missing MVP foundation.

## Responsibilities

- Expose a provider-agnostic AI runtime centered on the `ModelProvider` trait.
- Ship native provider adapters for `OpenAI-compatible`, `Anthropic`, and `Gemini`.
- Support native streaming text/tool-call parsing for `OpenAI-compatible`, `Anthropic`, and
  `Gemini` provider-backed runs.
- Orchestrate chat runs, direct-vs-MCP execution selection, MCP tool calls, and approval flows.
- Own task-profile-driven routing through `AiRouter` and typed execution decisions.
- Persist requested/resolved locale metadata on AI sessions and runs.
- Treat admin locale fields as optional overrides; when omitted, AI runtime falls back to the
  effective request locale first, then tenant default locale, then platform fallback.
- Support direct Alloy Script Assist jobs (`list_scripts`, `get_script`, `validate_script`, `run_script`).
- Support direct media image generation jobs that persist assets through `rustok-media`.
- Support direct localized product-copy jobs that persist translations through `rustok-commerce` /
  `CatalogService`.
- Support direct blog-draft jobs that create or update localized drafts through `rustok-blog` /
  `PostService`.
- Provide the canonical persisted control-plane service layer used by `apps/server`.
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
- Uses `apps/server` as the persisted control plane for provider profiles, tool profiles, sessions,
  task profiles, runs, traces, and approvals.
- Uses the shared GraphQL WebSocket surface in `apps/server` for live AI run streaming.
- Ships a Leptos admin UI package in `crates/rustok-ai/admin`.
- Ships a Next.js admin UI package through `apps/next-admin/packages/rustok-ai`.

## Entry points

- `ModelProvider`
- `OpenAiCompatibleProvider`
- `AnthropicProvider`
- `GeminiProvider`
- `AiRuntime`
- `AiRouter`
- `McpClientAdapter`
- `ToolExecutionPolicy`
- `ProviderProfile`, `TaskProfile`, `ExecutionMode`, `ExecutionOverride`
- `ChatSession`, `ChatMessage`, `ChatRun`
- `ToolTrace`
- `ApprovalRequest`, `ApprovalDecision`
- `AiManagementService` (`server` feature)

## Documentation

- Local component docs: [`./docs/`](./docs/)
- Leptos admin UI package: [`./admin/README.md`](./admin/README.md)
- Platform docs map: [`../../docs/index.md`](../../docs/index.md)
