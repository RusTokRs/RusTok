# rustok-installer-persistence

Public surface and contract invariants are documented in [CRATE_API.md](CRATE_API.md).

## Purpose

`rustok-installer-persistence` is the SeaORM adapter for installer database
readiness, schema application, durable sessions, step receipts, and reusable
bootstrap persistence. It owns the entity mappings and persistence service for
the schema created by `rustok-migrations`.

## Responsibilities

- Persist installer sessions, state changes, tenant association, and locks.
- Provide a conditional terminal write for an HTTP claim whose INSERT result is
  ambiguous but whose executor never began: only the original job/session/
  submitted-at identity in `running` may be marked failed. This does not
  authorize recovery of an installation that may have performed side effects.
- Claim installer sessions once and fail closed while an unreconciled session
  (including `recovery_required`) remains; lease expiry alone does not release
  global admission.
- Persist and read redacted, idempotent step receipts.
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

## PostgreSQL bootstrap test

The signed base-distribution test uses the installer receipt loader, the real
schema ports, and the Modules release ledger. Set
`RUSTOK_MIGRATION_SMOKE_ADMIN_URL` to an isolated PostgreSQL instance with
permission to create and drop a fixture database before running the test.

```sh
cargo test --locked -p rustok-installer-persistence --test postgres_base_distribution_import postgres_signed_base_distribution_survives_install_schema -- --ignored --exact
```

## Interactions

- `rustok-installer` owns the neutral plan, state, receipt, seed, and port
  contracts implemented here with SeaORM.
- `rustok-migrations` supplies the globally composed `Migrator`; this adapter
  applies it once for the shared installation database.
- `apps/server` and `rustok-installer-cli` compose this adapter with their
  selected `rustok-distribution` registry and must not duplicate its database
  mappings or bootstrap writers.
