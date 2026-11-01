# rustok-installer-persistence — Crate API

SeaORM persistence adapter for durable installer state.

## Public surface

- `InstallerPersistenceService` — installer database readiness, schema application, durable sessions, and step receipts.
- `InstallStepReceiptItem` — step receipt record exposed to wrappers.
- `InstallHttpJobModel` — the durable install HTTP job entity model.
- `SeaOrmInstallerPorts`, `SeaOrmInstallerBootstrapPorts`, `SeaOrmInstallerApplyPorts` — reusable SeaORM-backed installer ports.
- `entities` module — SeaORM entities for installer-owned tables.

## Invariants

- Schema application goes through `rustok-migrations` (`apply_installer_owner_schema`, `apply_installer_remaining_schema`); this crate never issues ad-hoc DDL.
- Install job transitions are single-writer: the enforced single running job constraint is respected by every service entry point.
- Step receipts are durable within the same transaction as the work they describe.
