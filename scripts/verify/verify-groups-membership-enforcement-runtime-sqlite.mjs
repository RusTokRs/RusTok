import fs from "node:fs";

const testPath = "apps/server/tests/groups_membership_enforcement_runtime_sqlite.rs";
const docsPath = "crates/rustok-groups/docs/membership-enforcement-runtime-sqlite-contract.md";
const registryPath = "crates/rustok-groups/contracts/groups-fba-registry.json";
const test = fs.readFileSync(testPath, "utf8");
const docs = fs.readFileSync(docsPath, "utf8");
const registry = JSON.parse(fs.readFileSync(registryPath, "utf8"));

function requireText(source, marker, message) {
  if (!source.includes(marker)) throw new Error(message);
}

for (const marker of [
  '#![cfg(feature = "mod-groups")]',
  "tempfile::tempdir()",
  "mode=rwc",
  "rustok_groups::migrations::migrations()",
  "GroupMembershipEnforcementCommandService::new",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  "GroupMembershipEnforcementCommandPort::revoke_membership_suspension",
  '.with_claim("groups:moderate")',
  '"groups.membership_enforcement_self_target"',
  '"groups.membership_enforcement_owner_protected"',
  '"groups.manager_required"',
  "assert_no_material_change",
  "(0, 0, 0)",
  '"owner-suspend-admin"',
  '"admin-suspend-moderator"',
  '"moderator-suspend-member"',
  '"platform-suspend-member"',
  "(9, 6)",
  "(8, 8, 8)",
  'let suspend_key = "atomic-suspend"',
  'let revoke_key = "atomic-revoke"',
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
  "revoked_marker, 1",
]) {
  requireText(test, marker, `Groups direct enforcement SQLite runtime source is missing ${marker}`);
}

for (const forbidden of [
  "rustok_moderation::",
  "MembershipEnforcementProvenance",
  "INSERT INTO group_membership_enforcements",
  "UPDATE group_membership_enforcements",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups direct enforcement SQLite runtime source contains owner shortcut ${forbidden}`);
  }
}

for (const marker of [
  "executable source added / maintainer execution pending",
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
  requireText(docs, marker, `Groups direct enforcement SQLite runtime handoff is missing ${marker}`);
}

if (registry?.evidence?.membership_enforcement_command_runtime !== null) {
  throw new Error("unexecuted membership enforcement command runtime evidence must remain null");
}

console.log("Groups direct membership-enforcement SQLite runtime source guard passed");
