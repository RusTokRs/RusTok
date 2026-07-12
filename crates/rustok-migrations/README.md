# rustok-migrations

## Purpose

`rustok-migrations` is the neutral platform schema-composition crate.

It combines the platform-owned SeaORM migrations with migration sources
exported by selected domain modules and validates declared cross-module
ordering.

## Responsibilities

- Compose platform and selected module migration sources into one ordered
  `Migrator`.
- Validate declared cross-module migration dependencies.
- Keep schema composition independent from HTTP routing and executable hosts.

## Interactions

- `rustok-installer-persistence` applies `Migrator` once for the shared
  installation database.
- `rustok-installer-cli`, selected by `rustok-cli`, exposes `migrate up` and
  `migrate status` without importing `apps/server`.
- Domain modules export their own `MigrationSource` implementations; this crate
  aggregates them but does not own their tables or migration logic.

The crate does not depend on `apps/server`. The HTTP host owns request runtime
only; operational schema changes are invoked through the platform CLI or the
installer workflow.

## Entry point

- `rustok_migrations::Migrator`

See [the database architecture](../../docs/architecture/database.md) and the
[Loco exit plan](../../docs/architecture/loco-exit-plan.md).
