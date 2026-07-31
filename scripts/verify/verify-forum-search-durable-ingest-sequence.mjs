#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];
const paths = {
  contract:
    "crates/rustok-forum/contracts/forum-search-durable-ingest-sequence.json",
  note:
    "crates/rustok-forum/docs/forum-23b2g1-search-durable-ingest-sequence.md",
  forumPlan: "crates/rustok-forum/docs/implementation-plan.md",
  searchPlan: "crates/rustok-search/docs/implementation-plan.md",
  migration:
    "crates/rustok-search/src/migrations/m20260731_000010_add_forum_projection_ingest_sequence.rs",
  migrationRegistry: "crates/rustok-search/src/migrations/mod.rs",
  inbox: "crates/rustok-search/src/forum_inbox.rs",
  reconciliation: "crates/rustok-search/src/forum_reconciliation.rs",
};

function read(relativePath) {
  const target = path.join(root, relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target, "utf8");
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

function parseJson(relativePath) {
  try {
    return JSON.parse(read(relativePath));
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return null;
  }
}

const contract = parseJson(paths.contract);
const note = read(paths.note);
const forumPlan = read(paths.forumPlan);
const searchPlan = read(paths.searchPlan);
const migration = read(paths.migration);
const migrationRegistry = read(paths.migrationRegistry);
const inbox = read(paths.inbox);
const reconciliation = read(paths.reconciliation);

requireAll(
  migration,
  [
    "search_projection_inbox_ingest_sequence_seq",
    "ADD COLUMN IF NOT EXISTS ingest_sequence BIGINT",
    "ORDER BY created_at ASC, revision_at ASC, event_id ASC",
    "SET DEFAULT nextval",
    "ALTER COLUMN ingest_sequence SET NOT NULL",
    "ux_search_projection_inbox_ingest_sequence",
    "ADD COLUMN IF NOT EXISTS ingest_sequence BIGINT NOT NULL DEFAULT 0",
    "DatabaseBackend::Sqlite => Ok(())",
  ],
  paths.migration,
);
requireAll(
  migrationRegistry,
  [
    "mod m20260731_000010_add_forum_projection_ingest_sequence;",
    "m20260731_000010_add_forum_projection_ingest_sequence::Migration",
  ],
  paths.migrationRegistry,
);

requireAll(
  inbox,
  [
    "SELECT event_id, scope_key, revision_at, ingest_sequence, envelope_json",
    "ORDER BY ingest_sequence ASC",
    "let ingest_sequence: i64 =",
    "ingest_sequence <= watermark_sequence",
    "ingest_sequence: i64",
    "tenant_id, source_module, scope_key, ingest_sequence, revision_at, event_id, updated_at",
    "WHERE search_projection_watermarks.ingest_sequence < EXCLUDED.ingest_sequence",
    "SELECT ingest_sequence",
    "fn max_watermark(left: Option<i64>, right: Option<i64>) -> Option<i64>",
    "ingest_sequence_order_is_numeric",
  ],
  paths.inbox,
);
rejectAll(
  inbox,
  [
    "ORDER BY revision_at ASC, event_id ASC\n                    LIMIT 1",
    "fn is_newer_revision(",
    "std::cmp::Ordering",
  ],
  paths.inbox,
);

requireAll(
  reconciliation,
  [
    "ingest_sequence",
    "ORDER BY tenant_id, ingest_sequence ASC",
    "ORDER BY ingest_sequence ASC",
  ],
  paths.reconciliation,
);
rejectAll(
  reconciliation,
  ["ORDER BY tenant_id, revision_at ASC, event_id ASC"],
  paths.reconciliation,
);

requireAll(
  forumPlan,
  [
    "FORUM-23B2G1",
    "durable PostgreSQL ingest sequence",
    "verify-forum-search-durable-ingest-sequence.mjs",
  ],
  paths.forumPlan,
);
requireAll(
  searchPlan,
  [
    "FORUM-23B2G1",
    "source_complete_execution_pending",
    "durable Forum inbox ingest sequence",
  ],
  paths.searchPlan,
);
requireAll(
  note,
  [
    "# FORUM-23B2G1 durable Forum Search inbox ingest sequence",
    "not** the final",
    "does not determine execution or watermark order",
  ],
  paths.note,
);

if (contract) {
  if (contract.task !== "FORUM-23B2G1") failures.push(`${paths.contract}: unexpected task`);
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${paths.contract}: unexpected status`);
  }
  if (contract.ordering?.claim_order !== "ingest_sequence_ascending") {
    failures.push(`${paths.contract}: claim order drift`);
  }
  if (contract.ordering?.wall_clock_used_for_execution_order !== false) {
    failures.push(`${paths.contract}: wall clock ordering must be disabled`);
  }
  if (contract.ordering?.event_uuid_used_for_execution_order !== false) {
    failures.push(`${paths.contract}: UUID ordering must be disabled`);
  }
  if (contract.compatibility?.event_schema_changed !== false) {
    failures.push(`${paths.contract}: event schema compatibility drift`);
  }
  if (contract.compatibility?.forum_owner_write_changed !== false) {
    failures.push(`${paths.contract}: Forum owner compatibility drift`);
  }
  if (contract.migration?.sqlite_schema_changed !== false) {
    failures.push(`${paths.contract}: SQLite must remain unchanged`);
  }
}

if (failures.length > 0) {
  console.error("FORUM-23B2G1 durable ingest sequence verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("FORUM-23B2G1 durable ingest sequence source contract is consistent.");
