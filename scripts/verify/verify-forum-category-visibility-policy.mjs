#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function rejectText(source, marker, message) {
  if (source.includes(marker)) failures.push(message);
}

const contractPath =
  "crates/rustok-forum/contracts/forum-category-visibility-policy.json";
const contract = JSON.parse(read(contractPath) || "{}");
const visibility = read(contract.enum_file ?? "");
const entity = read(contract.entity_file ?? "");
const owner = read(contract.owner_file ?? "");
const migration = read(contract.migration_file ?? "");
const migrations = read(contract.migration_registry_file ?? "");
const topicPolicy = read(contract.topic_policy_file ?? "");
const services = read(contract.services_file ?? "");
const crate = read(contract.crate_file ?? "");
const testSource = read(contract.test_file ?? "");
const plan = read(contract.canonical_plan ?? "");
const topicVisibilityContract = JSON.parse(
  read("crates/rustok-forum/contracts/forum-topic-visibility-scope.json") || "{}",
);

if (contract.schema_version !== 1) {
  failures.push("category visibility contract must use schema_version=1");
}
if (contract.task !== "FORUM-20B") {
  failures.push("category visibility contract must belong to FORUM-20B");
}
if (contract.category_bound !== 512 || contract.maximum_depth !== 16) {
  failures.push("category visibility bounds must match the canonical category tree");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("source publication must not claim unexecuted visibility evidence");
}
if (topicVisibilityContract.task !== "FORUM-20C") {
  failures.push("the cumulative topic visibility contract must remain at FORUM-20C");
}
for (const residual of [
  "role visibility",
  "trust-level visibility",
  "channel membership visibility",
  "group membership visibility",
  "explicit allow and deny",
  "create reply and moderate audience policy",
  "remaining non-category-topic-reply read composition",
  "visibility-scoped category and all-read mutations",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`category visibility contract must keep ${residual} explicitly open`);
  }
}

for (const marker of [
  "pub enum ForumCategoryVisibility",
  "ForumCategoryVisibility::Public",
  "ForumCategoryVisibility::Authenticated",
  '#[sea_orm(string_value = "public")]',
  '#[sea_orm(string_value = "authenticated")]',
  "pub const fn allows(self, is_authenticated: bool) -> bool",
]) {
  requireText(visibility, marker, `category visibility enum is missing ${marker}`);
}

for (const marker of [
  "pub visibility_override: Option<ForumCategoryVisibility>",
  "use crate::visibility::ForumCategoryVisibility",
]) {
  requireText(entity, marker, `category policy entity is missing ${marker}`);
}

for (const marker of [
  "pub struct ForumCategoryVisibilityPolicyService",
  "pub struct ForumCategoryVisibilityPolicy",
  "pub struct SetForumCategoryVisibilityPolicyInput",
  "MAX_FORUM_CATEGORY_TREE_NODES + 1",
  "MAX_FORUM_CATEGORY_TREE_DEPTH",
  "Forum category visibility cannot broaden an authenticated ancestor",
  "ForumCategoryVisibility::Authenticated =>",
  "ForumCategoryVisibility::Public =>",
  "effective_from_category_id: Some(current_id)",
  "visibility_override: Set(requested_override)",
  "Column::VisibilityOverride",
  "snapshot.resolve(category_id)?",
]) {
  requireText(owner, marker, `category visibility owner is missing ${marker}`);
}
for (const forbidden of [
  "rustok_profiles",
  "rustok_groups",
  "rustok_channels",
  "ForumTopicVisibilityService",
  "forum_topic::Entity",
]) {
  rejectText(owner, forbidden, `category visibility owner must not compose premature dependency ${forbidden}`);
}

for (const marker of [
  "add_column(",
  'Alias::new("visibility_override")',
  "visibility_override = 'authenticated'",
  "forum_category_visibility_override_insert",
  "forum_category_visibility_override_update",
  "must narrow to authenticated",
]) {
  requireText(migration, marker, `category visibility migration is missing ${marker}`);
}
requireText(
  migrations,
  "m20260724_000002_add_forum_category_visibility_policy",
  "Forum migration registry must include the category visibility migration",
);

for (const marker of [
  "visibility_override: NotSet",
  "Column::AllowsTopics",
]) {
  requireText(topicPolicy, marker, `topic placement policy must preserve visibility: ${marker}`);
}

for (const marker of [
  "mod category_visibility;",
  "ForumCategoryVisibilityPolicyService",
  "SetForumCategoryVisibilityPolicyInput",
]) {
  requireText(services, marker, `services export is missing ${marker}`);
}
for (const marker of [
  "pub mod visibility;",
  "ForumCategoryVisibilityPolicyService",
  "pub use visibility::ForumCategoryVisibility;",
]) {
  requireText(crate, marker, `crate export is missing ${marker}`);
}

for (const marker of [
  "authenticated_visibility_inherits_and_cannot_be_broadened",
  "effective_from_category_id, Some(child)",
  "cannot broaden",
  "topic policy write must preserve visibility",
  "database must reject a broadening public override",
  "ForumCategoryVisibility::Authenticated",
  "ForumCategoryVisibility::Public",
]) {
  requireText(testSource, marker, `category visibility SQLite scenario is missing ${marker}`);
}

for (const marker of [
  "Delivered in `FORUM-20B`",
  "Delivered in `FORUM-20C`",
  "Delivered in `FORUM-20E`",
  "ForumCategoryVisibilityPolicyService",
  "forum-category-visibility-policy.json",
  "category_visibility_policy_sqlite",
  "verify-forum-category-visibility-policy.mjs",
]) {
  requireText(plan, marker, `canonical FORUM-20 plan is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum category visibility policy verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum category visibility policy contract is source-ready.");
