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

const ownerPath = "crates/rustok-notifications/src/inbox_reconcile.rs";
const surfacePath = "crates/rustok-notifications/src/lib.rs";
const queryPath = "apps/server/src/graphql/forum_notification_reconciliation.rs";
const graphqlModPath = "apps/server/src/graphql/mod.rs";
const schemaPath = "apps/server/src/graphql/schema.rs";
const packetPath =
  "docs/modules/forum-33-notification-reconciliation-status-actualization-2026-08-09.md";

const owner = read(ownerPath);
const surface = read(surfacePath);
const query = read(queryPath);
const graphqlMod = read(graphqlModPath);
const schema = read(schemaPath);
const packet = read(packetPath);

for (const marker of [
  "pub struct NotificationInboxReconcileInspectionPage",
  "pub scanned: u16",
  "pub unavailable: u16",
  "pub next_cursor: Option<String>",
  "pub has_more: bool",
  "async fn load_raw_page(",
  "notification::Entity::find()",
  ".filter(notification::Column::TenantId.eq(request.tenant_id))",
  ".filter(notification::Column::RecipientId.eq(request.recipient_id))",
  ".filter(notification::Column::State.ne(NotificationState::Archived))",
  ".order_by_desc(notification::Column::CreatedAt)",
  ".order_by_desc(notification::Column::Id)",
  ".limit(limit + 1)",
  "decode_inbox_cursor",
  "encode_inbox_cursor",
  "pub async fn inspect_page(",
  "validate_request(&request)?",
  "let raw = load_raw_page(&self.db, &request).await?",
  ".authorize_open(NotificationInboxOpenRequest {",
  "NotificationInboxOpenDecision::Unavailable => unavailable += 1",
  "pub async fn reconcile_page(",
  "self.state.archive(identity).await?",
]) {
  requireText(owner, marker, `${ownerPath}: missing ${marker}`);
}

const inspectStart = owner.indexOf("pub async fn inspect_page(");
const reconcileStart = owner.indexOf("pub async fn reconcile_page(");
if (inspectStart < 0 || reconcileStart <= inspectStart) {
  throw new Error(`${ownerPath}: inspect/reconcile method ordering is invalid`);
}
const inspect = owner.slice(inspectStart, reconcileStart);
for (const forbidden of [
  ".archive(",
  "mark_seen",
  "mark_read",
  "mark_unread",
  "delivery_attempt",
  "ActiveModel",
  "UPDATE ",
  "INSERT ",
  "DELETE ",
]) {
  requireAbsent(inspect, forbidden, `${ownerPath}: inspect_page must not contain ${forbidden}`);
}

for (const marker of [
  "NotificationInboxReconcileInspectionPage",
  "NotificationInboxReconcilePage",
  "NotificationInboxReconcileRequest",
  "NotificationInboxReconcileService",
]) {
  requireText(surface, marker, `${surfacePath}: missing ${marker}`);
}

for (const marker of [
  "pub struct ForumNotificationReconciliationQuery",
  "pub struct GqlForumNotificationReconciliationStatus",
  "forum_notification_reconciliation_status",
  "recipient_id: Uuid",
  "require_module_enabled(ctx, FORUM_MODULE_SLUG).await?",
  "require_module_enabled(ctx, NOTIFICATIONS_MODULE_SLUG).await?",
  "auth.tenant_id != tenant.id",
  "Permission::SETTINGS_READ",
  "Permission::FORUM_CATEGORIES_MANAGE",
  "Permission::FORUM_TOPICS_MANAGE",
  "settings_read && categories_manage && topics_manage",
  "Arc<ModuleRuntimeExtensions>",
  "Arc<NotificationSourceRegistry>",
  "NotificationRecipientPolicyRuntime",
  "NotificationInboxReconcileService::new",
  ".inspect_page(NotificationInboxReconcileRequest {",
  "tenant_id: tenant.id",
  "recipient_id",
  "scanned: u64::from(page.scanned)",
  "unavailable: u64::from(page.unavailable)",
  "clean: page.unavailable == 0",
  '"notification_reconciliation_status"',
  '"forum.notification_reconciliation_status"',
]) {
  requireText(query, marker, `${queryPath}: missing ${marker}`);
}

for (const forbidden of [
  "notification::Entity",
  "search_projection_",
  "forum_projection_revision_ledger",
  ".reconcile_page(",
  ".archive(",
  "delivery_attempt",
  "UPDATE ",
  "INSERT ",
  "DELETE ",
]) {
  requireAbsent(query, forbidden, `${queryPath}: forbidden ${forbidden}`);
}

for (const marker of [
  '#[cfg(all(feature = "mod-forum", feature = "mod-notifications"))]',
  "pub mod forum_notification_reconciliation;",
]) {
  requireText(graphqlMod, marker, `${graphqlModPath}: missing ${marker}`);
}

for (const marker of [
  "use super::forum_notification_reconciliation::ForumNotificationReconciliationQuery;",
  "ForumNotificationReconciliationQuery,",
]) {
  requireText(schema, marker, `${schemaPath}: missing ${marker}`);
}

for (const marker of [
  "FORUM-33G",
  "FORUM-33F",
  "Attachments remain blocked on FORUM-14",
  "NotificationInboxReconcileService::inspect_page",
  "scanned",
  "unavailable",
  "forumNotificationReconciliationStatus",
  "settings:read",
  "forum_categories:manage",
  "forum_topics:manage",
  "page-local",
  "existing Notifications `reconcile_page` remains the durable archive owner",
  "no Cargo command",
]) {
  requireText(packet, marker, `${packetPath}: missing ${marker}`);
}

console.log("Forum FORUM-33G notification reconciliation status source: ok");
