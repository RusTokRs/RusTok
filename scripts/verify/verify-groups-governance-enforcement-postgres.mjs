import fs from "node:fs";

const testPath = "apps/server/tests/groups_governance_enforcement_postgres.rs";
const docsPath = "crates/rustok-groups/docs/governance-enforcement-postgres-contract.md";
const planPath = "crates/rustok-groups/docs/implementation-plan.md";

const test = fs.readFileSync(testPath, "utf8");
const docs = fs.readFileSync(docsPath, "utf8");
const plan = fs.readFileSync(planPath, "utf8");

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

for (const marker of [
  '#![cfg(feature = "mod-groups")]',
  '#[ignore = "requires RUSTOK_GROUPS_TEST_POSTGRES_URL"]',
  "RUSTOK_GROUPS_TEST_POSTGRES_URL",
  "rustok_groups::migrations::migrations()",
  "GroupGovernanceCommandPort::change_group_role",
  "GroupGovernanceCommandPort::transfer_group_ownership",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  'assert_eq!(wrong_actor.code, "groups.conflict")',
  'assert_eq!(suspended_admin.code, "groups.membership_suspended")',
  '"groups.membership_enforcement_revision_conflict"',
  '"groups.membership_suspended"',
  "tokio::join!",
  "race_revision + 1",
  "moderation_decision",
  "groups:manage",
  "install_moderation_owned_owner_suspension",
  "DROP SCHEMA",
  "options=-csearch_path%3D",
]) {
  requireText(test, marker, `Groups PostgreSQL governance evidence is missing ${marker}`);
}

for (const forbidden of [
  "rustok_moderation::",
  "rustok_forum::",
  "SET search_path",
  "cargo test",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups PostgreSQL governance evidence contains forbidden shortcut ${forbidden}`);
  }
}

for (const marker of [
  "receipt-first lost-response replay and actor binding",
  "concurrent role mutation versus direct suspension",
  "platform ownership recovery",
  "exactly one material change",
  "RUSTOK_GROUPS_TEST_POSTGRES_URL",
  "execution pending",
]) {
  requireText(docs, marker, `Groups PostgreSQL governance evidence handoff is missing ${marker}`);
}

for (const marker of [
  "governance/enforcement PostgreSQL evidence source",
  "groups_governance_enforcement_postgres.rs",
  "maintainer execution pending",
  "governance_concurrency",
]) {
  requireText(plan, marker, `Canonical Groups plan is missing ${marker}`);
}

console.log("Groups PostgreSQL governance/enforcement evidence source guard passed");
