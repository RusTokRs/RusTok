import fs from "node:fs";

const test = fs.readFileSync(
  "crates/rustok-moderation/tests/postgres_dispatch_contract.rs",
  "utf8",
);
const docs = fs.readFileSync(
  "crates/rustok-moderation/docs/postgres-dispatch-contract.md",
  "utf8",
);

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

for (const marker of [
  'RUSTOK_MODERATION_TEST_DATABASE_URL',
  'rustok_moderation_dispatch_',
  'TestMigrator::up(&migration, None).await?',
  'impl ModerationSubjectCommandPort for RecordingAdapter',
  'concurrent_dispatch_calls_exact_adapter_once',
  'tokio::join!',
  'ModerationSubjectKind::ForumPost',
  'ModerationSubjectKind::ForumTopic',
  'assert!(wrong.calls().is_empty())',
  'APPLICATION_ADAPTER_DEADLINE_SECONDS',
  'missing_exact_adapter_never_falls_back',
  'moderation.application_adapter_missing',
  'retryable_attempt_reuses_decision_idempotency_on_next_attempt',
  'assert_ne!(calls[0].correlation_id, calls[1].correlation_id)',
  'assert_eq!(calls[0].command, calls[1].command)',
  'adapter_errors_and_invalid_success_are_classified_fail_closed',
  'PortError::conflict',
  'PortError::validation',
  'AdapterBehavior::InvalidEvidence',
  'moderation.application_evidence_invalid',
  'make_operation_due',
]) {
  requireText(test, marker, `Moderation PostgreSQL dispatcher contract is missing ${marker}`);
}

for (const marker of [
  'multi-host CAS + exact adapter routing',
  'missing exact adapter is retryable',
  'retry identity across attempts',
  'fail-closed outcome classification',
  'not** a replacement for a domain receipt implementation',
  'cargo test -p rustok-moderation --test postgres_dispatch_contract',
]) {
  requireText(docs, marker, `Moderation PostgreSQL dispatcher handoff is missing ${marker}`);
}

console.log("Moderation PostgreSQL dispatcher contract source guard passed");
