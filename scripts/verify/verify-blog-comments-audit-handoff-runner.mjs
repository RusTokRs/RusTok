#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const read = (relativePath) =>
  fs.readFileSync(path.join(root, relativePath), "utf8");

const workerPath =
  "apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_handoff_worker.rs";
const runtimePath = "apps/server/src/services/comments_provider_runtime.rs";
const bootstrapPath = "apps/server/src/services/server_bootstrap.rs";
const ownerPath =
  "apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_handoff_postgres.rs";
const planPath = "crates/rustok-blog/docs/implementation-plan-slice-91.md";
const evidencePath =
  "crates/rustok-blog/contracts/evidence/blog-comments-audit-handoff-runner.json";

for (const file of [
  workerPath,
  runtimePath,
  bootstrapPath,
  ownerPath,
  planPath,
  evidencePath,
]) {
  if (!fs.existsSync(path.join(root, file))) {
    throw new Error(`required slice-91 file is missing: ${file}`);
  }
}

const worker = read(workerPath);
const runtime = read(runtimePath);
const bootstrap = read(bootstrapPath);
const owner = read(ownerPath);
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

requireAll("worker configuration", worker, [
  "RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_ENABLED",
  "RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_CONTROL_PLANE_TENANT_ID",
  "RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_CLAIM_TTL_SECONDS",
  "RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_IDLE_POLL_MS",
  "RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_RETRY_DELAY_MS",
  "RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_MAX_CLAIMS_PER_CYCLE",
  "DEFAULT_CLAIM_TTL_SECONDS: u64 = 60",
  "DEFAULT_IDLE_POLL_MS: u64 = 1_000",
  "DEFAULT_RETRY_DELAY_MS: u64 = 1_000",
  "DEFAULT_MAX_CLAIMS_PER_CYCLE: usize = 32",
  "MAX_CLAIMS_PER_CYCLE: usize = 256",
  "parse_required_canonical_uuid",
  "parsed.is_nil() || parsed.to_string() != value",
]);

requireAll("worker lifecycle", worker, [
  "pub struct CommentsTcpDelegationScheduleAuditHandoffWorkerHandle",
  "CommentsTcpDelegationScheduleAuditHandoffWorkerLifecycleReservation",
  "pub fn start_comments_tcp_delegation_schedule_audit_handoff_worker_if_enabled",
  "runtime.runs_background_workers()",
  "shared_insert_if_absent",
  "shared_take::<CommentsTcpDelegationScheduleAuditHandoffWorkerLifecycleReservation>",
  "let stop_handle = ensure_stop_handle(runtime_ctx)",
  "let stop_rx = stop_handle.subscribe()",
  "tokio::spawn(run_handoff_worker",
]);

requireAll("bounded worker loop", worker, [
  "while outcome.calls < max_claims_per_cycle",
  "match handoff.publish_next().await",
  "CommentsTcpDelegationScheduleAuditHandoffError::Conflict",
  "CommentsTcpDelegationScheduleAuditHandoffError::Unavailable",
  "config.retry_delay",
  "config.idle_poll",
  "ACTIVE_CYCLE_DELAY",
  "tokio::select!",
  "stop_rx.changed()",
]);

const cycleStart = worker.indexOf("async fn run_handoff_cycle");
const publishNext = worker.indexOf("handoff.publish_next().await", cycleStart);
const cycleEnd = worker.indexOf("fn ensure_stop_handle", cycleStart);
if (cycleStart < 0 || publishNext < cycleStart || cycleEnd < publishNext) {
  throw new Error("bounded cycle does not call the slice-90 publish_next owner");
}

forbidAll("worker ownership", worker, [
  "OutboxRelay",
  "OutboxTransport",
  "sys_events",
  "FOR UPDATE SKIP LOCKED",
  "handoff_claim_token =",
  "dead_letter",
  "requeue",
  "attempt_exhausted",
  "exponential",
  "jitter",
]);

requireAll("runtime publication", runtime, [
  "mod keyring_schedule_audit_handoff_worker",
  "comments_provider_runtime_keyring_schedule_audit_handoff_worker.rs",
  "CommentsTcpDelegationScheduleAuditHandoffWorkerConfig",
  "CommentsTcpDelegationScheduleAuditHandoffWorkerHandle",
  "start_comments_tcp_delegation_schedule_audit_handoff_worker_if_enabled",
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
  throw new Error("handoff worker must be mounted exactly once in server bootstrap");
}

requireAll("slice-90 owner preserved", owner, [
  "pub async fn publish_next",
  "self.publish_claimed(claim).await.map(Some)",
  "FOR UPDATE SKIP LOCKED",
  ".write_once_in_transaction(&transaction, &publication)",
]);

requireAll("plan", plan, [
  "# rustok-blog implementation plan — slice 91 continuation",
  "canonical_handoff_runner_source_ready_maintainer_execution_pending",
  "shared `StopHandle`",
  "no more than the configured",
  "does not cancel a",
  "`rustok-outbox` continues to own canonical delivery",
]);

if (
  evidence.status !==
  "canonical_handoff_runner_source_ready_maintainer_execution_pending"
) {
  throw new Error(`unexpected evidence status: ${evidence.status}`);
}

const requiredTrue = [
  [evidence.configuration?.disabled_by_default, "configuration.disabled_by_default"],
  [
    evidence.configuration?.control_plane_tenant_required_when_enabled,
    "configuration.control_plane_tenant_required_when_enabled",
  ],
  [evidence.startup?.single_bootstrap_path_mounted, "startup.single_bootstrap_path_mounted"],
  [evidence.startup?.typed_lifecycle_reservation, "startup.typed_lifecycle_reservation"],
  [evidence.startup?.shared_stop_handle_used, "startup.shared_stop_handle_used"],
  [evidence.worker?.slice_90_publish_next_used, "worker.slice_90_publish_next_used"],
  [evidence.worker?.bounded_calls_per_cycle, "worker.bounded_calls_per_cycle"],
  [evidence.worker?.sleep_interruptible_by_stop_handle, "worker.sleep_interruptible_by_stop_handle"],
  [evidence.ownership?.rustok_outbox_owns_canonical_relay, "ownership.rustok_outbox_owns_canonical_relay"],
];
for (const [value, label] of requiredTrue) {
  if (value !== true) {
    throw new Error(`evidence must retain ${label}=true`);
  }
}

const requiredFalse = [
  [evidence.startup?.independent_shutdown_channel_added, "startup.independent_shutdown_channel_added"],
  [evidence.ownership?.second_relay_added, "ownership.second_relay_added"],
  [evidence.ownership?.second_canonical_claim_added, "ownership.second_canonical_claim_added"],
  [evidence.ownership?.source_dead_letter_added, "ownership.source_dead_letter_added"],
  [evidence.ownership?.operator_requeue_added, "ownership.operator_requeue_added"],
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

console.log("Blog Comments canonical audit handoff runner source guard passed.");
