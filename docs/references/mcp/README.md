---
id: doc://docs/references/mcp/README.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Справочный пакет MCP

Дата последней актуализации: **2026-03-20**.

Этот пакет не дублирует спецификацию MCP и не пересказывает документацию `rmcp`. Его задача:

- дать команде короткий индекс официальных источников истины;
- зафиксировать, что именно RusToK использует из MCP сегодня;
- защитить локальные документы от деградации в устаревший пересказ активно развивающейся экосистемы.

## Источники истины

### Официальная документация и спецификация

- MCP docs: [modelcontextprotocol.io/docs](https://modelcontextprotocol.io/docs)
- MCP spec: [modelcontextprotocol.io/specification](https://modelcontextprotocol.io/specification/2025-03-26)
- Server tools: [Tools](https://modelcontextprotocol.io/specification/2025-03-26/server/tools)
- Server resources: [Resources](https://modelcontextprotocol.io/specification/2025-03-26/server/resources)
- Server prompts: [Prompts](https://modelcontextprotocol.io/specification/2025-03-26/server/prompts)

### Security и authorization

- Authorization guide: [Understanding Authorization in MCP](https://modelcontextprotocol.io/docs/tutorials/security/authorization)
- Security guide: [Security Best Practices](https://modelcontextprotocol.io/docs/tutorials/security/security_best_practices)

### Rust SDK

- `rmcp` docs: [docs.rs/rmcp](https://docs.rs/rmcp/latest/rmcp/)
- Official Rust SDK repo: [modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk)

## Как использовать этот пакет

Если вопрос касается:

- структуры протокола;
- capability surface MCP;
- server/client semantics;
- authorization flow;
- security requirements;
- конкретного поведения `rmcp`;

нужно идти в официальные ссылки выше. Локальные документы RusToK должны ссылаться на них, а не
копировать фрагменты спецификации к себе.

## Что фиксируем локально в RusToK

Локально мы документируем только интеграционный слой:

- `rustok-mcp` как thin adapter над `rmcp`;
- какой tool surface уже реализован;
- какие RusToK-specific ограничения и gaps остаются;
- как MCP связан с Alloy и platform RBAC/tenant model.

## Текущее состояние RusToK

На сегодня `rustok-mcp` покрывает:

- MCP server/tool surface через `rmcp`;
- module discovery tools;
- Alloy-related tools при наличии `AlloyMcpState`;
- identity/policy foundation через `McpIdentity`, `McpAccessContext`, `McpAccessPolicy`;
- introspection tool `mcp_whoami`;
- compatibility shim через legacy `enabled_tools`;
- session-start runtime binding hooks (`McpSessionContext`, `McpAccessResolver`, `McpRuntimeBinding`);
- runtime allow/deny audit hook через `McpAuditSink`;
- первый реальный Alloy product-slice: `alloy_scaffold_module`, который stage-ит draft `crates/rustok-<slug>` module scaffold, а `alloy_review_module_scaffold` / `alloy_apply_module_scaffold` дают review/apply boundary.
- persisted server-side control plane для Alloy scaffold drafts в `apps/server` через REST `/api/mcp/scaffold-drafts*` и GraphQL `mcpModuleScaffoldDraft*`.
- live runtime hook `McpScaffoldDraftStore`, через который `DbBackedMcpRuntimeBridge` может уводить Alloy scaffold flow из process-local памяти в persisted drafts `apps/server`.

На сегодня `rustok-mcp` не покрывает как production-ready слой:

- server-owned remote MCP transport/session bootstrap beyond текущего stdio adapter path;
- admin UI для MCP clients, tokens, policies и audit;
- полную upstream capability surface (`resources`, `prompts`, `roots`, `sampling` и т.д.);
- product UI и server-owned remote MCP bootstrap поверх уже существующего persisted scaffold draft control plane;
- более богатый review/apply/codegen pipeline, который превращает draft Alloy scaffold в production-ready модуль.

Persisted management layer уже есть на стороне `apps/server`: таблицы `mcp_clients`, `mcp_tokens`,
`mcp_policies`, `mcp_audit_logs`, `mcp_scaffold_drafts`, REST `/api/mcp/*`, GraphQL `mcp*` и
`DbBackedMcpRuntimeBridge` для связывания plaintext MCP token с runtime access context и runtime
allow/deny audit.

## Связанные локальные документы

- [`crates/rustok-mcp/README.md`](../../../crates/rustok-mcp/README.md)
- [`crates/rustok-mcp/docs/README.md`](../../../crates/rustok-mcp/docs/README.md)
- [`crates/rustok-mcp/docs/implementation-plan.md`](../../../crates/rustok-mcp/docs/implementation-plan.md)
- [`docs/modules/registry.md`](../../modules/registry.md)
- [`docs/modules/crates-registry.md`](../../modules/crates-registry.md)

## Правило сопровождения

Перед любым обновлением локальных MCP-доков:

1. Проверить актуальные официальные MCP/rmcp документы.
2. Не переносить в RusToK длинные пересказы spec/SDK.
3. Документировать только локальную интеграцию, ограничения и решения RusToK.
