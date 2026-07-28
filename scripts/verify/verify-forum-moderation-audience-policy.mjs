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
  "crates/rustok-forum/contracts/forum-moderation-audience-policy.json";
const contract = JSON.parse(read(contractPath) || "{}");
const migration = read(contract.migration_file ?? "");
const policy = read(contract.policy_service ?? "");
const authorization = read(contract.authorization_service ?? "");
const moderation = read(contract.moderation_owner ?? "");
const entities = read(contract.entities_module ?? "");
const services = read(contract.services_module ?? "");
const crateRoot = read(contract.crate_root ?? "");
const test = read(contract.runtime_test_file ?? "");
const note = read(contract.owner_note ?? "");
const crateApi = read(contract.crate_api ?? "");

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-20AY" ||
  contract.upstream_task !== "FORUM-20AX"
) {
  failures.push("moderation audience contract must identify FORUM-20AY after FORUM-20AX");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("FORUM-20AY must not claim unexecuted verification evidence");
}
for (const key of [
  "separate_from_content_visibility",
  "separate_from_topic_and_reply_create_policy",
  "normalized_policy_table",
  "normalized_role_relations",
  "normalized_channel_relations",
  "normalized_group_relations",
  "normalized_explicit_user_relations",
  "category_manage_get_command",
  "category_manage_replace_command",
  "empty_constraints_restore_inheritance",
  "root_to_category_conjunction",
  "explicit_deny_precedence",
  "topic_target_category_resolution",
  "reply_target_topic_category_resolution",
  "topic_moderation_commands_gated",
  "reply_moderation_commands_gated",
  "moderator_solution_commands_gated",
  "topic_author_solution_owner_scope_preserved",
  "authorization_before_owner_write_transaction",
  "exact_optional_owner_facts",
  "missing_context_fail_closed",
  "missing_provider_fail_closed",
  "generic_public_denial",
  "context_aware_owner_methods",
  "tenant_category_composite_fk",
  "shared_postgres_owner_trigger_lock",
  "raw_channel_bound",
  "raw_group_bound",
  "raw_allow_deny_user_bounds",
  "immutable_relation_rows",
  "postgres_and_sqlite_migration",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`moderation audience contract must record ${key}`);
  }
}
for (const key of [
  "transport_changed",
  "public_dto_changed",
  "dependency_changed",
  "trust_owner_state_added",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`moderation audience contract must keep ${key}=false`);
  }
}

for (const marker of [
  "CREATE TABLE IF NOT EXISTS forum_category_moderation_audience_policies",
  "CREATE TABLE IF NOT EXISTS forum_category_moderation_audience_roles",
  "CREATE TABLE IF NOT EXISTS forum_category_moderation_audience_channels",
  "CREATE TABLE IF NOT EXISTS forum_category_moderation_audience_groups",
  "CREATE TABLE IF NOT EXISTS forum_category_moderation_audience_users",
  "FOREIGN KEY (tenant_id, category_id)",
  "REFERENCES forum_categories (tenant_id, id)",
  "CHECK (minimum_trust_level IS NULL OR minimum_trust_level BETWEEN 0 AND 100)",
  "forum_validate_category_moderation_audience_channel_insert",
  "forum_validate_category_moderation_audience_group_insert",
  "forum_validate_category_moderation_audience_user_insert",
  "forum_reject_category_moderation_audience_update",
  ":moderation",
  ") >= 32",
  ") >= 100",
  "DatabaseBackend::Postgres",
  "DatabaseBackend::Sqlite",
]) {
  requireText(migration, marker, `moderation audience migration is missing ${marker}`);
}

for (const marker of [
  "pub struct ForumCategoryModerationAudiencePolicyLayer",
  "pub struct ForumCategoryModerationAudiencePolicy",
  "pub struct SetForumCategoryModerationAudiencePolicyInput",
  "pub struct ForumCategoryModerationAudiencePolicyService",
  "enforce_scope(&security, Resource::ForumCategories, Action::Manage)?",
  "lock_category_tree_in_tx(&txn, tenant_id)",
  "lock_category_moderation_audience_in_tx(&txn, tenant_id, category_id)",
  "forum_category_moderation_audience_policy::Entity::delete_many()",
  "load_category_ancestor_ids",
  "effective_layers",
  "configured_constraints",
  "{tenant_id}:{category_id}:moderation",
]) {
  requireText(policy, marker, `moderation audience policy owner is missing ${marker}`);
}
for (const forbidden of [
  "forum_topic_audience_policy::",
  "forum_category_reply_create_audience_policy::",
  "forum_category_topic_create_audience_policy::",
  "forum_user_stats",
]) {
  rejectText(
    policy,
    forbidden,
    `moderation audience persistence must remain separate from ${forbidden}`,
  );
}

for (const marker of [
  "pub struct ForumModerationAudienceAuthorization",
  "pub struct ForumModerationAudienceAuthorizationService",
  "enforce_scope(security, Resource::ForumTopics, Action::Moderate)?",
  "enforce_scope(security, Resource::ForumReplies, Action::Moderate)?",
  "forum_reply::Entity::find_by_id(reply_id)",
  "Reply belongs to another topic",
  "load_category_moderation_audience_policy(&self.db, tenant_id, category_id)",
  "for layer in policy.effective_layers",
  "ForumAudienceEvaluator::decide(",
  "context.ok_or_else(||",
  "resolve_for_constraints(tenant_id, context, security, constraints)",
  "Forum moderation is unavailable for the current audience",
]) {
  requireText(authorization, marker, `moderation audience authorization is missing ${marker}`);
}
for (const forbidden of [
  "forum_user_stats",
  "UserStatsService",
  "publish_in_tx(",
  "ActiveModel",
  "set_status_in_tx",
  "set_pinned_in_tx",
  "set_locked_in_tx",
]) {
  rejectText(
    authorization,
    forbidden,
    `moderation authorization must not own writes or derive trust through ${forbidden}`,
  );
}

for (const marker of [
  "audience: ForumModerationAudienceAuthorizationService",
  "pub fn with_audience_facts(",
  "approve_reply_with_audience_context",
  "reject_reply_with_audience_context",
  "hide_reply_with_audience_context",
  "pin_topic_with_audience_context",
  "unpin_topic_with_audience_context",
  "lock_topic_with_audience_context",
  "unlock_topic_with_audience_context",
  "close_topic_with_audience_context",
  "reopen_topic_with_audience_context",
  "archive_topic_with_audience_context",
  "mark_solution_with_audience_context",
  "clear_solution_with_audience_context",
  ".require_reply(tenant_id, reply_id, topic_id, &security, context)",
  ".require_topic(tenant_id, topic_id, &security, context)",
  "if !is_exact_topic_author(&security, topic.author_id)",
  "security.user_id == topic_author_id",
]) {
  requireText(moderation, marker, `ModerationService is missing ${marker}`);
}
const topicAuthorizationIndex = moderation.indexOf(
  ".require_topic(tenant_id, topic_id, &security, context)",
);
const topicWriteIndex = moderation.indexOf("TopicService::set_pinned_in_tx");
if (
  topicAuthorizationIndex < 0 ||
  topicWriteIndex < 0 ||
  topicAuthorizationIndex > topicWriteIndex
) {
  failures.push("topic moderation audience authorization must run before topic writes");
}
const replyAuthorizationIndex = moderation.indexOf(
  ".require_reply(tenant_id, reply_id, topic_id, &security, context)",
);
const replyWriteIndex = moderation.indexOf("ReplyService::set_status_in_tx");
if (
  replyAuthorizationIndex < 0 ||
  replyWriteIndex < 0 ||
  replyAuthorizationIndex > replyWriteIndex
) {
  failures.push("reply moderation audience authorization must run before reply writes");
}

for (const marker of [
  "pub mod forum_category_moderation_audience_policy;",
  "pub mod forum_category_moderation_audience_role;",
  "pub mod forum_category_moderation_audience_channel;",
  "pub mod forum_category_moderation_audience_group;",
  "pub mod forum_category_moderation_audience_user;",
]) {
  requireText(entities, marker, `Forum entities module is missing ${marker}`);
}
for (const marker of [
  "mod category_moderation_audience;",
  "mod moderation_audience_authorization;",
  "ForumCategoryModerationAudiencePolicy",
  "ForumCategoryModerationAudiencePolicyService",
  "SetForumCategoryModerationAudiencePolicyInput",
  "ForumModerationAudienceAuthorization",
  "ForumModerationAudienceAuthorizationService",
]) {
  requireText(services, marker, `Forum services module is missing ${marker}`);
  if (!marker.startsWith("mod ")) {
    requireText(crateRoot, marker, `Forum crate root is missing ${marker}`);
  }
}

for (const marker of [
  "moderation_audience_gates_topic_reply_and_solution_owner_paths",
  "moderation_audience_inherits_clears_and_enforces_database_bounds",
  "denied moderation must not mutate reply status",
  "Forum moderation is unavailable for the current audience",
  "matching exact group facts should allow moderation",
  "topic author owner scope should not be narrowed by moderator audience",
  "database must reject a thirty-third moderation channel",
  "database must reject mutable moderation policy updates",
]) {
  requireText(test, marker, `moderation audience SQLite proof is missing ${marker}`);
}

for (const marker of [
  "# FORUM-20AY moderation audience policy",
  "source-ready / unvalidated",
  "The exact tenant-scoped topic author remains independently authorized",
  "transport context composition remains `FORUM-20AZ`",
  "canonical `crates/rustok-forum/docs/implementation-plan.md` is intentionally not rewritten",
  "were not run by the implementation agent",
]) {
  requireText(note, marker, `FORUM-20AY owner note is missing ${marker}`);
}
for (const marker of [
  "ForumCategoryModerationAudiencePolicyService",
  "ForumModerationAudienceAuthorizationService",
  "FORUM-20AY",
]) {
  requireText(crateApi, marker, `Forum CRATE_API is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum moderation audience verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("Forum moderation audience contract is source-ready.");
