# План реализации `rustok-ai`

Статус: multiprovider/routing foundation implemented.
Текущее состояние: `OpenAI-compatible + Anthropic + Gemini providers + task profiles + hybrid direct/MCP execution metadata + RBAC-first AI permissions + dual admin UI packages`.

## Состояние на 2026-04-03

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
- `apps/admin` и `apps/next-admin` оставлены в роли host/composition root.

## Реализованный MVP-контур

### Backend/runtime

- [x] `ModelProvider`
- [x] `OpenAiCompatibleProvider`
- [x] `AiRuntime`
- [x] `ToolExecutionPolicy`
- [x] `ChatSession`, `ChatMessage`, `ChatRun`
- [x] `ToolTrace`
- [x] `ApprovalRequest`, `ApprovalDecision`
- [x] `AiManagementService`

### Persisted server control plane

- [x] миграция control-plane таблиц
- [x] CRUD provider profiles
- [x] CRUD tool profiles
- [x] start/send/resume/cancel chat runs
- [x] trace persistence для MCP tool calls
- [x] approval persistence для sensitive tool execution
- [x] test-connection flow для provider profile

### API

- [x] GraphQL surface для headless/Next.js
- [x] native `#[server]` functions как preferred internal data layer для Leptos UI
- [x] dual-path contract без удаления GraphQL

### UI

- [x] Leptos package `crates/rustok-ai/admin`
- [x] Next.js package `apps/next-admin/packages/rustok-ai`
- [x] provider profile create/test flow
- [x] provider capability/usage-policy edit flow
- [x] tool profile create flow
- [x] operator chat sessions
- [x] session/run execution metadata in admin UI
- [x] tool trace panel
- [x] approval actions approve/reject

## Зафиксированные архитектурные решения

1. `rustok-ai` — capability crate, а не platform module.
2. `rustok-mcp` остаётся MCP server boundary.
3. Provider abstraction живёт вне `rustok-mcp`.
4. Leptos и Next.js UI поставляются отдельными capability-owned пакетами.
5. Для Leptos internal data layer остаётся native `#[server]` first, GraphQL parallel.

## Что отложено после MVP

- [ ] token streaming / incremental assistant output
- [ ] дополнительные provider families кроме `OpenAI-compatible`
- [ ] richer provider routing / fallback / multi-model policy
- [ ] полноценный remote MCP bootstrap за пределами текущего server wiring
- [ ] отдельные publish/export workflows для AI artifacts
- [ ] более богатые update/deactivate UX flows во всех admin surfaces

## Проверка

Минимальная локальная проверка, которой уже закрыт текущий срез:

- [x] `cargo check -p rustok-ai --features server`
- [x] `cargo check -p migration`
- [x] `cargo check -p rustok-server`
- [x] `cargo check -p rustok-ai-admin --features ssr`
- [ ] Next.js host build/typecheck после `npm install`

## Связанные документы

- [README crate](../README.md)
- [README capability docs](./README.md)
- [ADR `rustok-ai` capability module](../../../DECISIONS/2026-04-03-rustok-ai-capability-module.md)
