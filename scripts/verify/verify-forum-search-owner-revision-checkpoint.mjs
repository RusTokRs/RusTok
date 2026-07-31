#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];
const paths = {
  contract: "crates/rustok-forum/contracts/forum-search-owner-revision-checkpoint.json",
  note: "crates/rustok-forum/docs/forum-23b2g2b2-search-owner-revision-checkpoint.md",
  sourceContract: "crates/rustok-forum/contracts/forum-search-owner-revision-source.json",
  ledgerContract: "crates/rustok-forum/contracts/forum-search-owner-revision-ledger.json",
  hardeningContract: "crates/rustok-forum/contracts/forum-search-owner-revision-counter-hardening.json",
  ingestContract: "crates/rustok-forum/contracts/forum-search-durable-ingest-sequence.json",
  migration: "crates/rustok-search/src/migrations/m20260731_000012_create_forum_owner_revision_checkpoints.rs",
  migrationRegistry: "crates/rustok-search/src/migrations/mod.rs",
  predecessorMigration: "crates/rustok-search/src/migrations/m20260731_000010_add_forum_projection_ingest_sequence.rs",
  lookupMigration: "crates/rustok-search/src/migrations/m20260731_000011_add_forum_projection_ingest_sequence_lookup.rs",
  checkpointOwner: "crates/rustok-search/src/forum_owner_checkpoint.rs",
  reconciler: "crates/rustok-search/src/forum_reconciliation.rs",
  inbox: "crates/rustok-search/src/forum_inbox.rs",
  searchLib: "crates/rustok-search/src/lib.rs",
  forumDto: "crates/rustok-forum/src/dto/event.rs",
  forumOwner: "crates/rustok-forum/src/services/event.rs",
  hostAdapter: "apps/server/src/services/forum_search_owner_revision.rs",
  worker: "apps/server/src/services/forum_search_inbox_worker.rs",
  oldSweeperTest: "crates/rustok-search/tests/forum_projection_sweeper_contract.rs",
  oldSweeperVerifier: "scripts/verify/verify-forum-search-inbox-sweeper.mjs",
  rootEvents: "crates/rustok-events/src/types.rs",
  eventContracts: "crates/rustok-events/src/contract.rs",
  forumPlan: "crates/rustok-forum/docs/implementation-plan.md",
  searchPlan: "crates/rustok-search/docs/implementation-plan.md",
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
const sourceContract = parseJson(paths.sourceContract);
const ledgerContract = parseJson(paths.ledgerContract);
const hardeningContract = parseJson(paths.hardeningContract);
const ingestContract = parseJson(paths.ingestContract);
const note = read(paths.note);
const migration = read(paths.migration);
const migrationRegistry = read(paths.migrationRegistry);
const predecessorMigration = read(paths.predecessorMigration);
const lookupMigration = read(paths.lookupMigration);
const checkpointOwner = read(paths.checkpointOwner);
const reconciler = read(paths.reconciler);
const inbox = read(paths.inbox);
const searchLib = read(paths.searchLib);
const forumDto = read(paths.forumDto);
const forumOwner = read(paths.forumOwner);
const hostAdapter = read(paths.hostAdapter);
const worker = read(paths.worker);
const oldSweeperTest = read(paths.oldSweeperTest);
const oldSweeperVerifier = read(paths.oldSweeperVerifier);
const rootEvents = read(paths.rootEvents);
const eventContracts = read(paths.eventContracts);
const forumPlan = read(paths.forumPlan);
const searchPlan = read(paths.searchPlan);

requireAll(migration, [
  "search_projection_owner_checkpoints",
  "search_projection_owner_scan_cursors",
  "owner_revision BIGINT NOT NULL",
  "event_id UUID NOT NULL",
  "event_id <> '00000000-0000-0000-0000-000000000000'::uuid",
  "after_tenant_id IS NULL",
  "outcome IN ('delivery_covered', 'rebuild_repaired')",
  "NEW.owner_revision <> 1",
  "NEW.owner_revision <> OLD.owner_revision + 1",
  "checkpoint cannot be deleted",
  "BEFORE TRUNCATE ON search_projection_owner_checkpoints",
  "DatabaseBackend::Sqlite => Ok(())",
], paths.migration);
requireAll(migrationRegistry, [
  "mod m20260731_000012_create_forum_owner_revision_checkpoints;",
  "Box::new(m20260731_000011_add_forum_projection_ingest_sequence_lookup::Migration)",
  "Box::new(m20260731_000012_create_forum_owner_revision_checkpoints::Migration)",
], paths.migrationRegistry);
if (
  migrationRegistry.indexOf("m20260731_000012_create_forum_owner_revision_checkpoints::Migration") <
  migrationRegistry.indexOf("m20260731_000011_add_forum_projection_ingest_sequence_lookup::Migration")
) {
  failures.push(`${paths.migrationRegistry}: migration 000012 must follow 000011`);
}
rejectAll(predecessorMigration, [
  "search_projection_owner_checkpoints",
  "search_projection_owner_scan_cursors",
], `${paths.predecessorMigration} immutable predecessor`);
rejectAll(lookupMigration, [
  "search_projection_owner_checkpoints",
  "search_projection_owner_scan_cursors",
], `${paths.lookupMigration} immutable predecessor`);

requireAll(forumDto, [
  "pub struct ForumProjectionOwnerTenantHeadResponse",
  "pub tenant_id: Uuid",
  "pub latest_owner_revision: i64",
], paths.forumDto);
requireAll(forumOwner, [
  "MAX_FORUM_PROJECTION_OWNER_TENANT_PAGE: usize = 256",
  "pub async fn list_projection_owner_revision_tenants",
  "FROM forum_projection_revision_ledger",
  "MAX(revision) AS latest_owner_revision",
  "GROUP BY tenant_id",
  "ORDER BY tenant_id ASC",
  "projection owner tenant cursor must not be nil",
], paths.forumOwner);
rejectAll(forumOwner, [
  "search_projection_owner_checkpoints",
  "search_projection_owner_scan_cursors",
  "search_projection_inbox",
], paths.forumOwner);

requireAll(reconciler, [
  "async fn list_owner_revision_tenants",
  "ForumProjectionOwnerTenantPageRequest",
  "pub fn with_owner_revision_source",
  "owner_checkpoint: Option<ForumOwnerCheckpointReconciler>",
  "self.inbox.claim_next(tenant_id).await?",
  "owner_checkpoint.sweep_due",
  "MAX_FORUM_OWNER_REVISION_PAGE_LIMIT",
], paths.reconciler);
requireAll(searchLib, [
  "mod forum_owner_checkpoint;",
  "ForumProjectionOwnerTenantHead",
  "ForumProjectionOwnerTenantPageRequest",
  "resolve_forum_projection_owner_tenant_heads",
], paths.searchLib);
requireAll(hostAdapter, [
  "async fn list_owner_revision_tenants",
  "list_projection_owner_revision_tenants",
  "ForumProjectionOwnerTenantHead",
  "latest_owner_revision: head.latest_owner_revision",
], paths.hostAdapter);
requireAll(worker, [
  "SharedForumProjectionOwnerRevisionSourcePort",
  "requires the Forum owner revision source",
  "ForumProjectionReconciler::with_owner_revision_source",
  "owner_revisions_checkpointed",
  "owner_rebuilds",
  "Forum Search inbox sweep failed",
], paths.worker);
rejectAll(worker, [
  "search_projection_owner_checkpoints",
  "search_projection_owner_scan_cursors",
  "forum_projection_revision_ledger",
], paths.worker);

requireAll(checkpointOwner, [
  "MAX_FORUM_OWNER_TENANT_PAGE_LIMIT: usize = 256",
  "pub async fn resolve_forum_projection_owner_tenant_heads",
  "owner tenant heads must be strictly ordered",
  "recover_abandoned_processing(tenant_limit, revision_limit)",
  "SELECT DISTINCT tenant_id",
  "WITH abandoned AS (",
  "ORDER BY ingest_sequence ASC",
  "LIMIT $2",
  "status = 'processing'",
  "processing_lease_expired",
  "pg_try_advisory_xact_lock",
  "search:{FORUM_SOURCE_MODULE}:{tenant_id}:{FULL_SCOPE_KEY}",
  "status IN ('pending', 'processing', 'retryable_error')",
  "WHERE event_id = $1",
  '"completed" | "skipped"',
  '"dead_letter" => Ok(DeliveryCoverage::Missing)',
  "self.forum_projector.rebuild_tenant(head.tenant_id).await?",
  "advance_checkpoint(",
  "VALUES ($1, 'forum', $2, $3, $4, CURRENT_TIMESTAMP)",
  "search_projection_owner_checkpoints.owner_revision = $5",
  "search_projection_owner_scan_cursors.after_tenant_id",
  "IS NOT DISTINCT FROM $2",
  "Forum owner revision source failed with stable code",
], paths.checkpointOwner);
rejectAll(checkpointOwner, [
  "forum_projection_revision_ledger",
  "forum_domain_events",
  "format!(\"Forum owner revision source failed: {error}\")",
], paths.checkpointOwner);

const rebuildPosition = checkpointOwner.indexOf(
  "self.forum_projector.rebuild_tenant(head.tenant_id).await?"
);
const checkpointPosition = checkpointOwner.indexOf("advance_checkpoint(", rebuildPosition);
if (rebuildPosition < 0 || checkpointPosition < 0 || checkpointPosition < rebuildPosition) {
  failures.push(`${paths.checkpointOwner}: checkpoint must follow successful repair rebuild`);
}

requireAll(inbox, [
  "ORDER BY ingest_sequence ASC",
  "INSERT INTO search_projection_watermarks",
  "ingest_sequence",
], `${paths.inbox} G1 execution boundary`);
rejectAll(inbox, [
  "owner_revision",
  "search_projection_owner_checkpoints",
], `${paths.inbox} independent owner checkpoint boundary`);

for (const source of [oldSweeperTest, oldSweeperVerifier]) {
  requireAll(source, [
    "ORDER BY tenant_id, ingest_sequence ASC",
    "ORDER BY ingest_sequence ASC",
  ], "historical sweeper evidence");
  rejectAll(source, [
    "ORDER BY tenant_id, revision_at ASC, event_id ASC",
    "ORDER BY revision_at ASC, event_id ASC",
  ], "historical sweeper evidence");
}

rejectAll(rootEvents, [
  "ForumProjectionOwnerRevision",
  "forum_projection_owner_revision",
], `${paths.rootEvents} root event boundary`);
rejectAll(eventContracts, [
  "ForumProjectionOwnerRevision",
  "forum_projection_owner_revision",
], `${paths.eventContracts} sealed event boundary`);

requireAll(note, [
  "# FORUM-23B2G2B2 Search owner-revision checkpoint",
  "first Forum invalidation was committed by the owner but never reached Search",
  "bounded by both the tenant and event page limits",
  "pending`, `processing` or `retryable_error",
  "The projection transaction commits before Search attempts to advance the checkpoint",
  "versioned owner-revision wire contract",
  "did not run these commands",
], paths.note);
requireAll(forumPlan, [
  "| `FORUM-23` | `in_progress` |",
  "owner-issued revision reconciliation plus maintainer runtime evidence remain",
], `${paths.forumPlan} canonical open boundary`);
requireAll(searchPlan, [
  "owner-issued monotonic aggregate revisions",
  "LINK-FORUM-03",
], `${paths.searchPlan} canonical open boundary`);

if (contract) {
  if (contract.task !== "FORUM-23B2G2B2") {
    failures.push(`${paths.contract}: unexpected task`);
  }
  if (contract.status !== "source_complete_runtime_evidence_pending") {
    failures.push(`${paths.contract}: unexpected status`);
  }
  if (contract.owner_discovery?.first_lost_delivery_tenant_discoverable !== true) {
    failures.push(`${paths.contract}: first lost delivery must be discoverable`);
  }
  if (contract.checkpoint_storage?.exact_increment !== 1) {
    failures.push(`${paths.contract}: checkpoint must advance by one`);
  }
  if (contract.commit_protocol?.pending_blocks_checkpoint !== true
      || contract.commit_protocol?.processing_blocks_checkpoint !== true
      || contract.commit_protocol?.retryable_error_blocks_checkpoint !== true) {
    failures.push(`${paths.contract}: non-terminal inbox barrier drift`);
  }
  if (contract.commit_protocol?.checkpoint_advances_after_projection_commit !== true) {
    failures.push(`${paths.contract}: checkpoint ordering drift`);
  }
  if (contract.commit_protocol?.owner_revision_compared_numerically_with_ingest_sequence !== false) {
    failures.push(`${paths.contract}: owner and ingest counters must remain independent`);
  }
  if (contract.processing_recovery?.bounded_by_tenant_page_limit !== true
      || contract.processing_recovery?.bounded_by_event_page_limit !== true
      || contract.processing_recovery?.oldest_ingest_sequence_first !== true) {
    failures.push(`${paths.contract}: bounded processing recovery drift`);
  }
  if (contract.compatibility?.root_event_changed !== false
      || contract.compatibility?.sealed_event_family_added !== false) {
    failures.push(`${paths.contract}: event compatibility drift`);
  }
  if (contract.canonical_plan_boundary?.forum_23_status_changed !== false) {
    failures.push(`${paths.contract}: canonical task must remain in progress`);
  }
}

if (sourceContract?.task !== "FORUM-23B2G2B1") {
  failures.push(`${paths.sourceContract}: predecessor task drift`);
}
if (ledgerContract?.task !== "FORUM-23B2G2A") {
  failures.push(`${paths.ledgerContract}: ledger predecessor drift`);
}
if (hardeningContract?.task !== "FORUM-23B2G2A1") {
  failures.push(`${paths.hardeningContract}: hardening predecessor drift`);
}
if (ingestContract?.task !== "FORUM-23B2G1") {
  failures.push(`${paths.ingestContract}: ingest predecessor drift`);
}

if (failures.length > 0) {
  console.error("FORUM-23B2G2B2 owner revision checkpoint verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("FORUM-23B2G2B2 owner revision checkpoint contract is consistent.");
