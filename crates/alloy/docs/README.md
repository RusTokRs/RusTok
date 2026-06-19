# Документация `alloy`

`alloy` — capability-модуль платформенного script/runtime слоя на базе Rhai.
Он входит в `ModuleRegistry` и устанавливается/удаляется как остальные optional
модули, но при этом остаётся capability-only слоем, а не tenant-бизнес-доменом.

## Назначение

- публиковать канонический runtime entry point для script execution;
- держать storage, execution log, scheduler и bridge/helper слой внутри capability crate;
- предоставлять единый contract для host integration без размазывания script runtime по `apps/server`.

## Зона ответственности

- `ScriptEngine`, `ScriptOrchestrator`, `Scheduler` и execution lifecycle;
- storage/migrations для scripts и execution log;
- GraphQL/HTTP transport surfaces (`graphql::*`, `controllers::routes`), включая tenant-scoped execution history;
- интеграционные контракты `ScriptableEntity` и `HookExecutor` для host-модулей;
- отсутствие превращения script runtime в отдельный tenant-бизнес-домен.

## Интеграция

- подключается `apps/server` через generated module wiring из `modules.toml` и `rustok-module.toml`;
- регистрируется в `ModuleRegistry` как обычный optional модуль и публикует script permission surface;
- использует Rhai как embedded engine и должен удерживать sandbox/resource-limit semantics;
- может вызываться доменными модулями через hook/integration contracts, не размывая их собственные runtime boundaries.

## Проверка

- `cargo xtask module validate alloy`
- `cargo xtask module test alloy`
- targeted runtime tests для script execution, scheduler и bridge semantics при изменении capability surface

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Alloy Concept](../../../docs/alloy-concept.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)

## Контракт runtime hardening

Alloy применяет resource controls во встроенном Rhai engine до компиляции и
выполнения каждого скрипта. Профиль по умолчанию намеренно консервативен для
host-triggered hooks:

- `max_operations = 50_000` enforced by Rhai и возвращает `ScriptError::OperationLimit`;
- `timeout = 100ms` измеряется вокруг evaluated AST и возвращает `ScriptError::Timeout`, если запуск превысил configured budget;
- `max_call_depth = 16` enforced by Rhai function-call limits;
- `max_string_size = 64 KiB`, `max_array_size = 10_000` и `max_map_depth = 16` enforced as data-size limits и мапятся в `ScriptError::ResourceLimit`.

Используйте `EngineConfig::strict()` для latency-sensitive pre-commit hooks и
`EngineConfig::relaxed()` только для operator-controlled maintenance scripts.
Public callers могут получить снимок effective limits через `EngineConfig::limits()`
без зависимости от Rhai internals. `PhaseCapabilities` фиксирует helper families,
разрешённые для каждой execution phase, чтобы integrations не выводили bridge
availability из побочных эффектов регистрации.

## Runbook для scheduler и hook debugging

1. Проверьте `execution_id`, `script.id`, `script.name` и `execution.phase` в
   tracing span `alloy.script.execute`.
2. Для scheduler failures вызовите scheduler status surface и убедитесь, что job
   не завис с `running = true`; scheduler сбрасывает flag после successful,
   aborted или failed execution и обновляет `next_run` из cron expression.
3. Для hook failures разделяйте `Before` rejection и runtime failure:
   `ScriptError::Aborted` означает intentional business rejection, а
   `OperationLimit`, `Timeout` и `ResourceLimit` указывают на sandbox pressure.
4. Используйте execution log как canonical operator history перед replay script.
   `ScriptExecutor` пишет execution-history запись для каждого runtime path,
   подключённого через `AlloyRuntime`: GraphQL/HTTP manual runs, hooks,
   on-commit scripts и scheduler jobs. Replay должен сохранять тот же phase и
   tenant context, чтобы bridge/helper availability оставалась phase-aware.
   Для чтения истории используйте поддержанные transport surfaces:
   GraphQL `scriptExecutionHistory(scriptId, pagination)` /
   `recentScriptExecutions(pagination)`, HTTP/Loco
   `GET /api/alloy/executions`, `GET /api/alloy/scripts/{id}/executions` или
   generic Axum router `GET /executions`, `GET /scripts/{id}/executions`.
   Все ответы основаны на `SeaOrmExecutionLog`, используют DB-level offset/limit
   pagination, применяют tenant filter до offset и сортируются newest-first.
   Ответы возвращают canonical fields: execution id, script id/name, phase,
   outcome, duration, error, user/tenant context и creation time.
5. Не обходите GraphQL/HTTP/module wiring при debugging production scripts; эти
   surfaces входят в supported capability contract и удерживают audit и
   permission checks в едином path.
