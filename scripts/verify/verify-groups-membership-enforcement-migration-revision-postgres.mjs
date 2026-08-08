import fs from "node:fs";

const testPath = "apps/server/tests/groups_membership_enforcement_migration_revision_postgres.rs";
const docsPath = "crates/rustok-groups/docs/membership-enforcement-migration-revision-postgres-contract.md";
const registryPath = "crates/rustok-groups/contracts/groups-fba-registry.json";
const migrationsPath = "crates/rustok-groups/src/migrations/mod.rs";
const enforcementMigrationPath =
  "crates/rustok-groups/src/migrations/m20260723_000008_create_group_membership_enforcement_state.rs";

const test = fs.readFileSync(testPath, "utf8");
const docs = fs.readFileSync(docsPath, "utf8");
const registry = JSON.parse(fs.readFileSync(registryPath, "utf8"));
const migrations = fs.readFileSync(migrationsPath, "utf8");
const enforcementMigration = fs.readFileSync(enforcementMigrationPath, "utf8");

function requireText(source, marker, message) {
  if (!source.includes(marker)) throw new Error(message);
}

for (const marker of [
  '#![cfg(feature = "mod-groups")]',
  '#[ignore = "requires RUSTOK_GROUPS_TEST_POSTGRES_URL"]',
  "RUSTOK_GROUPS_TEST_POSTGRES_URL",
  "options=-csearch_path%3D",
  "CREATE SCHEMA",
  "DROP SCHEMA",
  "current_schema()",
  "information_schema.columns",
  "information_schema.tables",
  "rustok_groups::migrations::migrations()",
  "migrations.len() >= 9",
  "migrations.iter().take(7)",
  "migrations[7]",
  "migrations.iter().skip(8)",
  "revision column must not exist before PostgreSQL enforcement migration",
  "enforcement projection table must not exist before PostgreSQL migration 000008",
  "membership_revision(&db, tenant_id, group_id, target_id).await, 1",
  "SET role = 'moderator'",
  "PostgreSQL membership revision decrease must fail closed",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  "expected_membership_revision: 2",
  "suspended.membership_revision, 3",
  "suspended.enforcement_revision, 1",
  "GroupMembershipEnforcementCommandPort::revoke_membership_suspension",
  "expected_membership_revision: 3",
  "revoked.membership_revision, 4",
  "revoked.enforcement_revision, 2",
  "group_snapshot(&db, tenant_id, group_id).await, (3, 2)",
  "SET status = 'left'",
  "membership_revision(&db, tenant_id, group_id, target_id).await, 5",
  "PostgreSQL revision must remain monotonic after enforcement and lifecycle mutations",
]) {
  requireText(test, marker, `Groups enforcement migration/revision PostgreSQL source is missing ${marker}`);
}

for (const forbidden of [
  "SET search_path",
  "INSERT INTO group_membership_enforcements",
  "UPDATE group_membership_enforcements",
  "rustok_moderation::",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups enforcement migration/revision PostgreSQL source contains shortcut ${forbidden}`);
  }
}

const migration8 = "m20260723_000008_create_group_membership_enforcement_state";
const migration9 = "m20260808_000009_extend_group_domain_events_for_membership_enforcement";
const migration8Index = migrations.indexOf(migration8);
const migration9Index = migrations.indexOf(migration9);
if (migration8Index < 0 || migration9Index < 0 || migration8Index >= migration9Index) {
  throw new Error("Groups migration order must retain enforcement state before event-ledger extension");
}

for (const marker of [
  "group_memberships",
  "revision",
  "group_membership_enforcements",
  "restore_status",
  "source_kind",
  "moderation_decision_id",
  "moderation_decision_hash",
  "effective_until",
  "revoked_at",
]) {
  requireText(
    enforcementMigration,
    marker,
    `Groups enforcement migration source is missing ${marker}`,
  );
}

for (const marker of [
  "executable source added / maintainer execution pending",
  "PostgreSQL isolation",
  "Real pre-000008 backfill",
  "Membership revision monotonicity",
  "Enforcement-trigger revision sources",
  "Migration ordering",
  "arbitrary SQL no-op update behavior",
  "membership_enforcement_migration",
  "membership_enforcement_revision_runtime",
]) {
  requireText(docs, marker, `Groups enforcement migration/revision PostgreSQL handoff is missing ${marker}`);
}

if (registry?.evidence?.membership_enforcement_migration !== null) {
  throw new Error("unexecuted membership enforcement migration evidence must remain null");
}
if (registry?.evidence?.membership_enforcement_revision_runtime !== null) {
  throw new Error("unexecuted membership enforcement revision runtime evidence must remain null");
}

console.log("Groups membership-enforcement migration/revision PostgreSQL source guard passed");
