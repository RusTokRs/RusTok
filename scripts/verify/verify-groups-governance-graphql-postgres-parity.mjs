import fs from "node:fs";

const testPath = "apps/server/tests/groups_governance_graphql_postgres_parity.rs";
const docsPath = "crates/rustok-groups/docs/governance-graphql-postgres-parity-contract.md";
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
]) {
  requireText(test, marker, `Groups governance GraphQL PostgreSQL parity source is missing ${marker}`);
}

for (const forbidden of [
  "SET search_path",
  "GroupsGovernanceMutation::default()",
  "groups:manage",
  "rustok_moderation::",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups governance GraphQL PostgreSQL parity source contains shortcut ${forbidden}`);
  }
}

for (const marker of [
  "executable source added / maintainer execution pending",
  "PostgreSQL isolation",
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
  requireText(docs, marker, `Groups governance GraphQL PostgreSQL parity handoff is missing ${marker}`);
}

if (registry?.evidence?.governance_transport_parity !== null) {
  throw new Error("unexecuted governance transport parity evidence must remain null");
}

console.log("Groups governance native/GraphQL PostgreSQL parity source guard passed");
