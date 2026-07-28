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
  "crates/rustok-forum/contracts/forum-topic-reply-create-audience-policy.json";
const contract = JSON.parse(read(contractPath) || "{}");
const migration = read(contract.migration_file ?? "");
const service = read(contract.service_file ?? "");
const authorization = read(contract.authorization_service ?? "");
const facade = read("crates/rustok-forum/src/services/reply_facade.rs");
const entities = read(contract.entities_module ?? "");
const services = read(contract.services_module ?? "");
const crateRoot = read(contract.crate_root ?? "");
const test = read(contract.runtime_test_file ?? "");
const note = read(contract.owner_note ?? "");
const crateApi = read(contract.crate_api ?? "");

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-20AX" ||
  contract.upstream_task !== "FORUM-20AW"
) {
  failures.push("topic reply-create audience contract must identify FORUM-20AX after FORUM-20AW");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("FORUM-20AX must not claim unexecuted verification evidence");
}
for (const key of [
  "separate_from_topic_visibility",
  "separate_from_category_reply_create_storage",
  "normalized_policy_table",
  "normalized_role_relations",
  "normalized_channel_relations",
  "normalized_group_relations",
  "normalized_explicit_user_relations",
  "topic_manage_get_command",
  "topic_manage_replace_command",
  "empty_constraints_clear_topic_layer",
  "root_category_to_topic_conjunction",
  "topic_layer_cannot_broaden_category",
  "category_and_topic_denial_attribution",
  "authorization_before_raw_owner",
  "legacy_and_inline_create_paths_inherit_enforcement",
  "graphql_and_rest_paths_inherit_enforcement",
  "tenant_topic_composite_fk",
  "raw_channel_bound",
  "raw_group_bound",
  "raw_allow_deny_user_bounds",
  "immutable_relation_rows",
  "shared_postgres_advisory_lock",
  "postgres_and_sqlite_migration",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`topic reply-create audience contract must record ${key}`);
  }
}
for (const key of [
  "transport_changed",
  "public_dto_changed",
  "dependency_changed",
  "topic_visibility_changed",
  "trust_owner_state_added",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`topic reply-create audience contract must keep ${key}=false`);
  }
}

for (const marker of [
  "CREATE TABLE IF NOT EXISTS forum_topic_reply_create_audience_policies",
  "CREATE TABLE IF NOT EXISTS forum_topic_reply_create_audience_roles",
  "CREATE TABLE IF NOT EXISTS forum_topic_reply_create_audience_channels",
  "CREATE TABLE IF NOT EXISTS forum_topic_reply_create_audience_groups",
  "CREATE TABLE IF NOT EXISTS forum_topic_reply_create_audience_users",
  "FOREIGN KEY (tenant_id, topic_id)",
  "REFERENCES forum_topics (tenant_id, id)",
  "CHECK (minimum_trust_level IS NULL OR minimum_trust_level BETWEEN 0 AND 100)",
  "forum_validate_topic_reply_create_audience_channel_insert",
  "forum_validate_topic_reply_create_audience_group_insert",
  "forum_validate_topic_reply_create_audience_user_insert",
  "forum_reject_topic_reply_create_audience_update",
  "NEW.topic_id::text || ':reply-create'",
  ") >= 32",
  ") >= 100",
  "DatabaseBackend::Postgres",
  "DatabaseBackend::Sqlite",
]) {
  requireText(migration, marker, `topic reply-create audience migration is missing ${marker}`);
}

for (const marker of [
  "pub struct ForumTopicReplyCreateAudiencePolicy",
  "pub struct SetForumTopicReplyCreateAudiencePolicyInput",
  "pub struct ForumTopicReplyCreateAudiencePolicyService",
  "enforce_scope(&security, Resource::ForumTopics, Action::Manage)?",
  "lock_category_tree_in_tx(&txn, tenant_id)",
  "lock_topic_reply_create_audience_in_tx(&txn, tenant_id, topic_id)",
  "format!(\"{tenant_id}:{topic_id}:reply-create\")",
  "load_category_reply_create_audience_policy(&txn, tenant_id, topic.category_id)",
  "forum_topic_reply_create_audience_policy::Entity::delete_many()",
  "if !constraints_are_empty(&constraints)",
  "load_topic_reply_create_audience_policy_for_topic",
  "inherited_category_layers",
  "configured_constraints",
]) {
  requireText(service, marker, `topic reply-create audience owner is missing ${marker}`);
}
for (const forbidden of [
  "forum_topic_audience_policy::",
  "forum_topic_audience_role::",
  "ForumTopicAudiencePolicyService",
]) {
  rejectText(
    service,
    forbidden,
    `topic reply-create policy must remain separate from visibility through ${forbidden}`,
  );
}

for (const marker of [
  "load_topic_reply_create_audience_policy_for_topic(&self.db, tenant_id, &topic)",
  "for layer in policy.inherited_category_layers",
  "if let Some(constraints) = policy.configured_constraints",
  "denied_by_category_id: Some(layer.category_id)",
  "denied_by_category_id: None",
  "already-present `topic_id` as the final topic-local denying layer",
  "Forum reply creation is unavailable for the current audience",
]) {
  requireText(authorization, marker, `reply-create authorization is missing ${marker}`);
}
const categoryIndex = authorization.indexOf("for layer in policy.inherited_category_layers");
const topicIndex = authorization.indexOf("if let Some(constraints) = policy.configured_constraints");
if (categoryIndex < 0 || topicIndex < 0 || categoryIndex > topicIndex) {
  failures.push("reply-create authorization must evaluate category layers before the topic layer");
}
const requireIndex = facade.indexOf(".require(tenant_id, topic_id, &security, context)");
const rawOwnerIndex = facade.indexOf(".create_command(tenant_id, security, topic_id, input)");
if (requireIndex < 0 || rawOwnerIndex < 0 || requireIndex > rawOwnerIndex) {
  failures.push("topic reply-create narrowing must remain before the raw owner command");
}

for (const marker of [
  "pub mod forum_topic_reply_create_audience_policy;",
  "pub mod forum_topic_reply_create_audience_role;",
  "pub mod forum_topic_reply_create_audience_channel;",
  "pub mod forum_topic_reply_create_audience_group;",
  "pub mod forum_topic_reply_create_audience_user;",
]) {
  requireText(entities, marker, `Forum entities module is missing ${marker}`);
}
for (const marker of [
  "mod topic_reply_create_audience;",
  "ForumTopicReplyCreateAudiencePolicy",
  "ForumTopicReplyCreateAudiencePolicyService",
  "SetForumTopicReplyCreateAudiencePolicyInput",
]) {
  requireText(services, marker, `Forum services module is missing ${marker}`);
  if (marker !== "mod topic_reply_create_audience;") {
    requireText(crateRoot, marker, `Forum crate root is missing ${marker}`);
  }
}

for (const marker of [
  "topic_reply_create_layer_narrows_categories_and_clears_locally",
  "topic_reply_create_policy_is_separate_and_database_bounded",
  "topic-local denial must occur before reply and body writes",
  "clearing topic layer should restore category-only authorization",
  "database must reject a thirty-third topic reply-create channel",
  "database must reject mutable topic reply-create policy updates",
  "database must reject a cross-tenant topic reply-create policy",
]) {
  requireText(test, marker, `topic reply-create SQLite proof is missing ${marker}`);
}

for (const marker of [
  "# FORUM-20AX topic-local reply-create audience narrowing",
  "source-ready / unvalidated",
  "topic can narrow but never broaden",
  "does not read or mutate `forum_topic_audience_*` visibility rows",
  "canonical `crates/rustok-forum/docs/implementation-plan.md` is intentionally not rewritten",
  "were not run by the implementation agent",
]) {
  requireText(note, marker, `FORUM-20AX owner note is missing ${marker}`);
}
for (const marker of [
  "ForumTopicReplyCreateAudiencePolicyService",
  "FORUM-20AX",
  "category layers followed by the optional topic layer",
]) {
  requireText(crateApi, marker, `Forum CRATE_API is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum topic reply-create audience verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("Forum topic reply-create audience contract is source-ready.");
