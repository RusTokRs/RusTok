# Документация `rustok-mcp`

`rustok-mcp` — thin adapter crate для MCP integration в RusToK поверх `rmcp`.
Он держит RusToK-специфичный tool/runtime слой, не подменяя официальный MCP
spec и не превращаясь в provider/model host.

## Назначение

- публиковать канонический MCP adapter contract для RusToK;
- держать tool surface, runtime binding, access policy и audit hooks поверх `rmcp`;
- связывать Alloy-related MCP capabilities и persisted server-side control plane с runtime session flow.

## Зона ответственности

- MCP server adapter поверх `rmcp`;
- typed tools, `McpToolResponse`, runtime binding и access policy contracts;
- session-start access resolution, allow/deny audit и introspection surface;
- Alloy-related MCP tools и scaffold draft review/apply boundary;
- отсутствие ownership над provider-specific AI orchestration и над самим MCP spec.

## Интеграция

- протокол, security и authorization semantics берутся из официальных MCP/rmcp документов, а не из локальной docs-папки;
- `rustok-ai` использует `rustok-mcp` как MCP tool boundary, не расширяя его до model host;
- `apps/server` держит persisted MCP management/control plane и runtime bridges для токенов, policy и scaffold drafts;
- Alloy подключается как capability через runtime state, а не как отдельный MCP transport stack.

## Проверка

- structural verification для local docs и RusToK-specific MCP boundary;
- targeted compile/tests при изменении tool surface, access policy, runtime binding или audit path;
- при изменении protocol/security assumptions обязательна сверка с официальными MCP/rmcp источниками.

## Внешние источники истины

- [MCP docs](https://modelcontextprotocol.io/docs)
- [MCP specification](https://modelcontextprotocol.io/specification/2025-03-26)
- [`rmcp` docs](https://docs.rs/rmcp/latest/rmcp/)
- [Rust SDK repository](https://github.com/modelcontextprotocol/rust-sdk)
- [Authorization guide](https://modelcontextprotocol.io/docs/tutorials/security/authorization)
- [Security Best Practices](https://modelcontextprotocol.io/docs/tutorials/security/security_best_practices)

## Связанные документы

- [План реализации](./implementation-plan.md)
- [Центральный reference-индекс MCP](../../../docs/references/mcp/README.md)
- [README crate](../README.md)
