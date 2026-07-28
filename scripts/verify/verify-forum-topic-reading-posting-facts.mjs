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
  "crates/rustok-forum/contracts/forum-topic-reading-posting-facts.json";
const contract = JSON.parse(read(contractPath) || "{}");
const source = read(contract.adapter_file);
const production = source.split("#[cfg(test)]", 1)[0];
const owner = read(contract.owner_entity);
const ownerWrite = read(contract.owner_write_service);
const services = read(contract.service_registry);
const crateRoot = read(contract.crate_root);
const host = read(contract.host_composition);
const note = read(contract.owner_note);
const upstream = JSON.parse(read(contract.upstream_contract) || "{}");
const plan = read(contract.canonical_plan);

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-26G" ||
  contract.upstream_task !== "FORUM-26F"
) {
  failures.push("topic reading facts must identify FORUM-26G after FORUM-26F");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("FORUM-26G must not claim unexecuted verification evidence");
}

for (const key of [
  "forum_topic_read_states_is_authority",
  "read_state_primary_key_is_tenant_topic_user",
  "one_retained_row_counts_once_per_topic",
  "exact_tenant_user_count",
  "empty_ledger_is_authoritative_zero",
  "exact_user_actor_context",
  "read_deadline_policy_required",
  "storage_failure_is_retryable_unavailable",
  "trust_provider_preserved",
  "account_age_provider_preserved",
  "reading_provider_added",
  "providers_unique_by_fact_kind",
  "shared_composer_runtime_extension_preserved",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`FORUM-26G must record ${key}=true`);
  }
}
for (const key of [
  "empty_ledger_is_unavailable",
  "topic_visibility_join_added",
  "topic_soft_delete_reinterpretation_added",
  "user_existence_lookup_added",
  "forum_user_stats_read",
  "read_ledger_write_added",
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
    failures.push(`FORUM-26G must keep ${key}=false`);
  }
}

if (
  JSON.stringify(contract.published_profile?.owner_facts) !==
    JSON.stringify(["trust_level", "account_age_seconds", "topics_read"]) ||
  JSON.stringify(contract.published_profile?.local_candidate_metrics) !==
    JSON.stringify(["link_count", "mention_count", "attachment_count"]) ||
  contract.published_profile?.undelivered_required_facts_remain_explicit !== true
) {
  failures.push(
    "FORUM-26G published profile must be trust, account age, topics read and local candidate metrics",
  );
}

for (const residual of [
  "authoritative approved-post owner fact adapter",
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
    failures.push(`FORUM-26G must keep ${residual} explicitly open`);
  }
}

for (const marker of [
  "pub struct ForumTopicReadPostingFactPort",
  "ForumPostingPolicyFactKind::TopicsRead",
  "request.normalize()",
  "validate_context(&context, request.tenant_id, request.user_id)?",
  "context.require_policy(PortCallPolicy::read())?",
  "PortActorKind::User",
  "forum_topic_read_state::Entity::find()",
  ".filter(forum_topic_read_state::Column::TenantId.eq(request.tenant_id))",
  ".filter(forum_topic_read_state::Column::UserId.eq(request.user_id))",
  ".count(&self.db)",
  "PortError::unavailable(",
  "ForumPostingPolicyOwnerFactValue::TopicsRead(topics_read)",
  "pub fn shared(db: DatabaseConnection)",
  "empty ledger is authoritative zero",
]) {
  requireText(production, marker, `reading fact production source is missing ${marker}`);
}

for (const forbidden of [
  "forum_user_stat::",
  "UserStatsService",
  "ActiveModelTrait",
  "TransactionTrait",
  ".insert(",
  ".update(",
  "update_many(",
  "delete_many(",
  "forum_topic::Entity",
  "TopicStatus",
  "DeletedAt",
  ".join(",
  "JoinType",
  "ForumPostingPolicyEvaluator",
  "redis",
  "reqwest",
  "sha2",
  "openai",
]) {
  rejectText(production, forbidden, `reading fact production source must not use ${forbidden}`);
}

for (const marker of [
  "exact_user_topic_read_rows_are_counted_once_each",
  "empty_exact_user_read_ledger_is_authoritative_zero",
  "reading_provider_composes_exact_required_fact",
  "storage_failure_is_retryable_unavailable",
  "foreign_actor_is_rejected_before_storage_access",
  "ForumPostingPolicyOwnerFactValue::TopicsRead(2)",
  "ForumPostingPolicyOwnerFactValue::TopicsRead(0)",
  "minimum_topics_read: Some(2)",
  "input.facts.topics_read, Some(2)",
  "PortErrorKind::Unavailable",
  "PortErrorKind::Forbidden",
]) {
  requireText(source, marker, `reading fact source proof is missing ${marker}`);
}

for (const marker of [
  '#[sea_orm(table_name = "forum_topic_read_states")]',
  "pub tenant_id: Uuid",
  "pub topic_id: Uuid",
  "pub user_id: Uuid",
  "pub last_read_position: i64",
  "pub last_read_revision: i64",
  "on_delete = \"Cascade\"",
]) {
  requireText(owner, marker, `topic read-state owner entity is missing ${marker}`);
}

for (const marker of [
  "pub struct ForumTopicReadStateService",
  "pub async fn mark_topic_read(",
  "pub async fn mark_category_read(",
  "pub async fn mark_all_read(",
  "forum_topic_read_state::Entity::find()",
  "upsert_topic_read_high_water_in_tx(",
]) {
  requireText(ownerWrite, marker, `topic read-state owner service is missing ${marker}`);
}

for (const marker of [
  "mod posting_policy_reading_facts;",
  "pub use posting_policy_reading_facts::ForumTopicReadPostingFactPort;",
]) {
  requireText(services, marker, `Forum service registry is missing ${marker}`);
}
requireText(
  crateRoot,
  "ForumTopicReadPostingFactPort",
  "Forum crate root must export ForumTopicReadPostingFactPort",
);

for (const marker of [
  "ForumPostingTrustFactPort::shared(audience_facts)",
  "ServerForumAccountAgeFactPort::shared(db.clone())",
  "ForumTopicReadPostingFactPort::shared(db)",
  "ForumPostingPolicyFactsComposer::new(vec![",
  "Arc<ForumPostingPolicyFactsComposer>",
]) {
  requireText(host, marker, `host posting fact composition is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-26F" ||
  upstream.upstream_task !== "FORUM-26E" ||
  upstream.downstream_reading_task !== "FORUM-26G" ||
  upstream.downstream_reading_contract !== contractPath ||
  upstream.composition?.trust_provider_preserved !== true ||
  upstream.composition?.account_age_provider_added !== true ||
  upstream.composition?.forum_user_stats_read !== false
) {
  failures.push("FORUM-26G must remain grounded in the historical FORUM-26F contract");
}

for (const marker of [
  "# FORUM-26G topic reading posting facts",
  "source-ready / unvalidated",
  "authoritative source is `forum_topic_read_states`",
  "lifetime reading-ledger count",
  "empty exact-user ledger is authoritative `0`",
  "does not join the current topic visibility scope",
  "Storage failure returns retryable `Unavailable`",
  "requires the exact user actor",
  "registers three unique owner providers",
  "does not expose topic identities",
  "`forum_user_stats`",
  "no shared distributed rate-limit reservation",
  "next bounded FORUM-26 slice",
  "were not run by the implementation agent",
]) {
  requireText(note, marker, `FORUM-26G owner note is missing ${marker}`);
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
  console.error("Forum topic reading posting facts verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum topic reading posting facts are source-ready.");
