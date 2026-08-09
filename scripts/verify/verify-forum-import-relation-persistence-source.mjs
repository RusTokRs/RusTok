#!/usr/bin/env node

import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function need(text, marker, label) {
  if (!text.includes(marker)) throw new Error(`${label}: missing ${marker}`);
}

function forbid(text, marker, label) {
  if (text.includes(marker)) throw new Error(`${label}: forbidden ${marker}`);
}

const files = {
  lib: "crates/rustok-forum/src/lib.rs",
  mentions: "crates/rustok-forum/src/mentions.rs",
  mentionsImport: "crates/rustok-forum/src/mentions_import.rs",
  relationPreparation: "crates/rustok-forum/src/import_relation_preparation.rs",
  relationOwner: "crates/rustok-forum/src/services/mention_relation.rs",
  relationImport: "crates/rustok-forum/src/services/mention_relation_import.rs",
  services: "crates/rustok-forum/src/services/mod.rs",
  packet: "docs/modules/forum-34-import-relation-persistence-actualization-2026-08-09.md",
};

const source = Object.fromEntries(
  Object.entries(files).map(([key, path]) => [key, read(path)]),
);

for (const marker of [
  'pub mod mentions {',
  'include!("mentions_import.rs");',
  'include!("mentions.rs");',
]) need(source.lib, marker, "forum import mention include wiring");
if (source.lib.indexOf('include!("mentions_import.rs");') > source.lib.indexOf('include!("mentions.rs");')) {
  throw new Error("forum import mention include must precede mentions.rs");
}

for (const marker of [
  'mod mention_relation {',
  'include!("mention_relation_import.rs");',
  'include!("mention_relation.rs");',
  'pub(crate) use mention_relation::MentionRelationService;',
]) need(source.services, marker, "forum import relation persistence wiring");
if (source.services.indexOf('include!("mention_relation_import.rs");') > source.services.indexOf('include!("mention_relation.rs");')) {
  throw new Error("forum import relation persistence include must precede mention_relation.rs");
}

for (const marker of [
  'pub(crate) fn from_import_admission(',
  'ProfileService::normalize_handle',
  'handle must already be normalized',
  'maps multiple handles onto one user',
  'FORUM_MAX_MENTION_TARGETS_PER_REVISION',
  'ResolvedForumMention { user_id, handle }',
]) need(source.mentionsImport, marker, "FORUM-34N admitted mention constructor");
for (const marker of [
  'ProfilesReader',
  'resolve_forum_mentions(',
  '.await',
  'Uuid::new_v4',
]) forbid(source.mentionsImport, marker, "admitted mention constructor boundary");

for (const marker of [
  'pub(crate) async fn persist_import_admitted_in_tx(',
  'txn: &DatabaseTransaction',
  'ForumPreparedImportContentRelations',
  'ForumImportRelationEventMode',
  'validate_import_relation_source(relation)?;',
  'normalize_locale_tag(&relation.locale)',
  'extract_forum_mention_candidates(document, policy)',
  'ForumResolvedMentions::from_import_admission(',
  'projection_fingerprint(',
  'PreparedMentionRelations {',
  'persistence.persist_in_tx(txn, prepared).await?',
  'self.persist_in_tx(txn, prepared).await?',
  'event_bus: None',
  'EmitAddedTargetEvents',
  'SuppressAddedTargetEvents',
  'does not support admitted quote revisions yet',
]) need(source.relationImport, marker, "FORUM-34N relation persistence bridge");

for (const marker of [
  'SecurityContext',
  'resolve_forum_mentions(',
  'ProfileService::',
  'DatabaseConnection',
  'TransactionTrait',
  '.begin(',
  '.commit(',
  '.insert(',
  'ActiveModel',
  'Uuid::new_v4',
]) forbid(source.relationImport, marker, "relation bridge single-owner boundary");

for (const marker of [
  'pub(crate) async fn persist_in_tx(',
  'lock_source_in_tx(txn, prepared.tenant_id, prepared.target).await?;',
  'ensure_prepared_matches_source_in_tx(txn, &prepared).await?;',
  'latest_revision_in_tx',
  'projection_fingerprint',
  'publish_added_target_events_in_tx',
]) need(source.relationOwner, marker, "existing relation owner persistence baseline");

for (const marker of [
  'pub enum ForumImportRelationMode',
  'SuppressRelations',
  'MaterializeRelations',
  'pub enum ForumImportRelationEventMode',
  'SuppressAddedTargetEvents',
  'EmitAddedTargetEvents',
  'pub struct ForumPreparedImportContentRelations',
]) need(source.relationPreparation, marker, "34M relation admission baseline");

for (const marker of [
  'FORUM_MAX_MENTION_TARGETS_PER_REVISION: usize = 32',
  'pub struct ForumResolvedMentions',
  'pub fn extract_forum_mention_candidates',
]) need(source.mentions, marker, "Forum mention owner baseline");

for (const marker of [
  'FORUM-34N',
  'FORUM-34A through FORUM-34M',
  'owner-internal bridge',
  'does not infer current visibility/status or historical permissions',
  'calls the already-established `MentionRelationService::persist_in_tx`',
  'writes no relation revision and returns `None`',
  '`SuppressAddedTargetEvents`',
  'first FORUM-34 import slice that can cause persistence',
  'No complete category/topic/reply import batch has been persisted yet',
  'FORUM-34O',
  'no tests, Cargo commands',
]) need(source.packet, marker, "FORUM-34N packet");

console.log("Forum FORUM-34N owner import relation persistence source: ok");
