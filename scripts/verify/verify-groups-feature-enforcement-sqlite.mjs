import fs from "node:fs";

const testPath = "apps/server/tests/groups_feature_enforcement_sqlite.rs";
const docsPath = "crates/rustok-groups/docs/feature-enforcement-sqlite-contract.md";
const planPath = "crates/rustok-groups/docs/implementation-plan.md";

const test = fs.readFileSync(testPath, "utf8");
const docs = fs.readFileSync(docsPath, "utf8");
const plan = fs.readFileSync(planPath, "utf8");

function requireText(source, marker, message) {
  if (!source.includes(marker)) throw new Error(message);
}

for (const marker of [
  '#![cfg(feature = "mod-groups")]',
  "tempfile::tempdir()",
  "mode=rwc",
  "PRAGMA busy_timeout",
  "PRAGMA journal_mode = WAL",
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
  "base_version + 2",
  "base_version + 1",
  '("active".to_string(), 2)',
  "member_count",
]) {
  requireText(test, marker, `Groups feature enforcement SQLite evidence is missing ${marker}`);
}

for (const forbidden of [
  "database is locked",
  "groups.persistence_unavailable",
  "rustok_moderation::",
  "group_feature_bindings SET",
  "INSERT INTO group_feature_bindings",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups feature enforcement SQLite evidence contains shortcut ${forbidden}`);
  }
}

for (const marker of [
  "executable source added / maintainer execution pending",
  "Storage profile",
  "Suspension and owner-clock expiry",
  "Enforcement-versus-feature-write serialization",
  "groups.membership_suspended",
  "lifecycle `member_count`",
  "Owner surfaces only",
]) {
  requireText(docs, marker, `Groups feature enforcement SQLite handoff is missing ${marker}`);
}

for (const marker of [
  "executed feature-settings suspension/expiry and concurrent enforcement-vs-write evidence",
  "Source-complete feature-settings effective authorization",
]) {
  requireText(plan, marker, `canonical Groups plan is missing feature evidence gate ${marker}`);
}

console.log("Groups feature enforcement SQLite source guard passed");
