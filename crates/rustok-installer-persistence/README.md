# rustok-installer-persistence

`rustok-installer-persistence` is the SeaORM adapter for installer database
readiness, schema application, durable sessions and step receipts. It owns the
entity mappings and persistence service for the schema created by
`rustok-migrations`.

## Responsibilities

- Persist installer sessions, state changes, tenant association, and locks.
- Persist and read redacted, idempotent step receipts.
- Prepare PostgreSQL/SQLite targets and apply the composed platform migrator.
- Keep SeaORM and database concerns outside the neutral `rustok-installer`
  contracts and outside HTTP hosts.

## Entry points

- `InstallerPersistenceService`
- `SeaOrmInstallerPorts`
- `entities::install_session`
- `entities::install_step_receipt`

`apps/server` and the platform CLI are adapters over this crate; neither owns a
second mapping or persistence implementation.
