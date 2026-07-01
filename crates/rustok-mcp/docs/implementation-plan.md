# План реализации `rustok-mcp`

Статус: governed MCP tool adapter уже работает поверх `rmcp`; следующая работа
не про переписывание протокола, а про доведение RusToK-specific runtime,
identity/audit и Alloy-related control plane до platform-grade уровня.

## Execution checkpoint

- Текущая фаза: `mcp_leptos_host_composition`; предыдущий owner UI checkpoint: `mcp_admin_owner_ui_slice`.
- Последняя контрольная точка: MCP GraphQL query/mutation/types перенесены из `apps/server` в `rustok-mcp` и работают через единый owner-defined `McpManagementPort`; server provider делегирует reads/writes каноническому `McpManagementService`. Leptos native `#[server]` mutations используют тот же `McpManagementRuntime`, поэтому GraphQL и FFA сохраняют единые transaction claim/recovery semantics без host-owned resolver/DTO.
- Следующий шаг: добавить authenticated browser-level parity smoke для Next `/dashboard/mcp` и Leptos `/mcp` management workflows поверх уже усиленного draft stage/apply boundary.
- Открытые блокеры: нет актуальных локальных блокеров для текущего MCP slice; `cargo check -p rustok-server --offline`, `cargo test -p rustok-mcp --lib --offline`, `cargo fmt -p rustok-server --check` и `npm run verify:mcp:admin-boundary` проходят.
- Передача следующему агенту: сохранять `rustok-mcp` как MCP protocol/tool adapter, persisted draft storage оставлять в `apps/server`, а UI - в owner surface MCP, не в `rustok-ai`. После изменений tool surface повторять `cargo check -p rustok-mcp-admin --features ssr`, `npm run verify:mcp:admin-boundary`, `cargo check -p rustok-server`, `cargo check -p rustok-mcp` и `cargo test -p rustok-mcp --lib`.
- Обновлено (UTC): 2026-07-01T00:00:00Z

## FFA/FBA status

- Structural shape: `core_transport_ui`

- FFA status: `in_progress`
- FBA status: `in_progress`
- Подтверждения:
  - Next owner surface `apps/next-admin/packages/rustok-mcp` владеет UI ревью MCP/Alloy scaffold drafts, audit events, clients/policies/tokens и management mutations; host route только монтирует `McpAdminPage`.
  - Leptos FFA surface `crates/rustok-mcp/admin` содержит Leptos-free `core.rs`, `transport::{native_server_adapter,graphql_adapter}` и явный `ui/leptos.rs` adapter.
  - Leptos host `apps/admin` подключает owner crate через тонкий маршрут `/mcp`; CSR, hydrate WASM и SSR feature profiles компилируются.
  - Native `#[server]` functions являются основным внутренним data layer Leptos; mutations и owner GraphQL получают `McpManagementRuntime` из `ModuleRuntimeExtensions`, а server provider делегирует client/policy/token/audit/scaffold reads/writes `McpManagementService`. GraphQL operation documents параллельно остаются в `transport/graphql_adapter.rs`.
  - Boundary guardrail `scripts/verify/verify-mcp-admin-boundary.mjs` проверяет owner placement, требует stage/apply delegation через mutation port, запрещает scaffold persistence/audit SQL в UI adapter и запрещает MCP draft UI внутри `rustok-ai`.
- Последняя проверка (UTC): 2026-07-01T00:00:00Z.
- Владелец: `rustok-mcp`.

## Область работ

- удерживать `rustok-mcp` как thin MCP adapter crate поверх `rmcp`;
- синхронизировать tool surface, runtime binding, access policy и local docs;
- не допускать смешивания MCP protocol boundary с AI provider orchestration.

## Текущее состояние

- crate уже интегрирован с `rmcp` и поставляется как library + binary;
- module discovery tools, health/introspection, Alloy-related tools и scaffold review/apply boundary уже подняты;
- persisted server-side scaffold drafts и runtime draft-store bridge уже связаны с MCP flow;
- identity/policy foundation, session-start runtime binding и allow/deny audit уже являются частью live contract.

## Этапы

### 1. Contract stability

- [x] зафиксировать `rustok-mcp` как thin adapter поверх `rmcp`;
- [x] поднять typed tool surface, response envelope и access-policy baseline;
- [x] встроить Alloy-related scaffold/review/apply vertical и runtime draft-store binding;
- [ ] удерживать sync между runtime contracts, management/control plane и local docs.

### 2. Platform hardening

- [ ] довести server-owned remote MCP transport/session bootstrap beyond текущий stdio path;
- [ ] расширить audit trail от allow/deny к richer execution telemetry;
- [ ] удерживать identity/policy layer совместимым с official MCP authorization guidance.

### 3. Product surface

- [ ] добавить UI-слой для MCP access management и Alloy draft review;
- [ ] расширять Alloy/codegen vertical без автоматического размывания review/apply boundary;
- [ ] добавлять новые MCP capabilities (`resources`, `prompts`, `sampling` и др.) только как explicit staged rollout.

## Проверка

- structural verification для RusToK-specific MCP docs и boundary;
- targeted compile/tests при изменении tool surface, access policy, runtime binding или draft-store integration;
- обязательная сверка с official MCP/rmcp docs при изменении protocol/security assumptions.

- контрактные тесты покрывают все публичные use-case MCP surface.

## Правила обновления

1. При изменении RusToK-specific MCP contract сначала обновлять этот файл.
2. Сначала сверять изменения с official MCP/rmcp источниками, потом обновлять local docs.
3. При изменении public crate behavior синхронизировать `README.md` и `docs/README.md`.
4. При изменении reference-map обновлять `docs/references/mcp/README.md` и при необходимости `docs/index.md`.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
