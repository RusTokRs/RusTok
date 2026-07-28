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
  "crates/rustok-forum/contracts/forum-posting-policy-contract.json";
const contract = JSON.parse(read(contractPath) || "{}");
const source = read(contract.contract_file);
const services = read(contract.service_registry);
const crateRoot = read(contract.crate_root);
const proof = read(contract.source_proof);
const note = read(contract.owner_note);
const upstream = JSON.parse(read(contract.upstream_contract) || "{}");
const plan = read(contract.canonical_plan);

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-26C" ||
  contract.upstream_task !== "FORUM-26B"
) {
  failures.push("posting policy contract must identify FORUM-26C after FORUM-26B");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("FORUM-26C must not claim unexecuted verification evidence");
}

for (const key of [
  "public_domain_contract",
  "typed_posting_actions",
  "exact_tenant_user_identity",
  "candidate_metrics_without_content",
  "bounded_required_fact_set",
  "explicit_available_or_unavailable_partition",
  "stable_bounded_unavailable_reason_codes",
  "typed_usage_windows",
  "three_state_outcome",
  "allowed_shape_is_metadata_free",
  "denied_reason_fact_mapping",
  "typed_numeric_evidence",
  "temporal_retry_delay",
  "indeterminate_preserves_fact_retryability",
  "trust_bound_reused",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`posting policy contract must record ${key}=true`);
  }
}
for (const key of [
  "forum_user_stats_read",
  "posting_policy_evaluator_added",
  "owner_fact_ports_added",
  "posting_owner_enforcement_added",
  "trust_state_write_changed",
  "rate_limit_execution_added",
  "duplicate_hashing_added",
  "external_ai_scoring_added",
  "migration_changed",
  "transport_changed",
  "public_transport_dto_changed",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`posting policy contract must keep ${key}=false`);
  }
}

for (const residual of [
  "deterministic posting-policy evaluator and rule precedence",
  "policy configuration persistence and administration",
  "account age reading approved-post flag reputation and moderation-history fact owners",
  "topic reply edit and bump owner enforcement",
  "shared distributed rate-limit execution",
  "duplicate-content hashing and fingerprint retention",
  "external or AI spam scoring",
  "automatic trust promotion and demotion",
  "PostgreSQL and cross-consumer runtime evidence",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`FORUM-26C must keep ${residual} explicitly open`);
  }
}

for (const marker of [
  "pub const MAX_FORUM_POSTING_POLICY_FACTS: usize = 11;",
  "pub enum ForumPostingAction",
  "CreateTopic",
  "CreateReply",
  "EditTopic",
  "EditReply",
  "BumpTopic",
  "pub enum ForumPostingPolicyFactKind",
  "TrustLevel",
  "AccountAgeSeconds",
  "TopicsRead",
  "ApprovedPosts",
  "ActiveFlags",
  "Reputation",
  "RecentModerationActions",
  "TopicCreatesWindow",
  "ReplyCreatesWindow",
  "EditsWindow",
  "SecondsSinceLastBump",
  "pub struct ForumPostingWindowCount",
  "if self.window_seconds == 0",
  "pub struct ForumPostingPolicyUnavailableFact",
  "pub retryable: bool",
  "pub reason_code: String",
  "pub struct ForumPostingPolicyFacts",
  "pub required_facts: Vec<ForumPostingPolicyFactKind>",
  "pub unavailable_facts: Vec<ForumPostingPolicyUnavailableFact>",
  "if required && available == unavailable",
  "if !required && (available || unavailable)",
  "pub struct ForumPostingCandidateMetrics",
  "pub body_bytes: u32",
  "pub link_count: u16",
  "pub mention_count: u16",
  "pub attachment_count: u16",
  "pub struct ForumPostingPolicyEvaluationInput",
  "pub tenant_id: Uuid",
  "pub user_id: Uuid",
  "self.facts = self.facts.normalize()?",
  "pub enum ForumPostingPolicyOutcome",
  "Allowed",
  "Denied",
  "Indeterminate",
  "pub enum ForumPostingPolicyDecisionReason",
  "RequiredFactUnavailable",
  "pub struct ForumPostingPolicyEvidence",
  "pub observed: i64",
  "pub threshold: i64",
  "pub struct ForumPostingPolicyDecision",
  "pub retry_after_seconds: Option<u64>",
  "pub const fn allowed() -> Self",
  "pub fn denied(",
  "pub const fn indeterminate(",
  "fn validate_allowed(self)",
  "fn validate_denied(self)",
  "fn validate_indeterminate(self)",
  "if self.fact != expected_fact(self.reason)",
  "if temporal_reason(self.reason)",
  "MAX_FORUM_AUDIENCE_TRUST_LEVEL",
]) {
  requireText(source, marker, `posting policy source is missing ${marker}`);
}

for (const forbidden of [
  "DatabaseConnection",
  "EntityTrait",
  "QueryFilter",
  "TransactionTrait",
  "ActiveModelTrait",
  "PortContext",
  "PortError",
  "async_trait",
  "ForumPostingPolicyService",
  "ForumPostingPolicyEvaluator",
  "forum_user_stats",
  "UserStatsService",
  "body: String",
  "content: String",
  "serde_json::Value",
  "Vec<u8>",
  "HashMap",
  "sha2",
  "reqwest",
  "openai",
  "redis",
]) {
  rejectText(source, forbidden, `posting policy contract must not use ${forbidden}`);
}

for (const marker of [
  "required_facts_are_exactly_available_or_explicitly_unavailable",
  "missing_or_duplicated_required_fact_state_is_rejected",
  "undeclared_fact_and_invalid_window_are_rejected",
  "allowed_denied_and_indeterminate_decisions_have_distinct_shapes",
  "decision_reason_fact_evidence_and_retry_metadata_cannot_drift",
  '"profiles.age-unavailable"',
  "ForumPostingPolicyOutcome::Indeterminate",
  "ForumPostingPolicyDecisionReason::RequiredFactUnavailable",
  "ForumPostingPolicyFactKind::ReplyCreatesWindow",
]) {
  requireText(proof, marker, `posting policy source proof is missing ${marker}`);
}

for (const marker of [
  "mod posting_policy;",
  "pub use posting_policy::{",
  "ForumPostingPolicyEvaluationInput",
  "ForumPostingPolicyDecision",
  "ForumPostingPolicyUnavailableFact",
]) {
  requireText(services, marker, `Forum services registry is missing ${marker}`);
}
for (const marker of [
  "ForumPostingAction",
  "ForumPostingPolicyEvaluationInput",
  "ForumPostingPolicyDecision",
  "MAX_FORUM_POSTING_POLICY_FACTS",
]) {
  requireText(crateRoot, marker, `Forum crate root is missing ${marker}`);
}

for (const marker of [
  "# FORUM-26C posting policy contract",
  "source-ready / unvalidated",
  "Every declared fact must be represented exactly once",
  "`indeterminate` from `denied`",
  "does not expose an evaluator, service or owner port",
  "`forum_user_stats` is not read",
  "no shared distributed rate-limit call",
  "no duplicate-content hashing",
  "no external or AI spam-scoring call",
  "next bounded FORUM-26 slice",
  "canonical `crates/rustok-forum/docs/implementation-plan.md` is not replaced",
  "were not run by the implementation agent",
]) {
  requireText(note, marker, `FORUM-26C owner note is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-26B" ||
  upstream.upstream_task !== "FORUM-26A" ||
  upstream.composition?.forum_owned_authoritative_state_read !== true ||
  upstream.composition?.trust_derived_from_forum_user_stats !== false ||
  upstream.composition?.posting_policy_evaluator_added !== false
) {
  failures.push("FORUM-26C must remain grounded in the bounded FORUM-26B trust facts contract");
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
  console.error("Forum posting policy contract verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum posting policy contract is source-ready.");
