import fs from "node:fs";

const testPath = "apps/server/tests/groups_governance_graphql_sqlite_parity.rs";
const docsPath = "crates/rustok-groups/docs/governance-graphql-sqlite-parity-contract.md";
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
  "GroupGovernanceService::new",
  "GroupGovernanceCommandPort::change_group_role",
  "GroupGovernanceCommandPort::transfer_group_ownership",
  "changeGroupRole",
  "transferGroupOwnership",
  'assert_eq!(native_forbidden.kind, PortErrorKind::Forbidden)',
  'assert_eq!(native_forbidden.code, "groups.forbidden")',
  'Some("PERMISSION_DENIED".to_string())',
  "graphql_forbidden_error.message, native_forbidden.message",
  "native_role_replay.replayed",
  "native_transfer_replay.replayed",
  '"MEMBER",\n        "MODERATOR"',
  '"MEMBER",\n        "OWNER"',
  "native_transfer.group_version, native_role.group_version + 1",
  'membership_role(&db, tenant_id, group_id, replacement_id).await,\n            "owner"',
]) {
  requireText(test, marker, `Groups governance GraphQL SQLite parity source is missing ${marker}`);
}

for (const forbidden of [
  "GroupsGovernanceMutation::default()",
  "groups:manage",
  "rustok_moderation::",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups governance GraphQL SQLite parity source contains shortcut ${forbidden}`);
  }
}

for (const marker of [
  "executable source added / maintainer execution pending",
  "Equivalent owner fixtures",
  "Role change",
  "Ownership transfer",
  "groups.forbidden",
  "PERMISSION_DENIED",
  "receipt-first contract",
  "Final owner state",
  "Final-root composition",
  "governance_transport_parity",
  "empty effective permission list",
]) {
  requireText(docs, marker, `Groups governance GraphQL SQLite parity handoff is missing ${marker}`);
}

if (registry?.evidence?.governance_transport_parity !== null) {
  throw new Error("unexecuted governance transport parity evidence must remain null");
}

console.log("Groups governance native/GraphQL SQLite parity source guard passed");
