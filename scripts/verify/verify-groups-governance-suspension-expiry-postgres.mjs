import fs from "node:fs";

const testPath = "apps/server/tests/groups_governance_suspension_expiry_postgres.rs";
const docsPath = "crates/rustok-groups/docs/governance-suspension-expiry-postgres-contract.md";
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
  "GroupGovernanceService::new",
  "GroupGovernanceCommandPort::change_group_role",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  "postgres-admin-baseline-role-change",
  "temporary_governance_review",
  'assert_eq!(admin_role_during, "admin")',
  'assert_eq!(admin_status_during, "active")',
  'assert_eq!(denied.code, "groups.membership_suspended")',
  "blocked_target_revision, 1",
  "tokio::time::sleep",
  "expired suspension should restore PostgreSQL administrator governance authority without cleanup",
  "restored.group_version, suspended.group_version as u64 + 1",
  "admin_revision_after, suspended.membership_revision",
  "restored_target_revision, blocked_target_revision + 1",
  "group_member_count",
]) {
  requireText(test, marker, `Groups governance suspension/expiry PostgreSQL source is missing ${marker}`);
}

for (const forbidden of [
  "SET search_path",
  "groups:manage",
  "rustok_moderation::",
  "MembershipEnforcementProvenance",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups governance suspension/expiry PostgreSQL source contains shortcut ${forbidden}`);
  }
}

for (const marker of [
  "executable source added / maintainer execution pending",
  "PostgreSQL isolation",
  "stored administrator role remains `admin`",
  "stored membership status remains `active`",
  "groups.membership_suspended",
  "no revoke or cleanup mutation",
  "exact suspension revision",
  "role-versus-suspension race",
  "native/GraphQL governance parity",
]) {
  requireText(docs, marker, `Groups governance suspension/expiry PostgreSQL handoff is missing ${marker}`);
}

console.log("Groups governance PostgreSQL suspension/expiry source guard passed");
