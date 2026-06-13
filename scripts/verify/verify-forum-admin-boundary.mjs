#!/usr/bin/env node
// RusTok forum admin FFA boundary guardrails.
// Fast source-level checks for the module-owned core/transport/ui split.

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function readRepo(relativePath) {
  return readFileSync(repoPath(relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

function assertExists(relativePath, description) {
  if (!existsSync(repoPath(relativePath))) fail(description);
}

function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) fail(description);
}

function assertNotContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) fail(description);
}

const libPath = "crates/rustok-forum/admin/src/lib.rs";
const corePath = "crates/rustok-forum/admin/src/core.rs";
const uiPath = "crates/rustok-forum/admin/src/ui/leptos.rs";
const transportPath = "crates/rustok-forum/admin/src/transport.rs";
const apiPath = "crates/rustok-forum/admin/src/api.rs";
const implementationPlanPath = "crates/rustok-forum/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";

for (const filePath of [libPath, corePath, uiPath, transportPath, apiPath, implementationPlanPath, registryPath]) {
  assertExists(filePath, `${filePath}: expected forum admin FFA boundary file`);
}

const lib = readRepo(libPath);
const core = readRepo(corePath);
const ui = readRepo(uiPath);
const transport = readRepo(transportPath);
const api = readRepo(apiPath);
const implementationPlan = readRepo(implementationPlanPath);
const registry = readRepo(registryPath);

assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub use ui::leptos::ForumAdmin;", `${libPath}: crate root must re-export ForumAdmin`);

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server", "LocalResource", "WriteSignal", "web_sys::"]) {
  assertNotContains(core, marker, `${corePath}: core must stay Leptos/server-function free (${marker})`);
}
for (const marker of [
  "CategoryFormSnapshot",
  "TopicFormSnapshot",
  "ForumAdminRouteQueryIntent",
  "ForumAdminDeleteOutcome",
  "forum_admin_delete_outcome",
  "forum_admin_busy_key",
  "ForumAdminBusySurface",
  "ForumAdminFormErrorLabels",
  "ForumAdminCategorySelectOption",
  "category_select_options",
  "forum_admin_topic_tag_count_label",
  "forum_admin_editing_thread_label",
  "forum_admin_position_value",
  "forum_admin_sidebar_category_class",
  "forum_admin_status_badge_class",
  "forum_admin_tag_chips",
  "forum_admin_form_error_message",
  "forum_admin_transport_error_message",
  "selected_category_filter_label",
  "forum_admin_collection_state",
  "category_card_view_model",
  "topic_card_view_model",
]) {
  assertContains(core, marker, `${corePath}: expected core-owned FFA helper ${marker}`);
}

assertContains(ui, "use crate::core::{", `${uiPath}: Leptos adapter must import core-owned helpers`);
assertContains(ui, "use crate::transport;", `${uiPath}: Leptos adapter must call the module-owned transport facade`);
assertContains(ui, "forum_admin_busy_key", `${uiPath}: UI must consume core-owned busy-key construction`);
assertContains(ui, "forum_admin_form_error_message", `${uiPath}: UI must consume core-owned form error policy`);
assertContains(ui, "forum_admin_transport_error_message", `${uiPath}: UI must consume core-owned transport error formatting`);
assertContains(ui, "category_select_options", `${uiPath}: UI must consume core-owned category select options`);
assertContains(ui, "forum_admin_topic_tag_count_label", `${uiPath}: UI must consume core-owned tag count label policy`);
assertContains(ui, "forum_admin_editing_thread_label", `${uiPath}: UI must consume core-owned editing thread label policy`);
assertContains(ui, "forum_admin_position_value", `${uiPath}: UI must consume core-owned position parsing policy`);
assertContains(ui, "forum_admin_sidebar_category_class", `${uiPath}: UI must consume core-owned sidebar class policy`);
assertContains(ui, "forum_admin_status_badge_class", `${uiPath}: UI must consume core-owned status badge class policy`);
assertContains(ui, "forum_admin_tag_chips", `${uiPath}: UI must consume core-owned tag chip parsing policy`);
assertContains(ui, "forum_admin_delete_outcome", `${uiPath}: UI must consume core-owned delete outcome policy`);
assertContains(ui, "CategoryFormSnapshot", `${uiPath}: UI must consume core-owned category form snapshots`);
assertContains(ui, "TopicFormSnapshot", `${uiPath}: UI must consume core-owned topic form snapshots`);
for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "#[server", "ForumService"]) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw transport or services (${marker})`);
}
for (const rawBusyMarker of ["category:edit", "category:save", "category:delete", "topic:edit", "topic:save", "topic:delete"]) {
  assertNotContains(ui, rawBusyMarker, `${uiPath}: busy-key strings must stay in core helper (${rawBusyMarker})`);
}
assertNotContains(ui, /format!\("\{\}: \{err\}"/, `${uiPath}: transport error message composition must stay in core helper`);

for (const marker of ["fetch_categories", "fetch_category", "create_category", "update_category", "delete_category", "fetch_topics", "fetch_topic", "create_topic", "update_topic", "delete_topic", "fetch_replies"]) {
  assertContains(transport, marker, `${transportPath}: transport facade must expose ${marker}`);
}
assertContains(transport, "use crate::api", `${transportPath}: transport facade may delegate to the current REST/api adapter`);
assertContains(api, "reqwest", `${apiPath}: forum admin api adapter must keep the REST transport contract`);

assertContains(implementationPlan, "verify-forum-admin-boundary.mjs", `${implementationPlanPath}: local plan must mention the forum fast boundary guardrail`);
assertContains(registry, "verify-forum-admin-boundary.mjs", `${registryPath}: central readiness board must mention the forum fast boundary guardrail`);

if (failures.length > 0) {
  console.error("forum admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum admin boundary verification passed");
