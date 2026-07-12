# rustok-installer-persistence

## Purpose

`rustok-installer-persistence` is the SeaORM adapter for installer database
readiness, schema application, durable sessions, step receipts, and reusable
bootstrap persistence. It owns the entity mappings and persistence service for
the schema created by `rustok-migrations`.

## Responsibilities

- Persist installer sessions, state changes, tenant association, and locks.
- Persist and read redacted, idempotent step receipts.
- Prepare PostgreSQL/SQLite targets and apply the composed platform migrator.
- Compose tenant, identity, RBAC, and module lifecycle writers for typed seed
  profiles and administrator provisioning without duplicating them in HTTP or
  CLI hosts.
- Provide a complete standalone SeaORM apply adapter that verifies the tenant,
  administrator, and effective module set from the installer seed defaults and
  persisted tenant overrides.
- Keep SeaORM and database concerns outside the neutral `rustok-installer`
  contracts and outside HTTP hosts.

## Entry points

- `InstallerPersistenceService`
- `SeaOrmInstallerPorts`
- `SeaOrmInstallerBootstrapPorts`
- `SeaOrmInstallerApplyPorts`
- `entities::install_session`
- `entities::install_step_receipt`

`apps/server` and the platform CLI are adapters over this crate; neither owns a
second mapping or persistence implementation.

## Interactions

- `rustok-installer` owns the neutral plan, state, receipt, seed, and port
  contracts implemented here with SeaORM.
- `rustok-migrations` supplies the globally composed `Migrator`; this adapter
  applies it once for the shared installation database.
- `apps/server` and `rustok-installer-cli` compose this adapter with their
  selected `rustok-distribution` registry and must not duplicate its database
  mappings or bootstrap writers.
