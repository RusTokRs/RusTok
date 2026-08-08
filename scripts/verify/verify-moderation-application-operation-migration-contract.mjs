import fs from "node:fs";

const test = fs.readFileSync(
  "crates/rustok-moderation/tests/application_operation_migration_contract.rs",
  "utf8",
);
const docs = fs.readFileSync(
  "crates/rustok-moderation/docs/application-operation-migration-contract.md",
  "utf8",
);
const migration = fs.readFileSync(
  "crates/rustok-moderation/src/migrations/m20260807_000004_create_moderation_application_operations.rs",
  "utf8",
);

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

for (const marker of [
  "sqlite_clean_install_has_application_operation_schema",
  "sqlite_upgrade_backfills_only_typed_decisions",
  "postgres_clean_and_upgrade_application_operation_migration_contract",
  "TestMigrator::up(&db, Some(3)).await?",
  "TestMigrator::up(&upgrade_db, Some(3)).await?",
  "seed_legacy_decisions",
  "moderation_decision_effects",
  '"schema_version":1',
  '"type":"no_domain_mutation"',
  "assert_application_schema",
  "idx_moderation_application_operations_due",
  "idx_moderation_application_operations_case",
  "tenant_matches",
  "case_matches",
  "hash_matches",
  "module_matches",
  "kind_matches",
  "subject_matches",
  "revision_matches",
  "due_matches",
  "created_matches",
  "updated_matches",
  "lease_empty",
  "error_empty",
  "applied_empty",
  "fixture.untyped_decision_id",
]) {
  requireText(test, marker, `Moderation application migration contract is missing ${marker}`);
}

for (const marker of [
  "JOIN moderation_decision_effects e",
  "WHERE NOT EXISTS",
  "'pending'",
  "d.created_at",
  "subject_revision >= 1",
  "attempt_count >= 0",
]) {
  requireText(migration, marker, `Moderation application migration source invariant is missing ${marker}`);
}

for (const marker of [
  "Clean install",
  "Upgrade fixture",
  "backfill **only** the typed decision",
  "old `effect: None` decisions remain non-dispatchable",
  "SQLite and PostgreSQL",
  "cargo test -p rustok-moderation --test application_operation_migration_contract",
]) {
  requireText(docs, marker, `Moderation application migration handoff is missing ${marker}`);
}

console.log("Moderation application-operation migration contract source guard passed");
