# RusToK MCP App

`apps/mcp` — точка входа для MCP-приложения RusToK (stdio/runtime-обвязка), использующего `crates/rustok-mcp` как основной адаптер протокола.

## Роль в платформе

- запуск MCP-сервера для внешних AI/agent-интеграций;
- экспонирование инструментов и ресурсов RusToK через единый MCP-контракт;
- композиция runtime-конфигурации окружения (tenant, auth, transport, observability).

## Взаимодействие

- `crates/rustok-mcp` — основная логика MCP-интеграции;
- `apps/server` — источник бизнес-API и доменных данных;
- `crates/rustok-telemetry` — трассировка/метрики и операционная диагностика.

## Документация

- Локальная: `apps/mcp/docs/README.md`
- План развития: `apps/mcp/docs/implementation-plan.md`
- Платформенная карта: `docs/index.md`
