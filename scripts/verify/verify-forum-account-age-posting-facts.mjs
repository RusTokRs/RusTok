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
  "crates/rustok-forum/contracts/forum-account-age-posting-facts.json";
const contract = JSON.parse(read(contractPath) || "{}");
const source = read(contract.adapter_file);
const production = source.split("#[cfg(test)]", 1)[0];
const services = read(contract.server_service_registry);
const host = read(contract.host_composition);
const owner = read(contract.owner_source);
const note = read(contract.owner_note);
const upstream = JSON.parse(read(contract.upstream_contract) || "{}");
const plan = read(contract.canonical_plan);

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-26F" ||
  contract.upstream_task !== "FORUM-26E"
) {
  failures.push("account-age facts must identify FORUM-26F after FORUM-26E");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("FORUM-26F must not claim unexecuted verification evidence");
}

for (const key of [
  "users_created_at_is_authority",
  "exact_tenant_user_query",
  "exact_user_actor_context",
  "read_deadline_policy_required",
  "single_clock_observation_per_call",
  "clock_is_injectable_for_source_proof",
  "future_created_at_is_invariant_violation",
  "missing_user_is_not_found",
  "storage_failure_is_retryable_unavailable",
  "upstream_composer_preserves_not_found_retryability",
  "account_age_is_never_synthesized_as_zero",
  "user_status_is_not_interpreted_as_account_age",
  "trust_provider_preserved",
  "account_age_provider_added",
  "providers_unique_by_fact_kind",
  "shared_composer_runtime_extension_published",
  "existing_audience_facts_runtime_extension_preserved",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`FORUM-26F must record ${key}=true`);
  }
}
if (contract.composition?.missing_user_retryable !== false) {
  failures.push("missing exact user must remain non-retryable");
}
for (const key of [
  "forum_crate_imports_server_user_entity",
  "forum_user_stats_read",
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
    failures.push(`FORUM-26F must keep ${key}=false`);
  }
}

if (
  JSON.stringify(contract.published_profile?.owner_facts) !==
    JSON.stringify(["trust_level", "account_age_seconds"]) ||
  JSON.stringify(contract.published_profile?.local_candidate_metrics) !==
    JSON.stringify(["link_count", "mention_count", "attachment_count"]) ||
  contract.published_profile?.undelivered_required_facts_remain_explicit !== true
) {
  failures.push("FORUM-26F published profile must be trust plus account age and local candidate metrics");
}

for (const residual of [
  "authoritative reading and approved-post owner fact adapters",
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
    failures.push(`FORUM-26F must keep ${residual} explicitly open`);
  }
}

for (const marker of [
  "pub(crate) struct ServerForumAccountAgeFactPort",
  "type AccountAgeClock = Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>",
  "now: Arc::new(Utc::now)",
  "fn with_clock(",
  "ForumPostingPolicyFactKind::AccountAgeSeconds",
  "request.normalize()",
  "validate_context(&context, request.tenant_id, request.user_id)?",
  "context.require_policy(PortCallPolicy::read())?",
  "PortActorKind::User",
  "UsersEntity::find()",
  ".filter(UsersColumn::Id.eq(request.user_id))",
  ".filter(UsersColumn::TenantId.eq(request.tenant_id))",
  "PortError::unavailable(",
  "PortError::not_found(",
  "let observed_at = (self.now)();",
  "user.created_at.with_timezone(&Utc)",
  "if created_at > observed_at {",
  "signed_duration_since(created_at).num_seconds()",
  "u64::try_from(age_seconds)",
  "PortError::invariant_violation(",
  "ForumPostingPolicyOwnerFactValue::AccountAgeSeconds(age_seconds)",
  "pub(crate) struct ServerForumPostingPolicyFactsComposer",
  "ForumPostingTrustFactPort::shared(audience_facts)",
  "ServerForumAccountAgeFactPort::shared(db)",
  "ForumPostingPolicyFactsComposer::new(vec![",
]) {
  requireText(production, marker, `account-age production source is missing ${marker}`);
}

for (const forbidden of [
  "ActiveModelTrait",
  "TransactionTrait",
  ".insert(",
  "update_many(",
  "delete_many(",
  "forum_user_stats",
  "UserStatsService",
  "ForumPostingPolicyEvaluator",
  "user.status",
  "email_verified_at",
  "last_login_at",
  "redis",
  "reqwest",
  "sha2",
  "openai",
]) {
  rejectText(production, forbidden, `account-age production source must not use ${forbidden}`);
}

for (const marker of [
  "exact_user_created_at_resolves_exact_account_age_seconds",
  "trust_and_account_age_compose_without_synthetic_facts",
  "missing_exact_user_is_non_retryable_not_found",
  "future_created_at_is_an_invariant_violation",
  "foreign_actor_is_rejected_before_storage_access",
  "setup_test_db_with_migrations::<Migrator>()",
  "fixed_clock(observed_at)",
  "ChronoDuration::milliseconds(1)",
  "ForumPostingPolicyOwnerFactValue::AccountAgeSeconds(259_217)",
  "input.facts.account_age_seconds, Some(864_000)",
  "PortErrorKind::NotFound",
  "PortErrorKind::InvariantViolation",
  "PortErrorKind::Forbidden",
]) {
  requireText(source, marker, `account-age source proof is missing ${marker}`);
}

for (const marker of [
  '#[cfg(feature = "mod-forum")]\npub mod forum_posting_policy_facts;',
]) {
  requireText(services, marker, `server service registry is missing ${marker}`);
}

for (const marker of [
  "ServerForumPostingPolicyFactsComposer::shared(",
  "audience_facts.clone()",
  "extensions.insert(audience_facts);",
  "extensions.insert(posting_policy_facts);",
  "SharedForumPostingPolicyFactsComposer",
  "extensions.contains::<rustok_forum::SharedForumAudienceFactsPort>()",
]) {
  requireText(host, marker, `host composition is missing ${marker}`);
}

for (const marker of [
  '#[sea_orm(table_name = "users")]',
  "pub id: Uuid",
  "pub tenant_id: Uuid",
  "pub created_at: DateTimeWithTimeZone",
]) {
  requireText(owner, marker, `authoritative user owner source is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-26E" ||
  upstream.upstream_task !== "FORUM-26D" ||
  upstream.downstream_account_age_task !== "FORUM-26F" ||
  upstream.downstream_account_age_contract !== contractPath ||
  upstream.composition?.unavailable_timeout_not_found_preserve_retryability !== true ||
  upstream.composition?.account_age_provider_delivered !== false ||
  upstream.composition?.server_runtime_publication_added !== false
) {
  failures.push("FORUM-26F must remain grounded in the historical FORUM-26E composition contract");
}

for (const marker of [
  "# FORUM-26F account-age posting facts",
  "source-ready / unvalidated",
  "authoritative `users.created_at`",
  "one observation timestamp per call",
  "never clamped to zero",
  "typed non-retryable `NotFound`",
  "Storage failures return retryable `Unavailable`",
  "publishes `Arc<ForumPostingPolicyFactsComposer>`",
  "`user.status`, email verification, sessions and login activity are not interpreted",
  "`forum_user_stats` is not imported or read",
  "no shared distributed rate-limit reservation",
  "next bounded FORUM-26 slice",
  "were not run by the implementation agent",
]) {
  requireText(note, marker, `FORUM-26F owner note is missing ${marker}`);
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
  console.error("Forum account-age posting facts verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum account-age posting facts are source-ready.");
