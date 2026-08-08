import fs from "node:fs";

const testPath = "apps/server/tests/groups_feature_enforcement_postgres.rs";
const docsPath = "crates/rustok-groups/docs/feature-enforcement-postgres-contract.md";
const planPath = "crates/rustok-groups/docs/implementation-plan.md";

const test = fs.readFileSync(testPath, "utf8");
const docs = fs.readFileSync(docsPath, "utf8");
const plan = fs.readFileSync(planPath, "utf8");

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
  ".max_connections(1)",
  "rustok_groups::migrations::migrations()",
  "GroupCommandPort::set_group_feature",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  "GroupMembershipEnforcementReadPort::read_membership_enforcement",
  "GroupAccessReadPort::enabled_group_features",
  '"groups.membership_suspended"',
  "GroupMembershipEffectiveStatus::Suspended",
  "GroupMembershipEffectiveStatus::Active",
  "const ROUNDS: usize = 8",
  "Barrier::new(3)",
  "race_base_version + 2",
  "race_base_version + 1",
  '("active".to_string(), 2)',
  "member_count",
]) {
  requireText(test, marker, `Groups feature enforcement PostgreSQL evidence is missing ${marker}`);
}

for (const forbidden of [
  "SET search_path",
  "groups.persistence_unavailable",
  "rustok_moderation::",
  "group_feature_bindings SET",
  "INSERT INTO group_feature_bindings",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups feature enforcement PostgreSQL evidence contains shortcut ${forbidden}`);
  }
}

for (const marker of [
  "executable source added / maintainer execution pending",
  "PostgreSQL isolation",
  "Suspension and owner-clock expiry",
  "Enforcement-versus-feature-write serialization",
  "groups.membership_suspended",
  "member count remains two",
  "Owner surfaces only",
]) {
  requireText(docs, marker, `Groups feature enforcement PostgreSQL handoff is missing ${marker}`);
}

for (const marker of [
  "executed feature-settings suspension/expiry and concurrent enforcement-vs-write evidence",
  "Source-complete feature-settings effective authorization",
]) {
  requireText(plan, marker, `canonical Groups plan is missing feature evidence gate ${marker}`);
}

console.log("Groups feature enforcement PostgreSQL source guard passed");
