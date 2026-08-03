#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const read = (relativePath) =>
  fs.readFileSync(path.join(root, relativePath), "utf8");

const migrationPath =
  "crates/rustok-blog/src/migrations/m20260803_000011_create_blog_comments_audit_recovery.rs";
const migrationsModPath = "crates/rustok-blog/src/migrations/mod.rs";
const recoveryPath =
  "apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_recovery_postgres.rs";
const operatorPath =
  "apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_operator.rs";
const runtimePath = "apps/server/src/services/comments_provider_runtime.rs";
const bootstrapPath = "apps/server/src/services/server_bootstrap.rs";
const workerPath =
  "apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_handoff_worker_source_retry.rs";
const planPath = "crates/rustok-blog/docs/implementation-plan-slice-94.md";
const evidencePath =
  "crates/rustok-blog/contracts/evidence/blog-comments-audit-operator-requeue.json";

for (const file of [
  migrationPath,
  migrationsModPath,
  recoveryPath,
  operatorPath,
  runtimePath,
  bootstrapPath,
  workerPath,
  planPath,
  evidencePath,
]) {
  if (!fs.existsSync(path.join(root, file))) {
    throw new Error(`required slice-94 file is missing: ${file}`);
  }
}

const migration = read(migrationPath);
const migrationsMod = read(migrationsModPath);
const recovery = read(recoveryPath);
const operator = read(operatorPath);
const runtime = read(runtimePath);
const bootstrap = read(bootstrapPath);
const worker = read(workerPath);
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
  "handoff_recovery_epoch BIGINT NOT NULL DEFAULT 0",
  "handoff_recovery_epoch >= 0",
  "blog_comments_tcp_delegation_schedule_audit_recovery_audits",
  "control_plane_tenant_id UUID NOT NULL",
  "action = 'requeue'",
  "octet_length(reason) BETWEEN 1 AND 512",
  "reason !~ '[[:cntrl:]]'",
  "UNIQUE (request_id, recovery_epoch)",
  "recovery_immutable_update",
  "recovery_immutable_delete",
  "append-only",
  "intentionally irreversible",
]);

requireAll("migration registration", migrationsMod, [
  "mod m20260803_000011_create_blog_comments_audit_recovery;",
  "m20260803_000011_create_blog_comments_audit_recovery::Migration",
]);

requireAll("recovery store", recovery, [
  "pub struct PostgresCommentsTcpDelegationScheduleAuditRecoveryStore",
  "pub struct CommentsTcpDelegationScheduleAuditRecoveryRequest",
  "pub struct CommentsTcpDelegationScheduleAuditRecoveryInspection",
  "pub enum CommentsTcpDelegationScheduleAuditRecoveryOutcome",
  "pub async fn inspect_dead_letter",
  "pub async fn requeue_dead_letter",
  "FOR UPDATE",
  "handoff_attempt_count = $2",
  "handoff_recovery_epoch = $3",
  "handoff_claim_token IS NULL",
  "handoff_claim_expires_at IS NULL",
  "handoff_next_attempt_at IS NULL",
  "handoff_dead_letter_reason = 'attempt_budget_exhausted'",
  "handoff_attempt_count = 0",
  "handoff_recovery_epoch = $4",
  "handoff_last_failure_at = NULL",
  "handoff_dead_lettered_at = NULL",
  "INSERT INTO {table}",
  "control_plane_tenant_id, request_id, actor_id, action, reason",
  "transaction.commit().await",
  "self.reconcile_requeue(audit_id, &request, recovery_epoch)",
  "CommentsTcpDelegationScheduleAuditRecoveryOutcome::StaleInspection",
]);

const transactionStart = recovery.indexOf("async fn requeue_in_transaction(");
const transactionEnd = recovery.indexOf("\nfn decode_inspection", transactionStart);
const transactionBody = recovery.slice(transactionStart, transactionEnd);
const rowLock = transactionBody.indexOf("read_recovery_row_for_update_statement");
const update = transactionBody.indexOf("requeue_source_statement(request, recovery_epoch)");
const audit = transactionBody.indexOf("insert_recovery_audit_statement(");
if (rowLock < 0 || update < rowLock || audit < update) {
  throw new Error(
    "recovery transaction must lock, reset, and append the audit in order",
  );
}
const publicRequeueStart = recovery.indexOf("    pub async fn requeue_dead_letter(");
const publicRequeueEnd = recovery.indexOf("\n    async fn reconcile_requeue", publicRequeueStart);
const publicRequeueBody = recovery.slice(publicRequeueStart, publicRequeueEnd);
if (
  publicRequeueBody.indexOf("match transaction.commit().await") < 0 ||
  publicRequeueBody.indexOf("self.reconcile_requeue(audit_id, &request, recovery_epoch)") < 0
) {
  throw new Error("requeue must reconcile an ambiguous commit acknowledgement");
}

forbidAll("recovery store ownership", recovery, [
  "tokio::spawn",
  "std::thread::spawn",
  "OutboxRelay",
  "OutboxTransport",
  "sys_events",
  "axum",
  "async_graphql",
  "mcp",
]);

requireAll("operator boundary", operator, [
  "pub struct CommentsTcpDelegationScheduleAuditOperatorContext",
  "Permission::MODULES_MANAGE",
  "permissions_for(&self.tenant_id, &self.actor_id)",
  "CommentsTcpDelegationScheduleAuditOperatorError::TenantMismatch",
  "CommentsTcpDelegationScheduleAuditOperatorError::MissingRequestAuthority",
  "CommentsTcpDelegationScheduleAuditOperatorError::Forbidden",
  "pub struct CommentsTcpDelegationScheduleAuditOperatorRuntime",
  "pub async fn inspect_dead_letter",
  "pub async fn requeue_dead_letter",
  "context.authorize_for(self.control_plane_tenant_id)?",
  "CommentsTcpDelegationScheduleAuditRecoveryRequest::new(",
  "context.tenant_id",
  "context.actor_id",
  "pub fn materialize_comments_tcp_delegation_schedule_audit_operator",
  "shared_insert_if_absent(runtime)",
  "pub fn start_comments_tcp_delegation_schedule_audit_handoff_worker_with_operator_if_enabled",
  "materialize_comments_tcp_delegation_schedule_audit_operator(runtime_ctx)",
  "start_comments_tcp_delegation_schedule_audit_handoff_worker_with_source_retry_if_enabled",
]);

const inspectStart = operator.indexOf("    pub async fn inspect_dead_letter(");
const inspectEnd = operator.indexOf("    pub async fn requeue_dead_letter(", inspectStart);
const inspectBody = operator.slice(inspectStart, inspectEnd);
if (
  inspectBody.indexOf("context.authorize_for") < 0 ||
  inspectBody.indexOf("context.authorize_for") >
    inspectBody.indexOf(".inspect_dead_letter(request_id)")
) {
  throw new Error("inspection must authorize before storage delegation");
}
const requeueStart = inspectEnd;
const requeueEnd = operator.indexOf("}\n\nimpl fmt::Debug", requeueStart);
const requeueBody = operator.slice(requeueStart, requeueEnd);
if (
  requeueBody.indexOf("context.authorize_for") < 0 ||
  requeueBody.indexOf("context.authorize_for") >
    requeueBody.indexOf("CommentsTcpDelegationScheduleAuditRecoveryRequest::new")
) {
  throw new Error("requeue must authorize before request validation");
}

forbidAll("operator ownership", operator, [
  "tokio::spawn",
  "std::thread::spawn",
  "OutboxRelay",
  "OutboxTransport",
  "sys_events",
  "axum",
  "async_graphql",
  "Http",
  "GraphQL",
  "Cli",
]);

requireAll("runtime publication", runtime, [
  "comments_provider_runtime_keyring_schedule_audit_recovery_postgres.rs",
  "comments_provider_runtime_keyring_schedule_audit_operator.rs",
  "CommentsTcpDelegationScheduleAuditOperatorRuntime",
  "CommentsTcpDelegationScheduleAuditRecoveryInspection",
  "PostgresCommentsTcpDelegationScheduleAuditRecoveryStore",
  "start_comments_tcp_delegation_schedule_audit_handoff_worker_with_operator_if_enabled as start_comments_tcp_delegation_schedule_audit_handoff_worker_if_enabled",
]);

requireAll("single bootstrap mount", bootstrap, [
  "start_comments_tcp_delegation_schedule_audit_handoff_worker_if_enabled",
  "&runtime_ctx",
]);
if (
  bootstrap.split(
    "start_comments_tcp_delegation_schedule_audit_handoff_worker_if_enabled",
  ).length !== 2
) {
  throw new Error("operator-composed handoff worker must remain mounted exactly once");
}

forbidAll("existing worker remains recovery-free", worker, [
  "CommentsTcpDelegationScheduleAuditOperatorRuntime",
  "requeue_dead_letter",
  "recovery_audits",
  "handoff_recovery_epoch",
]);

requireAll("plan", plan, [
  "# rustok-blog implementation plan — slice 94 continuation",
  "source_dead_letter_operator_recovery_ready_maintainer_execution_pending",
  "Permission::MODULES_MANAGE",
  "authorization before adapter validation",
  "expected_attempt_count",
  "expected_recovery_epoch",
  "FOR UPDATE",
  "same transaction",
  "append-only",
  "`rustok-outbox` remains the sole canonical delivery owner",
]);

if (
  evidence.status !==
  "source_dead_letter_operator_recovery_ready_maintainer_execution_pending"
) {
  throw new Error(`unexpected evidence status: ${evidence.status}`);
}

const requiredTrue = [
  [evidence.migration?.irreversible, "migration.irreversible"],
  [evidence.migration?.recovery_audit_table_added, "migration.audit_table"],
  [evidence.migration?.request_epoch_unique, "migration.unique_epoch"],
  [evidence.authorization?.context_tenant_must_equal_control_plane_tenant, "authorization.tenant"],
  [evidence.authorization?.effective_modules_manage_required, "authorization.permission"],
  [evidence.authorization?.authorization_before_database_access, "authorization.order"],
  [evidence.inspection?.exact_request_id, "inspection.request"],
  [evidence.inspection?.recovery_epoch_returned, "inspection.epoch"],
  [evidence.requeue?.exact_row_for_update, "requeue.lock"],
  [evidence.requeue?.attempt_count_fence, "requeue.attempt"],
  [evidence.requeue?.recovery_epoch_fence, "requeue.epoch"],
  [evidence.requeue?.attempt_budget_exhausted_fence, "requeue.dead_letter"],
  [evidence.atomicity?.source_reset_and_audit_same_transaction, "atomicity.transaction"],
  [evidence.composition?.public_startup_name_preserved, "composition.startup"],
  [evidence.composition?.operator_installed_before_worker_start, "composition.order"],
  [evidence.ownership?.rustok_outbox_owns_canonical_relay, "ownership.relay"],
];
for (const [value, label] of requiredTrue) {
  if (value !== true) {
    throw new Error(`evidence must retain ${label}=true`);
  }
}

const requiredFalse = [
  [evidence.authorization?.caller_selected_storage_tenant_accepted, "authorization.caller_tenant"],
  [evidence.authorization?.caller_selected_storage_actor_accepted, "authorization.caller_actor"],
  [evidence.requeue?.replacement_source_row_created, "requeue.replacement_row"],
  [evidence.requeue?.replacement_canonical_event_created, "requeue.replacement_event"],
  [evidence.composition?.operator_task_spawned, "composition.operator_task"],
  [evidence.composition?.second_worker_task_added, "composition.second_worker"],
  [evidence.ownership?.second_relay_added, "ownership.second_relay"],
  [evidence.transport?.http_added, "transport.http"],
  [evidence.transport?.graphql_added, "transport.graphql"],
  [evidence.transport?.automatic_requeue_added, "transport.automatic"],
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

console.log("Blog Comments audit operator recovery source guard passed.");
