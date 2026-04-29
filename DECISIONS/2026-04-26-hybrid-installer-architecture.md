# Гибридный установщик RusTok

- Date: 2026-04-26
- Status: Accepted

## Context

RusTok уже имеет dev bootstrap через `cargo xtask install-dev` и Docker launcher
`scripts/dev-start.sh`, но это не production-grade installer. Текущие механизмы
частично идемпотентны, но не имеют общей модели состояния, receipts, resumable
логики, install lock, строгой политики секретов и отдельного operator UX.

Серверный `Migrator` в `apps/server/migration` собирает platform-core и
module-owned migrations в один глобально отсортированный список. Tenant-level
enable/disable работает поверх уже собранной platform composition. Поэтому
installer не должен обещать, что выбор optional modules в v1 физически исключает
их schema artifacts из БД.

## Decision

RusTok использует гибридную модель установщика:

1. `crates/rustok-installer` становится shared installer-core и источником
   истины для install plan, state machine, preflight policy, secret references,
   receipts и checksum/idempotency contract.
2. CLI `rustok-server install ...` будет каноническим operator interface для
   automation, CI/CD и production runs.
3. Web wizard будет thin facade поверх того же installer-core и HTTP adapter,
   а не отдельной реализацией bootstrap logic.
4. `cargo xtask install-dev` и `scripts/dev-start.sh` сохраняются как backward
   compatible dev entrypoints, но должны делегировать установочную логику в
   installer-core по мере внедрения CLI adapter.
5. Installer явно различает build composition, schema composition и tenant
   enablement.
6. PostgreSQL является first-class production DB. SQLite допускается только для
   local/demo/test сценариев. Production installer не использует silent SQLite
   fallback.
7. Rollback после применения схемы не трактуется как универсальная reverse
   migration. Для production recovery используется backup/snapshot restore.

## Consequences

- Installer foundation является support crate, а не platform module.
- Первая версия может управлять tenant enablement и deployment profile intent,
  но не удаляет module-owned schema для disabled modules.
- Секреты должны передаваться через `env`, `mounted-file` или
  `external-secret`; `dotenv-file` остаётся local/dev режимом.
- Server startup guardrails вроде sample-secret checks должны стать частью
  installer preflight/finalize, а не существовать только как runtime abort.
- Для web wizard обязательны setup token, install lock, rate limiting,
  CSRF/origin checks и отключение setup routes после `Completed`.
- Leptos admin монтирует wizard на `/install`: он формирует `InstallPlan`,
  делает preflight, запускает background apply job и отображает persisted
  receipts. CLI остаётся каноническим интерфейсом для automation и production
  runbook-ов.
