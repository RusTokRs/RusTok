import fs from "node:fs";

const test = fs.readFileSync(
  "crates/rustok-moderation/tests/postgres_application_operation_contract.rs",
  "utf8",
);
const docs = fs.readFileSync(
  "crates/rustok-moderation/docs/postgres-application-operation-contract.md",
  "utf8",
);

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

for (const marker of [
  'RUSTOK_MODERATION_TEST_DATABASE_URL',
  'rustok_moderation_application_',
  'TestMigrator::up(&migration, None).await?',
  'due_reads_are_ordered_and_bounded',
  'MAX_DUE_APPLICATION_OPERATIONS',
  'list_due_application_operations(tenant_id, 0)',
  'concurrent_claim_has_exactly_one_winner',
  'tokio::join!',
  'application_attempt_claimed',
  'case_application_started',
  'expired_lease_reclaims_without_second_case_revision_and_fences_stale_worker',
  "lease_expires_at = NOW() - INTERVAL '1 second'",
  'assert_ne!(live_token, stale_token)',
  'reclaimed.attempt_count, 2',
  'ModerationError::ApplicationLeaseConflict',
  'after_reclaim.revision, after_first_claim.revision',
  'retryable_deadline_controls_due_visibility',
  'mark_application_retryable',
  'set_next_attempt_offset',
]) {
  requireText(test, marker, `Moderation PostgreSQL application-operation contract is missing ${marker}`);
}

for (const marker of [
  'bounded ordered due reads',
  'concurrent claim convergence',
  'expired lease reclaim + stale-token fence',
  'retryable deadline visibility',
  'cargo test -p rustok-moderation --test postgres_application_operation_contract',
]) {
  requireText(docs, marker, `Moderation PostgreSQL application-operation handoff is missing ${marker}`);
}

console.log("Moderation PostgreSQL application-operation contract source guard passed");
