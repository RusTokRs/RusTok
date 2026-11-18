# rustok-installer-persistence

Public surface and contract invariants are documented in [CRATE_API.md](CRATE_API.md).

## Purpose

`rustok-installer-persistence` is the SeaORM adapter for installer database
readiness, schema application, durable sessions, step receipts, and reusable
bootstrap persistence. It owns the entity mappings and persistence service for
the schema created by `rustok-migrations`.

## Responsibilities

- Persist installer sessions, state changes, tenant association, and locks.
- Claim installer sessions once with a monotonic attempt generation and fail
  closed while an unreconciled session (including `recovery_required`) remains;
  lease expiry alone does not release global admission.
- Fence every state, tenant, and receipt mutation with the persisted owner and
  generation. PostgreSQL/MySQL acquire a row lock before checking the fence;
  SQLite reserves its writer before reading. Receipt rows retain their
  generation for audit and stale-attempt diagnosis.
- Require successful per-step receipts from the current generation before
  advancing the corresponding durable state; `Verified` requires deploy and
  verify receipts, and `Completed` requires finalize.
- Persist and read redacted, append-only step receipts. In
  `recovery_required`, only rollback receipts are accepted; `failed` and
  `fresh_install_cleaned` can release admission only with a successful
  rollback receipt from the current attempt generation. Idempotent signed
  distribution-receipt import is a separate operation; automatic replay or
  resume from installer step receipts is not implemented yet.
- Prepare PostgreSQL/SQLite targets and apply the composed platform migrator.
- Compose tenant, identity, RBAC, and module lifecycle writers for typed seed
  profiles and administrator provisioning without duplicating them in HTTP or
  CLI hosts.
- Replay seed module toggles with the original owner-journal revision and reject
  a replay when the currently observed enablement intent has changed. Fresh
  seed enables fail closed while admitted static releases do not provide their
  settings schemas; this blocks installations requiring new module enables until
  release-bound schema composition is implemented. Disable and exact replay
  do not require a new schema.
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
