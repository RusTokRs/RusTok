# rustok-installer-persistence

`rustok-installer-persistence` is the SeaORM adapter for durable installer
sessions and step receipts. It owns the entity mappings and persistence service
for the schema created by `rustok-migrations`.

## Responsibilities

- Persist installer sessions, state changes, tenant association, and locks.
- Persist and read redacted, idempotent step receipts.
- Keep SeaORM and database concerns outside the neutral `rustok-installer`
  contracts and outside HTTP hosts.

## Entry points

- `InstallerPersistenceService`
- `entities::install_session`
- `entities::install_step_receipt`

`apps/server` and the platform CLI are adapters over this crate; neither owns a
second mapping or persistence implementation.
