#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  owner: "crates/rustok-pages/src/graphql/builder_rollout.rs",
  transport: "crates/rustok-pages/admin/src/transport/builder_rollout_adapter.rs",
  transportMod: "crates/rustok-pages/admin/src/transport/mod.rs",
  snapshot: "crates/rustok-pages/admin/src/builder_rollout_settings.rs",
  composition: "crates/rustok-pages/admin/src/composition.rs",
  builder: "crates/rustok-pages/admin/src/builder.rs",
  adminMain: "apps/admin/src/main.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-transport-source.json",
  serverBinding: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-server-binding-source.json",
  consumerBinding: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-consumer-binding-source.json",
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
let evidence = {};
let serverBinding = {};
let consumerBinding = {};
if (failures.length === 0) {
  for (const [label, assign] of [
    ["evidence", (value) => (evidence = value)],
    ["serverBinding", (value) => (serverBinding = value)],
    ["consumerBinding", (value) => (consumerBinding = value)],
  ]) {
    try {
      assign(JSON.parse(sources[label]));
    } catch (error) {
      failures.push(`${label}: invalid JSON: ${error.message}`);
    }
  }
}

if (evidence.format !== "pages_builder_provider_health_transport_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "typed_transport_server_and_consumer_binding_source_ready_execution_pending") {
  failures.push("evidence status drifted");
}
if (serverBinding.format !== "pages_builder_provider_health_server_binding_source_v1") {
  failures.push("server binding continuation format drifted");
}
if (consumerBinding.format !== "pages_builder_provider_health_consumer_binding_source_v1") {
  failures.push("consumer binding continuation format drifted");
}

for (const [object, key, expected] of [
  [evidence.graphql_owner, "default_server_observed_value", false],
  [evidence.graphql_owner, "default_server_health_payload", null],
  [evidence.graphql_owner, "server_binding_source_present", true],
  [evidence.graphql_owner, "fresh_accepted_authority_can_supply_observed_health", true],
  [evidence.admin_transport, "observed_false_requires_payload_absent", true],
  [evidence.admin_transport, "observed_true_requires_payload_present", true],
  [evidence.admin_transport, "canonical_snapshot_recomputed_client_side", true],
  [evidence.admin_transport, "state_must_equal_canonical_recalculation", true],
  [evidence.admin_transport, "degradation_reasons_must_equal_canonical_recalculation", true],
  [evidence.admin_snapshot, "effective_runtime_flags_use_provider_status", true],
  [evidence.admin_snapshot, "workspace_consumes_validated_provider_status", true],
  [evidence.admin_snapshot, "standalone_browser_intent_consumes_validated_snapshot", true],
  [evidence.admin_snapshot, "authoritative_ssr_consumes_health_limited_runtime_flags", true],
  [evidence.anti_promotion, "accepted_packet_installed", false],
  [evidence.anti_promotion, "pages_reference_consumer_gate_accepted", false],
  [evidence.validation, "tests_run", false],
  [evidence.validation, "node_verifier_run", false],
  [evidence.validation, "cargo_run", false],
]) {
  if (object?.[key] !== expected) failures.push(`${key} must equal ${JSON.stringify(expected)}`);
}

for (const marker of [
  "GqlPageBuilderProviderHealthSnapshot",
  "pub preview_p95_ms: u64",
  "pub publish_p95_ms: u64",
  "pub provider_health: Option<GqlPageBuilderProviderHealthSnapshot>",
  "provider_health_observed: false",
  "provider_health: None",
  "preview_p95_ms: snapshot.observed.preview_p95_ms",
  "publish_p95_ms: snapshot.observed.publish_p95_ms",
  "GqlPageBuilderProviderHealthSnapshot::from(health)",
  ".with_provider_health",
  "PagesGraphqlRuntimeData::provider_health_snapshot",
]) need(sources.owner ?? "", marker, "Pages GraphQL health shape");
forbid(sources.owner ?? "", "page_builder_provider_health_deployment_evaluation_v1", "Pages GraphQL must not load evaluator evidence directly");

for (const marker of [
  "providerHealthObserved providerHealth { state degradationReasons previewP95Ms publishP95Ms sanitizeFailureRate runtimeErrorRate }",
  "preview_p95_ms: u64",
  "publish_p95_ms: u64",
  "fn parse_provider_health(",
  "(false, None) => Ok(None)",
  "(false, Some(_)) => Err",
  "(true, None) => Err",
  "ProviderHealthSnapshot::evaluate(ProviderSloObservations",
  "preview_p95_ms: payload.preview_p95_ms",
  "publish_p95_ms: payload.publish_p95_ms",
  "must be finite and between 0 and 1",
  "does not match canonical evaluation",
  "degradationReasons do not match canonical evaluation",
  "payload.provider_health_observed",
  "payload.provider_health",
]) need(sources.transport ?? "", marker, "admin health transport");
for (const forbidden of [
  "rustok_page_builder_provider_operation_duration_seconds",
  "rustok_page_builder_provider_operation_completed_total",
  "rustok_page_builder_provider_last_observation_unix_seconds",
  "prometheus",
]) forbid(sources.transport ?? "", forbidden, "admin transport must not consume raw metrics");

for (const marker of [
  "Option<ProviderHealthSnapshot>",
  "builder_rollout_adapter::fetch(token, tenant_slug).await",
]) need(sources.transportMod ?? "", marker, "transport health propagation");

for (const marker of [
  "pub provider_health: Option<ProviderHealthSnapshot>",
  "pub fn provider_status(&self) -> PageBuilderAdminProviderStatus",
  "PageBuilderAdminProviderStatus::observed(self.flags.clone(), health)",
  "PageBuilderAdminProviderStatus::unobserved(self.flags.clone())",
  "pub fn effective_runtime_flags(&self) -> BuilderCapabilityFlags",
  "pages_editor_capabilities_for_rollout(",
  "pages_editor_capabilities_for_snapshot(",
]) need(sources.snapshot ?? "", marker, "validated admin snapshot");

for (const marker of [
  ".provider_status();",
  ".with_provider_status(provider_status)",
]) need(sources.composition ?? "", marker, "workspace observed health propagation");
for (const marker of [
  "provider_status: Option<PageBuilderAdminProviderStatus>",
  "pub fn with_provider_status(",
  "trusted_rollout.effective_runtime_flags()",
  "compose_fly_page_builder_handlers(store, renderer, effective_flags)",
]) need(sources.builder ?? "", marker, "SSR observed health propagation");
for (const marker of [
  "pages_editor_capabilities_for_snapshot",
  "pages_editor_capabilities_for_snapshot(role_capabilities, &rollout)",
]) need(sources.adminMain ?? "", marker, "standalone browser intent observed health propagation");

for (const marker of [
  "provider-health-transport-source-ready",
  "provider-health-server-binding-source-ready",
  "provider-health-consumer-binding-source-ready",
  "UI / SSR / browser-intent provider-health binding [source-ready]",
]) need(sources.parity ?? "", marker, "plan parity actualization");

if (evidence.next_cursor?.typed_provider_health_transport !== "source_ready") {
  failures.push("typed provider-health transport must be source-ready");
}
if (evidence.next_cursor?.server_owner_health_binding !== "source_ready_maintainer_activation_pending") {
  failures.push("server owner binding cursor drifted");
}
if (evidence.next_cursor?.ui_ssr_browser_intent_health_binding !== "source_ready_runtime_activation_pending") {
  failures.push("consumer health binding cursor drifted");
}
if (evidence.next_cursor?.retained_identity_evaluator_owner_acceptance !== "maintainer_execution_pending") {
  failures.push("runtime evidence must remain maintainer-execution pending");
}

if (failures.length > 0) {
  console.error("[verify-pages-builder-provider-health-transport] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("[verify-pages-builder-provider-health-transport] PASS transport=source_ready server_binding=source_ready consumer_binding=source_ready execution=pending");
