import fs from "node:fs";

const testPath = "apps/server/tests/groups_governance_enforcement_sqlite.rs";
const docsPath = "crates/rustok-groups/docs/governance-enforcement-sqlite-contract.md";
const planPath = "crates/rustok-groups/docs/implementation-plan.md";

const test = fs.readFileSync(testPath, "utf8");
const docs = fs.readFileSync(docsPath, "utf8");
const plan = fs.readFileSync(planPath, "utf8");

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

for (const marker of [
  '#![cfg(feature = "mod-groups")]',
  "tempfile::tempdir()",
  "mode=rwc",
  "rustok_groups::migrations::migrations()",
  "GroupGovernanceCommandPort::change_group_role",
  "GroupGovernanceCommandPort::transfer_group_ownership",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  'assert_eq!(wrong_actor.code, "groups.conflict")',
  '"groups.membership_enforcement_revision_conflict"',
  '"groups.membership_suspended"',
  "tokio::join!",
  "race_revision + 1",
  "moderation_decision",
  "groups:manage",
  "install_moderation_owned_owner_suspension",
]) {
  requireText(test, marker, `Groups SQLite governance evidence is missing ${marker}`);
}

for (const forbidden of [
  "sqlite::memory:",
  "rustok_moderation::",
  "rustok_forum::",
  "cargo test",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups SQLite governance evidence contains forbidden shortcut ${forbidden}`);
  }
}

for (const marker of [
  "real temporary SQLite file",
  "SQLite writer serialization",
  "Replay parity",
  "Platform recovery parity",
  "execution pending",
]) {
  requireText(docs, marker, `Groups SQLite governance evidence handoff is missing ${marker}`);
}

for (const marker of [
  "governance/enforcement SQLite evidence source",
  "groups_governance_enforcement_sqlite.rs",
  "maintainer execution pending",
  "SQLite concurrency/replay/recovery",
]) {
  requireText(plan, marker, `Canonical Groups plan is missing ${marker}`);
}

console.log("Groups SQLite governance/enforcement evidence source guard passed");
