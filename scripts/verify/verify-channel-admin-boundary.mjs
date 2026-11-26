#!/usr/bin/env node
// RusTok channel admin FFA boundary guardrails.
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
  if (!existsSync(repoPath(relativePath))) {
    fail(description);
  }
}

function assertMissing(relativePath, description) {
  if (existsSync(repoPath(relativePath))) {
    fail(description);
  }
}

function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) {
    fail(description);
  }
}

function assertNotContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) {
    fail(description);
  }
}

function assertChannelAdminBoundary() {
  const libPath = "crates/modules/rustok-channel/admin/src/lib.rs";
  const corePath = "crates/modules/rustok-channel/admin/src/core.rs";
  const uiModPath = "crates/modules/rustok-channel/admin/src/ui/leptos/mod.rs";
  const uiComponentPaths = [
    "crates/modules/rustok-channel/admin/src/ui/leptos/runtime_context.rs",
    "crates/modules/rustok-channel/admin/src/ui/leptos/policy_workbench.rs",
    "crates/modules/rustok-channel/admin/src/ui/leptos/policy_set_card.rs",
    "crates/modules/rustok-channel/admin/src/ui/leptos/channel_card.rs",
  ];
  const transportModPath = "crates/modules/rustok-channel/admin/src/transport/mod.rs";
  const nativeAdapterPath = "crates/modules/rustok-channel/admin/src/transport/native_server_adapter.rs";
  const restAdapterPath = "crates/modules/rustok-channel/admin/src/transport/rest_adapter.rs";

  for (const path of [libPath, corePath, uiModPath, ...uiComponentPaths, transportModPath, nativeAdapterPath, restAdapterPath]) {
    assertExists(path, `${path}: expected channel admin FFA boundary file`);
  }
  assertMissing(
    "crates/modules/rustok-channel/admin/src/api.rs",
    "crates/modules/rustok-channel/admin/src/api.rs: pre-FFA api facade must stay removed",
  );
  assertMissing(
    "crates/modules/rustok-channel/admin/src/transport.rs",
    "crates/modules/rustok-channel/admin/src/transport.rs: transport must remain split into transport/ adapters",
  );

  const lib = readRepo(libPath);
  const core = readRepo(corePath);
  const uiMod = readRepo(uiModPath);
  const ui = [uiMod, ...uiComponentPaths.map(readRepo)].join("\n");
  const transportMod = readRepo(transportModPath);
  const nativeAdapter = readRepo(nativeAdapterPath);
  const restAdapter = readRepo(restAdapterPath);

  assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
  assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
  assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
  assertContains(lib, "pub use ui::leptos::ChannelAdmin;", `${libPath}: crate root must re-export the Leptos adapter surface`);
  assertNotContains(lib, "mod api;", `${libPath}: crate root must not wire the pre-FFA api facade`);

  for (const marker of ["leptos::", "leptos_", "#[component]", "#[server]", "LocalResource"] ) {
    assertNotContains(core, marker, `${corePath}: core must stay Leptos/server-function free (${marker})`);
  }
  assertContains(core, "channel_selection_exists", `${corePath}: core must own selected-channel route cleanup policy`);
  assertContains(core, "ChannelPolicySelectionCleanup", `${corePath}: core must own policy-set/rule route cleanup outcome`);
  assertContains(core, "channel_policy_selection_cleanup", `${corePath}: core must own policy-set/rule route cleanup policy`);
  assertContains(core, "PolicyRuleFormState", `${corePath}: core must own policy-rule form state`);
  assertContains(core, "policy_rule_create_form_state", `${corePath}: core must own policy-rule create defaults`);
  assertContains(core, "policy_rule_edit_form_state", `${corePath}: core must own policy-rule edit mapping`);
  assertContains(core, "reorder_policy_rule_ids", `${corePath}: core must own policy-rule reorder boundary policy`);
  assertContains(core, "fn create_payload(&self)", `${corePath}: core form state must build create payloads`);
  assertContains(core, "fn update_payload(&self)", `${corePath}: core form state must build update payloads`);
  assertContains(core, "policy_rule_active_update_payload", `${corePath}: core must build active toggle payloads`);
  assertContains(core, "ChannelLifecycleToggle", `${corePath}: core must own the channel lifecycle transition`);
  assertContains(core, "channel_lifecycle_toggle", `${corePath}: core must derive the channel lifecycle command`);
  assertContains(core, "channel_rename_command", `${corePath}: core must validate and build the channel rename command`);
  assertContains(core, "CHANNEL_DELETE_FEEDBACK", `${corePath}: core must own the channel delete feedback copy`);
  assertContains(core, "channel_status_badge", `${corePath}: core must own the channel status badge copy`);

  assertContains(uiMod, "mod channel_card;", `${uiModPath}: adapter directory must wire ChannelCard`);
  assertContains(uiMod, "mod policy_set_card;", `${uiModPath}: adapter directory must wire PolicySetCard`);
  assertContains(uiMod, "mod policy_workbench;", `${uiModPath}: adapter directory must wire PolicyWorkbench`);
  assertContains(uiMod, "mod runtime_context;", `${uiModPath}: adapter directory must wire RuntimeContext`);
  assertContains(ui, "use crate::transport;", `${uiModPath}: Leptos adapter must call the module-owned transport facade`);
  assertContains(ui, "channel_selection_exists", `${uiModPath}: Leptos adapter must consume core-owned route selection policy`);
  assertContains(ui, "channel_policy_selection_cleanup", `${uiModPath}: Leptos adapter must consume core-owned policy selection cleanup`);
  assertContains(ui, "policy_rule_edit_form_state", `${uiModPath}: Leptos adapter must consume core-owned policy-rule form mapping`);
  assertContains(ui, "reorder_policy_rule_ids", `${uiModPath}: Leptos adapter must consume core-owned reorder policy`);
  assertContains(ui, ".create_payload()", `${uiModPath}: Leptos adapter must consume core-owned create payload preparation`);
  assertContains(ui, ".update_payload()", `${uiModPath}: Leptos adapter must consume core-owned update payload preparation`);
  assertContains(ui, "policy_rule_active_update_payload", `${uiModPath}: Leptos adapter must consume core-owned active payload preparation`);
  assertContains(ui, "channel_lifecycle_toggle", `${uiModPath}: Leptos adapter must consume core-owned lifecycle transition`);
  assertContains(ui, "channel_rename_command", `${uiModPath}: Leptos adapter must consume core-owned rename command`);
  assertContains(ui, "CHANNEL_DELETE_FEEDBACK", `${uiModPath}: Leptos adapter must consume core-owned delete feedback`);
  assertContains(ui, "channel_status_badge", `${uiModPath}: Leptos adapter must consume core-owned status badge`);
  assertNotContains(ui, ".find(|policy_set| policy_set.policy_set.id", `${uiModPath}: UI must not own policy-set selection lookup`);
  assertNotContains(ui, "fn policy_rule_edit_form_state(", `${uiModPath}: UI must not define policy-rule edit mapping`);
  assertNotContains(ui, "fn reorder_rule_ids(", `${uiModPath}: UI must not define policy-rule reorder bounds policy`);
  assertNotContains(ui, "CreateResolutionRulePayload {", `${uiModPath}: UI must not construct policy-rule create payloads inline`);
  assertNotContains(ui, "&UpdateResolutionRulePayload {", `${uiModPath}: UI must not construct policy-rule update payloads inline`);
  assertNotContains(ui, "UpdateChannelPayload {", `${uiModPath}: UI must not construct channel update payloads inline`);
  assertNotContains(ui, "channel.feedback.renamed", `${uiModPath}: UI must not own channel rename feedback copy`);
  assertNotContains(ui, "channel.feedback.deleted", `${uiModPath}: UI must not own channel delete feedback copy`);
  assertNotContains(ui, "channel.error.nameRequired", `${uiModPath}: UI must not own channel rename validation copy`);
  assertNotContains(ui, "channel.channel.status", `${uiModPath}: UI must not render the raw channel status wire value`);
  for (const marker of [
    "mod api;",
    "crate::api",
    /(^|[^A-Za-z0-9_])api::/,
    "native_server_adapter::",
    "rest_adapter::",
    "reqwest::",
    "#[server",
  ]) {
    assertNotContains(ui, marker, `${uiModPath}: UI adapter must not call raw/pre-FFA transport (${marker})`);
  }

  assertContains(transportMod, "mod native_server_adapter;", `${transportModPath}: transport facade must wire native server adapter`);
  assertContains(transportMod, "mod rest_adapter;", `${transportModPath}: transport facade must wire the browser REST adapter`);
  assertContains(transportMod, "native_server_adapter::channel_bootstrap_native", `${transportModPath}: facade must prefer native bootstrap path`);
  assertContains(transportMod, "rest_adapter::get_json", `${transportModPath}: facade must keep the browser REST path`);
  assertContains(transportMod, "fn selected_transport_path()", `${transportModPath}: facade must select exactly one transport per build`);
  assertContains(transportMod, "async fn dispatch", `${transportModPath}: facade must route every call through one transport policy`);
  assertContains(transportMod, "transport = ", `${transportModPath}: transport failures must record the transport that produced them`);
  assertContains(transportMod, "path.as_str()", `${transportModPath}: recorded transport must be the selected path`);
  // A blind `native -> REST` fallback re-executes non-idempotent mutations and
  // masks 4xx domain rejections. The facade must never retry across transports.
  assertNotContains(transportMod, "Err(_) =>", `${transportModPath}: transport facade must not swallow the first transport error`);
  assertNotContains(transportMod, "execute_with_fallback", `${transportModPath}: channel admin mutations must not fall back to another transport`);
  assertNotContains(transportMod, "#[server", `${transportModPath}: server-function endpoints belong in native_server_adapter.rs`);
  assertNotContains(transportMod, "reqwest::", `${transportModPath}: raw REST client belongs in rest_adapter.rs`);

  assertContains(nativeAdapter, "#[server", `${nativeAdapterPath}: native adapter must contain server-function endpoints`);
  assertContains(nativeAdapter, "channel_bootstrap_native", `${nativeAdapterPath}: native adapter must own bootstrap server-function endpoint`);
  assertContains(nativeAdapter, "HostRuntimeContext", `${nativeAdapterPath}: native adapter must use neutral host runtime context`);
  assertContains(nativeAdapter, "db_clone()", `${nativeAdapterPath}: native adapter must use host runtime DB clone`);
  assertNotContains(nativeAdapter, "reqwest::", `${nativeAdapterPath}: native adapter must not own REST fallback HTTP calls`);

  assertContains(restAdapter, "reqwest::Client::new", `${restAdapterPath}: REST adapter must own raw HTTP fallback calls`);
  // The REST transport is browser-only and resolves against the origin that
  // served the bundle; a server-side base URL (`RUSTOK_API_URL`, defaulting to
  // localhost) pointed every SSR admin at the developer machine.
  assertNotContains(restAdapter, "RUSTOK_API_URL", `${restAdapterPath}: REST adapter must not invent a server-side API base URL`);
  assertNotContains(restAdapter, "localhost", `${restAdapterPath}: REST adapter must not default to a local API origin`);
  assertContains(restAdapter, "location().origin()", `${restAdapterPath}: REST adapter must resolve the served origin`);
  assertContains(restAdapter, "browser-only", `${restAdapterPath}: REST adapter must reject non-browser callers`);
  assertNotContains(restAdapter, "#[server", `${restAdapterPath}: REST adapter must not contain server-function endpoints`);
}

function assertChannelServerBoundary() {
  const dtoPath = "crates/modules/rustok-channel/src/dto/mod.rs";
  const controllerPath = "apps/server/src/controllers/channel.rs";
  assertExists(dtoPath, `${dtoPath}: expected channel owner DTO contract`);
  assertExists(controllerPath, `${controllerPath}: expected server channel controller adapter`);

  const dto = readRepo(dtoPath);
  const controller = readRepo(controllerPath);

  for (const marker of [
    "pub struct ChannelBootstrapResponse",
    "pub struct CreateResolutionPolicySetRequest",
    "pub struct CreateResolutionRuleRequest",
    "pub struct UpdateResolutionRuleRequest",
    "pub struct ReorderResolutionRulesRequest",
    "pub struct AvailableChannelModuleItem",
    "pub struct AvailableChannelOauthAppItem",
    "pub fn create_resolution_policy_set_input",
    "pub fn create_resolution_rule_input",
    "pub fn update_resolution_rule_input",
  ]) {
    assertContains(dto, marker, `${dtoPath}: channel owner DTO/helper contract must include ${marker}`);
  }

  for (const marker of [
    "struct ChannelBootstrapResponse",
    "struct CreateResolutionPolicySetRequest",
    "struct CreateResolutionRuleRequest",
    "struct UpdateResolutionRuleRequest",
    "struct ReorderResolutionRulesRequest",
    "struct AvailableModuleItem",
    "struct AvailableOauthAppItem",
    "fn build_rule_definition",
    "fn build_update_rule_input",
    "fn normalize_optional_string",
    "ResolutionAction::ResolveToChannel",
    "ResolutionPredicate::HostEquals",
  ]) {
    assertNotContains(
      controller,
      marker,
      `${controllerPath}: channel REST/control-plane shape must stay in rustok-channel (${marker})`,
    );
  }

  for (const marker of [
    "ChannelBootstrapResponse::<",
    "crate::context::ChannelContext",
    "create_resolution_policy_set_input(tenant.id, input)",
    "create_resolution_rule_input(input)",
    "update_resolution_rule_input(input)",
    "AvailableChannelModuleItem",
    "AvailableChannelOauthAppItem",
  ]) {
    assertContains(controller, marker, `${controllerPath}: controller must consume owner channel contract ${marker}`);
  }
}

assertChannelAdminBoundary();
assertChannelServerBoundary();

if (failures.length > 0) {
  console.error("channel admin boundary verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("channel admin boundary verification passed");
