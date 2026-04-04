# План реализации `rustok-ai`

Статус: MVP complete.
Текущее состояние: `OpenAI-compatible + Anthropic + Gemini providers + task profiles + hybrid direct/MCP execution metadata + RBAC-first AI permissions + dual admin UI packages + direct first-party verticals + streaming + diagnostics`.

## Состояние на 2026-04-04

`rustok-ai` уже существует как отдельный capability crate и не расширяет `rustok-mcp` до model host.

Что уже закрыто:

- выделен отдельный crate `crates/rustok-ai`;
- реализован provider abstraction через `ModelProvider`;
- добавлен `OpenAI-compatible` provider для cloud/local endpoint'ов;
- поднят `AiRuntime` с request/response orchestration;
- добавлен `McpClientAdapter` для вызова RusToK MCP tools;
- введён persisted control plane в `apps/server`;
- добавлены GraphQL queries/mutations для providers, tool profiles, sessions, traces и approvals;
- добавлен Leptos admin package `crates/rustok-ai/admin`;
- добавлен Next.js admin package `apps/next-admin/packages/rustok-ai`;
- добавлен real direct execution path для first-party verticals без обязательного MCP hop;
- реализованы direct verticals `alloy_code`, `image_asset`, `product_copy`, `blog_draft`;
- `product_copy` пишет локализованные переводы товаров напрямую через `rustok-commerce::CatalogService`;
- `blog_draft` создаёт или обновляет локализованные черновики напрямую через `rustok-blog::PostService`;
- multilingual contract принимает arbitrary BCP-47-style locale tags, а tenant locale policy применяется к content-bearing задачам вроде `product_copy`;
- multilingual contract также применяется к content-bearing blog flows, поэтому `blog_draft` использует tenant locale policy, а не free-locale path;
- `apps/admin` и `apps/next-admin` оставлены в роли host/composition root.

## MVP: закрыто

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

- [x] миграция control-plane таблиц
- [x] CRUD provider profiles
- [x] CRUD task profiles
- [x] CRUD tool profiles
- [x] start/send/resume/cancel chat runs
- [x] trace persistence для MCP tool calls
- [x] approval persistence для sensitive tool execution
- [x] test-connection flow для provider profile
- [x] runtime metrics snapshot
- [x] recent stream-event cache и recent persisted run history

### API

- [x] GraphQL surface для headless/Next.js
- [x] native `#[server]` functions как preferred internal data layer для Leptos UI
- [x] dual-path contract без удаления GraphQL
- [x] GraphQL subscription `aiSessionEvents`
- [x] diagnostics queries `aiRecentRunStreamEvents` и `aiRecentRuns`

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
- [x] direct job surfaces для `alloy_code`, `image_asset`, `product_copy`, `blog_draft`
- [x] focused diagnostics sub-route внутри AI surface для Leptos и Next.js hosts
- [x] diagnostics snapshot enriched with task-profile and resolved-locale buckets in both hosts
- [x] diagnostics recent stream history and recent run history in both hosts

## Зафиксированные архитектурные решения

1. `rustok-ai` — capability crate, а не platform module.
2. `rustok-mcp` остаётся MCP server boundary.
3. Provider abstraction живёт вне `rustok-mcp`.
4. Leptos и Next.js UI поставляются отдельными capability-owned пакетами.
5. Для Leptos internal data layer остаётся native `#[server]` first, GraphQL parallel.

## Итог по MVP

Текущий MVP для `rustok-ai` можно считать закрытым. Он уже покрывает:

- multiprovider AI runtime;
- RBAC-first AI access model;
- hybrid direct/MCP execution;
- multilingual locale-aware contract;
- persisted control plane;
- operator/admin UI для Leptos и Next.js;
- live streaming и базовую diagnostics/observability surface.

Дальнейшие пункты больше не относятся к обязательному MVP-контруру и считаются post-MVP backlog.

## Post-MVP backlog

- [x] bounded token streaming / incremental assistant output for provider-backed chat/text runs
- [x] universal streaming path for provider-backed text runs across `OpenAI-compatible`,
  `Anthropic` и `Gemini`
- [x] bounded runtime observability snapshot для router/direct/MCP execution outcomes
- [x] bounded recent stream-event history queryable через `AiManagementService::recent_stream_events`
  и GraphQL `aiRecentRunStreamEvents`
- [x] direct verticals участвуют в общем streaming contract, а не только runtime/MCP path
- [x] diagnostics/history surface показывает bounded recent persisted runs через
  `AiManagementService::list_recent_runs` и GraphQL `aiRecentRuns`
- [ ] более глубокие domain-direct verticals beyond Alloy/Media/Commerce/Blog
- [ ] дополнительные provider families beyond текущих `OpenAI-compatible`, `Anthropic`, `Gemini`
- [ ] richer provider routing / fallback / multi-model policy
- [ ] полноценный remote MCP bootstrap за пределами текущего server wiring
- [ ] отдельные publish/export workflows для AI artifacts
- [ ] более богатые update/deactivate UX flows во всех admin surfaces
- [ ] time-windowed diagnostics trends и richer historical observability
- [ ] persisted provider error/fallback analytics beyond in-process snapshot

## Проверка

Минимальная локальная проверка, которой уже закрыт текущий срез:

- [x] `cargo check -p rustok-ai --features server`
- [x] `cargo check -p migration`
- [x] `cargo check -p rustok-server`
- [x] `cargo check -p rustok-ai-admin --features ssr`
- [x] `cargo check -p rustok-ai-admin --features hydrate --target wasm32-unknown-unknown`
- [x] `cargo check -p rustok-admin`
- [x] `cmd /c npx.cmd tsc --noEmit --incremental false -p tsconfig.json` в `apps/next-admin`
- [x] `cargo test -p rustok-ai --features server metrics::tests direct::tests service::tests -- --nocapture`

## Связанные документы

- [README crate](../README.md)
- [README capability docs](./README.md)
- [ADR `rustok-ai` capability module](../../../DECISIONS/2026-04-03-rustok-ai-capability-module.md)
