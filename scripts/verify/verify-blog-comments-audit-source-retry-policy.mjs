#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const read = (relativePath) =>
  fs.readFileSync(path.join(root, relativePath), "utf8");

const migrationPath =
  "crates/rustok-blog/src/migrations/m20260803_000010_add_blog_comments_audit_source_retry_policy.rs";
const migrationsModPath = "crates/rustok-blog/src/migrations/mod.rs";
const policyPath =
  "apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_source_retry_postgres.rs";
const testSupportPath =
  "apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_handoff_postgres_test_support.rs";
const runtimePath = "apps/server/src/services/comments_provider_runtime.rs";
const workerPath =
  "apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_handoff_worker.rs";
const bootstrapPath = "apps/server/src/services/server_bootstrap.rs";
const planPath = "crates/rustok-blog/docs/implementation-plan-slice-92.md";
const evidencePath =
  "crates/rustok-blog/contracts/evidence/blog-comments-audit-source-retry-policy.json";

for (const file of [
  migrationPath,
  migrationsModPath,
  policyPath,
  testSupportPath,
  runtimePath,
  workerPath,
  bootstrapPath,
  planPath,
  evidencePath,
]) {
  if (!fs.existsSync(path.join(root, file))) {
    throw new Error(`required slice-92 file is missing: ${file}`);
  }
}

const migration = read(migrationPath);
const migrationsMod = read(migrationsModPath);
const policy = read(policyPath);
const runtime = read(runtimePath);
const worker = read(workerPath);
const bootstrap = read(bootstrapPath);
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

requireAll("migration", migration, [
  "HandoffNextAttemptAt",
  "HandoffLastFailureAt",
  "HandoffLastFailureCode",
  "HandoffDeadLetteredAt",
  "HandoffDeadLetterReason",
  "idx_blog_comments_delegation_audit_handoff_retry_ready",
  "idx_blog_comments_delegation_audit_handoff_dead_letter",
  "handoff_last_failure_code IN ('conflict', 'unavailable')",
  "handoff_dead_letter_reason = 'attempt_budget_exhausted'",
  "ck_blog_comments_delegation_audit_handoff_retry_unclaimed",
  "ck_blog_comments_delegation_audit_handoff_dead_letter_terminal",
  "ck_blog_comments_delegation_audit_handoff_published_not_retrying",
  "intentionally irreversible",
]);

requireAll("migration registration", migrationsMod, [
  "mod m20260803_000010_add_blog_comments_audit_source_retry_policy;",
  "m20260803_000010_add_blog_comments_audit_source_retry_policy::Migration",
]);

requireAll("policy owner", policy, [
  "pub struct PostgresCommentsTcpDelegationScheduleAuditSourceRetryPolicy",
  "COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_SOURCE_MAX_ATTEMPTS: u32 = 100",
  "COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_SOURCE_MAX_RETRY_DELAY_SECONDS: u64 = 86_400",
  "pub async fn record_failure",
  "pub async fn dead_letter_next_expired_exhausted",
  "pub async fn inspect_dead_letter",
  "handoff_claim_token = $2",
  "handoff_attempt_count = $3",
  "handoff_attempt_count >= $5",
  "handoff_next_attempt_at",
  "handoff_dead_lettered_at",
  "attempt_budget_exhausted",
  "FOR UPDATE SKIP LOCKED",
  "ORDER BY created_at ASC, request_id ASC",
  "LIMIT 1",
  "CommentsTcpDelegationScheduleAuditSourceFailureTransition::StaleClaim",
]);

forbidAll("policy owner", policy, [
  "tokio::spawn",
  "std::thread::spawn",
  "tokio::time::sleep",
  "OutboxRelay",
  "OutboxTransport",
  "sys_events SET",
  "operator_requeue",
  "reset_attempt",
  "GraphQL",
  "axum",
]);

requireAll("runtime publication", runtime, [
  "mod keyring_schedule_audit_source_retry_postgres",
  "comments_provider_runtime_keyring_schedule_audit_source_retry_postgres.rs",
  "PostgresCommentsTcpDelegationScheduleAuditSourceRetryPolicy",
  "CommentsTcpDelegationScheduleAuditSourceFailureTransition",
  "CommentsTcpDelegationScheduleAuditSourceDeadLetterInspection",
]);

forbidAll("runner remains uncomposed", worker, [
  "PostgresCommentsTcpDelegationScheduleAuditSourceRetryPolicy",
  "record_failure(",
  "dead_letter_next_expired_exhausted",
  "SOURCE_MAX_ATTEMPTS",
  "SOURCE_RETRY_DELAY_SECONDS",
]);
forbidAll("bootstrap remains uncomposed", bootstrap, [
  "PostgresCommentsTcpDelegationScheduleAuditSourceRetryPolicy",
  "dead_letter_next_expired_exhausted",
]);

requireAll("plan", plan, [
  "# rustok-blog implementation plan — slice 92 continuation",
  "source_retry_policy_ready_runner_composition_pending",
  "exact source claim",
  "FOR UPDATE SKIP LOCKED",
  "does not construct the owner in bootstrap",
  "claim selection honors `handoff_next_attempt_at`",
  "`rustok-outbox` remains the sole canonical delivery owner",
]);

if (evidence.status !== "source_retry_policy_ready_runner_composition_pending") {
  throw new Error(`unexpected evidence status: ${evidence.status}`);
}

const requiredTrue = [
  [evidence.migration?.irreversible, "migration.irreversible"],
  [evidence.migration?.next_attempt_at_added, "migration.next_attempt_at_added"],
  [evidence.migration?.dead_letter_pair_added, "migration.dead_letter_pair_added"],
  [evidence.policy?.record_failure_exact_request_fence, "policy.request_fence"],
  [evidence.policy?.record_failure_exact_claim_token_fence, "policy.claim_fence"],
  [evidence.policy?.record_failure_exact_attempt_fence, "policy.attempt_fence"],
  [evidence.policy?.retry_timestamp_durable, "policy.retry_timestamp_durable"],
  [evidence.policy?.exhaustion_dead_letter_durable, "policy.dead_letter_durable"],
  [evidence.crash_gap?.one_row_per_call, "crash_gap.one_row_per_call"],
  [evidence.crash_gap?.for_update_skip_locked, "crash_gap.skip_locked"],
  [evidence.inspection?.exact_request_id, "inspection.exact_request_id"],
  [evidence.ownership?.rustok_outbox_owns_canonical_relay, "ownership.relay"],
];
for (const [value, label] of requiredTrue) {
  if (value !== true) {
    throw new Error(`evidence must retain ${label}=true`);
  }
}

const requiredFalse = [
  [evidence.crash_gap?.task_spawned, "crash_gap.task_spawned"],
  [evidence.crash_gap?.polling_loop_added, "crash_gap.polling_loop_added"],
  [evidence.inspection?.transport_exposed, "inspection.transport_exposed"],
  [evidence.inspection?.authorization_boundary_added, "inspection.authorization_boundary_added"],
  [evidence.inspection?.operator_requeue_added, "inspection.operator_requeue_added"],
  [evidence.composition?.bootstrap_constructed, "composition.bootstrap_constructed"],
  [evidence.composition?.slice_91_runner_modified, "composition.slice_91_runner_modified"],
  [evidence.composition?.claim_sql_honors_retry_timestamp, "composition.claim_retry"],
  [evidence.composition?.claim_sql_excludes_dead_letter, "composition.claim_dead_letter"],
  [evidence.composition?.runner_records_failures, "composition.runner_records_failures"],
  [evidence.ownership?.second_relay_added, "ownership.second_relay_added"],
  [evidence.validation?.cargo_check_run, "validation.cargo_check_run"],
  [evidence.validation?.rust_unit_tests_run, "validation.rust_unit_tests_run"],
  [evidence.validation?.javascript_verifier_run, "validation.javascript_verifier_run"],
  [evidence.validation?.postgresql_run, "validation.postgresql_run"],
];
for (const [value, label] of requiredFalse) {
  if (value !== false) {
    throw new Error(`evidence must retain ${label}=false`);
  }
}

if (!evidenceText.endsWith("\n")) {
  throw new Error("evidence JSON must end with a newline");
}

console.log("Blog Comments source retry policy source guard passed.");
