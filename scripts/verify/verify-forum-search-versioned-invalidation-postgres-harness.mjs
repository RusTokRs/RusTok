#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : process.cwd();
const failures = [];

const paths = {
  contract: "crates/rustok-forum/contracts/evidence/forum-search-versioned-invalidation-postgres-harness.json",
  note: "crates/rustok-forum/docs/forum-23b2g2b3d1-versioned-invalidation-postgres-evidence.md",
  test: "crates/rustok-search/tests/forum_contract_ingress_postgres_test.rs",
  ingress: "crates/rustok-search/src/forum_contract_ingress.rs",
  migrations: "crates/rustok-search/src/migrations/mod.rs",
  protocolContract: "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json",
  protocolNote: "crates/rustok-forum/docs/forum-23b2g2b3d-runtime-evidence.md",
  forumPlan: "crates/rustok-forum/docs/implementation-plan.md",
};

function target(relativePath) {
  return path.join(root, relativePath);
}

function read(relativePath) {
  if (!fs.existsSync(target(relativePath))) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return fs.readFileSync(target(relativePath), "utf8");
}

function readJson(relativePath) {
  try {
    return JSON.parse(read(relativePath));
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return null;
  }
}

function requireAll(source, markers, label) {
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
  }
}

function rejectAll(source, markers, label) {
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
  }
}

const contract = readJson(paths.contract);
const protocolContract = readJson(paths.protocolContract);
const protocolNote = read(paths.protocolNote);
const note = read(paths.note);
const test = read(paths.test);
const ingress = read(paths.ingress);
const migrations = read(paths.migrations);
const forumPlan = read(paths.forumPlan);

if (protocolContract?.task !== "FORUM-23B2G2B3D0"
    || protocolContract?.status !== "source_ready_maintainer_execution_pending"
    || protocolContract?.evidence_artifact?.generation !== "executable_runtime_only") {
  failures.push(`${paths.protocolContract}: predecessor protocol drift`);
}

if (contract) {
  if (contract.task !== "FORUM-23B2G2B3D1"
      || contract.status !== "source_ready_maintainer_execution_pending"
      || contract.predecessor !== "FORUM-23B2G2B3D0") {
    failures.push(`${paths.contract}: task, status or predecessor drift`);
  }
  if (contract.database?.backend !== "postgresql"
      || contract.database?.environment !== "RUSTOK_SEARCH_TEST_DATABASE_URL"
      || contract.database?.isolated_schema_per_test !== true
      || contract.database?.real_search_module_migrations !== true
      || contract.database?.sleep_or_polling !== false) {
    failures.push(`${paths.contract}: database evidence boundary drift`);
  }
  if (contract.scenarios?.legacy_first_then_typed?.single_root_row !== true
      || contract.scenarios?.typed_first_then_legacy?.single_root_row !== true
      || contract.scenarios?.identity_conflict?.stable_error_code
        !== "forum.search_projection.contract_inbox_identity_conflict"
      || contract.scenarios?.identity_conflict?.retryable !== false) {
    failures.push(`${paths.contract}: scenario contract drift`);
  }
  if (contract.single_execution_path?.table !== "search_projection_inbox"
      || contract.single_execution_path?.second_inbox !== false
      || contract.single_execution_path?.second_projector !== false
      || contract.single_execution_path?.typed_identity
        !== "ContractEventEnvelope.causation_id") {
    failures.push(`${paths.contract}: single execution path drift`);
  }
  if (contract.not_claimed?.test_executed !== false
      || contract.not_claimed?.postgresql_output_captured !== false
      || contract.not_claimed?.iggy_cursor_restart !== false
      || contract.not_claimed?.raw_poison_dlq !== false
      || contract.not_claimed?.semantic_poison_dlq !== false
      || contract.not_claimed?.dlq_duplicate_suppression !== false
      || contract.not_claimed?.owner_checkpoint_repair !== false
      || contract.not_claimed?.link_forum_03_closed !== false) {
    failures.push(`${paths.contract}: execution claims drift`);
  }
  if (contract.remaining_task !== "FORUM-23B2G2B3D") {
    failures.push(`${paths.contract}: remaining task drift`);
  }
}

requireAll(test, [
  "RUSTOK_SEARCH_TEST_DATABASE_URL",
  "for migration in SearchModule.migrations()",
  "legacy_first_then_typed_restart_reuses_one_exact_root_row",
  "typed_first_then_legacy_delivery_keeps_search_owned_sequence",
  "conflicting_legacy_identity_is_non_retryable_semantic_poison",
  "ContractEventEnvelope::new_caused_by",
  "ForumSearchContractIngress::new",
  "ON CONFLICT (event_id) DO NOTHING",
  "search_projection_inbox",
  "assert_ne!(typed_first.ingest_sequence, owner_revision)",
  "ForumSearchContractIngressError::InboxIdentityConflict",
  "forum.search_projection.contract_inbox_identity_conflict",
  "DROP SCHEMA IF EXISTS",
], paths.test);
rejectAll(test, [
  "tokio::time::sleep",
  "std::thread::sleep",
  "iggy",
  "owner_revision > ingest_sequence",
  "owner_revision < ingest_sequence",
], paths.test);

requireAll(ingress, [
  "ContractEventEnvelope.causation_id",
  "ON CONFLICT (event_id) DO NOTHING",
  "verify_durable_root",
  "InboxIdentityConflict",
], paths.ingress);
requireAll(migrations, [
  "m20260730_000009_create_search_projection_inbox",
  "m20260731_000010_add_forum_projection_ingest_sequence",
], paths.migrations);

requireAll(protocolNote, [
  "# FORUM-23B2G2B3D0 versioned invalidation runtime evidence protocol",
  "source_ready_maintainer_execution_pending",
  "legacy-first and typed-first duplicate races",
  "acknowledgement failure followed by restart",
  "FORUM-23B2G2B3D",
], paths.protocolNote);

requireAll(note, [
  "# FORUM-23B2G2B3D1 versioned Search invalidation PostgreSQL evidence harness",
  "source_ready_maintainer_execution_pending",
  "legacy-root delivery before the caused typed invalidation",
  "typed invalidation before legacy-root delivery",
  "FORUM-23B2G2B3D",
  "These commands were not run by the implementation agent.",
], paths.note);
requireAll(forumPlan, [
  "| `FORUM-23` | `in_progress` |",
  "owner-issued revision reconciliation plus maintainer runtime evidence remain",
  "LINK-FORUM-03",
], paths.forumPlan);

if (failures.length > 0) {
  console.error("Forum Search versioned invalidation PostgreSQL harness verification failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum Search versioned invalidation PostgreSQL harness source contract verified.");
