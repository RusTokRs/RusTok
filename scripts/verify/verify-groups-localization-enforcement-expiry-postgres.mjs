import fs from "node:fs";

const testPath = "apps/server/tests/groups_localization_enforcement_expiry_postgres.rs";
const docsPath = "crates/rustok-groups/docs/localization-enforcement-expiry-postgres-contract.md";
const test = fs.readFileSync(testPath, "utf8");
const docs = fs.readFileSync(docsPath, "utf8");

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
  "rustok_groups::migrations::migrations()",
  "GroupLocalizationService::new",
  "GroupLocalizationReadPort::list_group_translations",
  "GroupLocalizationCommandPort::upsert_group_translation",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  'assert_eq!(stored_status_during_suspension, "active")',
  'assert_eq!(read_error.code, "groups.membership_suspended")',
  'assert_eq!(write_error.code, "groups.membership_suspended")',
  "failed suspended write must not create French translation",
  "tokio::time::sleep",
  "expired suspension should restore administrator reads without cleanup",
  "expired suspension should restore administrator writes without cleanup",
  "stored_revision_after_expiry, suspended.membership_revision",
  "restored.group_version, suspended.group_version as u64 + 1",
  "group_member_count",
]) {
  requireText(test, marker, `Groups PostgreSQL localization expiry source is missing ${marker}`);
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
    throw new Error(`Groups PostgreSQL localization expiry source contains shortcut ${forbidden}`);
  }
}

for (const marker of [
  "executable source added / maintainer execution pending",
  "PostgreSQL isolation",
  "stored membership status remains `active`",
  "groups.membership_suspended",
  "no cleanup mutation",
  "membership revision must remain unchanged since suspension",
  "localization_transport_parity",
  "localization_concurrency",
]) {
  requireText(docs, marker, `Groups PostgreSQL localization expiry handoff is missing ${marker}`);
}

console.log("Groups localization PostgreSQL suspension/expiry source guard passed");
