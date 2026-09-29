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

const ownerPath = "crates/modules/rustok-notifications/src/inbox_reconcile.rs";
const surfacePath = "crates/modules/rustok-notifications/src/lib.rs";
const apiPath = "crates/modules/rustok-notifications-api/src/reconciliation.rs";
const forumQueryPath = "crates/modules/rustok-forum/src/graphql/reconciliation_query.rs";
const forumRuntimePath = "crates/modules/rustok-forum/src/graphql/runtime_data.rs";
const forumGraphqlModPath = "crates/modules/rustok-forum/src/graphql/mod.rs";
const serverShimPath = "apps/server/src/graphql/forum_notification_reconciliation.rs";
const serverGraphqlModPath = "apps/server/src/graphql/mod.rs";
const schemaPath = "apps/server/src/graphql/schema.rs";
const packetPath =
  "docs/modules/forum-33-notification-reconciliation-status-actualization-2026-08-09.md";

const owner = read(ownerPath);
const surface = read(surfacePath);
const api = read(apiPath);
const forumQuery = read(forumQueryPath);
const forumRuntime = read(forumRuntimePath);
const forumGraphqlMod = read(forumGraphqlModPath);
const serverShim = fs.existsSync(serverShimPath) ? read(serverShimPath) : "";
const serverGraphqlMod = read(serverGraphqlModPath);
const schema = read(schemaPath);
const packet = read(packetPath);

for (const marker of [
  "pub struct NotificationInboxReconcileInspectionPage",
  "pub async fn inspect_page(",
  "validate_request(&request)?",
  "let raw = self.load_raw_page(&request).await?",
  ".authorize_open(NotificationInboxOpenRequest {",
  "NotificationInboxOpenDecision::Unavailable => unavailable += 1",
  "pub async fn reconcile_page(",
  "self.state.archive(identity).await?",
]) {
  requireText(owner, marker, ownerPath + ": missing " + marker);
}

const inspectStart = owner.indexOf("pub async fn inspect_page(");
const reconcileStart = owner.indexOf("pub async fn reconcile_page(");
if (inspectStart < 0 || reconcileStart <= inspectStart) {
  throw new Error(ownerPath + ": inspect/reconcile method ordering is invalid");
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
  requireAbsent(inspect, forbidden, ownerPath + ": inspect_page must not contain " + forbidden);
}

for (const marker of [
  "NotificationInboxReconciliationInspectRequest",
  "NotificationInboxReconciliationInspectPage",
  "NotificationInboxReconciliationInspectPort",
  "NotificationInboxReconciliationInspectPortFactory",
]) {
  requireText(api, marker, apiPath + ": missing " + marker);
}

for (const marker of [
  "impl NotificationInboxReconciliationInspectPort for NotificationInboxReconcileService",
  "context.require_policy(PortCallPolicy::read())?",
  "NotificationInboxReconciliationInspectPortFactory for",
  "host.shared_get::<Arc<NotificationSourceRegistry>>()",
  "host.shared_get::<crate::NotificationRecipientPolicyRuntime>()",
  "reconciliation_error_to_port_error",
]) {
  requireText(owner, marker, ownerPath + ": missing " + marker);
}

for (const marker of [
  "ensure_notification_source_registry(extensions)",
  "NotificationInboxReconciliationInspectPortFactory",
  "NotificationInboxReconciliationInspectPortFactoryImpl",
]) {
  requireText(surface, marker, surfacePath + ": missing " + marker);
}

for (const marker of [
  "pub struct GqlForumNotificationReconciliationStatus",
  "forum_notification_reconciliation_status",
  "require_module_enabled(ctx, MODULE_SLUG).await?",
  "require_module_enabled(ctx, \"notifications\").await?",
  "require_operations_permissions(auth)?",
  "auth.tenant_id != tenant.id",
  "notification_reconciliation_port()",
  "NotificationInboxReconciliationInspectRequest",
  ".inspect_page(",
  "tenant.id.to_string()",
  "PortActor::user(auth.user_id.to_string())",
  ".with_deadline(Duration::from_secs(5))",
  '"operator"',
  '"forum.notification_reconciliation_status"',
]) {
  requireText(
    forumQuery,
    marker,
    forumQueryPath + ": missing Forum-owned notification reconciliation marker " + marker,
  );
}

for (const forbidden of [
  "notification::Entity",
  "NotificationInboxReconcileService::new",
  "NotificationRecipientPolicyRuntime",
  "NotificationSourceRegistry",
  "UPDATE ",
  "INSERT ",
  "DELETE ",
  ".archive(",
  ".reconcile_page(",
  "Permission::SETTINGS_READ",
]) {
  requireAbsent(
    forumQuery,
    forbidden,
    forumQueryPath + ": must not cross the Notifications persistence/owner boundary via " + forbidden,
  );
}

for (const marker of [
  "NotificationInboxReconciliationInspectPort",
  "NotificationInboxReconciliationInspectPortFactory",
  "notification_reconciliation: Option<Arc<dyn NotificationInboxReconciliationInspectPort>>",
  "inputs.shared_get::<Arc<dyn NotificationInboxReconciliationInspectPortFactory>>()",
  "factory.build(inputs.host())",
  "notification_reconciliation_port",
]) {
  requireText(forumRuntime, marker, forumRuntimePath + ": missing " + marker);
}

for (const marker of [
  "GqlForumNotificationReconciliationStatus",
]) {
  requireText(
    forumGraphqlMod,
    marker,
    forumGraphqlModPath + ": missing GraphQL status type export " + marker,
  );
}

for (const marker of [
  "forum notification reconciliation GraphQL must not be owned by apps/server",
]) {
  requireText(
    "forum notification reconciliation GraphQL must not be owned by apps/server",
    marker,
    "internal guard wording check",
  );
}

requireAbsent(
  serverShim,
  "ForumNotificationReconciliationQuery",
  serverShimPath + ": server-owned notification reconciliation shim must be removed",
);
requireAbsent(
  serverGraphqlMod,
  "forum_notification_reconciliation",
  serverGraphqlModPath + ": server GraphQL module must not mount a notification reconciliation shim",
);
requireAbsent(
  schema,
  "ForumNotificationReconciliationQuery",
  schemaPath + ": schema must not compose a server-owned notification reconciliation shim",
);

for (const marker of [
  "FORUM-33G",
  "Attachments remain blocked on FORUM-14",
  "NotificationInboxReconcileService::inspect_page",
  "NotificationInboxReconciliationInspectPort",
  "forumNotificationReconciliationStatus",
  "forum_categories:manage",
  "forum_topics:manage",
  "page-local",
  "existing Notifications `reconcile_page` remains the durable archive owner",
  "no Cargo command",
]) {
  requireText(packet, marker, packetPath + ": missing " + marker);
}

console.log("Forum FORUM-33G notification reconciliation status source: owner-port-composed");
