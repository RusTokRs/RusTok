#!/usr/bin/env node

import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function requireContains(text, needle, message) {
  if (!text.includes(needle)) throw new Error(message);
}

function requireAbsent(text, needle, message) {
  if (text.includes(needle)) throw new Error(message);
}

const contractPath =
  "crates/rustok-forum/contracts/evidence/forum-page-builder-runtime-authorization-execution-contract.json";
const runnerPath = "scripts/evidence/forum-page-builder-runtime-authorization-evidence.mjs";
const previewTransportPath = "crates/rustok-forum/admin/src/widget_preview_transport.rs";
const propertyTransportPath = "crates/rustok-forum/admin/src/widget_property_transport.rs";
const apiRuntimePath = "crates/rustok-api/src/runtime.rs";
const authPath = "crates/rustok-api/src/context/auth.rs";
const widgetPreviewPath = "crates/rustok-forum/src/services/widget_preview.rs";
const topicVisibilityPath = "crates/rustok-forum/src/services/topic_visibility.rs";
const categoryVisibilityPath = "crates/rustok-forum/src/services/category_visibility.rs";
const visibilityTestPath = "crates/rustok-forum/tests/page_builder_widget_visibility_sqlite.rs";
const packetPath =
  "docs/modules/forum-page-builder-runtime-authorization-evidence-actualization-2026-08-08.md";

const contractSource = read(contractPath);
const contract = JSON.parse(contractSource);
const runner = read(runnerPath);
const previewTransport = read(previewTransportPath);
const propertyTransport = read(propertyTransportPath);
const apiRuntime = read(apiRuntimePath);
const auth = read(authPath);
const widgetPreview = read(widgetPreviewPath);
const topicVisibility = read(topicVisibilityPath);
const categoryVisibility = read(categoryVisibilityPath);
const visibilityTest = read(visibilityTestPath);
const packet = read(packetPath);

if (contract.status !== "source_ready_maintainer_execution_pending") {
  throw new Error("runtime evidence contract must not claim execution");
}
if (contract.runner !== runnerPath) {
  throw new Error("runtime evidence contract must point to the retained runner");
}
if (contract.output?.format !== "forum_page_builder_runtime_authorization_execution_v1") {
  throw new Error("runtime evidence format drifted");
}
if (contract.output?.status !== "runtime_authorization_execution_passed_wave_pending") {
  throw new Error("runtime evidence success status must keep Wave pending");
}
const expectedCommandIds = [
  "transport_authorization",
  "tenant_module_state",
  "owner_moderator_policy",
  "owner_visibility_sqlite",
];
if (JSON.stringify(contract.commands?.map((command) => command.id)) !== JSON.stringify(expectedCommandIds)) {
  throw new Error("runtime evidence command matrix drifted");
}
for (const command of contract.commands ?? []) {
  if (command.program !== "cargo" || command.args?.[0] !== "test") {
    throw new Error(`runtime evidence command ${command.id} must remain cargo test`);
  }
}
for (const pending of [
  "runtime authorization execution",
  "browser execution",
  "deployed server-function transport attestation",
  "observed Page Builder Wave",
  "provider SLO health",
]) {
  if (!contract.not_claimed?.includes(pending)) {
    throw new Error(`runtime evidence contract must keep ${pending} pending`);
  }
}

for (const marker of [
  'spawnSync("git", ["rev-parse", "HEAD"]',
  'command.program !== "cargo"',
  'command.args[0] !== "test"',
  "shell: false",
  "encoding: null",
  "MAX_CAPTURE_BYTES",
  "rmSync(output, { force: true })",
  "sourceCommit = currentCommit()",
  "sourceHashes(contract)",
  "stdout: stdout",
  "stderr: stderr",
  "retained_raw_command_output: false",
  "runtime_authorization_execution_only: true",
  "deployed_server_fn_attestation_pending: true",
  "browser_execution_pending: true",
  "provider_slo_health_unobserved: true",
  "observed_page_builder_wave_pending: true",
]) {
  requireContains(runner, marker, `runtime evidence runner missing ${marker}`);
}
for (const forbidden of [
  "execSync(",
  "execFileSync(",
  "shell: true",
  "stdout.toString",
  "stderr.toString",
  "stdout_text",
  "stderr_text",
]) {
  requireAbsent(runner, forbidden, `runtime evidence runner must not retain raw output through ${forbidden}`);
}

for (const marker of [
  "pub(crate) fn require_forum_transport_authorization",
  "require_tenant_scope(auth, tenant)?",
  "has_any_effective_permission",
  "Permission::FORUM_TOPICS_READ",
  "require_forum_transport_authorization(&auth, &tenant)?",
  'is_tenant_module_enabled(host.db(), tenant_id, "forum")',
  "transport_authorization_accepts_exact_read_and_effective_manage",
  "transport_authorization_rejects_missing_read_and_cross_tenant_context",
  "module_state_fails_closed_for_disabled_and_unavailable_states",
]) {
  requireContains(previewTransport, marker, `preview transport missing runtime authorization marker: ${marker}`);
}
for (const marker of [
  "require_forum_transport_authorization",
  "require_forum_transport_authorization(&auth, &tenant)?",
  "require_forum_module_enabled(&host, tenant.id).await",
]) {
  requireContains(propertyTransport, marker, `property transport missing shared authorization marker: ${marker}`);
}
requireAbsent(
  propertyTransport,
  "has_any_effective_permission",
  "property transport must not keep a second effective-permission implementation",
);
requireAbsent(
  propertyTransport,
  "require_tenant_scope(&auth, &tenant)",
  "property transport must use the shared tenant/permission gate",
);

for (const marker of [
  "permissions.contains(required)",
  "Action::Manage",
  "has_any_effective_permission",
]) {
  requireContains(auth, marker, `generic effective-permission source missing ${marker}`);
}
for (const marker of [
  "tenant_module_enablement_is_exact_and_fail_closed",
  'Database::connect("sqlite::memory:")',
  "CREATE TABLE tenant_modules",
  "enabled = 1",
  'is_tenant_module_enabled(&db, tenant_id, "forum")',
  'is_tenant_module_enabled(&db, tenant_id, "pages")',
]) {
  requireContains(apiRuntime, marker, `tenant module runtime evidence missing ${marker}`);
}

for (const marker of [
  "fn reply_stream_preview_statuses",
  "PermissionScope::None",
  "forum_replies:moderate",
  "APPROVED_PREVIEW_REPLY_STATUSES",
  "MODERATOR_PREVIEW_REPLY_STATUSES",
  "non_approved_reply_stream_requires_effective_moderation_scope",
  "!statuses.contains(&ReplyStatus::Deleted)",
]) {
  requireContains(widgetPreview, marker, `Forum owner moderator evidence missing ${marker}`);
}
requireAbsent(
  widgetPreview.split("const MODERATOR_PREVIEW_REPLY_STATUSES")[1]?.split("];", 1)[0] ?? "",
  "ReplyStatus::Deleted",
  "moderator preview status set must not include deleted tombstones",
);

for (const marker of [
  "filter_visible_topic_ids",
  "hidden_category_ids_for_scope",
  "forum_topic::Column::TenantId.eq(tenant_id)",
  "forum_topic::Column::Status.eq(TopicStatus::Open)",
]) {
  requireContains(topicVisibility, marker, `topic visibility owner path missing ${marker}`);
}
for (const marker of [
  "hidden_category_ids_for_viewer",
  "ForumCategoryVisibility::Authenticated",
  "if is_authenticated",
]) {
  requireContains(categoryVisibility, marker, `category visibility owner path missing ${marker}`);
}
for (const marker of [
  'Database::connect("sqlite::memory:")',
  "forum_category_policies",
  "visibility_override",
  "authenticated",
  "ForumTopicVisibilityService::new",
  "filter_visible_topic_ids",
  "ForumTopicVisibilityScope::storefront(None)",
  "ForumTopicVisibilityScope::storefront_for_viewer(None, true)",
  "closed_public_topic",
  "foreign_topic",
]) {
  requireContains(visibilityTest, marker, `SQLite visibility evidence missing ${marker}`);
}

for (const marker of [
  "Status: `source-ready / maintainer-runtime-execution-pending / browser-execution-pending / wave-pending`",
  "forum_page_builder_runtime_authorization_execution_v1",
  "runtime_authorization_execution_passed_wave_pending",
  "manage -> read",
  "forum_replies:moderate",
  "Deleted",
  "deployed server-function transport attestation",
  "No runtime, Cargo, browser, database or verifier execution is claimed",
]) {
  requireContains(packet, marker, `runtime authorization actualization missing ${marker}`);
}

console.log("Forum Page Builder runtime authorization evidence source: ok");
