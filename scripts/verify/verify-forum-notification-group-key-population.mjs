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
  if (!existsSync(absolute)) {
    failures.push(`${relativePath}: required file is missing`);
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

const contractPath =
  "crates/rustok-forum/contracts/forum-notification-group-key-population.json";
const contract = JSON.parse(read(contractPath) || "{}");
const migration = read(contract.notifications_migration_file ?? "");
const registry = read(contract.notifications_migration_registry ?? "");
const candidate = read(contract.notifications_candidate_file ?? "");
const entity = read(contract.notifications_entity_file ?? "");
const readme = read(contract.notifications_readme ?? "");
const library = read(contract.notifications_lib_file ?? "");
const proof = read(contract.sqlite_proof ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const note = read(contract.owner_note ?? "");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("group-key population contract must use schema_version=1");
}
if (contract.task !== "FORUM-20AD" || contract.upstream_task !== "FORUM-20AC") {
  failures.push("group-key population contract must connect FORUM-20AC/20AD");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("group-key population contract must not claim unexecuted evidence");
}

for (const key of [
  "owner_level_group_key_population",
  "postgres_before_insert_trigger",
  "sqlite_after_insert_trigger",
  "existing_null_backfill",
  "explicit_group_keys_preserved",
  "stable_versioned_group_key",
  "bounded_group_key",
  "target_owner_isolation",
  "target_uuid_grouping",
  "candidate_null_compatibility",
  "no_shared_api_change",
  "no_producer_dependency",
  "notification_state_unchanged",
  "delivery_attempts_unchanged",
  "ordered_migration_dependency",
  "sqlite_contract_proof",
  "owner_contract_note",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`group-key population contract must record ${key}`);
  }
}
for (const key of [
  "grouped_aggregate_summary",
  "group_unread_total",
  "latest_item_projection",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`group-key population contract must keep ${key} false`);
  }
}
if (
  !contract.not_delivered?.includes(
    "grouped aggregate summaries unread totals and latest-item projections",
  )
) {
  failures.push("group aggregate projections must remain open");
}

const sync = contract.canonical_plan_sync ?? {};
if (sync.required_ledger_through !== "FORUM-20AD") {
  failures.push("canonical ledger must be required through FORUM-20AD");
}
if (sync.status === "pending") {
  if (sync.current_plan_through !== "FORUM-20G") {
    failures.push("pending plan sync must identify FORUM-20G");
  }
  requireText(plan, "FORUM-20A-G provide", "pending plan sync must remain grounded through G");
  rejectText(plan, "### Delivered in `FORUM-20AD`", "canonical plan sync status is stale");
}

for (const marker of [
  "pub struct Migration",
  "DatabaseBackend::Postgres",
  "DatabaseBackend::Sqlite",
  "CREATE OR REPLACE FUNCTION rustok_notifications_assign_group_key()",
  "DROP TRIGGER IF EXISTS trg_notifications_assign_group_key ON notifications",
  "CREATE TRIGGER trg_notifications_assign_group_key",
  "BEFORE INSERT ON notifications",
  "IF NEW.group_key IS NULL THEN",
  "NEW.group_key := 'g1:' || NEW.target_owner || ':' || NEW.target_id::text",
  "AFTER INSERT ON notifications",
  "WHEN NEW.group_key IS NULL",
  "SET group_key = 'g1:' || NEW.target_owner || ':' || NEW.target_id",
  "UPDATE notifications",
  "WHERE group_key IS NULL",
  "SET group_key = NULL",
  "WHERE group_key = 'g1:' || target_owner || ':' || target_id",
]) {
  requireText(migration, marker, `group-key migration is missing ${marker}`);
}
for (const forbidden of [
  "target_kind ||",
  "notification_type ||",
  "delivery_attempt",
  "seen_at =",
  "read_at =",
  "archived_at =",
]) {
  rejectText(migration, forbidden, `group-key migration must not use ${forbidden}`);
}

for (const marker of [
  "mod m20260726_000015_populate_notification_group_keys;",
  "Box::new(m20260726_000015_populate_notification_group_keys::Migration)",
  '"m20260726_000015_populate_notification_group_keys"',
  'vec!["m20260723_000014_add_outbox_intake_rejections"]',
]) {
  requireText(registry, marker, `migration registry is missing ${marker}`);
}

requireText(
  candidate,
  "group_key: Set(None)",
  "candidate must remain compatible with persistence-owned group-key assignment",
);
requireText(entity, "pub group_key: Option<String>", "notification entity must retain group_key");
requireText(library, "assert_eq!(module.migrations().len(), 6)", "module must expose six migrations");
requireText(
  library,
  "assert_eq!(module.migration_dependencies().len(), 6)",
  "module must expose six migration dependencies",
);

for (const marker of [
  "The owner exposes six ordered PostgreSQL/SQLite migrations",
  "m20260726_000015_populate_notification_group_keys",
  "g1:{target_owner}:{target_id}",
  "NotificationInboxGroupListService",
  "tests/group_key_population_sqlite.rs",
  "Group summaries, per-group unread totals",
]) {
  requireText(readme, marker, `notifications README is missing ${marker}`);
}

for (const marker of [
  "migration_backfills_null_group_keys_and_preserves_explicit_keys",
  "new_rows_receive_stable_target_group_keys_without_delivery_mutation",
  "migrations.iter().take(5)",
  "migrations[5]",
  "Some(\"source-owned-explicit-group\")",
  "Some(\"explicit-new-group\")",
  "expected_group_key(SOURCE, target_id)",
  "expected_group_key(\"other-source\", target_id)",
  "assert_ne!(first.group_key, other_owner.group_key)",
  "assert_eq!(row.state, NotificationState::Unread)",
  "delivery_attempt::Entity::find()",
]) {
  requireText(proof, marker, `SQLite group-key proof is missing ${marker}`);
}

for (const marker of [
  "# FORUM-20AD notification group-key population",
  "g1:{target_owner}:{target_id}",
  "BEFORE INSERT",
  "AFTER INSERT",
  "explicit non-`NULL` group keys are preserved",
  "tests/group_key_population_sqlite.rs",
]) {
  requireText(note, marker, `owner note is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20AC" ||
  upstream.composition?.bounded_group_list_owner !== true ||
  upstream.composition?.source_group_key_population !== false ||
  !upstream.not_delivered?.includes(
    "source or notifications grouping policy and production group key population",
  )
) {
  failures.push("FORUM-20AD must close the FORUM-20AC group-key population residual");
}

if (failures.length > 0) {
  console.error("Forum notification group-key population verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification group-key population contract is source-ready.");
