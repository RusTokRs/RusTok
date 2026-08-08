# Moderation application-operation migration contract

Status: **source-ready / maintainer execution pending**

## Scope

`crates/rustok-moderation/tests/application_operation_migration_contract.rs` retains clean-install and upgrade/backfill evidence for migration `m20260807_000004_create_moderation_application_operations` on both SQLite and PostgreSQL.

This slice is intentionally migration-focused. Runtime enqueue, due ordering, claim/reclaim, stale-token fencing, dispatcher classification, scheduler convergence and real Forum receipt replay are covered by the separate owner/runtime PostgreSQL contracts.

## Clean install

The clean path applies all four Moderation migrations to a new database/schema and requires:

- all four migration ledger entries are present;
- `moderation_application_operations` exposes the complete production column surface;
- both owner indexes exist: `idx_moderation_application_operations_due` and `idx_moderation_application_operations_case`;
- the application queue is empty when there are no historical decisions.

SQLite uses an in-memory database with foreign keys enabled. PostgreSQL uses an isolated `rustok_moderation_migration_clean_<uuid>` schema and the production migration list.

## Upgrade fixture

The upgrade path first applies exactly the original three migrations through `m20260723_000003_create_moderation_decision_effects`, then seeds two historically valid decisions:

1. a `warning` decision with a v1 typed `NoDomainMutation` effect row;
2. another `warning` decision with no `moderation_decision_effects` row, representing truthful legacy `effect: None` state.

Both decisions have ordinary `decided` cases and explicit immutable subject revision/hash/timestamps. The fixture does not create `moderation_application_operations` itself.

The normal migrator is then resumed. Migration `000004` must create the application table and backfill **only** the typed decision.

## Retained backfill assertions

For the typed historical decision, the backfilled row must preserve the immutable owner snapshot exactly:

- `tenant_id`, `case_id`, and `decision_hash` equal the decision;
- `subject_module`, `subject_kind`, and `subject_id` equal the owning case;
- `subject_revision` equals the immutable decision revision;
- status is exactly `pending` with `attempt_count = 0`;
- `next_attempt_at`, operation `created_at`, and operation `updated_at` all equal the historical decision `created_at`;
- lease tuple, error fields, applied revision and applied timestamp are all empty.

The historical decision without a typed effect must have **no** application-operation row after upgrade. This retains the implementation-plan invariant that old `effect: None` decisions remain non-dispatchable and are never converted into guessed enforcement work.

The same assertion helper is used for SQLite and PostgreSQL so backend-specific drift in the migration/backfill contract is visible.

## PostgreSQL isolation

The PostgreSQL target reads `RUSTOK_MODERATION_TEST_DATABASE_URL`, falling back to a PostgreSQL `DATABASE_URL`. Without a PostgreSQL URL the PostgreSQL portion exits successfully with a skip message; the SQLite migration tests remain available independently.

A control connection creates separate clean and upgrade schemas, every data connection sets `search_path`, and each schema is dropped with `CASCADE` after its scenario.

## Maintainer commands

Intentionally not run while preparing this slice:

```bash
cargo test -p rustok-moderation --test application_operation_migration_contract -- --nocapture

RUSTOK_MODERATION_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-moderation --test application_operation_migration_contract -- --nocapture

node scripts/verify/verify-moderation-application-operation-migration-contract.mjs
```

No tests, Cargo commands, Node verifiers, formatters, real PostgreSQL migrations, workflows or CI were executed while preparing this file.
