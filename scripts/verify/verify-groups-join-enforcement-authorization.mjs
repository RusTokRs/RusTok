import fs from "node:fs";

const sourcePath = "crates/rustok-groups/src/effective_service.rs";
const docsPath = "crates/rustok-groups/docs/join-enforcement-authorization-contract.md";
const planPath = "crates/rustok-groups/docs/implementation-plan.md";

const source = fs.readFileSync(sourcePath, "utf8");
const docs = fs.readFileSync(docsPath, "utf8");
const plan = fs.readFileSync(planPath, "utf8");

function requireText(sourceText, marker, message) {
  if (!sourceText.includes(marker)) throw new Error(message);
}

const start = source.indexOf("async fn join_group_owned(");
const end = source.indexOf("\n    async fn set_group_feature_owned(", start);
if (start < 0 || end < 0) {
  throw new Error("effective Groups join owner function could not be isolated");
}
const join = source.slice(start, end);

for (const marker of [
  "let transaction = self.db.begin().await?",
  "reserve_group_write_for_update(&transaction, tenant_id, request.group_id).await?",
  "resolve_group_membership_enforcement_for_update(",
  "GroupMembershipEffectiveStatus::Suspended",
  "GroupsError::MembershipSuspended",
  "GroupMembershipEffectiveStatus::LegacyBanned",
  "GroupsError::MembershipBanned",
  ".one(&transaction)",
  ".update(&transaction)",
  ".insert(&transaction)",
  "active.member_count = Set(next_member_count)",
  "active.version = Set(next_version)",
  "transaction.commit().await?",
]) {
  requireText(join, marker, `transactional Groups join is missing ${marker}`);
}

for (const forbidden of [
  "resolve_group_membership_enforcement(\n            &transaction",
  'GroupsError::Forbidden(\n                "group membership is suspended or banned"',
  ".one(&self.db)",
  ".update(&self.db)",
  ".insert(&self.db)",
  "rustok_moderation::",
]) {
  if (join.includes(forbidden)) {
    throw new Error(`transactional Groups join contains stale or foreign-owner shortcut ${forbidden}`);
  }
}

for (const marker of [
  "source complete / maintainer runtime execution pending",
  "Group -> GroupMembership -> GroupMembershipEnforcement",
  "groups.membership_suspended",
  "groups.membership_banned",
  "Concurrent enforcement semantics",
]) {
  requireText(docs, marker, `join enforcement handoff is missing ${marker}`);
}

for (const marker of [
  "Source-complete join/rejoin effective authorization",
  "join/rejoin suspension and enforcement-vs-join",
]) {
  requireText(plan, marker, `canonical Groups plan is missing join enforcement marker ${marker}`);
}

console.log("Groups transactional join enforcement source guard passed");
