import fs from "node:fs";

const testPath = "apps/server/tests/groups_membership_enforcement_runtime_postgres.rs";
const docsPath = "crates/rustok-groups/docs/membership-enforcement-runtime-postgres-contract.md";
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
  "rustok_groups::migrations::migrations()",
  "run_denials(&db).await",
  "run_hierarchy(&db).await",
  "run_atomicity(&db).await",
  "GroupMembershipEnforcementCommandService::new",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  "GroupMembershipEnforcementCommandPort::revoke_membership_suspension",
  '.with_claim("groups:moderate")',
  '"groups.membership_enforcement_self_target"',
  '"groups.membership_enforcement_owner_protected"',
  '"groups.manager_required"',
  "assert_no_material_change",
  "(0, 0, 0)",
  '"postgres-owner-suspend-admin"',
  '"postgres-admin-suspend-moderator"',
  '"postgres-moderator-suspend-member"',
  '"postgres-platform-suspend-member"',
  "(9, 6)",
  "(8, 8, 8)",
  'let suspend_key = "postgres-atomic-suspend"',
  'let revoke_key = "postgres-atomic-revoke"',
  '"groups.conflict"',
  "replay.replayed",
  "revoke_replay.replayed",
  "(1, 1, 1)",
  "(2, 2, 2)",
  '"group.membership_suspended"',
  '"groups.membership.suspended"',
  '"groups.membership.suspend.v1"',
  '"group.membership_suspension_revoked"',
  '"groups.membership.suspension_revoked"',
  '"groups.membership.suspension_revoke.v1"',
  'assert_eq!(source_kind, "direct_local")',
  'assert_eq!(actor_kind, "user")',
  "0::BIGINT ELSE 1::BIGINT",
  "revoked_marker, 1",
]) {
  requireText(test, marker, `Groups direct enforcement PostgreSQL runtime source is missing ${marker}`);
}

for (const forbidden of [
  "SET search_path",
  "rustok_moderation::",
  "MembershipEnforcementProvenance",
  "INSERT INTO group_membership_enforcements",
  "UPDATE group_membership_enforcements",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups direct enforcement PostgreSQL runtime source contains owner shortcut ${forbidden}`);
  }
}

for (const marker of [
  "executable source added / maintainer execution pending",
  "PostgreSQL isolation",
  "Denial and zero-side-effect contract",
  "groups.membership_enforcement_self_target",
  "groups.membership_enforcement_owner_protected",
  "groups.manager_required",
  "Exact hierarchy and platform bypass",
  "Atomic receipt/audit/event lifecycle",
  "groups.conflict",
  "Final provenance",
  "direct_local",
  "membership_enforcement_command_runtime",
]) {
  requireText(docs, marker, `Groups direct enforcement PostgreSQL runtime handoff is missing ${marker}`);
}

if (registry?.evidence?.membership_enforcement_command_runtime !== null) {
  throw new Error("unexecuted membership enforcement command runtime evidence must remain null");
}

console.log("Groups direct membership-enforcement PostgreSQL runtime source guard passed");
