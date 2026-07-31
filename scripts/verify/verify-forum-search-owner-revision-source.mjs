#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];
const paths = {
  contract: "crates/rustok-forum/contracts/forum-search-owner-revision-source.json",
  predecessor: "crates/rustok-forum/contracts/forum-search-owner-revision-ledger.json",
  note: "crates/rustok-forum/docs/forum-23b2g2b1-search-owner-revision-source.md",
  ledgerMigration:
    "crates/rustok-forum/src/migrations/m20260731_000007_add_forum_projection_revision_ledger.rs",
  forumDto: "crates/rustok-forum/src/dto/event.rs",
  forumOwner: "crates/rustok-forum/src/services/event.rs",
  searchOwner: "crates/rustok-search/src/forum_reconciliation.rs",
  searchLib: "crates/rustok-search/src/lib.rs",
  searchInbox: "crates/rustok-search/src/forum_inbox.rs",
  searchMigrations: "crates/rustok-search/src/migrations/mod.rs",
  hostAdapter: "apps/server/src/services/forum_search_owner_revision.rs",
  hostComposition: "apps/server/src/services/mod.rs",
  rootEvents: "crates/rustok-events/src/types.rs",
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
const predecessor = parseJson(paths.predecessor);
const note = read(paths.note);
const ledgerMigration = read(paths.ledgerMigration);
const forumDto = read(paths.forumDto);
const forumOwner = read(paths.forumOwner);
const searchOwner = read(paths.searchOwner);
const searchLib = read(paths.searchLib);
const searchInbox = read(paths.searchInbox);
const searchMigrations = read(paths.searchMigrations);
const hostAdapter = read(paths.hostAdapter);
const hostComposition = read(paths.hostComposition);
const rootEvents = read(paths.rootEvents);

requireAll(
  ledgerMigration,
  [
    "forum_projection_revision_counters",
    "forum_projection_revision_ledger",
    "PRIMARY KEY (tenant_id, revision)",
    "UNIQUE (event_id)",
    "CHECK (revision > 0)",
    "forum_reject_projection_revision_ledger_mutation",
  ],
  paths.ledgerMigration,
);

requireAll(
  forumDto,
  [
    "pub enum ForumProjectionOwnerRevisionImpact",
    "FullRebuild",
    "pub struct ForumProjectionOwnerRevisionResponse",
    "pub owner_revision: i64",
    "pub event_id: Uuid",
  ],
  paths.forumDto,
);
rejectAll(forumDto, ["NoProjectionChange"], paths.forumDto);

requireAll(
  forumOwner,
  [
    "MAX_FORUM_PROJECTION_OWNER_REVISION_PAGE: usize = 100",
    'FORUM_PROJECTION_INVALIDATION_EVENT_TYPE: &str = "index.reindex_requested"',
    "pub async fn list_projection_owner_revisions",
    "forum_projection_revision_ledger",
    "WHERE tenant_id = $1",
    "AND revision > $2",
    "ORDER BY revision ASC",
    "LIMIT $3",
    "FORUM_PROJECTION_REVISION_SOURCE_UNAVAILABLE",
    "ForumProjectionOwnerRevisionImpact::FullRebuild",
  ],
  paths.forumOwner,
);
rejectAll(
  forumOwner,
  [
    "SequenceNo.gt(after_owner_revision)",
    "projection_revision_impact",
    "NoProjectionChange",
    "search_projection_inbox",
    "search_projection_watermarks",
  ],
  paths.forumOwner,
);

requireAll(
  searchOwner,
  [
    "pub trait ForumProjectionOwnerRevisionSourcePort",
    "pub type SharedForumProjectionOwnerRevisionSourcePort",
    "pub async fn resolve_forum_projection_owner_revisions",
    "DEFAULT_FORUM_OWNER_REVISION_PAGE_LIMIT: usize = 64",
    "MAX_FORUM_OWNER_REVISION_PAGE_LIMIT: usize = 100",
    "owner revisions must be contiguous and strictly ordered",
    'event_type != "index.reindex_requested"',
    "owner_revision_port_requires_host_composition",
    "owner_revision_page_accepts_contiguous_tenant_ledger_sequence",
    "owner_revision_page_rejects_gap_or_replay",
  ],
  paths.searchOwner,
);
requireAll(
  searchLib,
  [
    "ForumProjectionOwnerRevisionSourcePort",
    "SharedForumProjectionOwnerRevisionSourcePort",
    "resolve_forum_projection_owner_revisions",
  ],
  paths.searchLib,
);
rejectAll(
  searchOwner,
  [
    "forum_domain_event::",
    "forum_domain_events",
    "forum_projection_revision_ledger",
    "rustok_forum",
    "UPDATE search_projection_watermarks",
    "INSERT INTO search_projection_watermarks",
    "NoProjectionChange",
  ],
  paths.searchOwner,
);

requireAll(
  hostAdapter,
  [
    "ServerForumProjectionOwnerRevisionSourcePort",
    "ForumEventService::new",
    "list_projection_owner_revisions",
    "ForumOwnerRevisionImpact::FullRebuild",
    "ForumProjectionOwnerRevisionImpact::FullRebuild",
  ],
  paths.hostAdapter,
);
rejectAll(
  hostAdapter,
  [
    "forum_domain_event::",
    "forum_domain_events",
    "forum_projection_revision_ledger",
    "search_projection_watermarks",
    "NoProjectionChange",
  ],
  paths.hostAdapter,
);
requireAll(
  hostComposition,
  [
    'mod forum_search_owner_revision {',
    "ServerForumProjectionOwnerRevisionSourcePort::shared",
    "extensions.insert(owner_revision);",
    "SharedForumProjectionOwnerRevisionSourcePort",
  ],
  paths.hostComposition,
);

requireAll(
  searchInbox,
  ["ingest_sequence", "ORDER BY ingest_sequence ASC"],
  `${paths.searchInbox} G1 boundary`,
);
rejectAll(
  searchInbox,
  ["owner_revision", "forum_projection_revision_ledger", "forum_domain_events"],
  `${paths.searchInbox} G2B1 non-consumer boundary`,
);
rejectAll(
  searchMigrations,
  ["owner_revision", "projection_owner_revision"],
  `${paths.searchMigrations} checkpoint migration boundary`,
);
rejectAll(
  rootEvents,
  ["ForumProjectionOwnerRevision", "forum_projection_owner_revision"],
  `${paths.rootEvents} root event boundary`,
);

requireAll(
  note,
  [
    "# FORUM-23B2G2B1 Search owner-revision source",
    "forum_projection_revision_ledger.revision",
    "committed rows for one tenant are contiguous",
    "does not connect it to the background sweeper",
    "FORUM-23B2G2B2",
    "did not run these commands",
  ],
  paths.note,
);

if (predecessor) {
  if (predecessor.task !== "FORUM-23B2G2A") {
    failures.push(`${paths.predecessor}: unexpected predecessor task`);
  }
  if (predecessor.revision_owner?.storage !== "forum_projection_revision_counters") {
    failures.push(`${paths.predecessor}: revision counter ownership drift`);
  }
  if (predecessor.ledger?.table !== "forum_projection_revision_ledger") {
    failures.push(`${paths.predecessor}: owner ledger drift`);
  }
}

if (contract) {
  if (contract.task !== "FORUM-23B2G2B1") {
    failures.push(`${paths.contract}: unexpected task`);
  }
  if (contract.status !== "source_complete_consumer_checkpoint_pending") {
    failures.push(`${paths.contract}: unexpected status`);
  }
  if (contract.owner_clock?.table !== "forum_projection_revision_ledger") {
    failures.push(`${paths.contract}: owner clock must use the Forum projection ledger`);
  }
  if (contract.owner_clock?.contiguous_committed_revisions !== true) {
    failures.push(`${paths.contract}: committed owner revisions must be contiguous`);
  }
  if (contract.owner_clock?.forum_domain_event_sequence_used !== false) {
    failures.push(`${paths.contract}: Forum journal sequence must not be reused`);
  }
  if (contract.search_contract?.contiguous_page_required !== true) {
    failures.push(`${paths.contract}: Search must fail closed on owner revision gaps`);
  }
  if (contract.search_contract?.independent_from_search_ingest_sequence !== true) {
    failures.push(`${paths.contract}: owner and ingest sequences must remain independent`);
  }
  if (contract.host_composition?.direct_search_read_of_forum_projection_revision_ledger !== false) {
    failures.push(`${paths.contract}: Search must not read the Forum ledger directly`);
  }
  if (contract.compatibility?.search_watermark_changed !== false) {
    failures.push(`${paths.contract}: G2B1 must not claim checkpoint mutation`);
  }
}

if (failures.length > 0) {
  console.error("FORUM-23B2G2B1 owner revision source verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("FORUM-23B2G2B1 owner revision source contract is consistent.");
