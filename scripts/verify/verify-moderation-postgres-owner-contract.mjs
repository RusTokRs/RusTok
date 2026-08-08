import fs from "node:fs";

const test = fs.readFileSync(
  "crates/rustok-moderation/tests/postgres_owner_contract.rs",
  "utf8",
);
const docs = fs.readFileSync(
  "crates/rustok-moderation/docs/postgres-owner-contract.md",
  "utf8",
);

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

for (const marker of [
  'const DATABASE_ENV: &str = "RUSTOK_MODERATION_TEST_DATABASE_URL"',
  'format!("rustok_moderation_contract_{suffix}")',
  'SET search_path TO',
  'DROP SCHEMA IF EXISTS',
  'TestMigrator::up(&migration, None).await?',
  'concurrent_active_case_admission_converges',
  'decision_effect_and_pending_application_commit_together',
  'concurrent_assignment_uses_revision_cas',
  'tokio::join!',
  'active_deduplication_key IS NOT NULL',
  'moderation_case_reports',
  "status = 'attached'",
  'moderation_decision_effects',
  'moderation_application_operations',
  'decision_hash',
  'subject_revision',
  '"pending"',
  'attempt_count',
  'ModerationError::RevisionConflict',
]) {
  requireText(test, marker, `Moderation PostgreSQL owner contract is missing ${marker}`);
}

for (const marker of [
  'active-case convergence',
  'typed decision/effect/application atomicity',
  'revision CAS contention',
  'cargo test -p rustok-moderation --test postgres_owner_contract',
]) {
  requireText(docs, marker, `Moderation PostgreSQL handoff is missing ${marker}`);
}

console.log("Moderation PostgreSQL owner contract source guard passed");
