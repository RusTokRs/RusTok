# CRATE_API: rustok-migrations

## 1. Architectural Role & Boundary Enforcement

`rustok-migrations` is the neutral platform schema-composition and migration orchestration authority for RusToK:
- **Centralized Schema Composition:** Aggregates core platform migrations (tenants, users, sessions, RBAC, platform settings, flex, AI control plane, installer tables) with domain-module migration sources exported via the `MigrationSource` contract across 30+ domain modules.
- **Topological Dependency Resolution:** Enforces that all cross-module table dependencies (e.g. catalog -> taxonomy, checkout -> cart/order/payment) are declared via `MigrationDependencyDescriptor` and topologically sorted prior to schema application.
- **Environment Boundary Separation:**
  - `Migrator`: The authoritative production migrator. Fully PostgreSQL-native, applying all platform migrations and triggers in strict chronological/dependency order.
  - `SqliteTestMigrator`: Scoped strictly for portable SQLite unit and component tests. Explicitly excludes PostgreSQL-native triggers, enums, and constraint migrations listed in `SQLITE_INCOMPATIBLE_MIGRATIONS`.
- **Zero Host Leakage:** Independent from `apps/server` and HTTP frameworks; operational schema changes are driven through platform CLI tools (`rustok-migrate`, `export-migration-plan`) or installer persistence ports.

---

## 2. Public Surface & Data Structures

### Primary Migrators & Migration Control
- `Migrator`: Authoritative global migrator implementing `sea_orm_migration::MigratorTrait`.
  - `Migrator::migrations() -> Vec<Box<dyn MigrationTrait>>`
- `SqliteTestMigrator`: Test-only portable schema migrator implementing `sea_orm_migration::MigratorTrait`.
  - `SqliteTestMigrator::migrations() -> Vec<Box<dyn MigrationTrait>>`
- `sqlite_test_migration_is_compatible(name: &str) -> bool`: Predicate querying whether a migration is part of the portable SQLite test schema.
- `apply_installer_owner_schema(db: &DatabaseConnection) -> Result<(), DbErr>`: Minimal bootstrap prefix ensuring tenants, publish tables, install sessions, and static distribution tables exist before base distribution import.
- `apply_installer_remaining_schema(db: &DatabaseConnection) -> Result<(), DbErr>`: Completes full platform schema execution after base distribution admission.

### Schema Diff & Safety Classification (`schema_diff`)

These composition exports are available with the default `composition` feature.
The independent `verification` module is also available with
`--no-default-features`:

- `observe_database(db, database_url, pg_dump) -> Result<DatabaseSnapshot, VerificationError>`:
  PostgreSQL 16 schema/enforcement/data observation on a quiescent fixture.
- `compare(reference, candidate, reference_plan, candidate_plan) -> Result<ComparisonReport, VerificationError>`:
  exact observed comparison with separate journal validation for each executable
  plan. An equivalent result preserves blocked replacement admission.
- `validate_journal(actual, expected)`: requires a nonempty, duplicate-free
  expected plan and exactly its migration identities in the observed journal.
- `DatabaseSnapshot`, `TableObservation`, `RelationName`, `SequenceObservation`:
  structured observation values; row values retain their exact text forms.
- `ComparisonReport`, `VerificationError`: scoped evidence and explicit failure
  semantics. The `verify-migrations` binary exposes fresh and prepared-fixture
  comparison through a process boundary. See [README.md](README.md#verification-tool)
  for fixture limits and invocation.

`schema_diff` remains a supplied-operation classifier rather than a database
observer:

- `SchemaOperation`: Enumeration of granular DDL operations (`CreateTable`, `AddColumn`, `AddIndex`, `AddForeignKey`, `DropTable`, `DropColumn`, `AlterColumnType`, `AddNotNullConstraintWithoutDefault`, `RenameColumn`, `RenameTable`, `RawSql`).
- `ColumnSummary`: Structural summary of column definitions (`name`, `column_type`, `is_nullable`, `has_default`, `is_primary_key`).
- `LockClassification`: DDL lock severity (`None`, `RowLevel`, `TableExclusive`, `AccessExclusive`).
- `MigrationSafetyClassification`: `AdditiveSafe` (backward-compatible, zero maintenance window required) vs `MaintenanceRequired` (destructive, table-locking, or breaking version N).
- `MigrationDryRunReceipt`: Immutable cryptographic receipt of a migration plan dry-run (`source_schema_digest`, `target_schema_digest`, `migration_plan_digest`, `operations`, `classification`, `lock_summary`, `is_automatic_eligible`).
- `classify_operations(ops: &[SchemaOperation]) -> (MigrationSafetyClassification, LockClassification)`
- `compute_migration_plan_digest(target_migrations: &[String], operations: &[SchemaOperation]) -> String`
- `compute_schema_state_digest(tables: &[String], columns: &[String]) -> String`
- `generate_migration_dry_run_receipt(...) -> MigrationDryRunReceipt`

### Descriptors
- `MigrationDescriptor`: Internal and exportable dependency descriptor (`migration: String`, `after: Vec<String>`).

---

## 3. Error Contracts & Edge Case Guarantees

- **Cycle Detection:** Topological sort detects circular dependencies among migrations and returns an explicit, typed error (`migration dependency cycle or unsatisfied dependency`).
- **Missing Dependency Validation:** Any descriptor referencing a non-existent migration or unsatisfied dependency immediately fails validation rather than partially applying schema.
- **Duplicate Descriptor Rejection:** Duplicate dependency declarations for the same migration are strictly rejected.
- **Non-Destructive CLI Operation:** The `rustok-migrate` operational binary strictly permits only `up` and `status` commands; destructive operations (`down`, `fresh`, `reset`, `refresh`) are forbidden at the CLI argument parser.

---

## 4. Concurrency & Concurrency Models

- **Durable Single Running Job Constraint:** Migration `m20261028_000001_enforce_single_running_install_http_job` enforces database-level uniqueness on running HTTP installer jobs (`WHERE status = 'running'`), preventing concurrent installer apply execution.
- **Immutable Dry-Run Digests:** `compute_migration_plan_digest` and `compute_schema_state_digest` use domain-separated SHA-256 prefixes (`rustok:migration_plan:v1:`, `rustok:schema_state:v1:`) guaranteeing reproducible, tamper-evident plan identity.
- **Safe Bootstrap Prefix Execution:** `apply_installer_owner_schema` executes with idempotent checks on `seaql_migrations` version records to prevent re-running already applied bootstrap migrations.

---

## 5. Audit History & Verification Evidence

- **Phase FS-09 Component Audit (2026-10-31):**
  - Standardized `Cargo.toml` with workspace metadata (`version.workspace = true`, `edition.workspace = true`, `license.workspace = true`, `rust-version.workspace = true`, `repository.workspace = true`, `readme = "README.md"`, and `[package.metadata.docs.rs]`).
  - Audited all 40+ migration definitions and verified 6 Universal Logic Dimensions across dependency sorting, cycle detection, portable SQLite filtering, and schema diff classification.
  - Authored comprehensive `contract_tests.rs` verifying migrator ordering, SQLite compatibility filtering, schema safety classification (safe vs maintenance-required), deterministic hash digests, dry-run receipts, and descriptor roundtrips.
  - Verified 42/42 unit tests (38 original + 4 contract tests) and 23/23 integration tests:
    - `cargo test -p rustok-migrations --lib`: 42 passed, 0 failed.
    - `cargo test -p rustok-migrations --test ecommerce_schema_smoke`: 10 passed.
    - `cargo test -p rustok-migrations --test checkout_reconciliation_smoke`: 3 passed.
    - `cargo test -p rustok-migrations --test payment_provider_operation_uncertain_outcome`: 2 passed.
    - `cargo test -p rustok-migrations --test payment_provider_event_lifecycle_smoke`: 2 passed.
    - `cargo test -p rustok-migrations --test payment_provider_event_immutability_smoke`: 1 passed.
    - `cargo test -p rustok-migrations --test payment_provider_event_inbox_smoke`: 2 passed.
    - `cargo test -p rustok-migrations --test payment_provider_event_recovery_smoke`: 2 passed.
    - `cargo test -p rustok-migrations --test payment_provider_event_replay_smoke`: 1 passed.
  - Clippy verification: 0 warnings with `-D warnings`.

