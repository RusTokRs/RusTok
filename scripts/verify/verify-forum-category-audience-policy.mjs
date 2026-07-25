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

function between(source, start, end, label) {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return "";
  }
  return source.slice(startIndex, endIndex);
}

const contractPath =
  "crates/rustok-forum/contracts/forum-category-audience-policy.json";
const contract = JSON.parse(read(contractPath) || "{}");
const owner = read(contract.owner_file ?? "");
const audience = read(contract.audience_contract_file ?? "");
const migration = read(contract.migration_file ?? "");
const migrations = read(contract.migration_registry_file ?? "");
const entities = read(contract.entities_file ?? "");
const services = read(contract.services_file ?? "");
const crate = read(contract.crate_file ?? "");
const testSource = read(contract.test_file ?? "");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("category audience policy contract must use schema_version=1");
}
if (contract.task !== "FORUM-20G") {
  failures.push("category audience policy contract must belong to FORUM-20G");
}
if (contract.delivered_through !== "FORUM-20H") {
  failures.push("category audience policy contract must be reconciled through FORUM-20H");
}
if (contract.composition?.topic_narrowing_storage !== true) {
  failures.push("category audience contract must record delivered topic narrowing storage");
}
for (const [key, expected] of Object.entries({
  category_tree_nodes: 512,
  category_tree_depth: 16,
  roles_per_layer: 4,
  channels_per_layer: 32,
  groups_per_layer: 32,
  allow_users_per_layer: 100,
  deny_users_per_layer: 100,
  maximum_trust_level: 100,
})) {
  if (contract.bounds?.[key] !== expected) {
    failures.push(`category audience bound ${key} must remain ${expected}`);
  }
}
if (contract.policy?.inheritance !== "root-to-target conjunction of non-empty local layers") {
  failures.push("category audience inheritance must remain conjunctive root-to-target layers");
}
if (contract.policy?.storage !== "normalized typed tables without JSON") {
  failures.push("category audience persistence must remain normalized and typed");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("source publication must not claim unexecuted category audience evidence");
}
for (const residual of [
  "category topic and reply read composition",
  "create reply and moderate audience write policy",
  "channel and group provider adapters",
  "trust-level owner provider",
  "visibility-scoped category and all-read mutations",
  "notification search index SEO and deep-link migration",
  "PostgreSQL and cross-consumer runtime evidence",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`category audience contract must keep ${residual} open`);
  }
}
if (contract.not_delivered?.includes("topic narrowing persistence and commands")) {
  failures.push("delivered topic narrowing must not remain open");
}

for (const table of contract.tables ?? []) {
  requireText(migration, table, `category audience migration is missing ${table}`);
}
for (const marker of [
  "CREATE UNIQUE INDEX IF NOT EXISTS uq_forum_categories_tenant_id",
  "FOREIGN KEY (tenant_id, category_id)",
  "REFERENCES forum_categories (tenant_id, id)",
  "REFERENCES forum_category_audience_policies (tenant_id, category_id)",
  "minimum_trust_level BETWEEN 0 AND 100",
  "role IN ('super_admin', 'admin', 'manager', 'customer')",
  "effect IN ('allow', 'deny')",
  "forum_validate_category_audience_channel_insert",
  "forum_validate_category_audience_group_insert",
  "forum_validate_category_audience_user_insert",
  "forum_reject_category_audience_relation_update",
  "forum_category_audience_policy_update",
  "forum category audience channels exceed bounded limit",
  "forum category audience groups exceed bounded limit",
  "forum category audience users exceed bounded limit",
]) {
  requireText(migration, marker, `category audience migration is missing ${marker}`);
}
for (const forbidden of ["JSONB", "jsonb", "serde_json", "metadata"]) {
  rejectText(migration, forbidden, `category audience migration must not use ${forbidden}`);
}
requireText(
  migrations,
  "m20260725_000001_add_forum_category_audience_policy",
  "Forum migration registry must include the category audience migration",
);

for (const marker of [
  "pub mod forum_category_audience_channel;",
  "pub mod forum_category_audience_group;",
  "pub mod forum_category_audience_policy;",
  "pub mod forum_category_audience_role;",
  "pub mod forum_category_audience_user;",
]) {
  requireText(entities, marker, `category audience entity registry is missing ${marker}`);
}

for (const marker of [
  "pub struct ForumCategoryAudiencePolicyLayer",
  "pub struct ForumCategoryAudiencePolicy",
  "pub struct SetForumCategoryAudiencePolicyInput",
  "pub struct ForumCategoryAudiencePolicyService",
  "pub async fn get(",
  "pub async fn set(",
  "enforce_scope(&security, Resource::ForumCategories, Action::Manage)?",
  "Policy details may contain explicit user and group identifiers",
  "lock_category_tree_in_tx(&txn, tenant_id).await?",
  "forum_category_audience_policy::Entity::delete_many()",
  "insert_roles(&txn, tenant_id, category_id, &constraints).await?",
  "insert_channels(&txn, tenant_id, category_id, &constraints).await?",
  "insert_groups(&txn, tenant_id, category_id, &constraints).await?",
  "insert_users(&txn, tenant_id, category_id, &constraints).await?",
  "ancestors.reverse()",
  "effective_layers",
  "Root-to-target non-empty layers. Every layer must allow the viewer.",
  "ensure_storage_bound(",
  "MAX_FORUM_CATEGORY_TREE_NODES + 1",
  "MAX_FORUM_CATEGORY_TREE_DEPTH",
]) {
  requireText(owner, marker, `category audience owner is missing ${marker}`);
}
const getBlock = between(
  owner,
  "pub async fn get(",
  "pub async fn set(",
  "category audience owner get",
);
for (const marker of [
  "Action::Manage",
  "let txn = self.db.begin().await?",
  "lock_category_tree_in_tx(&txn, tenant_id).await?",
  "txn.commit().await?",
]) {
  requireText(getBlock, marker, `managed category audience get is missing ${marker}`);
}
const setBlock = between(
  owner,
  "pub async fn set(",
  "async fn insert_roles(",
  "category audience owner set",
);
const deleteIndex = setBlock.indexOf("forum_category_audience_policy::Entity::delete_many()");
const policyInsertIndex = setBlock.indexOf("forum_category_audience_policy::ActiveModel");
const commitIndex = setBlock.indexOf("txn.commit().await?");
if (
  deleteIndex < 0 ||
  policyInsertIndex < 0 ||
  commitIndex < 0 ||
  deleteIndex > policyInsertIndex ||
  policyInsertIndex > commitIndex
) {
  failures.push("category audience owner must replace local layers atomically with delete then insert");
}
for (const forbidden of [
  "rustok_groups",
  "rustok_channel",
  "groups::",
  "channel::",
  "forum_user_stat",
  "serde_json::Value",
]) {
  rejectText(owner, forbidden, `category audience owner must not depend on ${forbidden}`);
}

for (const marker of [
  "pub struct ForumAudienceConstraints",
  "MAX_FORUM_AUDIENCE_CHANNELS",
  "MAX_FORUM_AUDIENCE_GROUPS",
  "MAX_FORUM_AUDIENCE_EXPLICIT_USERS",
]) {
  requireText(audience, marker, `shared audience contract is missing ${marker}`);
}

for (const marker of [
  "mod category_audience;",
  "ForumCategoryAudiencePolicyService",
  "SetForumCategoryAudiencePolicyInput",
]) {
  requireText(services, marker, `services export is missing ${marker}`);
}
for (const marker of [
  "ForumCategoryAudiencePolicy",
  "ForumCategoryAudiencePolicyLayer",
  "ForumCategoryAudiencePolicyService",
  "SetForumCategoryAudiencePolicyInput",
]) {
  requireText(crate, marker, `crate export is missing ${marker}`);
}

for (const marker of [
  "category_audience_layers_inherit_conjunctively_and_remain_bounded",
  "vec![root, child]",
  "SecurityContext::public_read()",
  "Err(ForumError::Forbidden(_))",
  "empty constraints should clear the local layer",
  "database must reject a thirty-third channel relation",
  "database must reject mutable relation-row updates",
  "database must reject mutable policy-row updates",
  "database must reject a cross-tenant category policy relation",
]) {
  requireText(testSource, marker, `category audience SQLite scenario is missing ${marker}`);
}

for (const marker of [
  "Delivered in `FORUM-20F`",
  "Delivered in `FORUM-20G`",
  "ForumCategoryAudiencePolicyService",
  "forum-category-audience-policy.json",
  "category_audience_policy_sqlite",
  "verify-forum-category-audience-policy.mjs",
]) {
  requireText(plan, marker, `canonical FORUM-20 plan is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum category audience policy verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum category audience policy contract is source-ready.");
