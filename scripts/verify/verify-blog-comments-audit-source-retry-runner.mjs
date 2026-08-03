#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const read = (relativePath) =>
  fs.readFileSync(path.join(root, relativePath), "utf8");

const runtimePath = "apps/server/src/services/comments_provider_runtime.rs";
const bootstrapPath = "apps/server/src/services/server_bootstrap.rs";
const legacyWorkerPath =
  "apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_handoff_worker.rs";
const workerPath =
  "apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_handoff_worker_source_retry.rs";
const legacyHandoffPath =
  "apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_handoff_postgres.rs";
const retryClaimPath =
  "apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_handoff_retry_ready.rs";
const legacyPolicyPath =
  "apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_source_retry_postgres.rs";
const activePolicyPath =
  "apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_source_retry_active.rs";
const planPath = "crates/rustok-blog/docs/implementation-plan-slice-93.md";
const evidencePath =
  "crates/rustok-blog/contracts/evidence/blog-comments-audit-source-retry-runner.json";

for (const file of [
  runtimePath,
  bootstrapPath,
  legacyWorkerPath,
  workerPath,
  legacyHandoffPath,
  retryClaimPath,
  legacyPolicyPath,
  activePolicyPath,
  planPath,
  evidencePath,
]) {
  if (!fs.existsSync(path.join(root, file))) {
    throw new Error(`required slice-93 file is missing: ${file}`);
  }
}

const runtime = read(runtimePath);
const bootstrap = read(bootstrapPath);
const legacyWorker = read(legacyWorkerPath);
const worker = read(workerPath);
const legacyHandoff = read(legacyHandoffPath);
const retryClaim = read(retryClaimPath);
const legacyPolicy = read(legacyPolicyPath);
const activePolicy = read(activePolicyPath);
const plan = read(planPath);
const evidenceText = read(evidencePath);
const evidence = JSON.parse(evidenceText);

function requireAll(label, text, markers) {
  for (const marker of markers) {
    if (!text.includes(marker)) {
      throw new Error(`${label} is missing required marker: ${marker}`);
    }
  }
}

function forbidAll(label, text, markers) {
  for (const marker of markers) {
    if (text.includes(marker)) {
      throw new Error(`${label} contains forbidden marker: ${marker}`);
    }
  }
}

requireAll("runtime composition", runtime, [
  "comments_provider_runtime_keyring_schedule_audit_handoff_retry_ready.rs",
  "comments_provider_runtime_keyring_schedule_audit_handoff_worker_source_retry.rs",
  "comments_provider_runtime_keyring_schedule_audit_source_retry_active.rs",
  "COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_SOURCE_MAX_ATTEMPTS_ENV",
  "COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_SOURCE_RETRY_DELAY_SECONDS_ENV",
  "CommentsTcpDelegationScheduleAuditSourceRetryWorkerConfig",
  "start_comments_tcp_delegation_schedule_audit_handoff_worker_with_source_retry_if_enabled as start_comments_tcp_delegation_schedule_audit_handoff_worker_if_enabled",
]);

requireAll("single bootstrap mount", bootstrap, [
  "#[cfg(feature = \"mod-comments\")]",
  "start_comments_tcp_delegation_schedule_audit_handoff_worker_if_enabled",
  "&runtime_ctx",
]);
if (
  bootstrap.split(
    "start_comments_tcp_delegation_schedule_audit_handoff_worker_if_enabled",
  ).length !== 2
) {
  throw new Error("source retry handoff worker must remain mounted exactly once");
}

requireAll("legacy compatibility surfaces", legacyWorker, [
  "pub fn start_comments_tcp_delegation_schedule_audit_handoff_worker_if_enabled",
  "CommentsTcpDelegationScheduleAuditHandoffWorkerLifecycleReservation",
  "CommentsTcpDelegationScheduleAuditHandoffWorkerHandle",
  "pub async fn publish_next",
]);
requireAll("legacy handoff compatibility", legacyHandoff, [
  "pub async fn claim_next",
  "pub async fn publish_claimed",
  "pub async fn publish_next",
  ".write_once_in_transaction(&transaction, &publication)",
  "canonical_envelope_id = request_id",
]);
requireAll("legacy source policy compatibility", legacyPolicy, [
  "pub struct PostgresCommentsTcpDelegationScheduleAuditSourceRetryPolicy",
  "pub async fn record_failure",
  "pub async fn dead_letter_next_expired_exhausted",
  "pub async fn inspect_dead_letter",
]);

requireAll("strict source retry configuration", worker, [
  "RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_SOURCE_MAX_ATTEMPTS",
  "RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_SOURCE_RETRY_DELAY_SECONDS",
  "DEFAULT_SOURCE_MAX_ATTEMPTS: u64 = 8",
  "DEFAULT_SOURCE_RETRY_DELAY_SECONDS: u64 = 30",
  "COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_SOURCE_MAX_ATTEMPTS",
  "COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_SOURCE_MAX_RETRY_DELAY_SECONDS",
  "parse_optional_bounded_u64",
]);

requireAll("single-lane source retry startup", worker, [
  "pub fn start_comments_tcp_delegation_schedule_audit_handoff_worker_with_source_retry_if_enabled",
  "CommentsTcpDelegationScheduleAuditHandoffWorkerLifecycleReservation",
  "shared_insert_if_absent",
  "shared_take::<CommentsTcpDelegationScheduleAuditHandoffWorkerLifecycleReservation>",
  "let stop_handle = ensure_stop_handle(runtime_ctx)",
  "let stop_rx = stop_handle.subscribe()",
  "tokio::spawn(run_source_retry_handoff_worker",
  "CommentsTcpDelegationScheduleAuditHandoffWorkerHandle",
]);

requireAll("bounded source retry cycle", worker, [
  "dead_letter_next_expired_exhausted().await",
  "while outcome.calls < max_claims_per_cycle",
  ".claim_next_retry_ready(source_max_attempts)",
  "handoff.publish_claimed(claim).await",
  "retry_policy.record_active_failure(claim, error).await",
  "CommentsTcpDelegationScheduleAuditSourceFailureTransition::RetryScheduled",
  "CommentsTcpDelegationScheduleAuditSourceFailureTransition::DeadLettered",
  "CommentsTcpDelegationScheduleAuditSourceFailureTransition::StaleClaim",
  "CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::InvalidStoredState",
  "CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::Unavailable",
  "tokio::select!",
  "stop_rx.changed()",
]);

const sweepCall = worker.indexOf("dead_letter_next_expired_exhausted().await");
const boundedLoop = worker.indexOf("while outcome.calls < max_claims_per_cycle");
const claimCall = worker.indexOf(".claim_next_retry_ready(source_max_attempts)");
const publishCall = worker.indexOf("handoff.publish_claimed(claim).await");
const failureCall = worker.indexOf("retry_policy.record_active_failure(claim, error).await");
if (
  sweepCall < 0 ||
  boundedLoop < sweepCall ||
  claimCall < boundedLoop ||
  publishCall < claimCall ||
  failureCall < publishCall
) {
  throw new Error(
    "source retry cycle must sweep once, then claim, publish, and record active failures in order",
  );
}

forbidAll("source retry worker ownership", worker, [
  "OutboxRelay",
  "OutboxTransport",
  "sys_events",
  "operator_requeue",
  "reset_attempt",
  "axum",
  "async_graphql",
  "std::thread::spawn",
]);
forbidAll("source retry composed loop", worker, [
  "handoff.publish_next().await",
]);

requireAll("retry-aware claim", retryClaim, [
  "pub async fn claim_next_retry_ready",
  "handoff_dead_lettered_at IS NULL",
  "handoff_attempt_count < $3",
  "handoff_next_attempt_at IS NULL OR handoff_next_attempt_at <= NOW()",
  "handoff_claim_token IS NULL OR handoff_claim_expires_at <= NOW()",
  "ORDER BY created_at ASC, request_id ASC",
  "FOR UPDATE SKIP LOCKED",
  "LIMIT 1",
  "handoff_next_attempt_at = NULL",
  "handoff_attempt_count = handoff_attempt_count + 1",
  "self.reconcile_claim(claim_token).await",
]);
forbidAll("retry-aware claim ownership", retryClaim, [
  "tokio::spawn",
  "OutboxRelay",
  "sys_events",
  "dead_letter_next_expired_exhausted",
]);

requireAll("active failure fence", activePolicy, [
  "pub async fn record_active_failure",
  "handoff_claim_token = $2",
  "handoff_attempt_count = $3",
  "handoff_claim_expires_at > NOW()",
  "handoff_next_attempt_at",
  "handoff_dead_lettered_at",
  "attempt_budget_exhausted",
  "CommentsTcpDelegationScheduleAuditSourceFailureTransition::StaleClaim",
]);
forbidAll("active failure ownership", activePolicy, [
  "tokio::spawn",
  "OutboxRelay",
  "sys_events",
  "operator_requeue",
  "reset_attempt",
]);

requireAll("plan", plan, [
  "# rustok-blog implementation plan — slice 93 continuation",
  "source_retry_runner_composed_maintainer_execution_pending",
  "claim_next_retry_ready(source_max_attempts)",
  "record_active_failure(claim, error)",
  "handoff_claim_expires_at > NOW()",
  "dead_letter_next_expired_exhausted()",
  "there is still exactly one lifecycle reservation",
  "`rustok-outbox` remains the sole canonical delivery owner",
]);

if (
  evidence.status !==
  "source_retry_runner_composed_maintainer_execution_pending"
) {
  throw new Error(`unexpected evidence status: ${evidence.status}`);
}

const requiredTrue = [
  [evidence.configuration?.enabled_by_existing_handoff_flag, "configuration.enable"],
  [evidence.configuration?.durable_retry_delay_distinct_from_loop_retry_delay, "configuration.delay_distinction"],
  [evidence.composition?.public_startup_name_preserved, "composition.startup_name"],
  [evidence.composition?.source_retry_implementation_aliased_to_existing_startup_name, "composition.alias"],
  [evidence.composition?.single_bootstrap_mount, "composition.single_mount"],
  [evidence.composition?.single_worker_task, "composition.single_task"],
  [evidence.composition?.shared_stop_handle_preserved, "composition.stop_handle"],
  [evidence.claim?.source_dead_letters_excluded, "claim.dead_letter"],
  [evidence.claim?.attempt_budget_enforced_before_increment, "claim.budget"],
  [evidence.claim?.future_retry_timestamp_excluded, "claim.retry_time"],
  [evidence.claim?.retry_timestamp_cleared_with_claim, "claim.clear_retry"],
  [evidence.failure_transition?.active_expiry_fence_repeated, "failure.expiry"],
  [evidence.failure_transition?.stale_claim_is_closed_noop, "failure.stale"],
  [evidence.crash_gap?.called_before_claim_batch, "crash_gap.order"],
  [evidence.crash_gap?.calls_per_cycle_max === 1, "crash_gap.bound"],
  [evidence.cycle?.explicit_claim_then_publish, "cycle.explicit_claim"],
  [evidence.ownership?.rustok_outbox_owns_canonical_relay, "ownership.relay"],
];
for (const [value, label] of requiredTrue) {
  if (value !== true) {
    throw new Error(`evidence must retain ${label}=true`);
  }
}

const requiredFalse = [
  [evidence.composition?.canonical_bootstrap_call_changed, "composition.bootstrap_changed"],
  [evidence.composition?.second_worker_task_added, "composition.second_task"],
  [evidence.composition?.independent_shutdown_channel_added, "composition.shutdown_channel"],
  [evidence.failure_transition?.runner_uses_compatibility_record_failure_api, "failure.legacy_api"],
  [evidence.crash_gap?.unbounded_drain_loop_added, "crash_gap.unbounded"],
  [evidence.cycle?.publish_next_used_by_composed_runner, "cycle.publish_next"],
  [evidence.ownership?.second_relay_added, "ownership.second_relay"],
  [evidence.operator_boundary?.authorized_requeue_added, "operator.requeue"],
  [evidence.validation?.cargo_check_run, "validation.cargo_check"],
  [evidence.validation?.rust_unit_tests_run, "validation.rust_tests"],
  [evidence.validation?.javascript_verifier_run, "validation.verifier"],
  [evidence.validation?.postgresql_run, "validation.postgresql"],
];
for (const [value, label] of requiredFalse) {
  if (value !== false) {
    throw new Error(`evidence must retain ${label}=false`);
  }
}

if (!evidenceText.endsWith("\n")) {
  throw new Error("evidence JSON must end with a newline");
}

console.log("Blog Comments source retry runner source guard passed.");
