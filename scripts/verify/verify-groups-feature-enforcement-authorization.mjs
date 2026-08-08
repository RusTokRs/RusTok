import fs from "node:fs";

const sourcePath = "crates/rustok-groups/src/effective_service.rs";
const planPath = "crates/rustok-groups/docs/implementation-plan.md";
const docsPath = "crates/rustok-groups/docs/feature-enforcement-authorization-contract.md";

const source = fs.readFileSync(sourcePath, "utf8");
const plan = fs.readFileSync(planPath, "utf8");
const docs = fs.readFileSync(docsPath, "utf8");

function requireText(text, marker, message) {
  if (!text.includes(marker)) throw new Error(message);
}

const featureStart = source.indexOf("async fn set_group_feature_owned(");
const featureEnd = source.indexOf("\n}\n\n#[async_trait]\nimpl GroupSummaryReadPort", featureStart);
if (featureStart < 0 || featureEnd < 0) {
  throw new Error("effective Groups feature owner function could not be isolated");
}
const feature = source.slice(featureStart, featureEnd);

for (const marker of [
  "let transaction = self.db.begin().await?",
  "reserve_group_write_for_update(&transaction, tenant_id, request.group_id).await?",
  "require_effective_manager_owned(",
  "GroupManagerCapability::ManageSettings",
  ".one(&transaction)",
  ".update(&transaction)",
  ".insert(&transaction)",
  "group_active.version = Set(",
  "transaction.commit().await?",
]) {
  requireText(feature, marker, `feature owner transaction is missing ${marker}`);
}

for (const forbidden of [
  ".one(&self.db)",
  ".update(&self.db)",
  ".insert(&self.db)",
  "resolve_group_membership_enforcement(\n            &self.db",
  "rustok_moderation::",
]) {
  if (feature.includes(forbidden)) {
    throw new Error(`feature owner transaction contains pre-transaction or foreign-owner shortcut ${forbidden}`);
  }
}

for (const marker of [
  "Source-complete feature-settings effective authorization",
  "Group -> GroupMembership -> GroupMembershipEnforcement",
  "groups.membership_suspended",
  "groups.membership_banned",
  "groups.manager_required",
  "Feature insert/update and the corresponding `groups.version` advance now commit in the same transaction",
]) {
  requireText(plan, marker, `canonical Groups plan is missing feature authorization marker ${marker}`);
}

for (const marker of [
  "source complete / maintainer runtime execution pending",
  "reserve_group_write_for_update",
  "require_effective_manager_owned",
  "groups.version",
  "groups.member_count",
  "groups.membership_suspended",
]) {
  requireText(docs, marker, `feature authorization handoff is missing ${marker}`);
}

console.log("Groups feature enforcement authorization source guard passed");