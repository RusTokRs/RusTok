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
  "crates/rustok-forum/contracts/forum-category-reply-create-audience-policy.json";
const contract = JSON.parse(read(contractPath) || "{}");
const migration = read(contract.migration_file ?? "");
const service = read(contract.service_file ?? "");
const entities = read(contract.entities_module ?? "");
const services = read(contract.services_module ?? "");
const crateRoot = read(contract.crate_root ?? "");
const test = read(contract.runtime_test_file ?? "");
const crateApi = read(contract.crate_api ?? "");
const plan = read(contract.canonical_plan ?? "");

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-20AU" ||
  contract.upstream_task !== "FORUM-20AT"
) {
  failures.push("reply-create audience contract must identify FORUM-20AU after FORUM-20AT");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("reply-create audience contract must not claim unexecuted evidence");
}
for (const key of [
  "separate_from_visibility_policy",
  "separate_from_topic_create_policy",
  "normalized_policy_table",
  "normalized_role_relations",
  "normalized_channel_relations",
  "normalized_group_relations",
  "normalized_explicit_user_relations",
  "root_to_category_conjunction",
  "managed_get_command",
  "managed_replace_command",
  "empty_constraints_restore_inheritance",
  "tenant_category_composite_fk",
  "raw_channel_bound",
  "raw_group_bound",
  "raw_allow_deny_user_bounds",
  "immutable_relation_rows",
  "postgres_and_sqlite_migration",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`reply-create audience contract must record ${key}`);
  }
}
for (const key of [
  "reply_create_enforcement_changed",
  "transport_changed",
  "dependency_changed",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`reply-create audience contract must keep ${key} false`);
  }
}

for (const marker of [
  "CREATE TABLE IF NOT EXISTS forum_category_reply_create_audience_policies",
  "CREATE TABLE IF NOT EXISTS forum_category_reply_create_audience_roles",
  "CREATE TABLE IF NOT EXISTS forum_category_reply_create_audience_channels",
  "CREATE TABLE IF NOT EXISTS forum_category_reply_create_audience_groups",
  "CREATE TABLE IF NOT EXISTS forum_category_reply_create_audience_users",
  "FOREIGN KEY (tenant_id, category_id)",
  "CHECK (minimum_trust_level IS NULL OR minimum_trust_level BETWEEN 0 AND 100)",
  "forum_validate_category_reply_create_audience_channel_insert",
  "forum_validate_category_reply_create_audience_group_insert",
  "forum_validate_category_reply_create_audience_user_insert",
  "forum_reject_category_reply_create_audience_update",
  ") >= 32",
  ") >= 100",
  "DatabaseBackend::Postgres",
  "DatabaseBackend::Sqlite",
]) {
  requireText(migration, marker, `reply-create audience migration is missing ${marker}`);
}

for (const marker of [
  "pub struct ForumCategoryReplyCreateAudiencePolicyLayer",
  "pub struct ForumCategoryReplyCreateAudiencePolicy",
  "pub struct SetForumCategoryReplyCreateAudiencePolicyInput",
  "pub struct ForumCategoryReplyCreateAudiencePolicyService",
  "enforce_scope(&security, Resource::ForumCategories, Action::Manage)",
  "let constraints = input.constraints.normalize()?",
  "lock_category_tree_in_tx(&txn, tenant_id)",
  "load_category_ancestor_ids(&txn, tenant_id, category_id)",
  "forum_category_reply_create_audience_policy::Entity::delete_many()",
  "if !constraints_are_empty(&constraints)",
  "load_category_reply_create_audience_policy",
  "effective_layers",
]) {
  requireText(service, marker, `reply-create audience owner is missing ${marker}`);
}
for (const forbidden of [
  "ForumAudienceFactsResolver",
  "resolve_for_constraints(",
  "ReplyService::new(",
  "create_command(",
]) {
  rejectText(
    service,
    forbidden,
    `reply-create audience persistence slice must not add command enforcement through ${forbidden}`,
  );
}

for (const marker of [
  "pub mod forum_category_reply_create_audience_policy;",
  "pub mod forum_category_reply_create_audience_role;",
  "pub mod forum_category_reply_create_audience_channel;",
  "pub mod forum_category_reply_create_audience_group;",
  "pub mod forum_category_reply_create_audience_user;",
]) {
  requireText(entities, marker, `Forum entities module is missing ${marker}`);
}
requireText(
  services,
  "mod category_reply_create_audience;",
  "Forum services module is missing the private reply-create audience module registration",
);
for (const marker of [
  "ForumCategoryReplyCreateAudiencePolicy",
  "ForumCategoryReplyCreateAudiencePolicyLayer",
  "ForumCategoryReplyCreateAudiencePolicyService",
  "SetForumCategoryReplyCreateAudiencePolicyInput",
]) {
  requireText(services, marker, `Forum services module is missing ${marker}`);
  requireText(crateRoot, marker, `Forum crate root is missing ${marker}`);
}
rejectText(
  crateRoot,
  "mod category_reply_create_audience;",
  "Forum crate root must expose public types rather than redeclare the private service module",
);

for (const marker of [
  "category_reply_create_audience_is_separate_inherited_and_database_bounded",
  "visibility_policy.configured_constraints.is_none()",
  "topic_create_policy.configured_constraints.is_none()",
  "vec![root, child]",
  "empty constraints should clear the local reply-create layer",
  "database must reject a thirty-third reply-create channel relation",
  "database must reject mutable reply-create relation-row updates",
  "database must reject mutable reply-create policy-row updates",
  "database must reject a cross-tenant reply-create category policy",
]) {
  requireText(test, marker, `reply-create audience SQLite proof is missing ${marker}`);
}

for (const marker of [
  "ForumCategoryReplyCreateAudiencePolicyService",
  "reply-create audience policy",
  "does not yet change `ReplyService::create`",
]) {
  requireText(crateApi, marker, `Forum CRATE_API is missing ${marker}`);
}
for (const marker of [
  "FORUM-20A-AU provide",
  "### Delivered in `FORUM-20AU`",
  "reply-create command-time audience enforcement",
  "Forum trust owner state and facts adapter",
  "verify-forum-category-reply-create-audience-policy.mjs",
]) {
  requireText(plan, marker, `canonical Forum plan is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum category reply-create audience verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("Forum category reply-create audience policy contract is source-ready.");
