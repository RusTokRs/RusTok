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

## Planned Module Release Safety Integration

The
[module release and rollback plan](../../docs/modules/module-release-rollback-plan.md)
keeps this crate as the trusted neutral migration executor.
`rustok-modules` owns update mode, the exact digest-bound migration phase plan,
fences, checkpoints, and recovery outcome; `rustok-migrations` validates and
executes only that authorized phase and returns an idempotent receipt. It does
not choose a rollback target, restore policy, or finalization time.

The initial installer may still apply the complete selected schema. A
production module update must not invoke an unbounded “up to latest” migrator:
it executes only the approved additive, backfill, or separately authorized
finalization phase. Process loss and unknown executor outcomes must reconcile
against the exact migration identity before any retry.

## Entry point

- `rustok_migrations::Migrator`

See [the database architecture](../../docs/architecture/database.md) and the
[Axum runtime and operations CLI boundary](../../DECISIONS/2026-07-02-axum-runtime-and-ops-cli-boundary.md).
