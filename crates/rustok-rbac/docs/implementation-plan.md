# План реализации `rustok-rbac`

Статус: переход на single-engine Casbin runtime завершён; модуль удерживается в
steady-state hardening и drift-prevention режиме.

## Область работ

- удерживать `rustok-rbac` как единственную каноническую границу RBAC runtime;
- синхронизировать permission contracts, integration events и server adapters;
- не допускать возврата к shadow-runtime, rollout-mode или server-owned policy logic.

## Текущее состояние

- relation-store остаётся source of truth для role/permission assignments;
- live authorization выполняется только через Casbin-backed evaluator;
- `RuntimePermissionResolver` и related contracts уже живут в модуле, а `apps/server` держит только adapters и observability;
- local docs, root `README.md` и manifest metadata входят в scoped audit path.

## Этапы

### 1. Contract stability

- [x] зафиксировать single-engine runtime contract;
- [x] перенести policy/evaluator semantics и resolver APIs в модуль;
- [x] стандартизировать integration events для role-assignment changes;
- [ ] удерживать sync между runtime contracts, server adapters и module metadata.

### 2. Drift prevention

- [ ] держать periodic verification зелёным для RBAC/server integration;
- [ ] продолжать вычищать presentation-only role inference вне primary authorization path;
- [ ] расширять guardrails при появлении новых RBAC-managed surfaces.

### 3. Operability

- [ ] удерживать decision/cache/latency telemetry частью live contract;
- [ ] документировать runbooks и adapter expectations вместе с изменениями runtime surface;
- [ ] покрывать новые event contracts и resolver paths точечными integration tests.

## Проверка

- `cargo xtask module validate rbac`
- `cargo xtask module test rbac`
- targeted tests для permission resolution, authorization decisions, cache semantics и integration events

## Правила обновления

1. При изменении RBAC runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata, dependency graph или verification expectations синхронизировать `rustok-module.toml` и профильные verification docs.
4. При изменении live contract обновлять также `apps/server/docs/README.md`.
