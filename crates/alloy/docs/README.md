# Документация `alloy`

`alloy` — capability-crate для платформенного script/runtime слоя на базе Rhai.
Он не входит в tenant module registry как обычный бизнес-модуль, но живёт в том
же module-standard contract и должен оставаться синхронизированным с host wiring.

## Назначение

- публиковать канонический runtime entry point для script execution;
- держать storage, execution log, scheduler и bridge/helper слой внутри capability crate;
- предоставлять единый contract для host integration без размазывания script runtime по `apps/server`.

## Зона ответственности

- `ScriptEngine`, `ScriptOrchestrator`, `Scheduler` и execution lifecycle;
- storage/migrations для scripts и execution log;
- GraphQL/HTTP transport surfaces (`graphql::*`, `controllers::routes`);
- интеграционные контракты `ScriptableEntity` и `HookExecutor` для host-модулей;
- отсутствие tenant-scoped enable/disable semantics в `tenant_modules`.

## Интеграция

- подключается `apps/server` через generated module wiring из `modules.toml` и `rustok-module.toml`;
- остаётся capability-only runtime crate без собственного tenant enablement toggle;
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
