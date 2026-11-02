---
id: doc://docs/guides/safe-database-migrations-and-squashing.md
doc_type: canonical_guide
status: current
owner: platform-architecture
canonical_for:
  - database-migrations-squashing
language: markdown
---

# Safe Database Migrations and Squashing Guide

This document describes the canonical protocol and tooling for managing and safely squashing database migrations across RusToK modules.

## Motivation: Why Squash Migrations in Development?

Before reaching a stable release (`v1.0.0`), modules rapidly evolve through tens of incremental migrations (e.g. adding columns, altering tables, staging data backfills, dropping deprecated intermediate tables). 

Carrying 30–70 micro-migrations into production:
- Slows down fresh tenant initialization and CI runs.
- Clutters the migration history with temporary intermediate tables and dropped columns.
- Creates fragile backward-compatibility debt.

However, **naive or manual squashing poses critical risks**:
1. **Lost Tables or Columns:** Missing an entity table or column in the consolidated migration breaks runtime CRUD queries.
2. **Nullability Mismatches:** Declaring an `Option<T>` entity field as `NOT NULL` in the migration crashes inserts when `None` is provided.
3. **Lost Constraints & Triggers:** Omitting foreign keys, composite tenant indexes (e.g. `(tenant_id, post_id)`), or SQLite/Postgres triggers breaks data integrity.
4. **Broken Verification Scripts:** Hardcoded migration filename references in `scripts/verify/` or test harnesses fail CI after old migrations are removed.
5. **Registry Desynchronization:** `src/migrations/mod.rs` failing to register the new migration or retaining dangling references to deleted files.

To eliminate these risks, RusToK provides the automated **Safe Migration Squashing & Schema Parity Tool**.

---

## Tooling: `safe-migration-squash.mjs`

The script [`scripts/dev/safe-migration-squash.mjs`](../../scripts/dev/safe-migration-squash.mjs) performs deep static analysis of SeaORM entities and SeaQuery migrations.

### Core Capabilities:
- **SeaORM Entity Parsing:** Reads all models in `src/entities/*.rs`, extracting table names, fields, types, primary keys, and `Option<T>` nullability.
- **Migration Schema Parsing:** Parses forward migrations (`async fn up`) across SeaQuery `Table::create()`, `Table::alter()`, raw SQL DDL, and `#[iden = "..."]` mappings.
- **Strict 100% Parity Assertion:** Validates that every table, column, and nullability rule matches between entities and migrations.
- **Registry & Dependency Guard:** Ensures `src/migrations/mod.rs` is 100% synchronized with physical files on disk.
- **Obsolete Reference Scanner:** Scans `scripts/verify/` and `tests/` for references to deleted migration files (`mYYYYMMDD_...`) and pinpoints the exact files and lines needing updates.

---

## Commands

```bash
# Check schema parity for a specific module:
node scripts/dev/safe-migration-squash.mjs --module <module-slug>
npm run verify:schema-parity -- --module <module-slug>

# Example for blog module:
node scripts/dev/safe-migration-squash.mjs --module blog

# Run schema parity check across all platform modules:
npm run verify:schema-parity:all

# Run the automated test suite for the squashing tool itself:
npm run test:verify:schema-parity
```

---

## Safe Squashing Protocol (Step-by-Step)

When squashing 20+ incremental migrations in a module into a single baseline:

### Step 1: Pre-Audit Existing Migrations & Entities
Run the tool against the active module:
```bash
node scripts/dev/safe-migration-squash.mjs --module <module>
```
Verify that existing entities match the current migration definitions.

### Step 2: Create Consolidated Baseline Migration
Create a single migration file:
`crates/modules/<module>/src/migrations/m<CURRENT_DATE>_000001_create_<module>_schema.rs`

Define:
- All active tables and columns in their final target state.
- Foreign keys with `tenant_id` isolation.
- Unique and composite indexes.
- Database check constraints and triggers (both SQLite and Postgres).
- Table and column `Iden` enums.

> [!IMPORTANT]
> Reference implementation: [`rustok-blog`](../../crates/modules/rustok-blog/src/migrations/m20261029_000001_create_blog_schema.rs) consolidates 32 historical migrations into 1 clean baseline covering 9 SeaORM entities and 3 system tables.

### Step 3: Remove Intermediate Migrations & Update `mod.rs`
1. Delete all squashed incremental migration files (`m202603...`, `m202607...`, etc.).
2. In `crates/modules/<module>/src/migrations/mod.rs`:
   - Declare only the consolidated migration: `pub mod m<DATE>_000001_create_<module>_schema;`
   - Register only this migration in `pub fn migrations() -> Vec<Box<dyn MigrationTrait>>`.
   - Update `MigrationDependencyDescriptor` to point to the new baseline.

### Step 4: Verify Schema Parity
Run the parity check:
```bash
node scripts/dev/safe-migration-squash.mjs --module <module>
```
The check MUST report:
```text
✅ PARITY PERFECT! All N tables and columns strictly verified.
✅ Migration registry matches 100% of physical files.
```

If any table, column, or nullability mismatch is reported, fix the consolidated migration before proceeding.

### Step 5: Remediate Obsolete Script References
If the tool warns about obsolete migration references in `scripts/verify/` or `tests/`:
1. Open the referenced script.
2. Update the hardcoded historical filename to the new consolidated migration or the canonical schema assertion.
3. Re-run `node scripts/dev/safe-migration-squash.mjs --module <module>` until **Zero obsolete migration references detected** is reached.

### Step 6: Scoped Test Verification
Run scoped compilation and unit tests for the crate:
```bash
cargo check -p <crate-name>
cargo test -p <crate-name> --lib
```
Confirm all tests pass without errors.

