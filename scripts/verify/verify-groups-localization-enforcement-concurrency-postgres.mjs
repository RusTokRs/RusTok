import fs from "node:fs";

const testPath = "apps/server/tests/groups_localization_enforcement_concurrency_postgres.rs";
const docsPath = "crates/rustok-groups/docs/localization-enforcement-concurrency-postgres-contract.md";
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
  "const ROUNDS: usize = 12",
  "options=-csearch_path%3D",
  "CREATE SCHEMA",
  "DROP SCHEMA",
  "rustok_groups::migrations::migrations()",
  "tokio::sync::Barrier",
  "Barrier::new(3)",
  "connect(&scoped_url).await",
  "GroupLocalizationCommandPort::upsert_group_translation",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  "owner suspension must serialize to a successful commit",
  "match localization_result",
  'assert_eq!(error.code, "groups.membership_suspended")',
  "result.group_version < suspension.group_version as u64",
  "a localization command denied after suspension must not write translation state",
  "GroupMembershipEnforcementReadPort::read_membership_enforcement",
  "GroupMembershipEffectiveStatus::Suspended",
  "effective.membership_revision, Some(2)",
]) {
  requireText(test, marker, `Groups PostgreSQL localization concurrency source is missing ${marker}`);
}

for (const forbidden of [
  "SET search_path",
  "UPDATE group_membership_enforcements",
  "DELETE FROM group_membership_enforcements",
  "UPDATE group_memberships SET status",
  "groups:manage",
  "rustok_moderation::",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups PostgreSQL localization concurrency source contains shortcut ${forbidden}`);
  }
}

for (const marker of [
  "executable source added / maintainer execution pending",
  "twelve unique fixtures",
  "Localization wins the Group lock first",
  "Suspension wins the Group lock first",
  "groups.membership_suspended",
  "localization_concurrency",
  "No session-local `SET search_path`",
]) {
  requireText(docs, marker, `Groups PostgreSQL localization concurrency handoff is missing ${marker}`);
}

if (registry?.evidence?.localization_concurrency !== null) {
  throw new Error("unexecuted localization concurrency evidence must remain null");
}

console.log("Groups localization PostgreSQL enforcement-vs-write concurrency source guard passed");
