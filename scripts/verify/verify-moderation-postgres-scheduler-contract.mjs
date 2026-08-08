import fs from "node:fs";

const test = fs.readFileSync(
  "crates/rustok-moderation/tests/postgres_scheduler_contract.rs",
  "utf8",
);
const docs = fs.readFileSync(
  "crates/rustok-moderation/docs/postgres-scheduler-contract.md",
  "utf8",
);
const scheduler = fs.readFileSync(
  "crates/rustok-moderation/src/application_scheduler.rs",
  "utf8",
);

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

for (const marker of [
  'RUSTOK_MODERATION_TEST_DATABASE_URL',
  'rustok_moderation_scheduler_',
  'TestMigrator::up(&migration, None).await?',
  'ModerationModule.register_runtime_extensions(&mut extensions)',
  'ModuleWorkRegistrations',
  'registrations.register_all(&host, &scheduler).await?',
  'two_schedulers_converge_on_one_domain_call',
  'tokio::join!(first.run_once(), second.run_once())',
  'assert_eq!(adapter.call_count(), 1)',
  'application_attempt_claimed',
  'case_application_started',
  'case_closed',
  'stop_signal_prevents_new_moderation_claim',
  '.run_until_stopped(stop_rx, Duration::from_millis(1))',
  'ModerationApplicationOperationStatus::Pending',
  'expired_claim_is_recovered_by_scheduler_without_duplicate_start_transition',
  'claim_application_operation',
  "lease_expires_at = NOW() - INTERVAL '1 second'",
  'operation.attempt_count, 2',
  'closed.revision, after_first_claim.revision + 1',
]) {
  requireText(test, marker, `Moderation PostgreSQL scheduler contract is missing ${marker}`);
}

for (const marker of [
  'Candidate discovery is intentionally read-only',
  'dispatch_application_operation_once',
  'run_until_stopped',
  'A stop prevents future claims',
]) {
  requireText(scheduler, marker, `Moderation scheduler source invariant is missing ${marker}`);
}

for (const marker of [
  'Multi-host convergence',
  'Graceful stop',
  'Crash / lease recovery',
  'read-only generic discovery does not become an alternative lease authority',
  'cargo test -p rustok-moderation --test postgres_scheduler_contract',
]) {
  requireText(docs, marker, `Moderation PostgreSQL scheduler handoff is missing ${marker}`);
}

console.log("Moderation PostgreSQL scheduler runtime contract source guard passed");
