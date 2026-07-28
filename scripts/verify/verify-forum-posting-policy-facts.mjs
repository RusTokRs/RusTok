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
  "crates/rustok-forum/contracts/forum-posting-policy-facts.json";
const contract = JSON.parse(read(contractPath) || "{}");
const source = read(contract.composition_file);
const postingContract = read(contract.contract_file);
const evaluator = read(contract.evaluator_file);
const trustAdapter = read(contract.trust_adapter_file);
const services = read(contract.service_registry);
const crateRoot = read(contract.crate_root);
const proof = read(contract.source_proof);
const note = read(contract.owner_note);
const upstream = JSON.parse(read(contract.upstream_contract) || "{}");
const trustContract = JSON.parse(read(contract.trust_contract) || "{}");
const plan = read(contract.canonical_plan);

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-26E" ||
  contract.upstream_task !== "FORUM-26D"
) {
  failures.push("posting fact composition must identify FORUM-26E after FORUM-26D");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("FORUM-26E must not claim unexecuted verification evidence");
}

for (const key of [
  "public_owner_fact_spi",
  "shared_owner_fact_port",
  "provider_registry_unique_by_fact_kind",
  "rules_and_action_derive_exact_required_facts",
  "exact_tenant_user_actor_context",
  "read_deadline_policy_required",
  "action_scoped_window_request",
  "response_identity_validation",
  "response_fact_value_validation",
  "response_window_validation",
  "authoritative_trust_bridge",
  "trust_bridge_reuses_audience_facts_port",
  "trust_bridge_requests_no_membership_dimensions",
  "missing_provider_is_explicit_unavailable",
  "unavailable_timeout_not_found_preserve_retryability",
  "invalid_provider_error_code_has_bounded_fallback",
  "validation_forbidden_conflict_invariant_propagate",
  "composed_input_reuses_forum_26c_normalization",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`posting fact composition must record ${key}=true`);
  }
}
if (contract.composition?.missing_provider_retryable !== false) {
  failures.push("missing owner providers must remain explicit non-retryable unavailable facts");
}
for (const key of [
  "composer_invokes_evaluator",
  "forum_user_stats_read",
  "account_age_provider_delivered",
  "reading_provider_delivered",
  "approved_posts_provider_delivered",
  "active_flags_provider_delivered",
  "reputation_provider_delivered",
  "moderation_history_provider_delivered",
  "usage_window_provider_delivered",
  "bump_age_provider_delivered",
  "policy_persistence_added",
  "posting_owner_enforcement_added",
  "rate_limit_execution_added",
  "duplicate_hashing_added",
  "external_ai_scoring_call_added",
  "trust_state_write_changed",
  "automatic_trust_change_added",
  "migration_changed",
  "transport_changed",
  "server_runtime_publication_added",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`posting fact composition must keep ${key}=false`);
  }
}

if (
  JSON.stringify(contract.initial_supported_profile?.owner_facts) !==
    JSON.stringify(["trust_level"]) ||
  JSON.stringify(contract.initial_supported_profile?.local_candidate_metrics) !==
    JSON.stringify(["link_count", "mention_count", "attachment_count"]) ||
  contract.initial_supported_profile?.undelivered_required_facts_remain_explicit !== true
) {
  failures.push("FORUM-26E initial supported profile must remain trust plus local bounded candidate metrics");
}

for (const residual of [
  "authoritative account-age owner fact adapter",
  "authoritative reading and approved-post owner fact adapters",
  "authoritative active-flag and moderation-history owner fact adapters",
  "authoritative reputation owner fact adapter and ledger",
  "shared usage-window and bump-age owner adapters",
  "policy configuration persistence and administration",
  "topic reply edit and bump owner enforcement",
  "shared distributed rate-limit reservation and commit execution",
  "duplicate-content hashing and retained fingerprint",
  "external or AI spam scoring",
  "automatic trust promotion and demotion",
  "GraphQL REST OpenAPI admin and storefront policy surfaces",
  "server runtime publication and cross-consumer evidence",
  "PostgreSQL runtime evidence",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`FORUM-26E must keep ${residual} explicitly open`);
  }
}

for (const marker of [
  "pub const FORUM_POSTING_POLICY_FACTS_CAPABILITY",
  "pub struct ForumPostingPolicyCompositionRequest",
  "pub struct ForumPostingPolicyOwnerFactRequest",
  "pub window_seconds: Option<u32>",
  "pub enum ForumPostingPolicyOwnerFactValue",
  "pub struct ForumPostingPolicyOwnerFactResponse",
  "pub trait ForumPostingPolicyOwnerFactPort",
  "pub type SharedForumPostingPolicyOwnerFactPort",
  "pub struct ForumPostingPolicyFactsComposer",
  "providers.sort_by_key(|provider| provider.fact_kind())",
  "DUPLICATE_PROVIDER_CODE",
  "rules.required_facts(request.action)",
  "validate_context(&context, request.tenant_id, request.user_id)?",
  "ForumPostingPolicyOwnerFactRequest::for_rules",
  "PROVIDER_MISSING_REASON_CODE",
  "retryable: false",
  "capability_error(&error.kind)",
  "unavailable_from_error",
  ".validate_for_request(&owner_request)",
  "ForumPostingPolicyEvaluationInput",
  ".normalize()",
  "pub struct ForumPostingTrustFactPort",
  "SharedForumAudienceFactsPort",
  "include_trust_level: true",
  "channel_slugs: Vec::new()",
  "group_ids: Vec::new()",
  "audience_facts.trust_level.ok_or_else",
  "PortCallPolicy::read()",
  "PortActorKind::User",
  "PortErrorKind::Unavailable",
  "PortErrorKind::Timeout",
  "PortErrorKind::NotFound",
  "PROVIDER_ERROR_REASON_CODE",
]) {
  requireText(source, marker, `posting fact composition source is missing ${marker}`);
}

for (const forbidden of [
  "DatabaseConnection",
  "EntityTrait",
  "QueryFilter",
  "TransactionTrait",
  "ActiveModelTrait",
  "chrono::Utc",
  "SystemTime",
  "Instant::now",
  "rand::",
  "forum_user_stats",
  "UserStatsService",
  "ForumPostingPolicyEvaluator::decide",
  "redis",
  "reqwest",
  "sha2",
  "openai",
  "insert(",
  "update_many(",
  "delete_many(",
]) {
  rejectText(source, forbidden, `posting fact composition must not use ${forbidden}`);
}

for (const marker of [
  "authoritative_trust_bridge_composes_exact_fact_and_evaluates",
  "missing_provider_is_explicit_and_never_synthesizes_zero",
  "retryable_capability_error_becomes_explicit_unavailable_fact",
  "forbidden_provider_error_is_not_hidden_as_unavailable",
  "invalid_provider_response_fails_as_invariant_violation",
  "duplicate_fact_providers_are_rejected",
  "exact_actor_context_is_checked_before_provider_access",
  "action_window_request_uses_exact_configured_window",
  "mismatched_window_response_is_an_invariant_violation",
  '"forum.posting_fact.provider_missing"',
  '"profiles.account_age.unavailable"',
  "ForumPostingPolicyOutcome::Indeterminate",
  "ForumPostingPolicyDecisionReason::Allowed",
]) {
  requireText(proof, marker, `posting fact source proof is missing ${marker}`);
}

for (const marker of [
  "mod posting_policy_facts;",
  "pub use posting_policy_facts::{",
  "ForumPostingPolicyFactsComposer",
  "ForumPostingPolicyOwnerFactPort",
  "ForumPostingTrustFactPort",
  "SharedForumPostingPolicyOwnerFactPort",
  "FORUM_POSTING_POLICY_FACTS_CAPABILITY",
]) {
  requireText(services, marker, `Forum services registry is missing ${marker}`);
}
for (const marker of [
  "ForumPostingPolicyCompositionRequest",
  "ForumPostingPolicyFactsComposer",
  "ForumPostingPolicyOwnerFactPort",
  "ForumPostingTrustFactPort",
  "SharedForumPostingPolicyOwnerFactPort",
  "FORUM_POSTING_POLICY_FACTS_CAPABILITY",
]) {
  requireText(crateRoot, marker, `Forum crate root is missing ${marker}`);
}

for (const marker of [
  "pub struct ForumPostingPolicyEvaluationInput",
  "pub struct ForumPostingPolicyFacts",
  "pub struct ForumPostingPolicyUnavailableFact",
  "pub fn normalize(mut self)",
]) {
  requireText(postingContract, marker, `FORUM-26C contract source is missing ${marker}`);
}
for (const marker of [
  "pub struct ForumPostingPolicyRules",
  "pub fn required_facts(",
  "pub struct ForumPostingPolicyEvaluator",
  "required_facts_for_normalized_rules",
]) {
  requireText(evaluator, marker, `FORUM-26D evaluator source is missing ${marker}`);
}
for (const marker of [
  "pub struct ForumUserTrustAudienceFactsPort",
  "forum_user_trust_state::Entity::find_by_id",
  ".map(|level| level.unwrap_or(0))",
  "forum_user_stats` is never read",
]) {
  requireText(trustAdapter, marker, `FORUM-26B trust adapter is missing ${marker}`);
}

for (const marker of [
  "# FORUM-26E posting policy facts",
  "source-ready / unvalidated",
  "Providers are registered uniquely",
  "A missing provider is represented",
  "validation, forbidden, conflict and invariant failures propagate",
  "ForumPostingTrustFactPort",
  "initial supported composition profile",
  "does not evaluate or enforce policy",
  "`forum_user_stats` is not imported or read",
  "no shared rate-limit reservation",
  "next bounded FORUM-26 slice",
  "were not run by the implementation agent",
]) {
  requireText(note, marker, `FORUM-26E owner note is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-26D" ||
  upstream.upstream_task !== "FORUM-26C" ||
  upstream.downstream_fact_composition_task !== "FORUM-26E" ||
  upstream.downstream_contract !== contractPath ||
  upstream.composition?.pure_evaluator !== true ||
  upstream.composition?.owner_fact_ports_added !== false ||
  upstream.composition?.forum_user_stats_read !== false
) {
  failures.push("FORUM-26E must remain grounded in the bounded FORUM-26D evaluator contract");
}
if (
  trustContract.schema_version !== 1 ||
  trustContract.task !== "FORUM-26B" ||
  trustContract.upstream_task !== "FORUM-26A" ||
  trustContract.composition?.forum_owned_authoritative_state_read !== true ||
  trustContract.composition?.absent_state_defaults_to_zero !== true ||
  trustContract.composition?.trust_derived_from_forum_user_stats !== false
) {
  failures.push("FORUM-26E trust bridge must remain grounded in authoritative FORUM-26B facts");
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
  console.error("Forum posting policy fact composition verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum posting policy fact composition is source-ready.");
