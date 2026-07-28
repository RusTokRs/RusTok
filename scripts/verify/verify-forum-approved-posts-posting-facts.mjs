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
  const absolute = path.join(repoRoot, relativePath ?? "");
  if (!relativePath || !existsSync(absolute)) {
    failures.push(`${relativePath || "<missing path>"}: required file is missing`);
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
  "crates/rustok-forum/contracts/forum-approved-posts-posting-facts.json";
const contract = JSON.parse(read(contractPath) || "{}");
const source = read(contract.adapter_file);
const production = source.split("#[cfg(test)]", 1)[0];
const topicEntity = read(contract.topic_owner_entity);
const replyEntity = read(contract.reply_owner_entity);
const stateMachine = read(contract.state_machine);
const topicOwner = read(contract.topic_owner_service);
const replyOwner = read(contract.reply_owner_service);
const topicDelete = read(contract.topic_soft_delete_service);
const replyDelete = read(contract.reply_soft_delete_service);
const softDeleteOwner = read(contract.soft_delete_owner);
const services = read(contract.service_registry);
const crateRoot = read(contract.crate_root);
const host = read(contract.host_composition);
const note = read(contract.owner_note);
const upstream = JSON.parse(read(contract.upstream_contract) || "{}");
const plan = read(contract.canonical_plan);

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-26H" ||
  contract.upstream_task !== "FORUM-26G"
) {
  failures.push("approved-posts facts must identify FORUM-26H after FORUM-26G");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("FORUM-26H must not claim unexecuted verification evidence");
}

for (const key of [
  "forum_topics_and_replies_are_authority",
  "current_retained_snapshot",
  "single_aggregate_statement",
  "postgresql_sqlite_parity",
  "exact_tenant_user_count",
  "retained_topics_count_in_all_lifecycle_statuses",
  "topic_lifecycle_status_is_not_approval_state",
  "soft_deleted_topics_excluded",
  "approved_replies_only",
  "soft_deleted_replies_excluded",
  "replies_under_soft_deleted_topics_excluded",
  "pending_replies_excluded",
  "rejected_replies_excluded",
  "hidden_replies_excluded",
  "flagged_replies_excluded",
  "deleted_status_replies_excluded",
  "empty_contribution_set_is_authoritative_zero",
  "exact_user_actor_context",
  "read_deadline_policy_required",
  "storage_failure_is_retryable_unavailable",
  "aggregate_decode_failure_is_invariant",
  "negative_count_is_invariant",
  "sum_overflow_is_invariant",
  "trust_provider_preserved",
  "account_age_provider_preserved",
  "reading_provider_preserved",
  "approved_posts_provider_added",
  "providers_unique_by_fact_kind",
  "shared_composer_runtime_extension_preserved",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`FORUM-26H must record ${key}=true`);
  }
}

for (const key of [
  "empty_contribution_set_is_unavailable",
  "user_existence_lookup_added",
  "forum_user_stats_read",
  "revision_history_read",
  "solution_count_read",
  "topic_or_reply_write_added",
  "composer_invokes_evaluator",
  "posting_owner_enforcement_added",
  "rate_limit_execution_added",
  "duplicate_hashing_added",
  "external_ai_scoring_call_added",
  "trust_state_write_changed",
  "automatic_trust_change_added",
  "migration_changed",
  "transport_changed",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`FORUM-26H must keep ${key}=false`);
  }
}

if (
  JSON.stringify(contract.published_profile?.owner_facts) !==
    JSON.stringify([
      "trust_level",
      "account_age_seconds",
      "topics_read",
      "approved_posts",
    ]) ||
  JSON.stringify(contract.published_profile?.local_candidate_metrics) !==
    JSON.stringify(["link_count", "mention_count", "attachment_count"]) ||
  contract.published_profile?.undelivered_required_facts_remain_explicit !== true
) {
  failures.push(
    "FORUM-26H published profile must be trust, account age, topics read, approved posts and local candidate metrics",
  );
}

for (const residual of [
  "authoritative active-flag and moderation-history owner fact adapters",
  "authoritative reputation owner fact adapter and ledger",
  "shared topic reply and edit usage-window owner adapters",
  "authoritative bump-age owner adapter",
  "policy configuration persistence and administration",
  "topic reply edit and bump owner enforcement",
  "shared distributed rate-limit reservation commit and release execution",
  "duplicate-content hashing and retained fingerprint",
  "external or AI spam scoring",
  "automatic trust promotion and demotion",
  "GraphQL REST OpenAPI admin and storefront policy surfaces",
  "PostgreSQL and cross-consumer runtime evidence",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`FORUM-26H must keep ${residual} explicitly open`);
  }
}

for (const marker of [
  "pub struct ForumApprovedPostsFactPort",
  "ForumPostingPolicyFactKind::ApprovedPosts",
  "request.normalize()",
  "validate_context(&context, request.tenant_id, request.user_id)?",
  "context.require_policy(PortCallPolicy::read())?",
  "PortActorKind::User",
  "approved_posts_statement(",
  "self.db.get_database_backend()",
  ".query_one(statement)",
  "DbBackend::Postgres",
  "DbBackend::Sqlite",
  "FROM forum_topics topic",
  "topic.tenant_id = $1",
  "topic.author_id = $2",
  "topic.deleted_at IS NULL",
  "FROM forum_replies reply",
  "JOIN forum_topics topic",
  "topic.id = reply.topic_id",
  "reply.tenant_id = $1",
  "reply.author_id = $2",
  "reply.status = 'approved'",
  "reply.deleted_at IS NULL",
  "topic.tenant_id = ?1",
  "reply.tenant_id = ?1",
  "fn read_count(",
  "try_get::<i64>(\"\", column)",
  "u64::try_from(value)",
  "approved_topics.checked_add(approved_replies)",
  "PortError::unavailable(",
  "PortError::invariant_violation(",
  "ForumPostingPolicyOwnerFactValue::ApprovedPosts(approved_posts)",
  "Forum approved-post owner requires PostgreSQL or SQLite",
]) {
  requireText(production, marker, `approved-post production source is missing ${marker}`);
}

if ((production.match(/\.query_one\(/g) ?? []).length !== 1) {
  failures.push("approved-post production source must use exactly one aggregate query");
}

for (const forbidden of [
  "forum_user_stat::",
  "UserStatsService",
  "forum_topic_revision",
  "forum_reply_revision",
  "forum_solution",
  "ActiveModelTrait",
  "TransactionTrait",
  ".execute(",
  ".execute_unprepared(",
  ".insert(",
  ".update(",
  "update_many(",
  "delete_many(",
  "ForumPostingPolicyEvaluator",
  "redis",
  "reqwest",
  "sha2",
  "openai",
]) {
  rejectText(production, forbidden, `approved-post production source must not use ${forbidden}`);
}

for (const marker of [
  "retained_topics_and_current_approved_replies_are_counted",
  "empty_exact_user_contribution_set_is_authoritative_zero",
  "approved_posts_provider_composes_exact_required_fact",
  "storage_failure_is_retryable_unavailable",
  "foreign_actor_is_rejected_before_storage_access",
  "ForumPostingPolicyOwnerFactValue::ApprovedPosts(5)",
  "ForumPostingPolicyOwnerFactValue::ApprovedPosts(0)",
  "minimum_approved_posts: Some(2)",
  "input.facts.approved_posts, Some(2)",
  'for status in ["pending", "rejected", "hidden", "flagged", "deleted"]',
  "deleted_topic",
  "PortErrorKind::Unavailable",
  "PortErrorKind::Forbidden",
]) {
  requireText(source, marker, `approved-post source proof is missing ${marker}`);
}

for (const marker of [
  '#[sea_orm(table_name = "forum_topics")]',
  "pub tenant_id: Uuid",
  "pub author_id: Option<Uuid>",
  "pub status: TopicStatus",
]) {
  requireText(topicEntity, marker, `topic owner entity is missing ${marker}`);
}
for (const marker of [
  '#[sea_orm(table_name = "forum_replies")]',
  "pub tenant_id: Uuid",
  "pub topic_id: Uuid",
  "pub author_id: Option<Uuid>",
  "pub status: ReplyStatus",
]) {
  requireText(replyEntity, marker, `reply owner entity is missing ${marker}`);
}
for (const marker of [
  "pub enum TopicStatus",
  "Open,",
  "Closed,",
  "Archived,",
  "pub enum ReplyStatus",
  "Pending,",
  "Approved,",
  "Rejected,",
  "Hidden,",
  "Flagged,",
  "Deleted,",
]) {
  requireText(stateMachine, marker, `Forum state machine is missing ${marker}`);
}
requireText(
  topicOwner,
  "status: Set(TopicStatus::Open)",
  "topic owner must create topics in the immediately public open lifecycle state",
);
for (const marker of [
  "let status = if category.moderated",
  "ReplyStatus::Pending",
  "ReplyStatus::Approved",
  "if status == ReplyStatus::Approved",
]) {
  requireText(replyOwner, marker, `reply owner service is missing ${marker}`);
}
for (const marker of [
  "ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ",
  "forum_topics",
  "forum_replies",
]) {
  requireText(softDeleteOwner, marker, `soft-delete owner migration is missing ${marker}`);
}
for (const marker of [
  "SET status = 'archived'",
  "deleted_at = CURRENT_TIMESTAMP",
]) {
  requireText(topicDelete, marker, `topic soft-delete owner is missing ${marker}`);
}
for (const marker of [
  "SET status = 'deleted', deleted_at = CURRENT_TIMESTAMP",
  "mark_reply_deleted_in_tx(",
]) {
  requireText(replyDelete, marker, `reply soft-delete owner is missing ${marker}`);
}

for (const marker of [
  "mod posting_policy_approved_facts;",
  "pub use posting_policy_approved_facts::ForumApprovedPostsFactPort;",
]) {
  requireText(services, marker, `Forum service registry is missing ${marker}`);
}
requireText(
  crateRoot,
  "ForumApprovedPostsFactPort",
  "Forum crate root must export ForumApprovedPostsFactPort",
);

for (const marker of [
  "ForumApprovedPostsFactPort",
  "ForumPostingTrustFactPort::shared(audience_facts)",
  "ServerForumAccountAgeFactPort::shared(db.clone())",
  "ForumApprovedPostsFactPort::shared(db.clone())",
  "ForumTopicReadPostingFactPort::shared(db)",
  "ForumPostingPolicyFactsComposer::new(vec![",
  "Arc<ForumPostingPolicyFactsComposer>",
]) {
  requireText(host, marker, `host posting fact composition is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-26G" ||
  upstream.upstream_task !== "FORUM-26F" ||
  upstream.downstream_approved_posts_task !== "FORUM-26H" ||
  upstream.downstream_approved_posts_contract !== contractPath ||
  upstream.composition?.trust_provider_preserved !== true ||
  upstream.composition?.account_age_provider_preserved !== true ||
  upstream.composition?.reading_provider_added !== true ||
  upstream.composition?.forum_user_stats_read !== false
) {
  failures.push("FORUM-26H must remain grounded in the historical FORUM-26G contract");
}

for (const marker of [
  "# FORUM-26H approved-posts posting facts",
  "source-ready / unvalidated",
  "current `forum_topics` and `forum_replies`",
  "one aggregate statement",
  "immediately `open`",
  "`open`, `closed`, and `archived`",
  "status is `approved`",
  "parent topic is not soft-deleted",
  "authoritative `0`",
  "Storage execution failures return retryable `Unavailable`",
  "PostgreSQL and SQLite",
  "requires the exact user actor",
  "does not expose topic IDs",
  "`forum_user_stats`",
  "registers four unique owner providers",
  "no shared distributed rate-limit reservation",
  "next bounded FORUM-26 slice",
  "were not run by the implementation agent",
]) {
  requireText(note, marker, `FORUM-26H owner note is missing ${marker}`);
}

for (const marker of [
  "## `FORUM-26` — anti-spam, limits and trust levels",
  "Implement forum-local trust levels and explainable posting policy",
  "account age, reading/activity, approved posts, flags, reputation and moderation",
  "External/AI scoring is optional and cannot be a synchronous correctness",
  "shared rate limiting owns distributed execution",
]) {
  requireText(plan, marker, `canonical Forum plan is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum approved-posts posting facts verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum approved-posts posting facts are source-ready.");
