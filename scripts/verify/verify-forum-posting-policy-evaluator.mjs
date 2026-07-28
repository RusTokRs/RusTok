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

function requireOrdered(source, markers, message) {
  let cursor = -1;
  for (const marker of markers) {
    const next = source.indexOf(marker, cursor + 1);
    if (next < 0 || next <= cursor) {
      failures.push(`${message}: missing or out of order ${marker}`);
      return;
    }
    cursor = next;
  }
}

const contractPath =
  "crates/rustok-forum/contracts/forum-posting-policy-evaluator.json";
const contract = JSON.parse(read(contractPath) || "{}");
const evaluator = read(contract.evaluator_file);
const contractSource = read(contract.contract_file);
const services = read(contract.service_registry);
const crateRoot = read(contract.crate_root);
const proof = read(contract.source_proof);
const note = read(contract.owner_note);
const upstream = JSON.parse(read(contract.upstream_contract) || "{}");
const plan = read("crates/rustok-forum/docs/implementation-plan.md");

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-26D" ||
  contract.upstream_task !== "FORUM-26C"
) {
  failures.push("posting policy evaluator contract must identify FORUM-26D after FORUM-26C");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("FORUM-26D must not claim unexecuted verification evidence");
}

for (const key of [
  "pure_evaluator",
  "public_rules_contract",
  "rules_normalized_before_evaluation",
  "required_facts_derived_from_rules_and_action",
  "caller_cannot_omit_required_facts",
  "caller_cannot_add_unrequired_facts",
  "all_required_facts_resolved_before_partial_decision",
  "unavailable_fact_precedence_is_stable",
  "unavailable_fact_retryability_preserved",
  "explicit_rule_precedence",
  "safety_history_before_trust_and_eligibility",
  "action_scoped_usage_windows",
  "observation_window_must_match_rule",
  "full_window_retry_hint_is_conservative",
  "bump_retry_delay_is_exact_difference",
  "empty_rules_allow",
  "reserved_duplicate_content_not_evaluated",
  "reserved_external_score_not_evaluated",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`posting policy evaluator contract must record ${key}=true`);
  }
}
for (const key of [
  "body_size_rule_invented",
  "forum_user_stats_read",
  "owner_fact_ports_added",
  "policy_persistence_added",
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
    failures.push(`posting policy evaluator contract must keep ${key}=false`);
  }
}

const expectedPrecedence = [
  "required_fact_unavailable",
  "active_flags",
  "moderation_history",
  "trust_level",
  "account_age",
  "reading_activity",
  "approved_posts",
  "reputation",
  "topic_rate_limit",
  "reply_rate_limit",
  "edit_rate_limit",
  "bump_interval",
  "link_limit",
  "mention_limit",
  "attachment_limit",
  "allowed",
];
if (JSON.stringify(contract.precedence) !== JSON.stringify(expectedPrecedence)) {
  failures.push("FORUM-26D machine precedence must remain exact and ordered");
}
if (
  JSON.stringify(contract.composition?.candidate_limit_precedence) !==
  JSON.stringify(["link_limit", "mention_limit", "attachment_limit"])
) {
  failures.push("FORUM-26D candidate-limit precedence must remain exact and ordered");
}

for (const residual of [
  "authoritative account age reading approved-post flag reputation and moderation-history fact adapters",
  "policy configuration persistence and administration",
  "topic reply edit and bump owner enforcement",
  "shared distributed rate-limit reservation and commit execution",
  "duplicate-content hashing and retained fingerprint",
  "external or AI spam scoring",
  "automatic trust promotion and demotion",
  "GraphQL REST OpenAPI admin and storefront policy surfaces",
  "PostgreSQL and cross-consumer runtime evidence",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`FORUM-26D must keep ${residual} explicitly open`);
  }
}

for (const marker of [
  "pub const FORUM_POSTING_POLICY_PRECEDENCE",
  "const MAX_SIGNED_EVIDENCE: u64",
  "pub struct ForumPostingWindowLimit",
  "pub maximum_count: u32",
  "pub window_seconds: u32",
  "if self.maximum_count == 0 || self.window_seconds == 0",
  "pub struct ForumPostingPolicyRules",
  "pub minimum_trust_level: Option<u8>",
  "pub minimum_account_age_seconds: Option<u64>",
  "pub minimum_topics_read: Option<u64>",
  "pub minimum_approved_posts: Option<u64>",
  "pub maximum_active_flags: Option<u32>",
  "pub minimum_reputation: Option<i64>",
  "pub maximum_recent_moderation_actions: Option<u32>",
  "pub topic_create_limit: Option<ForumPostingWindowLimit>",
  "pub reply_create_limit: Option<ForumPostingWindowLimit>",
  "pub edit_limit: Option<ForumPostingWindowLimit>",
  "pub minimum_seconds_between_bumps: Option<u64>",
  "pub maximum_links: Option<u16>",
  "pub maximum_mentions: Option<u16>",
  "pub maximum_attachments: Option<u16>",
  "validate_positive_unsigned_minimum",
  "value > MAX_SIGNED_EVIDENCE",
  "pub fn required_facts(",
  "required_facts_for_normalized_rules",
  "pub struct ForumPostingPolicyEvaluator",
  "pub fn decide(",
  "let rules = rules.clone().normalize()?",
  "let input = input.normalize()?",
  "if input.facts.required_facts != required_facts",
  "first_unavailable_fact(&input)",
  "unavailable.retryable",
  "observed.window_seconds != limit.window_seconds",
  "Some(u64::from(limit.window_seconds))",
  "Some(minimum - observed)",
  "Ok(ForumPostingPolicyDecision::allowed())",
]) {
  requireText(evaluator, marker, `posting policy evaluator source is missing ${marker}`);
}

requireOrdered(
  evaluator,
  [
    "ForumPostingPolicyDecisionReason::RequiredFactUnavailable",
    "ForumPostingPolicyDecisionReason::ActiveFlags",
    "ForumPostingPolicyDecisionReason::ModerationHistory",
    "ForumPostingPolicyDecisionReason::TrustLevel",
    "ForumPostingPolicyDecisionReason::AccountAge",
    "ForumPostingPolicyDecisionReason::ReadingActivity",
    "ForumPostingPolicyDecisionReason::ApprovedPosts",
    "ForumPostingPolicyDecisionReason::Reputation",
    "ForumPostingPolicyDecisionReason::TopicRateLimit",
    "ForumPostingPolicyDecisionReason::ReplyRateLimit",
    "ForumPostingPolicyDecisionReason::EditRateLimit",
    "ForumPostingPolicyDecisionReason::BumpInterval",
    "ForumPostingPolicyDecisionReason::LinkLimit",
    "ForumPostingPolicyDecisionReason::MentionLimit",
    "ForumPostingPolicyDecisionReason::AttachmentLimit",
    "ForumPostingPolicyDecisionReason::Allowed",
  ],
  "posting policy source precedence",
);

requireOrdered(
  evaluator,
  [
    "if let Some(unavailable) = first_unavailable_fact(&input)",
    "if let Some(maximum) = rules.maximum_active_flags",
    "if let Some(maximum) = rules.maximum_recent_moderation_actions",
    "if let Some(minimum) = rules.minimum_trust_level",
    "if let Some(minimum) = rules.minimum_account_age_seconds",
    "if let Some(minimum) = rules.minimum_topics_read",
    "if let Some(minimum) = rules.minimum_approved_posts",
    "if let Some(minimum) = rules.minimum_reputation",
    "match input.action",
    "if let Some(maximum) = rules.maximum_links",
    "if let Some(maximum) = rules.maximum_mentions",
    "if let Some(maximum) = rules.maximum_attachments",
    "Ok(ForumPostingPolicyDecision::allowed())",
  ],
  "posting policy evaluation flow",
);

for (const forbidden of [
  "DatabaseConnection",
  "EntityTrait",
  "QueryFilter",
  "TransactionTrait",
  "ActiveModelTrait",
  "PortContext",
  "PortError",
  "async_trait",
  "chrono::Utc",
  "SystemTime",
  "Instant::now",
  "rand::",
  "forum_user_stats",
  "UserStatsService",
  "redis",
  "reqwest",
  "sha2",
  "ForumPostingPolicyDecisionReason::DuplicateContent",
  "ForumPostingPolicyDecisionReason::ExternalSpamScore",
]) {
  rejectText(evaluator, forbidden, `posting policy evaluator must not use ${forbidden}`);
}

for (const marker of [
  "rules_derive_action_scoped_exact_required_facts",
  "caller_cannot_omit_or_add_required_facts",
  "unavailable_facts_precede_partial_denials_in_stable_order",
  "safety_history_precedes_trust_and_eligibility_denials",
  "action_window_limit_is_deterministic_and_window_bound",
  "bump_interval_returns_exact_remaining_delay",
  "candidate_limits_follow_link_mention_attachment_order",
  "passing_snapshot_is_allowed_and_body_size_is_not_invented_as_a_rule",
  "empty_rules_allow_without_owner_facts",
  "invalid_noop_or_unbounded_rules_fail_closed",
  "reserved_future_rules_are_not_in_current_precedence",
  "ForumPostingPolicyOutcome::Indeterminate",
  "ForumPostingPolicyDecisionReason::ActiveFlags",
  "ForumPostingPolicyDecisionReason::ReplyRateLimit",
  "ForumPostingPolicyDecisionReason::BumpInterval",
  "i64::MAX as u64 + 1",
]) {
  requireText(proof, marker, `posting policy evaluator source proof is missing ${marker}`);
}

for (const marker of [
  "pub enum ForumPostingPolicyDecisionReason",
  "RequiredFactUnavailable",
  "DuplicateContent",
  "ExternalSpamScore",
  "pub struct ForumPostingPolicyEvaluationInput",
]) {
  requireText(contractSource, marker, `FORUM-26C contract source is missing ${marker}`);
}
for (const marker of [
  "mod posting_policy_evaluator;",
  "pub use posting_policy_evaluator::{",
  "ForumPostingPolicyEvaluator",
  "ForumPostingPolicyRules",
  "ForumPostingWindowLimit",
  "FORUM_POSTING_POLICY_PRECEDENCE",
]) {
  requireText(services, marker, `Forum services registry is missing ${marker}`);
}
for (const marker of [
  "ForumPostingPolicyEvaluator",
  "ForumPostingPolicyRules",
  "ForumPostingWindowLimit",
  "FORUM_POSTING_POLICY_PRECEDENCE",
]) {
  requireText(crateRoot, marker, `Forum crate root is missing ${marker}`);
}

for (const marker of [
  "# FORUM-26D posting policy evaluator",
  "source-ready / unvalidated",
  "caller cannot omit a required fact",
  "Every required fact is resolved before a partial policy decision",
  "FORUM_POSTING_POLICY_PRECEDENCE",
  "conservative retry hint",
  "Shared rate limiting still owns distributed reservation",
  "`forum_user_stats` is not imported or read",
  "no duplicate-content hash",
  "no external or AI scoring call",
  "next bounded FORUM-26 slice",
  "were not run by the implementation agent",
]) {
  requireText(note, marker, `FORUM-26D owner note is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-26C" ||
  upstream.upstream_task !== "FORUM-26B" ||
  upstream.composition?.public_domain_contract !== true ||
  upstream.composition?.three_state_outcome !== true ||
  upstream.composition?.posting_policy_evaluator_added !== false ||
  upstream.composition?.forum_user_stats_read !== false
) {
  failures.push("FORUM-26D must remain grounded in the bounded FORUM-26C contract");
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
  console.error("Forum posting policy evaluator verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum posting policy evaluator is source-ready.");
