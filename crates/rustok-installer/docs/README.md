# Документация `rustok-installer`

`rustok-installer` — support crate для гибридного установщика RusToK. Он не
является platform module и не участвует в tenant-level enable/disable.

## Назначение

Crate фиксирует общий contract установщика, который должны переиспользовать:

- CLI `rustok-server install ...`;
- HTTP surface `/api/install/*`;
- web wizard первого запуска;
- dev wrappers вроде `cargo xtask install-dev`.

## Границы v1

- PostgreSQL — production DB по умолчанию и единственный production-ready engine.
- SQLite допустим только для `local`, `demo` и `test` сценариев.
- Выбор модулей в v1 управляет tenant enablement и build/profile intent, но не
  физическим исключением module-owned schema из глобального `Migrator`.
- Rollback после применения схемы не должен обещать универсальный reverse
  migration; production restore опирается на backup/snapshot.

## Состояния установки

Основной happy path:

```text
Draft
-> PreflightPassed
-> ConfigPrepared
-> DatabaseReady
-> SchemaApplied
-> SeedApplied
-> AdminProvisioned
-> Verified
-> Completed
```

Ошибочные/операционные состояния:

```text
Failed
RolledBackFreshInstall
RestoreRequired
```

## Текущий CLI adapter

`apps/server` уже подключает начальный CLI surface:

- `rustok-server install preflight ...` строит install plan и возвращает
  `PreflightReport` без подключения к БД.
- `rustok-server install plan ...` печатает redacted snapshot install plan.
- `rustok-server install apply ...` выполняет preflight, проверяет target DB,
  при `--create-database` создаёт PostgreSQL database/role через admin URL,
  запускает server `Migrator::up`, применяет tenant/module seed, создаёт или
  синхронизирует superadmin, выполняет verify/finalize, пишет `Preflight` /
  `Config` / `Database` / `Migrate` / `Seed` / `Admin` / `Verify` / `Finalize`
  receipts в `install_step_receipts` и переводит session в `completed`.

`apply` резолвит локальные secret refs `env:<VAR>`, `file:<path>`,
`mounted-file:<path>`, `dotenv:<path>#<VAR>` и `dotenv:<VAR>`. External
backends вроде `vault:*`, `kubernetes:*` и cloud secret managers пока остаются
contract-level refs для `plan`/`preflight` и fail-fast на `apply` до подключения
внешнего resolver-а.

HTTP adapter в `apps/server` публикует thin surface для Leptos wizard:
`GET /api/install/status`, `POST /api/install/plan`,
`POST /api/install/preflight`, `POST /api/install/apply`,
`GET /api/install/jobs/{job_id}` и
`GET /api/install/sessions/{session_id}/receipts`. HTTP `apply` стартует
background job и вызывает тот же server-side `apply_plan` pipeline, что и CLI;
UI не должен дублировать migration/seed/admin logic.

## Связанные документы

- [ADR гибридного установщика](../../../DECISIONS/2026-04-26-hybrid-installer-architecture.md)
- [Архитектура модулей](../../../docs/architecture/modules.md)
- [Схема данных платформы](../../../docs/architecture/database.md)
