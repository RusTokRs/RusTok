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
  "crates/rustok-forum/contracts/forum-notification-inbox-open-privacy.json";
const contract = JSON.parse(read(contractPath) || "{}");
const inbox = read(contract.notifications_owner_file ?? "");
const policyOwner = read(contract.recipient_policy_owner_file ?? "");
const docs = read(contract.notifications_live_contract ?? "");
const forumProvider = read(contract.forum_source_provider ?? "");
const proof = read(contract.sqlite_proof ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("forum notification inbox privacy contract must use schema_version=1");
}
if (
  contract.task !== "FORUM-20S" ||
  contract.upstream_task !== "FORUM-20R" ||
  contract.downstream_task !== "FORUM-20T"
) {
  failures.push("forum notification inbox privacy contract must connect FORUM-20R/S/T");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("inbox privacy recheck must not claim unexecuted evidence");
}

for (const delivered of [
  "mandatory_recipient_policy_dependency",
  "exact_policy_request_identity",
  "privacy_before_source_authorization",
  "suppression_fail_closed",
  "suppression_reason_not_exposed",
  "retryable_policy_failure_preserved",
  "cross_recipient_no_oracle_preserved",
  "fresh_source_route_preserved",
  "read_state_unchanged",
  "delivery_attempts_unchanged",
  "sqlite_contract_proof",
  "live_notifications_docs",
]) {
  if (contract.composition?.[delivered] !== true) {
    failures.push(`forum notification inbox privacy contract must record ${delivered} as delivered`);
  }
}

for (const residual of [
  "bounded inbox listing API",
  "seen read and archive state mutations",
  "channel delivery enqueue and transports",
  "delivery-time target authorization",
  "host trust facts adapter",
  "host channel membership facts adapter",
  "initially non-public topic-created descriptor materialization",
  "search index SEO and deep-link migration",
  "PostgreSQL and cross-consumer runtime evidence",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`forum notification inbox privacy contract must keep ${residual} explicitly open`);
  }
}

const deliveredSlices = [
  "FORUM-20H",
  "FORUM-20I",
  "FORUM-20J",
  "FORUM-20K",
  "FORUM-20L",
  "FORUM-20M",
  "FORUM-20N",
  "FORUM-20O",
  "FORUM-20P",
  "FORUM-20Q",
  "FORUM-20R",
  "FORUM-20S",
];
const planSync = contract.canonical_plan_sync ?? {};
if (planSync.required_ledger_through !== "FORUM-20S") {
  failures.push("inbox privacy contract must require the canonical ledger through FORUM-20S");
}
if (JSON.stringify(planSync.required_delivered_sections) !== JSON.stringify(deliveredSlices)) {
  failures.push("inbox privacy contract must require FORUM-20H through FORUM-20S delivered sections");
}
if (planSync.status === "pending") {
  if (planSync.current_plan_through !== "FORUM-20G") {
    failures.push("pending canonical plan synchronization must identify FORUM-20G as current");
  }
  requireText(
    plan,
    "FORUM-20A-G provide",
    "pending canonical plan synchronization must remain grounded in FORUM-20A-G",
  );
  for (const slice of deliveredSlices) {
    rejectText(
      plan,
      `### Delivered in \`${slice}\``,
      `canonical plan now contains ${slice}; update canonical_plan_sync before claiming pending through G`,
    );
  }
} else if (planSync.status === "synchronized") {
  requireText(plan, "FORUM-20A-S provide", "synchronized canonical plan must advance through S");
  for (const slice of deliveredSlices) {
    requireText(plan, `### Delivered in \`${slice}\``, `canonical plan is missing ${slice}`);
  }
} else {
  failures.push("canonical_plan_sync.status must be pending or synchronized");
}

for (const marker of [
  "policy: Arc<dyn NotificationRecipientPolicy>",
  "policy: Arc<dyn NotificationRecipientPolicy>,",
  ".evaluate(NotificationRecipientPolicyRequest {",
  "tenant_id: request.tenant_id",
  "recipient_id: request.recipient_id",
  "actor_id: stored.actor_id",
  "source_slug: source.as_str().to_string()",
  "source_event_id: stored.source_event_id",
  "source_revision: stored.source_revision",
  "notification_type: stored.notification_type",
  "target: target.clone()",
  "Ok(NotificationRecipientPolicyDecision::Allow) => {}",
  "Ok(NotificationRecipientPolicyDecision::Suppress { .. })",
  "return Ok(NotificationInboxOpenDecision::Unavailable)",
  "NotificationError::RecipientPolicyFailure",
  "retryable: error.retryable",
  ".authorize_target_open(AuthorizeNotificationTargetRequest",
]) {
  requireText(inbox, marker, `notification inbox privacy owner is missing ${marker}`);
}

const lookupIndex = inbox.indexOf("notification::Entity::find_by_id(request.notification_id)");
const missingIndex = inbox.indexOf("let Some(stored) = stored else");
const policyIndex = inbox.indexOf(".evaluate(NotificationRecipientPolicyRequest {");
const suppressionIndex = inbox.indexOf("Ok(NotificationRecipientPolicyDecision::Suppress { .. })");
const providerIndex = inbox.indexOf(".authorize_target_open(AuthorizeNotificationTargetRequest");
if (
  lookupIndex < 0 ||
  missingIndex < 0 ||
  policyIndex < 0 ||
  suppressionIndex < 0 ||
  providerIndex < 0 ||
  !(lookupIndex < missingIndex && missingIndex < policyIndex && policyIndex < suppressionIndex && suppressionIndex < providerIndex)
) {
  failures.push("ownership and recipient policy decisions must precede source authorization");
}

for (const forbidden of [
  "NotificationRecipientPolicyDecision::Suppress { reason }",
  "reason.stable_code()",
  "notification::ActiveModel",
  "delivery_attempt::",
  "seen_at: Set(",
  "read_at: Set(",
  "archived_at: Set(",
]) {
  rejectText(inbox, forbidden, `inbox privacy recheck must not expose or mutate through ${forbidden}`);
}

for (const marker of [
  "pub trait NotificationRecipientPolicy",
  "pub struct NotificationRecipientPolicyRequest",
  "pub actor_id: Option<Uuid>",
  "pub source_event_id: Uuid",
  "pub source_revision: i64",
  "pub notification_type: String",
  "pub target: NotificationTargetRef",
  "pub struct NotificationRecipientPolicyError",
]) {
  requireText(policyOwner, marker, `recipient policy owner is missing ${marker}`);
}
requireText(
  forumProvider,
  "async fn authorize_target_open(",
  "Forum source provider must remain the target authorization owner",
);

for (const marker of [
  "exact_recipient_passes_privacy_then_gets_fresh_route_without_oracle",
  "privacy_suppression_and_retryable_failure_stop_before_source_authorization",
  "stale_target_provider_failure_and_invalid_source_remain_distinct",
  "NotificationRecipientPolicyDecision::Suppress",
  "NotificationRecipientSuppression::Blocked",
  "suppressed recipient must not reach source authorization",
  "NotificationRecipientPolicyError::retryable()",
  "NOTIFICATION_RECIPIENT_POLICY_FAILURE",
  "failed recipient policy must not reach source authorization",
  "stored.state, NotificationState::Unread",
  "delivery_attempt::Entity::find()",
]) {
  requireText(proof, marker, `inbox privacy SQLite proof is missing ${marker}`);
}

for (const marker of [
  "### Inbox open-time authorization",
  "evaluates the same injected Profiles/Social Graph recipient policy",
  "Suppression returns `Unavailable` without invoking the source provider",
  "Only an allowed recipient reaches the registered source provider",
  "### Bounded authorized inbox listing",
  "seen/read/archive state APIs",
  "verify-forum-notification-inbox-open-privacy.mjs",
]) {
  requireText(docs, marker, `notifications live contract is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20R" ||
  upstream.upstream_task !== "FORUM-20Q" ||
  upstream.downstream_task !== "FORUM-20S" ||
  upstream.composition?.source_owned_authorization !== true
) {
  failures.push("FORUM-20S must remain linked to the FORUM-20R open authorization contract");
}

if (failures.length > 0) {
  console.error("Forum notification inbox open privacy verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification inbox open privacy contract is source-ready.");
