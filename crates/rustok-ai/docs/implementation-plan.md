# `rustok-ai` — Implementation Plan

Status: MVP complete.
Current state: `OpenAI-compatible + Anthropic + Gemini providers + task profiles + hybrid direct/MCP execution metadata + RBAC-first AI permissions + dual admin UI packages + direct first-party verticals + streaming + diagnostics`.

## Execution checkpoint

- Current phase: ai_admin_loco_free_native_runtime
- Last checkpoint: AI admin native server functions now consume `HostRuntimeContext` instead of Loco `AppContext`; DB, `TransactionalEventBus`, `SharedAiModuleRegistry`, `StorageService` and `SharedAlloyRuntime` are resolved through neutral host runtime handles, while the GraphQL/headless adapter remains parallel.
- Next step: Continue extracting remaining AI-specific host artifacts from `apps/server`, keeping only composition adapters there; keep GraphQL parity and live runtime evidence in the next transport-verification slices.
- Open blockers: Full end-to-end browser/runtime parity evidence remains pending; source guardrails and `rustok-ai-admin` SSR compile evidence are present.
- Hand-off notes for next agent: Update this block and the central FFA/FBA readiness board after each increment.
- Last updated at (UTC): 2026-07-08T07:12:30Z

## State as of 2026-04-04

`rustok-ai` already exists as a separate capability crate and does not extend `rustok-mcp` to a model host.

What is already closed:

- separate crate `crates/rustok-ai` created;
- provider abstraction implemented via `ModelProvider`;
- `OpenAI-compatible` provider added for cloud/local endpoints;
- `AiRuntime` with request/response orchestration stood up;
- `McpClientAdapter` added for calling RusToK MCP tools;
- persisted control plane introduced in `apps/server`;
- owner-owned GraphQL queries/mutations/subscriptions for providers, tool profiles, sessions,
  traces and approvals added in `crates/rustok-ai/src/graphql`;
- Leptos admin package `crates/rustok-ai/admin` added;
- Next.js admin package `apps/next-admin/packages/rustok-ai` added;
- real direct execution path for first-party verticals added without mandatory MCP hop;
- direct verticals `alloy_code`, `image_asset`, `product_copy`, `blog_draft` implemented;
- `product_copy` writes localized product translations directly via `rustok-commerce::CatalogService`;
- `blog_draft` creates or updates localized drafts directly via `rustok-blog::PostService`;
- multilingual contract accepts arbitrary BCP-47-style locale tags, and tenant locale policy applies to content-bearing tasks like `product_copy`;
- multilingual contract also applies to content-bearing blog flows, so `blog_draft` uses tenant locale policy rather than the free-locale path;
- `apps/admin` and `apps/next-admin` remain in the host/composition root role.

## MVP: closed

### Backend/runtime

- [x] `ModelProvider`
- [x] `OpenAiCompatibleProvider`
- [x] `AnthropicProvider`
- [x] `GeminiProvider`
- [x] `AiRuntime`
- [x] `AiRouter`
- [x] `DirectExecutionRegistry`
- [x] `ToolExecutionPolicy`
- [x] `ChatSession`, `ChatMessage`, `ChatRun`
- [x] `ToolTrace`
- [x] `ApprovalRequest`, `ApprovalDecision`
- [x] `AiManagementService`

### Persisted server control plane

- [x] control-plane table migration
- [x] CRUD provider profiles
- [x] CRUD task profiles
- [x] CRUD tool profiles
- [x] start/send/resume/cancel chat runs
- [x] trace persistence for MCP tool calls
- [x] approval persistence for sensitive tool execution
- [x] test-connection flow for provider profile
- [x] runtime metrics snapshot
- [x] recent stream-event cache and recent persisted run history

### API

- [x] GraphQL surface for headless/Next.js
- [x] GraphQL roots and DTO belong to `rustok-ai`; `apps/server` only composes schema and host adapters
- [x] native `#[server]` functions as preferred internal data layer for Leptos UI
- [x] dual-path contract without removing GraphQL
- [x] GraphQL subscription `aiSessionEvents`
- [x] diagnostics queries `aiRecentRunStreamEvents` and `aiRecentRuns`

### UI

- [x] Leptos package `crates/rustok-ai/admin`
- [x] Next.js package `apps/next-admin/packages/rustok-ai`
- [x] provider profile create/test flow
- [x] provider profile update/deactivate flow
- [x] provider capability/usage-policy edit flow
- [x] task profile create/update flow
- [x] tool profile create flow
- [x] operator chat sessions
- [x] session/run execution metadata in admin UI
- [x] bounded live streaming for provider-backed chat/session runs via `aiSessionEvents`
- [x] tool trace panel
- [x] approval actions approve/reject
- [x] direct job surfaces for `alloy_code`, `image_asset`, `product_copy`, `blog_draft`
- [x] focused diagnostics sub-route inside AI surface for Leptos and Next.js hosts
- [x] diagnostics snapshot enriched with task-profile and resolved-locale buckets in both hosts
- [x] diagnostics recent stream history and recent run history in both hosts

## Fixed architectural decisions

1. `rustok-ai` — capability crate, not platform module.
2. `rustok-mcp` remains MCP server boundary.
3. Provider abstraction lives outside `rustok-mcp`.
4. Leptos and Next.js UI are shipped as separate capability-owned packages.
5. For Leptos, internal data layer remains native `#[server]` first, GraphQL parallel.

## MVP summary

The current MVP for `rustok-ai` can be considered closed. It already covers:

- multiprovider AI runtime;
- RBAC-first AI access model;
- hybrid direct/MCP execution;
- multilingual locale-aware contract;
- persisted control plane;
- operator/admin UI for Leptos and Next.js;
- live streaming and basic diagnostics/observability surface.

Further items are no longer part of the mandatory MVP contour and are considered post-MVP backlog.

## Post-MVP backlog

- [x] bounded token streaming / incremental assistant output for provider-backed chat/text runs
- [x] universal streaming path for provider-backed text runs across `OpenAI-compatible`,
  `Anthropic` and `Gemini`
- [x] bounded runtime observability snapshot for router/direct/MCP execution outcomes
- [x] bounded recent stream-event history queryable via `AiManagementService::recent_stream_events`
  and GraphQL `aiRecentRunStreamEvents`
- [x] direct verticals participate in the common streaming contract, not just runtime/MCP path
- [x] diagnostics/history surface shows bounded recent persisted runs via
  `AiManagementService::list_recent_runs` and GraphQL `aiRecentRuns`
- [ ] deeper domain-direct verticals beyond Alloy/Media/Commerce/Blog
- [ ] additional provider families beyond current `OpenAI-compatible`, `Anthropic`, `Gemini`
- [ ] richer provider routing / fallback / multi-model policy
- [ ] full remote MCP bootstrap beyond current server wiring
- [ ] separate publish/export workflows for AI artifacts
- [ ] richer update/deactivate UX flows in all admin surfaces
- [ ] time-windowed diagnostics trends and richer historical observability
- [ ] persisted provider error/fallback analytics beyond in-process snapshot

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui` for the AI admin slice.
- Evidence: domain support crates `rustok-ai-product`, `rustok-ai-content` and `rustok-ai-order` expose `register_*_ai_vertical_handlers` adapter APIs consumed by `crates/rustok-ai/src/direct_domain_*.rs`; `rustok-ai-content` also owns `blog_draft` task/tool identity plus generated draft validation and compile-free static contract evidence, so direct handler binding follows domain-owned descriptors while `rustok-ai` remains the runtime composition owner; `crates/rustok-ai/admin/src/core.rs` owns Leptos-free request normalization, direct-job payload builders (`parse_csv`, `optional_text`, `alloy_task_payload`, `image_task_payload`, `product_task_payload`, `product_attributes_task_payload`, `blog_task_payload`) and diagnostics summary policy (`average_latency_ms`, `summarize_recent_runs`), `admin/src/transport/mod.rs` owns current facade, `admin/src/transport/native_server_adapter.rs` owns Loco-free native server-function endpoints through `HostRuntimeContext` shared handles, `admin/src/transport/graphql_adapter.rs` owns Leptos-free GraphQL/headless operation documents, request builders and live-stream GraphQL WebSocket endpoint/message construction, `admin/src/ui/leptos.rs` remains the explicit Leptos adapter consuming `core` + `transport`, and `admin/src/lib.rs` only wires/re-exports module layers.
- Guardrail: `scripts/verify/verify-ai-admin-boundary.mjs` checks core/transport slice, including diagnostics summary helpers, GraphQL/headless adapter markers and live-stream WebSocket message builders, and prevents moved request/payload helpers or raw `api::` calls from returning to the Leptos adapter.
- Static evidence: `scripts/verify/verify-ai-domain-verticals.mjs` locks product/content/order/media/alloy support-crate descriptors, runtime binding seams, generated payload validators, media size validation, alloy execution policy, and content moderation sensitive-tool policy merge without compiling; `scripts/verify/verify-ai-router-policy.mjs` locks router candidate status taxonomy, selected/fallback decision-trace evidence and unit-test markers for provider allow/deny/role/capability fallback policy without compiling.
- FBA baseline evidence: `crates/rustok-ai/contracts/ai-fba-registry.json` locks `rustok-ai` as the capability orchestrator that consumes support-adapter registries from `ai-content`, `ai-order`, `ai-product`, `ai-media` and `ai-alloy`; `crates/rustok-ai/contracts/evidence/ai-runtime-static-matrix.json` and `crates/rustok-ai/contracts/evidence/ai-runtime-fallback-smoke.json` source-lock direct registration, router policy and admin transport boundary fallback evidence under `scripts/verify/verify-ai-fba-baseline.mjs`.
- Runtime-order evidence: `crates/rustok-ai/contracts/evidence/ai-orchestrator-runtime-order-smoke.json` is verified by `scripts/verify/verify-orchestrator-fba-runtime-order.mjs` and locks support-adapter registry parity, direct runtime binding registration APIs, router fallback diagnostics markers, native server-function facade and parallel GraphQL/headless admin transport markers without compilation.
- Next step: continue parity/evidence hardening for domain-owned support crates, surface router candidate explanations in persisted diagnostics, and expand targeted verification evidence when compilation is allowed, without removing existing runtime composition in `rustok-ai`.

## Verification

Minimum local verification already covering the current slice:

- [x] `cargo check -p rustok-ai --features server`
- [x] `cargo check -p migration`
- [x] `cargo check -p rustok-server`
- [x] `cargo check -p rustok-ai-admin --features ssr`
- [x] `cargo check -p rustok-ai-admin --features hydrate --target wasm32-unknown-unknown`
- [x] `npm run verify:ai:admin-boundary`
- [x] `node scripts/verify/verify-api-surface-contract.mjs`
- [x] `cargo check -p rustok-admin`
- [x] `cmd /c npx.cmd tsc --noEmit --incremental false -p tsconfig.json` in `apps/next-admin`
- [x] `cargo test -p rustok-ai --features server metrics::tests direct::tests service::tests -- --nocapture`

## Related documents

- [README crate](../README.md)
- [README capability docs](./README.md)
- [ADR `rustok-ai` capability module](../../../DECISIONS/2026-04-03-rustok-ai-capability-module.md)


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and relevance of `README.md` and local docs.
- [ ] Lock/update verification gates for the current module state.
