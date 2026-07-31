#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];
const paths = {
  contract: "crates/rustok-forum/contracts/forum-search-owner-revision-source.json",
  note: "crates/rustok-forum/docs/forum-23b2g2a-search-owner-revision-source.md",
  forumDto: "crates/rustok-forum/src/dto/event.rs",
  forumOwner: "crates/rustok-forum/src/services/event.rs",
  forumJournal: "crates/rustok-forum/src/entities/forum_domain_event.rs",
  forumMigrations: "crates/rustok-forum/src/migrations/mod.rs",
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
const note = read(paths.note);
const forumDto = read(paths.forumDto);
const forumOwner = read(paths.forumOwner);
const forumJournal = read(paths.forumJournal);
const forumMigrations = read(paths.forumMigrations);
const searchOwner = read(paths.searchOwner);
const searchLib = read(paths.searchLib);
const searchInbox = read(paths.searchInbox);
const searchMigrations = read(paths.searchMigrations);
const hostAdapter = read(paths.hostAdapter);
const hostComposition = read(paths.hostComposition);
const rootEvents = read(paths.rootEvents);

requireAll(forumJournal, [
  'table_name = "forum_domain_events"',
  "pub sequence_no: i64",
  "pub event_id: Uuid",
], paths.forumJournal);

requireAll(forumDto, [
  "pub enum ForumProjectionOwnerRevisionImpact",
  "FullRebuild",
  "NoProjectionChange",
  "pub struct ForumProjectionOwnerRevisionResponse",
  "pub owner_revision: i64",
], paths.forumDto);

requireAll(forumOwner, [
  "MAX_FORUM_PROJECTION_OWNER_REVISION_PAGE: usize = 100",
  "pub async fn list_projection_owner_revisions",
  "after_owner_revision",
  "SequenceNo.gt(after_owner_revision)",
  "order_by_asc(forum_domain_event::Column::SequenceNo)",
  "projection_revision_impact",
  '"forum.topic.vote_changed"',
  '"forum.mention.audience_added"',
  "Unknown future Forum journal types fail safe",
], paths.forumOwner);
rejectAll(forumOwner, [
  "SearchProjection",
  "search_projection_inbox",
  "search_projection_watermarks",
], paths.forumOwner);

requireAll(searchOwner, [
  "pub trait ForumProjectionOwnerRevisionSourcePort",
  "pub type SharedForumProjectionOwnerRevisionSourcePort",
  "pub async fn resolve_forum_projection_owner_revisions",
  "DEFAULT_FORUM_OWNER_REVISION_PAGE_LIMIT: usize = 64",
  "MAX_FORUM_OWNER_REVISION_PAGE_LIMIT: usize = 100",
  "owner revisions must be strictly increasing",
  "gaps are valid",
  "owner_revision_port_requires_host_composition",
  "owner_revision_page_rejects_reordered_or_replayed_rows",
], paths.searchOwner);
requireAll(searchLib, [
  "ForumProjectionOwnerRevisionSourcePort",
  "SharedForumProjectionOwnerRevisionSourcePort",
  "resolve_forum_projection_owner_revisions",
], paths.searchLib);
rejectAll(searchOwner, [
  "forum_domain_event::",
  "forum_domain_events",
  "rustok_forum",
  "UPDATE search_projection_watermarks",
  "INSERT INTO search_projection_watermarks",
], paths.searchOwner);

requireAll(hostAdapter, [
  "ServerForumProjectionOwnerRevisionSourcePort",
  "ForumEventService::new",
  "list_projection_owner_revisions",
  "ForumOwnerRevisionImpact::FullRebuild",
  "ForumProjectionOwnerRevisionImpact::NoProjectionChange",
], paths.hostAdapter);
rejectAll(hostAdapter, [
  "forum_domain_event::",
  "forum_domain_events",
  "search_projection_watermarks",
], paths.hostAdapter);
requireAll(hostComposition, [
  'mod forum_search_owner_revision {',
  "ServerForumProjectionOwnerRevisionSourcePort::shared",
  "extensions.insert(owner_revision);",
  "SharedForumProjectionOwnerRevisionSourcePort",
], paths.hostComposition);

requireAll(searchInbox, [
  "ingest_sequence",
  "ORDER BY ingest_sequence ASC",
], `${paths.searchInbox} G1 boundary`);
rejectAll(searchInbox, [
  "owner_revision",
  "forum_domain_events",
], `${paths.searchInbox} G2A non-consumer boundary`);
rejectAll(rootEvents, [
  "ForumProjectionOwnerRevision",
  "forum_projection_owner_revision",
], `${paths.rootEvents} root event boundary`);
rejectAll([forumMigrations, searchMigrations].join("\n"), [
  "owner_revision_source",
  "projection_owner_revision",
], "migration boundary");

requireAll(note, [
  "# FORUM-23B2G2A Search owner-revision source",
  "forum_domain_events.sequence_no",
  "global while reads are tenant-scoped",
  "does not yet connect it to the background sweeper",
  "did not run these commands",
], paths.note);

if (contract) {
  if (contract.task !== "FORUM-23B2G2A") {
    failures.push(`${paths.contract}: unexpected task`);
  }
  if (contract.status !== "source_complete_consumer_reconciliation_pending") {
    failures.push(`${paths.contract}: unexpected status`);
  }
  if (contract.owner_clock?.field !== "sequence_no") {
    failures.push(`${paths.contract}: owner clock must reuse sequence_no`);
  }
  if (contract.owner_clock?.new_counter_added !== false) {
    failures.push(`${paths.contract}: a second owner counter is forbidden`);
  }
  if (contract.owner_clock?.migration_added !== false) {
    failures.push(`${paths.contract}: G2A must not add a migration`);
  }
  if (contract.forum_owner?.payload_exposed !== false) {
    failures.push(`${paths.contract}: owner journal payload must remain private`);
  }
  if (contract.search_contract?.independent_from_search_ingest_sequence !== true) {
    failures.push(`${paths.contract}: owner and ingest sequences must remain independent`);
  }
  if (contract.host_composition?.direct_search_read_of_forum_domain_events !== false) {
    failures.push(`${paths.contract}: Search must not read the Forum journal directly`);
  }
}

if (failures.length > 0) {
  console.error("FORUM-23B2G2A owner revision source verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("FORUM-23B2G2A owner revision source contract is consistent.");
