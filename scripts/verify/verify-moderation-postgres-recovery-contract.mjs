import fs from "node:fs";

const test = fs.readFileSync(
  "crates/rustok-moderation/tests/postgres_recovery_contract.rs",
  "utf8",
);
const docs = fs.readFileSync(
  "crates/rustok-moderation/docs/postgres-recovery-contract.md",
  "utf8",
);

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

for (const marker of [
  'RUSTOK_MODERATION_TEST_DATABASE_URL',
  'rustok_moderation_recovery_',
  'TestMigrator::up(&migration, None).await?',
  'claim_application_operation',
  'mark_application_rejected',
  'operator_requeue_application_replay_safe',
  'ModerationError::IdempotencyConflict',
  'attempt_count, 2',
  'mark_application_applied',
  'applied decisions must never be requeued',
  'operator_reconcile_legacy_application_replay_safe',
  "SET status = 'rejected'",
  "SET status = 'applied'",
  'applied_revision = subject_revision',
  'ModerationCaseStatus::Escalated',
  'ModerationCaseStatus::Closed',
  'active_deduplication_key IS NULL',
]) {
  requireText(test, marker, `Moderation PostgreSQL recovery contract is missing ${marker}`);
}

for (const marker of [
  'receipt replays identical operator input',
  '`applied` remains terminal',
  'legacy rejected/applied rows',
  'cargo test -p rustok-moderation --test postgres_recovery_contract',
]) {
  requireText(docs, marker, `Moderation PostgreSQL recovery handoff is missing ${marker}`);
}

console.log("Moderation PostgreSQL recovery contract source guard passed");
