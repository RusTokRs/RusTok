#!/usr/bin/env node

import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function requireText(text, marker, message) {
  if (!text.includes(marker)) throw new Error(message);
}

function requireAbsent(text, marker, message) {
  if (text.includes(marker)) throw new Error(message);
}

const queryPath = "crates/rustok-search/src/graphql/forum_projection_reconciliation.rs";
const graphqlModPath = "crates/rustok-search/src/graphql/mod.rs";
const serverSchemaPath = "apps/server/src/graphql/schema.rs";
const packetPath =
  "docs/modules/forum-33-shared-search-reconciliation-actualization-2026-08-09.md";

const query = read(queryPath);
const graphqlMod = read(graphqlModPath);
const serverSchema = read(serverSchemaPath);
const packet = read(packetPath);

for (const marker of [
  "pub struct ForumSearchProjectionReconciliationQuery",
  "forum_search_projection_reconciliation_status",
  "GqlForumSearchProjectionReconciliationStatus",
  "GqlForumSearchProjectionDrift",
  "CheckpointBehind",
  "CheckpointAhead",
  "CheckpointEventMismatch",
  "NonTerminalInboxWork",
  '"checkpoint_behind"',
  '"checkpoint_ahead"',
  '"checkpoint_event_mismatch"',
  '"non_terminal_inbox_work"',
  "require_module_enabled(ctx, SEARCH_MODULE_SLUG).await?",
  "require_module_enabled(ctx, FORUM_MODULE_SLUG).await?",
  "auth.tenant_id != tenant.id",
  "Permission::SETTINGS_READ",
  "Permission::FORUM_CATEGORIES_MANAGE",
  "Permission::FORUM_TOPICS_MANAGE",
  "settings_read && categories_manage && topics_manage",
  "Arc<ModuleRuntimeExtensions>",
  "SharedForumProjectionOwnerRevisionSourcePort",
  "resolve_forum_projection_owner_revisions",
  "after_owner_revision",
  "limit: 1",
  "search_projection_owner_checkpoints",
  "search_projection_inbox",
  "status IN ('pending', 'processing', 'retryable_error')",
  "IsolationLevel::RepeatableRead",
  "AccessMode::ReadOnly",
  "DbBackend::Postgres",
  '"forum_projection_reconciliation_status"',
  '"search.forum_projection_reconciliation_status"',
  "checkpoint_revision: String",
  "next_owner_revision: Option<String>",
]) {
  requireText(query, marker, `${queryPath}: missing ${marker}`);
}

for (const forbidden of [
  "forum_projection_revision_ledger",
  "UPDATE search_",
  "INSERT INTO search_",
  "DELETE FROM search_",
  "UPDATE forum_",
  "INSERT INTO forum_",
  "DELETE FROM forum_",
  "ActiveModel",
  " OFFSET ",
  "rebuild_tenant",
  "advance_checkpoint",
  "ForumEventService",
]) {
  requireAbsent(query, forbidden, `${queryPath}: forbidden ${forbidden}`);
}

for (const marker of [
  "mod forum_projection_reconciliation;",
  "ForumSearchProjectionReconciliationQuery",
  "GqlForumSearchProjectionDrift",
  "GqlForumSearchProjectionReconciliationStatus",
]) {
  requireText(graphqlMod, marker, `${graphqlModPath}: missing ${marker}`);
}

for (const marker of [
  "ForumSearchProjectionReconciliationQuery, ForumStorefrontSearchQuery",
  "#[cfg(feature = \"mod-forum\")] ForumSearchProjectionReconciliationQuery",
  ".data(runtime_extensions)",
]) {
  requireText(serverSchema, marker, `${serverSchemaPath}: missing ${marker}`);
}

for (const marker of [
  "FORUM-33F",
  "FORUM-33E",
  "attachments remain **blocked on FORUM-14**",
  "ForumProjectionOwnerRevisionSourcePort",
  "search_projection_owner_checkpoints",
  "search_projection_inbox",
  "forumSearchProjectionReconciliationStatus",
  "after_owner_revision = N - 1, limit = 1",
  "after_owner_revision = N, limit = 1",
  "checkpoint_behind",
  "checkpoint_ahead",
  "checkpoint_event_mismatch",
  "non_terminal_inbox_work",
  "settings:read",
  "forum_categories:manage",
  "forum_topics:manage",
  "REPEATABLE READ READ ONLY",
  "diagnostic convergence observation",
  "permitted shared-owner projection diagnostics and non-duplicative operational metrics",
]) {
  requireText(packet, marker, `${packetPath}: missing ${marker}`);
}

console.log("Forum FORUM-33F shared Search reconciliation source: ok");
