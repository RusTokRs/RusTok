#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  contract: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-consumer-binding-source.json",
  providerStatus: "crates/rustok-page-builder/admin/src/provider_status.rs",
  rollout: "crates/rustok-pages/admin/src/builder_rollout_settings.rs",
  facade: "crates/rustok-pages/admin/src/builder.rs",
  composition: "crates/rustok-pages/admin/src/composition.rs",
  adminMain: "apps/admin/src/main.rs",
  serverBinding: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-server-binding-source.json",
  transport: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-transport-source.json",
  gate: "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
  overlay: "docs/modules/pages-page-builder-provider-health-consumer-binding-actualization-2026-08-09.md",
  parity: "docs/modules/pages-page-builder-plan-parity-actualization-2026-08-08.md",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(absolute(relativePath))) {
    failures.push(`${label}: missing ${relativePath}`);
    continue;
  }
  const stats = fs.lstatSync(absolute(relativePath));
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${label}: ${relativePath} must be a regular non-symlink file`);
  }
}

const sources = failures.length === 0
  ? Object.fromEntries(Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]))
  : {};
let contract = {};
let serverBinding = {};
let transport = {};
let gate = {};
if (failures.length === 0) {
  for (const [label, assign] of [
    ["contract", (value) => (contract = value)],
    ["serverBinding", (value) => (serverBinding = value)],
    ["transport", (value) => (transport = value)],
    ["gate", (value) => (gate = value)],
  ]) {
    try {
      assign(JSON.parse(sources[label]));
    } catch (error) {
      failures.push(`${label}: invalid JSON: ${error.message}`);
    }
  }
}

if (contract.format !== "pages_builder_provider_health_consumer_binding_source_v1") {
  failures.push("consumer binding contract format drifted");
}
if (contract.status !== "source_ready_runtime_activation_pending") {
  failures.push("consumer binding must remain source-ready with runtime activation pending");
}
if (serverBinding.format !== "pages_builder_provider_health_server_binding_source_v1") {
  failures.push("server binding predecessor format drifted");
}
if (transport.format !== "pages_builder_provider_health_transport_source_v1") {
  failures.push("transport predecessor format drifted");
}

for (const [object, key, expected] of [
  [contract.canonical_provider_status, "configured_rollout_flags_remain_authoritative", true],
  [contract.canonical_provider_status, "observed_health_may_only_narrow", true],
  [contract.canonical_provider_status, "unobserved_preserves_configured_rollout", true],
  [contract.canonical_provider_status, "ready_preserves_configured_rollout", true],
  [contract.canonical_provider_status, "degraded_disables_publish", true],
  [contract.canonical_provider_status, "unavailable_disables_builder", true],
  [contract.canonical_provider_status, "runtime_flags_method", "effective_runtime_flags"],
  [contract.workspace_binding, "fetches_server_owned_rollout_snapshot", true],
  [contract.workspace_binding, "uses_validated_provider_status", true],
  [contract.workspace_binding, "facade_receives_provider_status", true],
  [contract.authoritative_ssr_binding, "rereads_server_owned_snapshot_per_capability_request", true],
  [contract.authoritative_ssr_binding, "uses_effective_runtime_flags", true],
  [contract.authoritative_ssr_binding, "missing_health_uses_configured_rollout_flags", true],
  [contract.standalone_browser_intent_binding, "uses_pages_editor_capabilities_for_snapshot", true],
  [contract.standalone_browser_intent_binding, "role_capabilities_evaluated_before_provider_narrowing", true],
  [contract.anti_promotion, "accepted_packet_installed", false],
  [contract.anti_promotion, "pages_reference_consumer_gate_accepted", false],
  [contract.anti_promotion, "forum_wave_accepted", false],
  [contract.validation, "tests_run", false],
  [contract.validation, "node_verifier_run", false],
  [contract.validation, "cargo_run", false],
]) {
  if (object?.[key] !== expected) failures.push(`${key} must equal ${JSON.stringify(expected)}`);
}

for (const marker of [
  "pub fn effective_runtime_flags(&self) -> BuilderCapabilityFlags",
  "PageBuilderAdminProviderState::Unavailable => BuilderCapabilityFlags",
  "PageBuilderAdminProviderState::Degraded =>",
  "flags.publish_enabled = false",
  "PageBuilderAdminProviderState::Ready | PageBuilderAdminProviderState::Unobserved",
]) need(sources.providerStatus ?? "", marker, "canonical provider status runtime flags");

for (const marker of [
  "pub fn provider_status(&self) -> PageBuilderAdminProviderStatus",
  "pub fn effective_runtime_flags(&self) -> BuilderCapabilityFlags",
  "self.provider_status().effective_runtime_flags()",
  "pages_editor_capabilities_for_snapshot(",
  "snapshot.provider_status().limit_capabilities(capabilities)",
]) need(sources.rollout ?? "", marker, "Pages rollout snapshot health seam");

for (const marker of [
  "provider_status: Option<PageBuilderAdminProviderStatus>",
  "pub fn with_provider_status(",
  "self.provider_status.clone()",
  "fetch_pages_builder_rollout_snapshot(",
  "let effective_flags = trusted_rollout.effective_runtime_flags();",
  "compose_fly_page_builder_handlers(store, renderer, effective_flags)",
]) need(sources.facade ?? "", marker, "Pages facade and authoritative SSR binding");
forbid(
  sources.facade ?? "",
  "compose_fly_page_builder_handlers(store, renderer, trusted_rollout.flags)",
  "SSR must not bypass health-limited runtime flags",
);

for (const marker of [
  ".provider_status();",
  "provider_status: PageBuilderAdminProviderStatus",
  ".with_provider_status(provider_status)",
]) need(sources.composition ?? "", marker, "Pages workspace provider-health binding");
forbid(sources.composition ?? "", ".with_provider_flags(provider_flags)", "workspace must not discard observed health");

for (const marker of [
  "pages_editor_capabilities_for_snapshot",
  "pages_editor_capabilities_for_snapshot(role_capabilities, &rollout)",
  "dispatch_pages_browser_intent_with_capabilities(snapshot, envelope, editor_capabilities)",
]) need(sources.adminMain ?? "", marker, "standalone browser intent provider-health binding");
forbid(
  sources.adminMain ?? "",
  "pages_editor_capabilities_for_rollout(role_capabilities, &rollout.flags)",
  "browser intent must not discard observed health",
);

if (gate.accepted !== false) failures.push("Pages reference-consumer gate must remain unaccepted");
if (gate.current_boundary?.provider_health !== "unobserved") {
  failures.push("retained gate execution boundary must remain provider-health unobserved");
}

for (const marker of [
  "consumer-provider-health-binding-source-ready",
  "workspace-observed-health-source-ready",
  "authoritative-ssr-health-guard-source-ready",
  "browser-intent-health-preflight-source-ready",
  "Pages remains `unobserved` without a live accepted packet",
  "Tests were not run",
]) need(sources.overlay ?? "", marker, "consumer binding actualization");

for (const marker of [
  "provider-health-consumer-binding-source-ready",
  "pages-page-builder-provider-health-consumer-binding-actualization-2026-08-09.md",
  "UI / SSR / browser-intent provider-health binding [source-ready]",
]) need(sources.parity ?? "", marker, "plan parity actualization");

if (contract.next_cursor?.provider_health_consumer_binding !== "source_ready_runtime_activation_pending") {
  failures.push("consumer binding cursor drifted");
}
if (contract.next_cursor?.retained_identity_evaluator_owner_acceptance !== "maintainer_execution_pending") {
  failures.push("maintainer runtime evidence cursor drifted");
}
if (contract.next_cursor?.observed_health_acceptance !== "pending") {
  failures.push("observed health acceptance must remain pending");
}

if (failures.length > 0) {
  console.error("[verify-pages-builder-provider-health-consumer-binding] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("[verify-pages-builder-provider-health-consumer-binding] PASS consumers=workspace+ssr+browser_intent source_ready=true runtime_activation=pending");
