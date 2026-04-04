# Capability `rustok-ai`

`rustok-ai` — capability-crate RusToK для AI host/orchestrator слоя поверх `rustok-mcp`.

Этот crate не является tenant-toggled модулем и не входит в taxonomy `Core` / `Optional`.
Его задача — держать слой между model provider и MCP tool surface, не расширяя `rustok-mcp`
до роли model host.

## Назначение

- держать provider-agnostic runtime contract для AI orchestration;
- поставлять multiprovider runtime для `OpenAI-compatible`, `Anthropic` и `Gemini`, сохраняя
  `OpenAI-compatible` как удобный путь для cloud и local endpoint'ов через `base_url`;
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
- `AiRouter` и direct-dispatch слой для first-party verticals без обязательного MCP hop.

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
- Runtime observability теперь идёт в двух слоях:
  - persisted `decision_trace` и run/session metadata в control plane;
  - in-process `AiManagementService::metrics_snapshot()` и Prometheus module/span telemetry для router resolution и run outcomes.
- diagnostics snapshot теперь включает breakdown не только по provider/execution target, но и по
  task profile / resolved locale, чтобы оператор видел routing и multilingual срезы без похода в raw traces.
- bounded streaming layer включает `AiRunStreamHub` в `rustok-ai`, GraphQL subscription
  `aiSessionEvents(sessionId)` в `apps/server` и live incremental output для operator chat /
  provider-backed text runs в обоих admin host'ах для `OpenAI-compatible`, `Anthropic` и `Gemini`.
- direct verticals используют тот же streaming contract, поэтому direct Alloy / content jobs не
  теряют live delta/update surface по сравнению с runtime/MCP path.
- помимо live subscription серверный слой теперь держит bounded recent-event cache; он доступен
  через `AiManagementService::recent_stream_events(...)` и GraphQL query
  `aiRecentRunStreamEvents(sessionId?, limit?)` для diagnostics и session detail.
- diagnostics surface теперь также использует bounded recent run history из persisted control
  plane через `AiManagementService::list_recent_runs(...)` и GraphQL query
  `aiRecentRuns(limit?)`, чтобы показывать статус/latency/provider/locale history без разбора raw traces.

### UI-пакеты

- Leptos admin UI package: `crates/rustok-ai/admin`;
- Next.js admin UI package: `apps/next-admin/packages/rustok-ai`;
- оба UI уже поддерживают provider registry с редактируемыми `capabilities` и `usage_policy`;
- оба UI показывают execution metadata (`execution_mode`, `execution_path`) для session/run inspection;
- оба UI поддерживают direct job surfaces для `alloy_code`, `image_asset`, `product_copy` и `blog_draft`;
- поля `locale` в admin UI являются optional override: пустое значение оставляет AI runtime
  использовать request locale chain (`request -> tenant default -> en`), а не форсирует `en`;
- оба UI теперь имеют focused diagnostics sub-surface для router/run observability:
  - Leptos host: `/ai/diagnostics`
  - Next host: `/dashboard/ai/diagnostics`
- оба UI теперь поддерживают live session stream card через `graphql-transport-ws` subscription
  `aiSessionEvents`, не заменяя persisted session detail и trace view.
- оба UI теперь показывают и bounded recent stream history, даже если пользователь открыл
  diagnostics/session detail уже после завершения live stream.
- оба UI теперь показывают recent run history как отдельный diagnostics slice поверх persisted
  `ai_chat_runs`, а не только aggregate metrics snapshot.
- оба host'а выступают только composition root:
  - `apps/admin` монтирует Leptos package;
  - `apps/next-admin` монтирует npm package `@rustok/ai-admin`.
- browser-target verification для Leptos package теперь включает отдельный `hydrate` check, чтобы
  WebSocket streaming path проверялся не только на SSR.

## Границы ответственности

### Что остаётся в `rustok-ai`

- orchestration runtime;
- provider abstraction;
- direct first-party execution registry;
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

- time-windowed diagnostics/trends поверх текущего snapshot/history surface;
- persisted provider fallback/error analytics beyond текущего in-process snapshot;
- дополнительные provider families сверх уже реализованных (`Anthropic`, `Gemini`, richer native adapters);
- удалённый MCP bootstrap beyond текущего Rustok server wiring;
- отдельный marketplace/publish flow для AI artifacts.

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [README crate `rustok-mcp`](../../rustok-mcp/README.md)
- [Карта документации платформы](../../../docs/index.md)
