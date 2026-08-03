#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const read = (relativePath) =>
  fs.readFileSync(path.join(root, relativePath), "utf8");

const migrationPath =
  "crates/rustok-blog/src/migrations/m20260803_000009_add_blog_comments_audit_canonical_handoff.rs";
const migrationsModPath = "crates/rustok-blog/src/migrations/mod.rs";
const servicePath =
  "apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_handoff_postgres.rs";
const runtimePath = "apps/server/src/services/comments_provider_runtime.rs";
const planPath = "crates/rustok-blog/docs/implementation-plan-slice-90.md";
const evidencePath =
  "crates/rustok-blog/contracts/evidence/blog-comments-audit-canonical-handoff-postgres.json";

for (const file of [
  migrationPath,
  migrationsModPath,
  servicePath,
  runtimePath,
  planPath,
  evidencePath,
]) {
  if (!fs.existsSync(path.join(root, file))) {
    throw new Error(`required slice-90 file is missing: ${file}`);
  }
}

const migration = read(migrationPath);
const migrationsMod = read(migrationsModPath);
const service = read(servicePath);
const runtime = read(runtimePath);
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
  "CanonicalEnvelopeId",
  "HandoffClaimToken",
  "HandoffClaimExpiresAt",
  "HandoffAttemptCount",
  "uq_blog_comments_delegation_audit_canonical_envelope",
  "uq_blog_comments_delegation_audit_handoff_claim_token",
  "idx_blog_comments_delegation_audit_handoff_pending",
  "handoff_attempt_count >= 0",
  "handoff_claim_token IS NULL) = (handoff_claim_expires_at IS NULL",
  "canonical_envelope_id = request_id AND published_at IS NOT NULL",
  "legacy Comments schedule audit rows already use published_at without canonical identity",
  "intentionally irreversible",
]);

requireAll("migration registration", migrationsMod, [
  "mod m20260803_000009_add_blog_comments_audit_canonical_handoff;",
  "m20260803_000009_add_blog_comments_audit_canonical_handoff::Migration",
]);

requireAll("handoff owner", service, [
  "pub struct PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff",
  "COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_MAX_CLAIM_SECONDS: u64 = 300",
  "pub async fn claim_next",
  "pub async fn publish_claimed",
  "pub async fn publish_next",
  "FOR UPDATE SKIP LOCKED",
  "handoff_claim_expires_at <= NOW()",
  "handoff_attempt_count = handoff_attempt_count + 1",
  "FOR UPDATE\"",
  ".write_once_in_transaction(&transaction, &publication)",
  "canonical_envelope_id = request_id",
  "handoff_claim_token = NULL",
  "handoff_attempt_count = $3",
  "handoff_claim_expires_at > NOW()",
  "self.reconcile_claim(claim_token).await",
  "self.reconcile_publication(claim.request_id).await",
  "canonical_envelope_id == Some(request_id)",
]);

const writerCall = service.indexOf(
  ".write_once_in_transaction(&transaction, &publication)",
);
const sourceTerminalUpdate = service.indexOf(
  "let updated = transaction\n            .execute(mark_published_statement(claim))",
);
const commit = service.indexOf("match transaction.commit().await", writerCall);
if (writerCall < 0 || sourceTerminalUpdate < writerCall || commit < sourceTerminalUpdate) {
  throw new Error(
    "canonical writer, fenced source update, and commit are not ordered in one transaction",
  );
}

forbidAll("handoff owner", service, [
  "tokio::spawn",
  "std::thread::spawn",
  "OutboxRelay",
  "OutboxTransport::new",
  "start_worker",
  "consumer_group",
  "dead_letter",
]);

requireAll("runtime publication", runtime, [
  "mod keyring_schedule_audit_handoff_postgres",
  "comments_provider_runtime_keyring_schedule_audit_handoff_postgres.rs",
  "PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff",
  "CommentsTcpDelegationScheduleAuditHandoffClaim",
  "CommentsTcpDelegationScheduleAuditHandoffError",
]);

forbidAll("runtime composition", runtime, [
  "PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff::new(",
  "publish_next().await",
  "start_comments_schedule_audit_handoff",
]);

requireAll("plan", plan, [
  "# rustok-blog implementation plan — slice 90 continuation",
  "canonical_handoff_source_ready_maintainer_execution_pending",
  "FOR UPDATE SKIP LOCKED",
  "same PostgreSQL transaction",
  "does not register the owner in runtime extensions",
  "rustok-outbox already owns canonical relay",
]);

if (
  evidence.status !==
  "canonical_handoff_source_ready_maintainer_execution_pending"
) {
  throw new Error(`unexpected evidence status: ${evidence.status}`);
}

const requiredEvidence = [
  [evidence.migration?.irreversible, "migration.irreversible"],
  [evidence.migration?.claim_token_unique, "migration.claim_token_unique"],
  [evidence.claim?.for_update_skip_locked, "claim.for_update_skip_locked"],
  [
    evidence.claim?.expired_claim_recovery,
    "claim.expired_claim_recovery",
  ],
  [
    evidence.publication?.canonical_writer_called_in_same_transaction,
    "publication.canonical_writer_called_in_same_transaction",
  ],
  [
    evidence.publication?.source_terminal_update_in_same_transaction,
    "publication.source_terminal_update_in_same_transaction",
  ],
  [
    evidence.publication?.terminal_update_repeats_claim_fence,
    "publication.terminal_update_repeats_claim_fence",
  ],
  [evidence.ownership?.rustok_outbox_owns_canonical_relay, "ownership.relay"],
];
for (const [value, label] of requiredEvidence) {
  if (value !== true) {
    throw new Error(`evidence must retain ${label}=true`);
  }
}

const requiredFalseEvidence = [
  [evidence.ownership?.second_relay_added, "ownership.second_relay_added"],
  [evidence.runtime_boundary?.worker_spawned, "runtime.worker_spawned"],
  [evidence.runtime_boundary?.polling_loop_added, "runtime.polling_loop_added"],
  [evidence.runtime_boundary?.heartbeat_added, "runtime.heartbeat_added"],
  [evidence.validation?.cargo_check_run, "validation.cargo_check_run"],
  [evidence.validation?.rust_unit_tests_run, "validation.rust_unit_tests_run"],
  [
    evidence.validation?.javascript_verifier_run,
    "validation.javascript_verifier_run",
  ],
  [evidence.validation?.postgresql_run, "validation.postgresql_run"],
];
for (const [value, label] of requiredFalseEvidence) {
  if (value !== false) {
    throw new Error(`evidence must retain ${label}=false`);
  }
}

if (!evidenceText.endsWith("\n")) {
  throw new Error("evidence JSON must end with a newline");
}

console.log(
  "Blog Comments PostgreSQL canonical audit handoff source guard passed.",
);
