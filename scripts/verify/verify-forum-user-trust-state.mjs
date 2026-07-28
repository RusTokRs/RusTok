#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!relativePath || !existsSync(absolute)) {
    failures.push(`${relativePath || "<missing path>"}: required file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function rejectText(source, marker, message) {
  if (source.includes(marker)) failures.push(message);
}

const contractPath = "crates/rustok-forum/contracts/forum-user-trust-state.json";
const contract = JSON.parse(read(contractPath) || "{}");
const migration = read(contract.migration);
const stateEntity = read(contract.state_entity);
const revisionEntity = read(contract.revision_entity);
const service = read(contract.owner_service);
const sqliteProof = read(contract.sqlite_proof);
const note = read(contract.owner_note);
const migrationMod = read("crates/rustok-forum/src/migrations/mod.rs");
const entityMod = read("crates/rustok-forum/src/entities/mod.rs");
const serviceMod = read("crates/rustok-forum/src/services/mod.rs");
const crateRoot = read("crates/rustok-forum/src/lib.rs");

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-26A" ||
  contract.parent_task !== "FORUM-26"
) {
  failures.push("user trust contract must identify FORUM-26A under FORUM-26");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("FORUM-26A must not claim unexecuted verification evidence");
}

for (const key of [
  "forum_owned_authoritative_trust_state",
  "separate_from_forum_user_stats",
  "absent_state_defaults_to_zero",
  "revision_advances_exactly_once",
  "previous_level_matches_current_state",
  "state_must_match_revision",
  "direct_revision_update_rejected",
  "direct_revision_delete_rejected",
  "direct_state_delete_rejected",
  "tenant_user_composite_foreign_key",
  "target_user_delete_restricted_to_preserve_audit",
  "managed_get_set_history",
  "authenticated_change_actor_required",
  "manual_override_only_in_this_slice",
  "idempotency_unique_per_tenant",
  "postgres_sqlite_parity",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`user trust contract must record ${key}`);
  }
}
for (const key of [
  "backfill_required",
  "trust_facts_adapter_added",
  "posting_policy_evaluator_added",
  "topic_reply_enforcement_changed",
  "graphql_rest_transport_changed",
  "public_transport_dto_changed",
  "external_ai_spam_scoring_added",
  "shared_rate_limit_changed",
  "trust_derived_from_forum_user_stats",
  "dependency_changed",
  "host_server_source_changed",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`user trust contract must keep ${key}=false`);
  }
}
if (
  contract.composition?.trust_level_min !== 0 ||
  contract.composition?.trust_level_max !== 100 ||
  contract.composition?.history_page_max !== 100 ||
  contract.composition?.postgres_owner_and_trigger_lock_salt !== 26
) {
  failures.push("user trust bounds and advisory lock salt must remain canonical");
}

for (const marker of [
  "CREATE TABLE IF NOT EXISTS forum_user_trust_states",
  "CREATE TABLE IF NOT EXISTS forum_user_trust_revisions",
  "CHECK (trust_level BETWEEN 0 AND 100)",
  "UNIQUE (tenant_id, idempotency_key)",
  "REFERENCES users (tenant_id, id)",
  "ON UPDATE CASCADE ON DELETE RESTRICT",
  "change_kind IN ('manual_override', 'policy_evaluation', 'reconciliation', 'migration')",
  "forum user trust revisions are append-only",
  "forum user trust state cannot be deleted directly",
  "forum user trust revision must advance exactly once",
  "forum user trust previous level does not match current state",
  "forum user trust state update must match the next immutable revision",
  "hashtextextended(NEW.tenant_id::text || ':' || NEW.user_id::text || ':trust', 26)",
  "idx_forum_user_trust_revisions_history",
]) {
  requireText(migration, marker, `user trust migration is missing ${marker}`);
}
rejectText(
  migration,
  "REFERENCES forum_user_stats",
  "trust persistence must not depend on forum_user_stats",
);
rejectText(
  migration,
  "ON DELETE CASCADE",
  "trust target ownership must preserve immutable audit instead of cascading deletion",
);
rejectText(
  migration,
  "ON DELETE SET NULL",
  "composite actor SET NULL must not corrupt tenant identity",
);

for (const marker of [
  'table_name = "forum_user_trust_states"',
  "pub trust_level: i16",
  "pub revision: i64",
]) {
  requireText(stateEntity, marker, `trust state entity is missing ${marker}`);
}
for (const marker of [
  'table_name = "forum_user_trust_revisions"',
  "pub enum ForumUserTrustChangeKind",
  "ManualOverride",
  "PolicyEvaluation",
  "Reconciliation",
  "Migration",
  "pub previous_trust_level: Option<i16>",
  "pub changed_by_user_id: Option<Uuid>",
  "pub idempotency_key: String",
]) {
  requireText(revisionEntity, marker, `trust revision entity is missing ${marker}`);
}

for (const marker of [
  "pub struct ForumUserTrustService",
  "pub async fn get(",
  "pub async fn set(",
  "pub async fn history(",
  "Resource::ForumTopics, Action::Manage",
  "trust_level: 0",
  "configured: false",
  "ForumUserTrustChangeKind::ManualOverride",
  "Forum trust changes require an authenticated user actor",
  "Forum trust idempotency key was already used for another change",
  "MAX_FORUM_USER_TRUST_HISTORY_PAGE: u16 = 100",
  'format!("{tenant_id}:{user_id}:trust")',
  '"SELECT pg_advisory_xact_lock(hashtextextended($1, 26))"',
  ".limit(u64::from(limit) + 1)",
]) {
  requireText(service, marker, `user trust owner service is missing ${marker}`);
}
for (const forbidden of [
  "forum_user_stat::",
  "forum_user_stats.user_id",
  "ForumAudienceFactsPort",
  "TopicService",
  "ReplyService",
  "ModerationService",
  "reqwest",
  "openai",
]) {
  rejectText(service, forbidden, `trust owner must not derive or enforce through ${forbidden}`);
}

for (const marker of [
  "trust_state_is_authoritative_versioned_and_independent_from_activity_counters",
  "topic_count, reply_count, solution_count",
  "still_default.trust_level, 0",
  "an identical idempotent replay should succeed",
  "trust_database_guards_reject_orphans_gaps_and_direct_mutation",
  "UPDATE forum_user_trust_revisions SET trust_level = 99",
  "DELETE FROM forum_user_trust_revisions",
  "UPDATE forum_user_trust_states SET trust_level = 50, revision = 2",
  "trust_owner_requires_manage_scope_and_exact_idempotent_payload",
]) {
  requireText(sqliteProof, marker, `SQLite trust proof is missing ${marker}`);
}

for (const marker of [
  "mod m20260728_000004_add_forum_user_trust_state;",
  "Box::new(m20260728_000004_add_forum_user_trust_state::Migration)",
]) {
  requireText(migrationMod, marker, `migration registry is missing ${marker}`);
}
for (const marker of [
  "pub mod forum_user_trust_revision;",
  "pub mod forum_user_trust_state;",
  "ForumUserTrustRevisionEntity",
  "ForumUserTrustStateEntity",
]) {
  requireText(entityMod, marker, `entity registry is missing ${marker}`);
}
for (const marker of [
  "mod user_trust;",
  "ForumUserTrustService",
  "SetForumUserTrustInput",
  "MAX_FORUM_USER_TRUST_LEVEL",
]) {
  requireText(serviceMod, marker, `service registry is missing ${marker}`);
  requireText(crateRoot, marker, `crate root is missing public trust API ${marker}`);
}

for (const marker of [
  "# FORUM-26A user trust state",
  "source-ready / unvalidated",
  "Absence of a current row means trust level `0`",
  "`forum_user_stats` remains an activity-counter projection",
  "no trust facts adapter",
  "no automatic posting-policy evaluator",
  "The next bounded slice should publish a read-only trust facts adapter",
  "canonical `crates/rustok-forum/docs/implementation-plan.md` is intentionally not rewritten",
  "were not run by the implementation agent",
]) {
  requireText(note, marker, `FORUM-26A owner note is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum user trust state verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum user trust state contract is source-ready.");
