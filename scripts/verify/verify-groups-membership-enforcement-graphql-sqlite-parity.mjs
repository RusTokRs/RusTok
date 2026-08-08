import fs from "node:fs";

const testPath = "apps/server/tests/groups_membership_enforcement_graphql_sqlite_parity.rs";
const docsPath = "crates/rustok-groups/docs/membership-enforcement-graphql-sqlite-parity-contract.md";
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
  "GroupsQueryRoot::default()",
  "GroupsMutationRoot::default()",
  "HostRuntimeContext::new(db)",
  "permissions: Vec::new()",
  "GroupMembershipEnforcementCommandService::new",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  "GroupMembershipEnforcementCommandPort::revoke_membership_suspension",
  "suspendGroupMembership",
  "revokeGroupMembershipSuspension",
  'assert_eq!(native_stale.kind, PortErrorKind::Conflict)',
  '"groups.membership_enforcement_revision_conflict"',
  'Some("BAD_USER_INPUT".to_string())',
  'extension_json(graphql_stale_error, "domainCode")',
  'extension_json(graphql_stale_error, "retryable")',
  "native_suspend_replay.replayed",
  "native_revoke_replay.replayed",
  "native_suspend_after_revoke.replayed",
  "native_revoke.group_version, native_suspend.group_version + 1",
  "native_final.4, 3",
  'native_final.6, "direct_local"',
  "native_final.7, 1",
]) {
  requireText(test, marker, `Groups membership-enforcement GraphQL SQLite parity source is missing ${marker}`);
}

for (const forbidden of [
  "GroupsMembershipEnforcementMutation::default()",
  "groups:moderate",
  "rustok_moderation::",
  "MembershipEnforcementProvenance",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups membership-enforcement GraphQL SQLite parity source contains shortcut ${forbidden}`);
  }
}

for (const marker of [
  "executable source added / maintainer execution pending",
  "Equivalent owner fixtures",
  "Suspend and replay parity",
  "Fresh stale-CAS parity",
  "BAD_USER_INPUT",
  "domainCode=groups.membership_enforcement_revision_conflict",
  "Revoke and historical replay parity",
  "historical suspended result",
  "Final owner state",
  "Final-root composition",
  "membership_enforcement_command_transport_parity",
  "empty effective permission list",
]) {
  requireText(docs, marker, `Groups membership-enforcement GraphQL SQLite parity handoff is missing ${marker}`);
}

if (registry?.evidence?.membership_enforcement_command_transport_parity !== null) {
  throw new Error("unexecuted membership enforcement transport parity evidence must remain null");
}

console.log("Groups membership-enforcement native/GraphQL SQLite parity source guard passed");
