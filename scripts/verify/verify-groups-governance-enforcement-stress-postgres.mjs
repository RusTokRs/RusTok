import fs from "node:fs";

const testPath = "apps/server/tests/groups_governance_enforcement_stress_postgres.rs";
const docsPath = "crates/rustok-groups/docs/governance-enforcement-stress-postgres-contract.md";
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
  "const ROUNDS: usize = 3",
  "const TARGETS_PER_ROUND: usize = 8",
  "options=-csearch_path%3D",
  "CREATE SCHEMA",
  "DROP SCHEMA",
  ".max_connections(1)",
  "Barrier::new(TARGETS_PER_ROUND * 2 + 1)",
  "let role_db = connect(&scoped_url).await",
  "let suspension_db = connect(&scoped_url).await",
  "GroupGovernanceService::new(role_db)",
  "GroupMembershipEnforcementCommandService::new(suspension_db)",
  "tokio::time::timeout(PAIR_TIMEOUT",
  "GroupGovernanceCommandPort::change_group_role",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  '"groups.membership_enforcement_revision_conflict"',
  '"groups.membership_suspended"',
  "role_wins + suspension_wins, TARGETS_PER_ROUND",
  "base_version + TARGETS_PER_ROUND as i64",
  "TARGETS_PER_ROUND + 1",
  "membership_revision",
  "active_enforcement_count",
]) {
  requireText(test, marker, `Groups governance/enforcement PostgreSQL stress source is missing ${marker}`);
}

for (const forbidden of [
  "SET search_path",
  "groups.persistence_unavailable",
  "deadlock detected",
  "rustok_moderation::",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups governance/enforcement PostgreSQL stress source accepts shortcut ${forbidden}`);
  }
}

for (const marker of [
  "executable source added / maintainer execution pending",
  "PostgreSQL isolation",
  "Fan-out shape",
  "Allowed per-target outcomes",
  "groups.membership_enforcement_revision_conflict",
  "groups.membership_suspended",
  "Aggregate invariants",
  "Group -> GroupMembership -> GroupMembershipEnforcement",
  "governance_concurrency",
]) {
  requireText(docs, marker, `Groups governance/enforcement PostgreSQL stress handoff is missing ${marker}`);
}

if (registry?.evidence?.governance_concurrency !== null) {
  throw new Error("unexecuted governance concurrency evidence must remain null");
}

console.log("Groups governance/enforcement PostgreSQL stress source guard passed");
