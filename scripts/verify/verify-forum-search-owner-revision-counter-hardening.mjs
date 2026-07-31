#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];
const paths = {
  contract:
    "crates/rustok-forum/contracts/forum-search-owner-revision-counter-hardening.json",
  note:
    "crates/rustok-forum/docs/forum-23b2g2a1-search-owner-revision-counter-hardening.md",
  baseline:
    "crates/rustok-forum/src/migrations/m20260731_000007_add_forum_projection_revision_ledger.rs",
  migration:
    "crates/rustok-forum/src/migrations/m20260731_000008_harden_forum_projection_revision_counter.rs",
  registry: "crates/rustok-forum/src/migrations/mod.rs",
  allocator: "crates/rustok-forum/src/services/projection_invalidation.rs",
};

function read(relativePath) {
  const target = path.join(root, relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target, "utf8");
}

function parseJson(relativePath) {
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

const contract = parseJson(paths.contract);
const note = read(paths.note);
const baseline = read(paths.baseline);
const migration = read(paths.migration);
const registry = read(paths.registry);
const allocator = read(paths.allocator);

requireAll(baseline, [
  "CREATE TABLE IF NOT EXISTS forum_projection_revision_counters",
  "CREATE TABLE IF NOT EXISTS forum_projection_revision_ledger",
  "forum_reject_projection_revision_ledger_mutation",
], `${paths.baseline} retained baseline`);
rejectAll(baseline, [
  "forum_enforce_projection_revision_counter",
  "forum_require_projection_revision_ledger_row",
  "forum_reject_projection_revision_truncate",
], `${paths.baseline} immutable baseline boundary`);

requireAll(migration, [
  "existing forum projection revision storage is inconsistent",
  "COUNT(ledger.revision) <> counters.revision",
  "MIN(ledger.revision) IS DISTINCT FROM 1",
  "MAX(ledger.revision) IS DISTINCT FROM counters.revision",
  "WHERE counters.tenant_id IS NULL",
  "forum_enforce_projection_revision_counter",
  "NEW.revision <> 1",
  "NEW.tenant_id <> OLD.tenant_id OR NEW.revision <> OLD.revision + 1",
  "forum projection revision counter cannot be deleted",
  "forum_projection_revision_counter_insert",
  "forum_projection_revision_counter_update",
  "forum_projection_revision_counter_delete",
  "forum_require_projection_revision_ledger_row",
  "forum projection revision counter requires a matching ledger row",
  "CREATE CONSTRAINT TRIGGER forum_projection_revision_counter_ledger_commit",
  "DEFERRABLE INITIALLY DEFERRED",
  "forum_reject_projection_revision_truncate",
  "forum_projection_revision_counter_truncate",
  "forum_projection_revision_ledger_truncate",
  "BEFORE TRUNCATE ON forum_projection_revision_counters",
  "BEFORE TRUNCATE ON forum_projection_revision_ledger",
  "DatabaseBackend::Sqlite => Ok(())",
], paths.migration);
rejectAll(migration, [
  "DROP TABLE",
  "DELETE FROM forum_projection_revision",
  "TRUNCATE forum_projection_revision",
  "sys_events",
  "serde_json",
], `${paths.migration} additive boundary`);

requireAll(registry, [
  "mod m20260731_000007_add_forum_projection_revision_ledger;",
  "mod m20260731_000008_harden_forum_projection_revision_counter;",
  "Box::new(m20260731_000007_add_forum_projection_revision_ledger::Migration)",
  "Box::new(m20260731_000008_harden_forum_projection_revision_counter::Migration)",
], paths.registry);
if (
  registry.indexOf("m20260731_000007_add_forum_projection_revision_ledger::Migration")
  > registry.indexOf("m20260731_000008_harden_forum_projection_revision_counter::Migration")
) {
  failures.push(`${paths.registry}: hardening migration must follow the baseline ledger migration`);
}

requireAll(allocator, [
  "INSERT INTO forum_projection_revision_counters",
  "VALUES ($1, 1, CURRENT_TIMESTAMP)",
  "revision = forum_projection_revision_counters.revision + 1",
  "RETURNING revision",
  "INSERT INTO forum_projection_revision_ledger",
], `${paths.allocator} compatibility`);
rejectAll(allocator, [
  "forum_enforce_projection_revision_counter",
  "forum_require_projection_revision_ledger_row",
  "forum_reject_projection_revision_truncate",
], `${paths.allocator} no duplicate hardening`);

requireAll(note, [
  "# FORUM-23B2G2A1 Search owner-revision counter hardening",
  "baseline migration",
  "ledger revisions must form one contiguous sequence",
  "fails the migration",
  "advance the previous revision by exactly `1`",
  "DEFERRABLE INITIALLY DEFERRED",
  "direct counter-only commit",
  "rejects truncation of both",
  "does not decode or validate `sys_events.payload`",
  "did not run these commands",
], paths.note);

if (contract) {
  if (contract.task !== "FORUM-23B2G2A1") {
    failures.push(`${paths.contract}: unexpected task`);
  }
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${paths.contract}: unexpected status`);
  }
  if (contract.upgrade_preflight?.existing_counter_requires_contiguous_ledger_from_one !== true
      || contract.upgrade_preflight?.existing_ledger_without_counter_rejected !== true
      || contract.upgrade_preflight?.inconsistent_upgrade_fails_closed !== true
      || contract.upgrade_preflight?.existing_rows_rewritten !== false) {
    failures.push(`${paths.contract}: upgrade preflight contract is incomplete`);
  }
  if (contract.counter_invariants?.first_revision_must_equal !== 1) {
    failures.push(`${paths.contract}: first revision invariant must equal 1`);
  }
  if (contract.counter_invariants?.update_must_equal_previous_plus !== 1) {
    failures.push(`${paths.contract}: update increment invariant must equal 1`);
  }
  if (contract.counter_invariants?.tenant_key_immutable !== true
      || contract.counter_invariants?.row_delete_forbidden !== true
      || contract.counter_invariants?.table_truncate_forbidden !== true
      || contract.counter_invariants?.committed_revision_requires_matching_ledger_row !== true) {
    failures.push(`${paths.contract}: counter lifecycle and commit invariants are incomplete`);
  }
  if (contract.counter_invariants?.ledger_coverage_check
      !== "deferred constraint trigger at transaction commit") {
    failures.push(`${paths.contract}: deferred ledger coverage contract is missing`);
  }
  if (contract.ledger_invariants?.table_truncate_forbidden !== true) {
    failures.push(`${paths.contract}: ledger truncate guard is missing`);
  }
  if (contract.migration_policy?.baseline_migration_modified !== false
      || contract.migration_policy?.additive_upgrade_migration !== true) {
    failures.push(`${paths.contract}: migration immutability policy is incorrect`);
  }
  if (contract.compatibility?.forum_owner_allocator_changed !== false
      || contract.compatibility?.outbox_api_changed !== false
      || contract.compatibility?.search_inbox_changed !== false) {
    failures.push(`${paths.contract}: runtime compatibility boundary changed`);
  }
}

if (failures.length > 0) {
  console.error("FORUM-23B2G2A1 counter hardening verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("FORUM-23B2G2A1 counter hardening source contract is consistent.");
