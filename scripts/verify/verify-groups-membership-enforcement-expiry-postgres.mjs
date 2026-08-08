import fs from "node:fs";

const testPath = "apps/server/tests/groups_membership_enforcement_expiry_postgres.rs";
const docsPath = "crates/rustok-groups/docs/membership-enforcement-expiry-postgres-contract.md";

const test = fs.readFileSync(testPath, "utf8");
const docs = fs.readFileSync(docsPath, "utf8");

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

for (const marker of [
  '#![cfg(feature = "mod-groups")]',
  '#[ignore = "requires RUSTOK_GROUPS_TEST_POSTGRES_URL"]',
  "RUSTOK_GROUPS_TEST_POSTGRES_URL",
  "options=-csearch_path%3D",
  "CREATE SCHEMA",
  "DROP SCHEMA",
  "rustok_groups::migrations::migrations()",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  "GroupMembershipEnforcementCommandPort::revoke_membership_suspension",
  "GroupMembershipEnforcementReadPort::read_membership_enforcement",
  'with_claim("groups:access:read")',
  "chrono::Duration::seconds(2)",
  "tokio::time::sleep",
  "GroupMembershipEffectiveStatus::Suspended",
  "GroupMembershipEffectiveStatus::Active",
  "has_effective_until",
  "is_revoked",
  "expired_projection.is_effective",
  "expired_projection.revoked_at.is_none()",
  "revoked_projection.revoked_at.is_some()",
  "before_revoke_projection.0 + 1",
  "revoke_initial_revision + 2",
  "group_member_count",
  '"direct_local"',
]) {
  requireText(test, marker, `Groups PostgreSQL expiry/revoke evidence is missing ${marker}`);
}

for (const forbidden of [
  "SET search_path",
  "DELETE FROM group_membership_enforcements",
  "UPDATE group_membership_enforcements SET effective_until",
  "rustok_moderation::",
  "rustok_forum::",
  "cargo test",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups PostgreSQL expiry/revoke evidence contains forbidden shortcut ${forbidden}`);
  }
}

for (const marker of [
  "canonical Groups plan already lists expiry/revoke",
  "PostgreSQL isolation",
  "Expiry contract",
  "Direct revoke contract",
  "no cleanup mutation",
  "member_count",
  "maintainer execution pending",
]) {
  requireText(docs, marker, `Groups PostgreSQL expiry/revoke handoff is missing ${marker}`);
}

console.log("Groups membership enforcement expiry/revoke PostgreSQL source guard passed");
