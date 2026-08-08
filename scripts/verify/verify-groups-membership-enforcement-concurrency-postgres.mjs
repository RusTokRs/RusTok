import fs from "node:fs";

const testPath = "apps/server/tests/groups_membership_enforcement_concurrency_postgres.rs";
const docsPath = "crates/rustok-groups/docs/membership-enforcement-concurrency-postgres-contract.md";
const registryPath = "crates/rustok-groups/contracts/groups-fba-registry.json";
const test = fs.readFileSync(testPath, "utf8");
const docs = fs.readFileSync(docsPath, "utf8");
const registry = JSON.parse(fs.readFileSync(registryPath, "utf8"));

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
  ".max_connections(1)",
  "Barrier::new(3)",
  "tokio::time::timeout(PAIR_TIMEOUT",
  "run_same_key_suspend(&scoped_url, &fixture_db).await",
  "run_distinct_key_suspend(&scoped_url, &fixture_db).await",
  "run_distinct_key_revoke(&scoped_url, &fixture_db).await",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  "GroupMembershipEnforcementCommandPort::revoke_membership_suspension",
  '"postgres-same-key-suspend"',
  "u8::from(left.replayed) + u8::from(right.replayed), 1",
  "assert_same_material_result",
  '"postgres-distinct-suspend-left"',
  '"postgres-distinct-suspend-right"',
  '"groups.membership_enforcement_revision_conflict"',
  '"postgres-baseline-suspend"',
  '"postgres-revoke-left"',
  '"postgres-revoke-right"',
  "group_snapshot(fixture_db, fixture).await, (2, 2)",
  "group_snapshot(fixture_db, fixture).await, (3, 2)",
  "enforcement_snapshot(fixture_db, fixture).await, (1, 0)",
  "enforcement_snapshot(fixture_db, fixture).await, (2, 1)",
  "ledger_counts(fixture_db, fixture).await, (1, 1, 1)",
  "ledger_counts(fixture_db, fixture).await, (2, 2, 2)",
  "0::BIGINT ELSE 1::BIGINT",
]) {
  requireText(test, marker, `Groups direct enforcement PostgreSQL concurrency source is missing ${marker}`);
}

for (const forbidden of [
  "SET search_path",
  "groups.persistence_unavailable",
  "deadlock detected",
  "rustok_moderation::",
  "INSERT INTO group_membership_enforcements",
  "UPDATE group_membership_enforcements",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups direct enforcement PostgreSQL concurrency source contains shortcut ${forbidden}`);
  }
}

for (const marker of [
  "executable source added / maintainer execution pending",
  "PostgreSQL isolation",
  "Same-key concurrent suspension",
  "Distinct-key concurrent suspension",
  "Distinct-key concurrent revoke",
  "groups.membership_enforcement_revision_conflict",
  "Timeout/deadlock boundary",
  "membership_enforcement_command_concurrency",
]) {
  requireText(docs, marker, `Groups direct enforcement PostgreSQL concurrency handoff is missing ${marker}`);
}

if (registry?.evidence?.membership_enforcement_command_concurrency !== null) {
  throw new Error("unexecuted membership enforcement command concurrency evidence must remain null");
}

console.log("Groups direct membership-enforcement PostgreSQL concurrency source guard passed");
