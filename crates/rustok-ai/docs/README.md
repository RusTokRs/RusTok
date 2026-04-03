# Capability `rustok-ai`

`rustok-ai` — capability-crate RusToK для AI host/orchestrator слоя поверх `rustok-mcp`.

Этот crate не является tenant-toggled модулем и не входит в taxonomy `Core` / `Optional`.
Его задача — держать слой между model provider и MCP tool surface, не расширяя `rustok-mcp`
до роли model host.

## Назначение

- держать provider-agnostic runtime contract для AI orchestration;
- поставлять MVP provider family `OpenAI-compatible` для cloud и local endpoint'ов через `base_url`;
- вызывать MCP tools через отдельный `McpClientAdapter`, а не смешивать provider logic с MCP server;
- хранить chat/runtime model: sessions, messages, runs, tool traces, approval requests;
- отдавать `apps/server` канонический service layer для persisted control plane.

## Что уже реализовано

### Provider/runtime слой

- `ModelProvider` trait;
- `OpenAiCompatibleProvider`;
- typed request/response model для chat runs;
- `AiRuntime` с request/response loop, tool-call orchestration и error normalization;
- `ToolExecutionPolicy` с выделением sensitive tool calls и approval boundary.

### MCP integration

- `McpClientAdapter` как отдельный слой поверх RusToK MCP tool surface;
- текущий MVP wiring использует `rustok-mcp` и не расширяет `rustok-mcp` provider-specific обязанностями;
- Alloy/MCP tool traces и approval-gated execution уже входят в persisted chat flow.

### Persisted control plane в `apps/server`

- таблицы:
  - `ai_provider_profiles`
  - `ai_tool_profiles`
  - `ai_chat_sessions`
  - `ai_chat_messages`
  - `ai_chat_runs`
  - `ai_tool_traces`
  - `ai_approval_requests`
- GraphQL query/mutation surface для providers, tool profiles, sessions, traces и approvals;
- server-side orchestration service `AiManagementService`;
- `apps/server` хранит секреты, runtime settings и audit trail, а не UI.

### UI-пакеты

- Leptos admin UI package: `crates/rustok-ai/admin`;
- Next.js admin UI package: `apps/next-admin/packages/rustok-ai`;
- оба UI уже поддерживают provider registry с редактируемыми `capabilities` и `usage_policy`;
- оба UI показывают execution metadata (`execution_mode`, `execution_path`) для session/run inspection;
- оба host'а выступают только composition root:
  - `apps/admin` монтирует Leptos package;
  - `apps/next-admin` монтирует npm package `@rustok/ai-admin`.

## Границы ответственности

### Что остаётся в `rustok-ai`

- orchestration runtime;
- provider abstraction;
- chat/session/approval contracts;
- server-side management service;
- capability-owned admin UI packages.

### Что остаётся в `rustok-mcp`

- MCP server transport/protocol boundary;
- tool surface и identity/policy/runtime binding;
- отсутствие provider-specific orchestration и model-host responsibilities.

### Что остаётся в `apps/server`

- persisted control plane;
- GraphQL contract;
- Leptos `#[server]` integration path;
- composition root для runtime wiring.

## Что ещё не реализовано

- token streaming как обязательный runtime path;
- дополнительные provider families (`Anthropic`, `Gemini`, native adapters);
- удалённый MCP bootstrap beyond текущего Rustok server wiring;
- отдельный marketplace/publish flow для AI artifacts.

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [README crate `rustok-mcp`](../../rustok-mcp/README.md)
- [Карта документации платформы](../../../docs/index.md)
