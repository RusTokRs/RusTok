# ADR: `rustok-ai` как отдельный capability-модуль

- Дата: 2026-04-03
- Статус: Accepted

## Контекст

В RusToK уже существует `rustok-mcp` как thin MCP adapter/server surface поверх официального SDK
`rmcp`. При этом продукту нужен полноценный AI host/orchestrator слой:

- подключение к локальным и облачным model provider'ам;
- orchestration chat runs;
- вызов MCP tools;
- persisted control plane для provider profiles, traces и approvals;
- UI для операторской работы.

Если встроить этот слой в `rustok-mcp`, MCP server boundary смешается с provider/runtime
orchestration, а `rustok-mcp` перестанет быть thin adapter'ом.

## Решение

Создать отдельный capability crate `crates/rustok-ai`.

`rustok-ai`:

- владеет `ModelProvider` abstraction;
- владеет `AiRuntime`, chat/session model и approval policy;
- использует `rustok-mcp` как MCP tool surface;
- отдаёт `apps/server` server-side `AiManagementService` и persisted control-plane wiring;
- поставляет отдельный Leptos admin UI package `crates/rustok-ai/admin`;
- поставляет отдельный Next.js admin UI package `apps/next-admin/packages/rustok-ai`.

`rustok-mcp` при этом остаётся:

- MCP transport/protocol boundary;
- identity/policy/runtime binding layer;
- tool surface для RusToK и Alloy;
- без provider-specific responsibilities.

## Причины

### 1. MCP SDK reuse вместо собственной MCP-библиотеки

RusToK не должен поддерживать собственный protocol stack для MCP. Протокол и SDK уже живут
в upstream (`modelcontextprotocol` / `rmcp`), а локальный код должен реализовывать только
интеграционный слой.

### 2. Provider abstraction не должна жить в `rustok-mcp`

Связь `LLM provider <-> host` не является responsibility MCP server layer. Этот слой должен жить
в AI host/orchestrator capability и использовать MCP как отдельную шину инструментов.

### 3. Persisted control plane принадлежит server composition root

Секреты, профили провайдеров, чат-сессии, traces и approvals должны храниться в `apps/server`,
а не в UI-хостах и не в `rustok-mcp`.

### 4. UI должен оставаться capability-owned, а host — только composition root

Leptos UI поставляется как `crates/rustok-ai/admin`, Next.js UI — как
`apps/next-admin/packages/rustok-ai`. Это сохраняет правило платформы:

- модульный/capability-specific business UI не уходит в `apps/admin` или `apps/next-admin`;
- host-приложения только монтируют surface и предоставляют shell/navigation/runtime context.

## Следствия

### Позитивные

- отделён AI host/orchestrator слой от MCP server boundary;
- один backend runtime покрывает и local, и cloud endpoint'ы через `OpenAI-compatible` family;
- сохранён dual-path contract для Leptos: native `#[server]` first, GraphQL parallel;
- Leptos и Next.js получают паритетный capability-owned UI surface;
- `rustok-mcp` остаётся thin adapter'ом и не разрастается в отдельный product runtime.

### Негативные

- появляется новый capability crate и отдельный persisted control plane;
- Next.js пакет требует ручного `package.json` wiring и ручной пересборки;
- первая версия ограничена `OpenAI-compatible` provider family и request/response runs без streaming.

## Что не делаем

- не превращаем `rustok-mcp` в AI host;
- не пишем собственную MCP-библиотеку;
- не переносим AI business UI в host-приложения;
- не делаем `rustok-ai` tenant-toggled optional module.
