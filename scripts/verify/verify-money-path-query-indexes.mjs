#!/usr/bin/env node
// Fast source-level guardrails for the query/index alignment of the money-path
// queues the relay and the admin surfaces read every iteration.

import { readFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function readRepo(relativePath) {
  try {
    return readFileSync(path.join(repoRoot, relativePath), "utf8");
  } catch {
    failures.push(`${relativePath}: source file is missing`);
    return "";
  }
}

function requireText(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) failures.push(description);
}

// --- outbox relay claim read ------------------------------------------------
// The claim predicate is (status = pending) AND liveness conditions on
// claimed_at / next_attempt_at, ordered by created_at, so the covering index
// leads with the status filter and carries the ordered column. A predicate or
// order change without the index (or the reverse) silently turns the relay into
// a full-table sort per iteration.
const outboxRelay = readRepo("crates/modules/rustok-outbox/src/relay.rs");
requireText(
  outboxRelay,
  "entity::Column::Status.eq(SysEventStatus::Pending)",
  "relay claim must filter the pending status",
);
requireText(
  outboxRelay,
  "entity::Column::ClaimedAt.is_null()",
  "relay claim must read never-claimed rows",
);
requireText(
  outboxRelay,
  "entity::Column::ClaimedAt.lte(stale_before)",
  "relay claim must bound the claim-liveness predicate",
);
requireText(
  outboxRelay,
  "order_by_asc(entity::Column::CreatedAt)",
  "relay claim must keep created-at order",
);
requireText(
  outboxRelay,
  "entity::Column::NextAttemptAt.lte(now)",
  "relay claim must respect the retry schedule",
);

const outboxMigration = readRepo("crates/modules/rustok-outbox/src/migration.rs");
requireText(
  outboxMigration,
  'name("idx_sys_events_pending_created_at")',
  "outbox claim index name",
);
const claimIndex = outboxMigration.slice(
  outboxMigration.indexOf("pub async fn create_sys_events_claim_index"),
  outboxMigration.indexOf("pub async fn drop_sys_events_claim_index"),
);
for (const column of [".col(SysEvents::Status)", ".col(SysEvents::CreatedAt)"]) {
  requireText(claimIndex, column, `outbox claim index column ${column}`);
}
requireText(
  outboxMigration,
  "create_sys_events_claim_index(manager).await?;",
  "module-owned sys_events migration must create the claim index",
);
requireText(
  outboxMigration,
  "pub async fn drop_sys_events_claim_index(",
  "outbox claim index drop helper",
);

const platformMigration = readRepo(
  "crates/utils/rustok-migrations/src/m20261007_000014_add_sys_events_claim_index.rs",
);
requireText(
  platformMigration,
  "rustok_outbox::migration::create_sys_events_claim_index(manager).await",
  "platform migration must create the outbox claim index through the owner helper",
);
requireText(
  platformMigration,
  "rustok_outbox::migration::drop_sys_events_claim_index(manager).await",
  "platform migration must drop the outbox claim index through the owner helper",
);
const platformMigrator = readRepo("crates/utils/rustok-migrations/src/lib.rs");
requireText(
  platformMigrator,
  "mod m20261007_000014_add_sys_events_claim_index;",
  "platform claim index migration module",
);
requireText(
  platformMigrator,
  "m20261007_000014_add_sys_events_claim_index::Migration,",
  "platform claim index migration registration",
);

// --- outbox delivered-event retention prune ---------------------------------
// The prune selects delivered rows older than the window in dispatched-at order.
// The liveness split matters: undelivered rows still owe a delivery and DLQ rows
// are an operator queue, so the prune may only ever read `dispatched` rows with a
// non-null `dispatched_at`.
const outboxRetention = readRepo("crates/modules/rustok-outbox/src/retention.rs");
requireText(
  outboxRetention,
  "entity::Column::Status.eq(SysEventStatus::Dispatched)",
  "retention prune must only select delivered rows",
);
requireText(
  outboxRetention,
  "entity::Column::DispatchedAt.is_not_null()",
  "retention prune must skip delivered rows without a delivery timestamp",
);
requireText(
  outboxRetention,
  "entity::Column::DispatchedAt.lte(cutoff)",
  "retention prune must bound the retention window",
);
requireText(
  outboxRetention,
  "order_by_asc(entity::Column::DispatchedAt)",
  "retention prune must read oldest delivered rows first",
);
requireText(
  outboxRetention,
  "limit(self.config.batch_size)",
  "retention prune must stay bounded per run",
);
const outboxRetentionWrites = outboxRetention.slice(0, outboxRetention.indexOf("#[cfg(test)]"));
for (const forbidden of ["SysEventStatus::Pending", "SysEventStatus::Failed"]) {
  if (outboxRetentionWrites.includes(forbidden)) {
    failures.push(`retention prune must never touch ${forbidden} rows`);
  }
}
requireText(
  outboxMigration,
  'name("idx_sys_events_dispatched_at")',
  "outbox retention index name",
);
const retentionIndex = outboxMigration.slice(
  outboxMigration.indexOf("pub async fn create_sys_events_retention_index"),
  outboxMigration.indexOf("pub async fn drop_sys_events_retention_index"),
);
for (const column of [".col(SysEvents::Status)", ".col(SysEvents::DispatchedAt)"]) {
  requireText(retentionIndex, column, `outbox retention index column ${column}`);
}
requireText(
  outboxMigration,
  "create_sys_events_retention_index(manager).await?;",
  "module-owned sys_events migration must create the retention index",
);
requireText(
  outboxMigration,
  "pub async fn drop_sys_events_retention_index(",
  "outbox retention index drop helper",
);

const platformRetentionMigration = readRepo(
  "crates/utils/rustok-migrations/src/m20261007_000015_add_sys_events_retention_index.rs",
);
requireText(
  platformRetentionMigration,
  "rustok_outbox::migration::create_sys_events_retention_index(manager).await",
  "platform migration must create the outbox retention index through the owner helper",
);
requireText(
  platformRetentionMigration,
  "rustok_outbox::migration::drop_sys_events_retention_index(manager).await",
  "platform migration must drop the outbox retention index through the owner helper",
);
requireText(
  platformMigrator,
  "mod m20261007_000015_add_sys_events_retention_index;",
  "platform retention index migration module",
);
requireText(
  platformMigrator,
  "m20261007_000015_add_sys_events_retention_index::Migration,",
  "platform retention index migration registration",
);

// --- sys_events has one schema owner ----------------------------------------
// The table shape lives once, in the owner helper. The platform wrapper orders
// it on the platform timeline and delegates; a second copy of the DDL here is
// what made the previous pair drift. The claim and retention indexes arrive
// through their own append-only migrations, and the pair superseded by the claim
// index is dropped by its own migration instead of being re-created.
const platformSchemaWrapper = readRepo(
  "crates/utils/rustok-migrations/src/m20260211_000002_create_sys_events.rs",
);
requireText(
  platformSchemaWrapper,
  "rustok_outbox::migration::create_sys_events_schema(manager).await",
  "platform sys_events migration must delegate to the owner schema helper",
);
requireText(
  platformSchemaWrapper,
  "rustok_outbox::migration::drop_sys_events_schema(manager).await",
  "platform sys_events rollback must delegate to the owner schema helper",
);
for (const duplicatedDdl of ["Table::create()", "Index::create()", "ColumnDef::new("]) {
  if (platformSchemaWrapper.includes(duplicatedDdl)) {
    failures.push(`platform sys_events migration must not duplicate DDL (${duplicatedDdl})`);
  }
}
const schemaHelper = outboxMigration.slice(
  outboxMigration.indexOf("pub async fn create_sys_events_schema"),
  outboxMigration.indexOf("pub async fn drop_sys_events_schema"),
);
if (schemaHelper.includes("Index::create()")) {
  failures.push("the sys_events schema helper must not create indexes");
}

const platformSupersededDrop = readRepo(
  "crates/utils/rustok-migrations/src/m20261007_000016_drop_sys_events_superseded_indexes.rs",
);
requireText(
  platformSupersededDrop,
  "rustok_outbox::migration::drop_sys_events_superseded_indexes(manager).await",
  "platform migration must drop the superseded sys_events indexes through the owner helper",
);
requireText(
  platformSupersededDrop,
  "rustok_outbox::migration::create_sys_events_superseded_indexes(manager).await",
  "the superseded index drop must be reversible",
);
requireText(
  outboxMigration,
  "pub async fn drop_sys_events_superseded_indexes(",
  "outbox superseded index drop helper",
);
requireText(
  platformMigrator,
  "mod m20261007_000016_drop_sys_events_superseded_indexes;",
  "platform superseded index migration module",
);
requireText(
  platformMigrator,
  "m20261007_000016_drop_sys_events_superseded_indexes::Migration,",
  "platform superseded index migration registration",
);

// --- admin checkout operation list ------------------------------------------
const journal = readRepo("crates/modules/rustok-commerce/src/services/checkout_operation.rs");
requireText(
  journal,
  "checkout_operation::Column::TenantId.eq(tenant_id)",
  "checkout operation listing must stay tenant-scoped",
);
requireText(
  journal,
  "order_by_desc(checkout_operation::Column::UpdatedAt)",
  "checkout operation listing must keep newest-update order",
);
const commerceIndexMigration = readRepo(
  "crates/modules/rustok-commerce/src/migrations/m20261007_000014_add_checkout_operation_admin_list_index.rs",
);
const commerceIndex = commerceIndexMigration.slice(
  commerceIndexMigration.indexOf("async fn up"),
  commerceIndexMigration.indexOf("async fn down"),
);
requireText(
  commerceIndex,
  'name("idx_checkout_operations_tenant_status_updated")',
  "admin checkout operation list index name",
);
for (const column of [
  ".col(CheckoutOperations::TenantId)",
  ".col(CheckoutOperations::Status)",
  ".col(CheckoutOperations::UpdatedAt)",
]) {
  requireText(commerceIndex, column, `admin checkout operation list index column ${column}`);
}
const commerceMigrationsMod = readRepo("crates/modules/rustok-commerce/src/migrations/mod.rs");
requireText(
  commerceMigrationsMod,
  "mod m20261007_000014_add_checkout_operation_admin_list_index;",
  "admin checkout operation list index migration module",
);
requireText(
  commerceMigrationsMod,
  "m20261007_000014_add_checkout_operation_admin_list_index::Migration,",
  "admin checkout operation list index migration registration",
);
requireText(
  commerceMigrationsMod,
  '"m20261007_000014_add_checkout_operation_admin_list_index",',
  "admin checkout operation list index migration dependency",
);

if (failures.length > 0) {
  console.error("money-path query index verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("money-path query index verification passed");
