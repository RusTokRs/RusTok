#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];
const paths = {
  contract: "crates/rustok-forum/contracts/forum-search-owner-revision-ledger.json",
  note: "crates/rustok-forum/docs/forum-23b2g2a-search-owner-revision-ledger.md",
  migration:
    "crates/rustok-forum/src/migrations/m20260731_000007_add_forum_projection_revision_ledger.rs",
  migrationRegistry: "crates/rustok-forum/src/migrations/mod.rs",
  owner: "crates/rustok-forum/src/services/projection_invalidation.rs",
  outbox: "crates/rustok-outbox/src/transactional.rs",
  outboxApi: "crates/rustok-outbox/CRATE_API.md",
  legacyContract: "crates/rustok-forum/contracts/forum-projection-invalidation.json",
  searchIngestion: "crates/rustok-search/src/ingestion.rs",
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

function requireOrdered(source, markers, label) {
  let cursor = -1;
  for (const marker of markers) {
    const next = source.indexOf(marker, cursor + 1);
    if (next < 0) {
      failures.push(`${label}: missing ordered marker ${marker}`);
      return;
    }
    if (next <= cursor) {
      failures.push(`${label}: marker out of order ${marker}`);
      return;
    }
    cursor = next;
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
const migration = read(paths.migration);
const migrationRegistry = read(paths.migrationRegistry);
const owner = read(paths.owner);
const outbox = read(paths.outbox);
const outboxApi = read(paths.outboxApi);
const legacyContract = read(paths.legacyContract);
const searchIngestion = read(paths.searchIngestion);
const forumPlan = read(paths.forumPlan);
const searchPlan = read(paths.searchPlan);

requireAll(
  migration,
  [
    "forum_projection_revision_counters",
    "forum_projection_revision_ledger",
    "PRIMARY KEY (tenant_id, revision)",
    "UNIQUE (event_id)",
    "CHECK (revision > 0)",
    "target_type = 'forum' AND target_id IS NULL",
    "target_type IN ('forum_category', 'forum_topic') AND target_id IS NOT NULL",
    "idx_forum_projection_revision_ledger_target",
    "forum_reject_projection_revision_ledger_mutation",
    "BEFORE UPDATE ON forum_projection_revision_ledger",
    "BEFORE DELETE ON forum_projection_revision_ledger",
    "DatabaseBackend::Sqlite => Ok(())",
  ],
  paths.migration,
);
requireAll(
  migrationRegistry,
  [
    "mod m20260731_000007_add_forum_projection_revision_ledger;",
    "m20260731_000007_add_forum_projection_revision_ledger::Migration",
  ],
  paths.migrationRegistry,
);

requireAll(
  outbox,
  [
    "pub async fn publish_root_in_tx_with_envelope_id<C>",
    "let envelope = EventEnvelope::new(tenant_id, actor_id, event);",
    "let envelope_id = envelope.id;",
    "OutboxTransport::write_envelope_in_tx(txn, envelope).await?;",
    "Ok(envelope_id)",
  ],
  paths.outbox,
);
requireOrdered(
  outbox,
  [
    "pub async fn publish_root_in_tx<C>",
    "Self::publish_root_in_tx_with_envelope_id",
    "pub async fn publish_root_in_tx_with_envelope_id<C>",
  ],
  `${paths.outbox} compatibility delegation`,
);
requireAll(
  outboxApi,
  [
    "TransactionalEventBus::publish_root_in_tx_with_envelope_id",
    "the exact root envelope",
    "do not publish a second envelope",
  ],
  paths.outboxApi,
);

requireAll(
  owner,
  [
    "allocate_projection_revision_in_tx",
    "forum_projection_revision_counters",
    "ON CONFLICT (tenant_id)",
    "revision = forum_projection_revision_counters.revision + 1",
    "RETURNING revision",
    "publish_root_in_tx_with_envelope_id",
    "publish_in_tx_with_envelope_id",
    "record_projection_revision_in_tx",
    "forum_projection_revision_ledger",
    "target_type.to_string().into()",
    "target_id.into()",
    "txn.get_database_backend() != DatabaseBackend::Postgres",
  ],
  paths.owner,
);
requireOrdered(
  owner,
  [
    "let revision = allocate_projection_revision_in_tx(txn, tenant_id).await?;",
    "let event_id = TransactionalEventBus::publish_root_in_tx_with_envelope_id",
    "record_projection_revision_in_tx(",
  ],
  `${paths.owner} direct transactional ordering`,
);
const injectedOwner = owner.slice(owner.indexOf("async fn publish_projection_invalidation_in_tx"));
requireOrdered(
  injectedOwner,
  [
    "let revision = allocate_projection_revision_in_tx(txn, tenant_id).await?;",
    "publish_in_tx_with_envelope_id",
    "record_projection_revision_in_tx(",
  ],
  `${paths.owner} injected transactional ordering`,
);
rejectAll(
  owner,
  ["ForumProjectionEvent", "publish_contract_in_tx", "SearchProjectionRevision"],
  `${paths.owner} wire rollout boundary`,
);

requireAll(
  legacyContract,
  [
    '"root_event": "rustok_events::DomainEvent::ReindexRequested"',
    '"event_type": "search.reindex_requested"',
    '"new_root_domain_event_added": false',
    '"new_event_schema_added": false',
  ],
  paths.legacyContract,
);
requireAll(
  searchIngestion,
  [
    "DomainEvent::ReindexRequested",
    "ForumProjectionScope::for_event(&envelope.event)",
    "inbox.enqueue(envelope, &scope).await?;",
  ],
  `${paths.searchIngestion} unchanged legacy consumer`,
);
requireAll(
  forumPlan,
  ["owner-issued monotonic projection revisions", "maintainer runtime evidence"],
  `${paths.forumPlan} remaining canonical scope`,
);
requireAll(
  searchPlan,
  ["Forum-owner-issued revisions remain pending", "Search ingest sequence"],
  `${paths.searchPlan} remaining canonical scope`,
);
requireAll(
  note,
  [
    "# FORUM-23B2G2A Forum Search owner revision ledger",
    "same owner transaction",
    "publish_root_in_tx_with_envelope_id",
    "Search does not yet receive or enforce the Forum revision",
    "FORUM-23B2G2B",
    "did not run these commands",
  ],
  paths.note,
);

if (contract) {
  if (contract.task !== "FORUM-23B2G2A") failures.push(`${paths.contract}: unexpected task`);
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${paths.contract}: unexpected status`);
  }
  if (contract.revision_owner?.module !== "forum") {
    failures.push(`${paths.contract}: revision owner drift`);
  }
  if (contract.revision_owner?.first_revision !== 1) {
    failures.push(`${paths.contract}: first revision drift`);
  }
  if (contract.ledger?.event_id_unique !== true) {
    failures.push(`${paths.contract}: envelope identity is not unique`);
  }
  if (contract.transaction?.partial_commit_allowed !== false) {
    failures.push(`${paths.contract}: partial commit must be forbidden`);
  }
  if (contract.compatibility?.legacy_root_event_retained !== true) {
    failures.push(`${paths.contract}: legacy rollout compatibility drift`);
  }
  if (contract.compatibility?.new_sealed_event_family_added !== false) {
    failures.push(`${paths.contract}: G2A must not claim the G2B wire family`);
  }
  if (contract.compatibility?.existing_public_api_signature_changed !== false) {
    failures.push(`${paths.contract}: existing public signature compatibility drift`);
  }
  if (contract.compatibility?.additive_outbox_envelope_identity_api_added !== true) {
    failures.push(`${paths.contract}: additive outbox identity API is not recorded`);
  }
  if (contract.canonical_plan_boundary?.forum_23_status_changed !== false) {
    failures.push(`${paths.contract}: canonical milestone status must remain unchanged`);
  }
}

if (failures.length > 0) {
  console.error("FORUM-23B2G2A owner revision ledger verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("FORUM-23B2G2A owner revision ledger source contract is consistent.");
