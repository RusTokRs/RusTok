#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];
const paths = {
  contract: "crates/rustok-forum/contracts/forum-search-ingest-sequence-index.json",
  note: "crates/rustok-forum/docs/forum-23b2g1a-search-ingest-sequence-index.md",
  forumPlan: "crates/rustok-forum/docs/implementation-plan.md",
  searchPlan: "crates/rustok-search/docs/implementation-plan.md",
  predecessor:
    "crates/rustok-search/src/migrations/m20260731_000010_add_forum_projection_ingest_sequence.rs",
  migration:
    "crates/rustok-search/src/migrations/m20260731_000011_add_forum_projection_ingest_sequence_lookup.rs",
  registry: "crates/rustok-search/src/migrations/mod.rs",
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
const predecessor = read(paths.predecessor);
const migration = read(paths.migration);
const registry = read(paths.registry);
const inbox = read(paths.inbox);
const reconciliation = read(paths.reconciliation);

requireAll(
  migration,
  [
    "idx_search_projection_inbox_due_ingest_sequence",
    "ON search_projection_inbox (source_module, tenant_id, ingest_sequence)",
    "WHERE status IN ('pending', 'retryable_error')",
    "conrelid = 'search_projection_inbox'::regclass",
    "conrelid = 'search_projection_watermarks'::regclass",
    "CHECK (ingest_sequence > 0)",
    "CHECK (ingest_sequence >= 0)",
    "DROP INDEX IF EXISTS idx_search_projection_inbox_due_ingest_sequence",
    "DatabaseBackend::Sqlite => Ok(())",
  ],
  paths.migration,
);
rejectAll(
  migration,
  [
    "ADD COLUMN",
    "DROP COLUMN",
    "CREATE SEQUENCE",
    "DROP SEQUENCE",
    "ALTER COLUMN ingest_sequence",
  ],
  paths.migration,
);
rejectAll(
  predecessor,
  ["idx_search_projection_inbox_due_ingest_sequence"],
  `${paths.predecessor} immutable history`,
);

requireAll(
  registry,
  [
    "mod m20260731_000010_add_forum_projection_ingest_sequence;",
    "mod m20260731_000011_add_forum_projection_ingest_sequence_lookup;",
    "m20260731_000010_add_forum_projection_ingest_sequence::Migration",
    "m20260731_000011_add_forum_projection_ingest_sequence_lookup::Migration",
  ],
  paths.registry,
);
if (
  registry.indexOf("m20260731_000010_add_forum_projection_ingest_sequence::Migration") >
  registry.indexOf("m20260731_000011_add_forum_projection_ingest_sequence_lookup::Migration")
) {
  failures.push(`${paths.registry}: 000011 must follow 000010`);
}

requireAll(
  inbox,
  [
    "WHERE tenant_id = $1",
    "AND source_module = 'forum'",
    "AND status IN ('pending', 'retryable_error')",
    "ORDER BY ingest_sequence ASC",
  ],
  `${paths.inbox} claim query`,
);
requireAll(
  reconciliation,
  [
    "SELECT DISTINCT ON (tenant_id)",
    "WHERE source_module = 'forum'",
    "AND status IN ('pending', 'retryable_error')",
    "ORDER BY tenant_id, ingest_sequence ASC",
    "ORDER BY ingest_sequence ASC",
  ],
  `${paths.reconciliation} due-tenant query`,
);

requireAll(
  note,
  [
    "# FORUM-23B2G1A Forum Search ingest-sequence lookup index",
    "left byte-for-byte unchanged",
    "idx_search_projection_inbox_due_ingest_sequence",
    "EXPLAIN (ANALYZE, BUFFERS)",
    "did not run these commands",
  ],
  paths.note,
);
requireAll(
  forumPlan,
  [
    "FORUM-23B2G1A",
    "partial ingest-sequence lookup index",
    "verify-forum-search-ingest-sequence-index.mjs",
  ],
  paths.forumPlan,
);
requireAll(
  searchPlan,
  [
    "FORUM-23B2G1A",
    "source_complete_execution_pending",
    "ingest-sequence lookup index",
  ],
  paths.searchPlan,
);

if (contract) {
  if (contract.task !== "FORUM-23B2G1A") failures.push(`${paths.contract}: unexpected task`);
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${paths.contract}: unexpected status`);
  }
  if (contract.index?.name !== "idx_search_projection_inbox_due_ingest_sequence") {
    failures.push(`${paths.contract}: unexpected index name`);
  }
  if (contract.index?.predicate !== "status IN ('pending', 'retryable_error')") {
    failures.push(`${paths.contract}: partial predicate drift`);
  }
  if (contract.upgrade_safety?.predecessor_migration_modified !== false) {
    failures.push(`${paths.contract}: predecessor migration must remain immutable`);
  }
  if (contract.upgrade_safety?.already_migrated_databases_receive_index !== true) {
    failures.push(`${paths.contract}: upgrade-safe delivery is not recorded`);
  }
  if (contract.constraint_repair?.existence_check_scoped_by_conrelid !== true) {
    failures.push(`${paths.contract}: scoped constraint repair is not recorded`);
  }
  if (contract.compatibility?.claim_order_changed !== false) {
    failures.push(`${paths.contract}: claim order compatibility drift`);
  }
}

if (failures.length > 0) {
  console.error("FORUM-23B2G1A ingest-sequence lookup verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("FORUM-23B2G1A ingest-sequence lookup source contract is consistent.");
