# rustok-migrations

`rustok-migrations` is the neutral platform schema-composition crate.

It combines the platform-owned SeaORM migrations with migration sources
exported by selected domain modules, validates declared cross-module ordering,
and exposes `Migrator` to `rustok-cli migrate` and installer execution.

The crate does not depend on `apps/server`. The HTTP host owns request runtime
only; operational schema changes are invoked through the platform CLI or the
installer workflow.

## Entry point

- `rustok_migrations::Migrator`

See [the database architecture](../../docs/architecture/database.md) and the
[Loco exit plan](../../docs/architecture/loco-exit-plan.md).
