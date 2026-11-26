#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-channel-admin-boundary.mjs");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function libSource({ includeApiModule = false } = {}) {
  return `
mod core;
mod i18n;
mod model;
${includeApiModule ? "mod api;" : "mod transport;"}
mod ui;

pub use ui::leptos::ChannelAdmin;
`;
}

function coreSource({ includeLeptos = false } = {}) {
  return `
${includeLeptos ? "use leptos::prelude::*;" : ""}
pub(crate) fn channel_selection_exists(bootstrap: &ChannelAdminBootstrap, channel_id: &str) -> bool {
    bootstrap.channels.iter().any(|channel| channel.channel.id == channel_id)
}
pub(crate) enum ChannelPolicySelectionCleanup { None, ClearRule, ClearPolicySetAndRule }
pub(crate) fn channel_policy_selection_cleanup() -> ChannelPolicySelectionCleanup { ChannelPolicySelectionCleanup::None }
pub(crate) struct PolicyRuleFormState;
impl PolicyRuleFormState {
    pub(crate) fn create_payload(&self) {}
    pub(crate) fn update_payload(&self) {}
}
pub(crate) fn policy_rule_create_form_state() -> PolicyRuleFormState { PolicyRuleFormState }
pub(crate) fn policy_rule_edit_form_state() -> PolicyRuleFormState { PolicyRuleFormState }
pub(crate) fn reorder_policy_rule_ids() -> Option<Vec<String>> { None }
pub(crate) fn policy_rule_active_update_payload() {}
pub(crate) struct ChannelLifecycleToggle { pub(crate) next_is_active: bool }
pub(crate) fn channel_lifecycle_toggle(is_active: bool) -> ChannelLifecycleToggle {
    ChannelLifecycleToggle { next_is_active: !is_active }
}
pub(crate) struct ChannelRenameCommand { pub(crate) feedback_key: &'static str }
pub(crate) fn channel_rename_command(raw_name: &str) -> Result<ChannelRenameCommand, ()> {
    let name = raw_name.trim();
    if name.is_empty() { return Err(()); }
    Ok(ChannelRenameCommand { feedback_key: "channel.feedback.renamed" })
}
pub(crate) const CHANNEL_DELETE_FEEDBACK: &str = "channel.feedback.deleted";
pub(crate) struct ChannelStatusBadge { pub(crate) label_key: &'static str }
pub(crate) fn channel_status_badge(is_active: bool) -> ChannelStatusBadge {
    ChannelStatusBadge { label_key: if is_active { "channel.card.statusActive" } else { "channel.card.statusInactive" } }
}
struct ChannelAdminBootstrap { channels: Vec<ChannelDetail> }
struct ChannelDetail { channel: ChannelRecord }
struct ChannelRecord { id: String }
`;
}

function uiSource({ rawTransport = false, includeApiCall = false, inlinePolicySelection = false, inlineRuleFormMapping = false, inlineReorderPolicy = false, inlineRulePayload = false, inlineChannelPayload = false, inlineLifecycleCopy = false } = {}) {
  return `
use crate::core::{CHANNEL_DELETE_FEEDBACK, channel_lifecycle_toggle, channel_policy_selection_cleanup, channel_rename_command, channel_selection_exists, channel_status_badge, policy_rule_active_update_payload, policy_rule_edit_form_state, reorder_policy_rule_ids, PolicyRuleFormState};
use crate::transport;

pub fn ChannelAdmin() {
    let _policy = channel_selection_exists;
    let _policy_cleanup = channel_policy_selection_cleanup;
    let _rule_form = policy_rule_edit_form_state;
    let _reorder = reorder_policy_rule_ids;
    let _active_payload = policy_rule_active_update_payload;
    let _toggle = channel_lifecycle_toggle(true);
    let _rename = channel_rename_command("web");
    let _delete_copy = CHANNEL_DELETE_FEEDBACK;
    let _badge = channel_status_badge(true);
    let state = PolicyRuleFormState;
    state.create_payload();
    state.update_payload();
    let _facade = transport::fetch_bootstrap;
    ${rawTransport ? "let _native = native_server_adapter::channel_bootstrap_native;" : ""}
    ${includeApiCall ? "api::fetch_bootstrap().await;" : ""}
    ${inlinePolicySelection ? "let _selected = policy_sets.iter().find(|policy_set| policy_set.policy_set.id == selected_id);" : ""}
    ${inlineRuleFormMapping ? "fn policy_rule_edit_form_state() {}" : ""}
    ${inlineReorderPolicy ? "fn reorder_rule_ids() {}" : ""}
    ${inlineRulePayload ? "let _payload = CreateResolutionRulePayload { priority: 1 };" : ""}
    ${inlineChannelPayload ? "let _payload = UpdateChannelPayload { name: None, settings: None };" : ""}
    ${inlineLifecycleCopy ? `let _copy = "channel.feedback.renamed";` : ""}
}
`;
}

function transportModSource({ includeServerEndpoint = false, includeRawRest = false } = {}) {
  return `
mod native_server_adapter;
mod rest_adapter;

enum TransportPath { NativeServer, Rest }

impl TransportPath {
    fn as_str(self) -> &'static str { "native_server" }
}

fn selected_transport_path() -> TransportPath { TransportPath::NativeServer }

async fn dispatch<T, N, NFut, R, RFut>(surface: &'static str, native: N, rest: R) -> Result<T, ApiError> {
    let path = selected_transport_path();
    let result = match path {
        TransportPath::NativeServer => native().await,
        TransportPath::Rest => rest().await,
    };
    let _ = (surface, format!("transport = {}", path.as_str()));
    result
}

pub async fn fetch_bootstrap() {
    dispatch(
        "channel.admin.bootstrap",
        || native_server_adapter::channel_bootstrap_native(),
        || rest_adapter::get_json("/api/channels/bootstrap", None, None),
    ).await;
}
${includeServerEndpoint ? '#[server(prefix = "/api/fn", endpoint = "bad")] async fn bad() {}' : ""}
${includeRawRest ? "fn bad_rest() { reqwest::Client::new(); }" : ""}
`;
}

function nativeAdapterSource({ includeRest = false } = {}) {
  return `
use leptos::prelude::*;
use rustok_api::HostRuntimeContext;
#[server(prefix = "/api/fn", endpoint = "channel/bootstrap")]
pub(super) async fn channel_bootstrap_native() -> Result<(), ServerFnError> {
    let runtime_ctx = expect_context::<HostRuntimeContext>();
    let _db = runtime_ctx.db_clone();
    ${includeRest ? "reqwest::Client::new();" : ""}
    Ok(())
}
`;
}

function restAdapterSource({ includeServerEndpoint = false } = {}) {
  return `
fn api_url(path: &str) -> Result<String, ApiError> {
    let origin = web_sys::window()
        .and_then(|window| window.location().origin().ok())
        .ok_or_else(|| ApiError::Rest("cannot resolve origin".to_string()))?;
    let _ = path;
    Err(ApiError::Rest("the channel REST transport is browser-only".to_string()))
}
pub(super) async fn get_json<T>(path: &str, token: Option<String>, tenant_slug: Option<String>) -> Result<T, ApiError> {
    let client = reqwest::Client::new();
    todo!()
}
${includeServerEndpoint ? '#[server(prefix = "/api/fn", endpoint = "bad")] async fn bad() {}' : ""}
`;
}

function channelDtoSource() {
  return `
pub struct ChannelBootstrapResponse<C> {}
pub struct CreateResolutionPolicySetRequest {}
pub struct CreateResolutionRuleRequest {}
pub struct UpdateResolutionRuleRequest {}
pub struct ReorderResolutionRulesRequest {}
pub struct AvailableChannelModuleItem {}
pub struct AvailableChannelOauthAppItem {}
pub fn create_resolution_policy_set_input() {}
pub fn create_resolution_rule_input() {}
pub fn update_resolution_rule_input() {}
`;
}

function channelControllerSource({ includeServerOwnedDto = false } = {}) {
  return `
use rustok_channel::{AvailableChannelModuleItem, AvailableChannelOauthAppItem, ChannelBootstrapResponse};
pub fn controller() {
    let _ = "ChannelBootstrapResponse::<crate::context::ChannelContext>";
    let _ = "create_resolution_policy_set_input(tenant.id, input)";
    let _ = "create_resolution_rule_input(input)";
    let _ = "update_resolution_rule_input(input)";
    let _ = "AvailableChannelModuleItem";
    let _ = "AvailableChannelOauthAppItem";
}
${includeServerOwnedDto ? "struct CreateResolutionRuleRequest; fn build_rule_definition() {}" : ""}
`;
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-channel-boundary-"));
  writeFixtureFile(root, "crates/modules/rustok-channel/src/dto/mod.rs", channelDtoSource());
  writeFixtureFile(root, "apps/server/src/controllers/channel.rs", channelControllerSource(options));
  writeFixtureFile(root, "crates/modules/rustok-channel/admin/src/lib.rs", libSource(options));
  writeFixtureFile(root, "crates/modules/rustok-channel/admin/src/core.rs", coreSource(options));
  writeFixtureFile(root, "crates/modules/rustok-channel/admin/src/ui/leptos/mod.rs", `
mod channel_card;
mod policy_set_card;
mod policy_workbench;
mod runtime_context;
${uiSource(options)}
`);
  writeFixtureFile(root, "crates/modules/rustok-channel/admin/src/ui/leptos/channel_card.rs", "pub(super) fn ChannelCard() {}");
  writeFixtureFile(root, "crates/modules/rustok-channel/admin/src/ui/leptos/policy_set_card.rs", "pub(super) fn PolicySetCard() {}");
  writeFixtureFile(root, "crates/modules/rustok-channel/admin/src/ui/leptos/policy_workbench.rs", "pub(super) fn PolicyWorkbench() {}");
  writeFixtureFile(root, "crates/modules/rustok-channel/admin/src/ui/leptos/runtime_context.rs", "pub(super) fn RuntimeContext() {}");
  writeFixtureFile(root, "crates/modules/rustok-channel/admin/src/transport/mod.rs", transportModSource(options));
  writeFixtureFile(root, "crates/modules/rustok-channel/admin/src/transport/native_server_adapter.rs", nativeAdapterSource(options));
  writeFixtureFile(root, "crates/modules/rustok-channel/admin/src/transport/rest_adapter.rs", restAdapterSource({ includeServerEndpoint: options.restServerEndpoint }));
  if (options.includeLegacyApiFile) {
    writeFixtureFile(root, "crates/modules/rustok-channel/admin/src/api.rs", "pub async fn fetch_bootstrap() {}");
  }
  if (options.includeLegacyTransportFile) {
    writeFixtureFile(root, "crates/modules/rustok-channel/admin/src/transport.rs", "pub async fn fetch_bootstrap() {}");
  }
  return root;
}

function runVerifier(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

test("channel admin boundary verifier passes canonical fixture", () => {
  const root = withFixture();
  try {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /channel admin boundary verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects legacy api facade", () => {
  const root = withFixture({ includeLegacyApiFile: true, includeApiModule: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected legacy api fixture to fail");
    assert.match(result.stderr, /pre-FFA api facade must stay removed|must not wire the pre-FFA api facade/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects legacy flat transport file", () => {
  const root = withFixture({ includeLegacyTransportFile: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected legacy transport fixture to fail");
    assert.match(result.stderr, /transport must remain split into transport\/ adapters/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects raw adapter calls from UI", () => {
  const root = withFixture({ rawTransport: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected raw UI transport fixture to fail");
    assert.match(result.stderr, /UI adapter must not call raw\/pre-FFA transport/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects Leptos-specific core", () => {
  const root = withFixture({ includeLeptos: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected Leptos core fixture to fail");
    assert.match(result.stderr, /core must stay Leptos\/server-function free/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects inline policy selection lookup", () => {
  const root = withFixture({ inlinePolicySelection: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected inline policy selection fixture to fail");
    assert.match(result.stderr, /UI must not own policy-set selection lookup/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects policy-rule form mapping in UI", () => {
  const root = withFixture({ inlineRuleFormMapping: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected inline rule-form mapping fixture to fail");
    assert.match(result.stderr, /UI must not define policy-rule edit mapping/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects policy-rule reorder policy in UI", () => {
  const root = withFixture({ inlineReorderPolicy: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected inline reorder policy fixture to fail");
    assert.match(result.stderr, /UI must not define policy-rule reorder bounds policy/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects inline policy-rule payload construction", () => {
  const root = withFixture({ inlineRulePayload: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected inline rule payload fixture to fail");
    assert.match(result.stderr, /UI must not construct policy-rule create payloads inline/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});


test("channel admin boundary verifier rejects inline channel update payload construction", () => {
  const root = withFixture({ inlineChannelPayload: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected inline channel payload fixture to fail");
    assert.match(result.stderr, /UI must not construct channel update payloads inline/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects lifecycle feedback copy in UI", () => {
  const root = withFixture({ inlineLifecycleCopy: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected inline lifecycle copy fixture to fail");
    assert.match(result.stderr, /UI must not own channel rename feedback copy/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects server functions in transport facade", () => {
  const root = withFixture({ includeServerEndpoint: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected facade server-function fixture to fail");
    assert.match(result.stderr, /server-function endpoints belong in native_server_adapter\.rs/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects raw REST calls in transport facade", () => {
  const root = withFixture({ includeRawRest: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected facade raw REST fixture to fail");
    assert.match(result.stderr, /raw REST client belongs in rest_adapter\.rs/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects REST calls in native adapter", () => {
  const root = withFixture({ includeRest: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected native adapter REST fixture to fail");
    assert.match(result.stderr, /native adapter must not own REST fallback HTTP calls/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects server functions in REST adapter", () => {
  const root = withFixture({ restServerEndpoint: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected REST adapter server-function fixture to fail");
    assert.match(result.stderr, /REST adapter must not contain server-function endpoints/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects server-owned channel DTO", () => {
  const root = withFixture({ includeServerOwnedDto: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected server-owned channel DTO fixture to fail");
    assert.match(result.stderr, /channel REST\/control-plane shape must stay in rustok-channel/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
