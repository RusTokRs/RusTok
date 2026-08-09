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
  overlay: "docs/modules/pages-page-builder-provider-health-transport-actualization-2026-08-09.md",
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
if (failures.length === 0) {
  try {
    evidence = JSON.parse(sources.evidence);
  } catch (error) {
    failures.push(`evidence: invalid JSON: ${error.message}`);
  }
}

if (evidence.format !== "pages_builder_provider_health_transport_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "typed_transport_source_ready_server_binding_blocked_execution_pending") {
  failures.push("evidence status drifted");
}

for (const [object, key, expected] of [
  [evidence.graphql_owner, "current_server_observed_value", false],
  [evidence.graphql_owner, "current_server_health_payload", null],
  [evidence.graphql_owner, "owner_binding_to_retained_evaluator_packet_present", false],
  [evidence.admin_transport, "observed_false_requires_payload_absent", true],
  [evidence.admin_transport, "observed_true_requires_payload_present", true],
  [evidence.admin_transport, "canonical_snapshot_recomputed_client_side", true],
  [evidence.admin_transport, "state_must_equal_canonical_recalculation", true],
  [evidence.admin_transport, "degradation_reasons_must_equal_canonical_recalculation", true],
  [evidence.admin_snapshot, "workspace_currently_consumes_flags_only", true],
  [evidence.admin_snapshot, "standalone_browser_intent_currently_consumes_flags_only", true],
  [evidence.anti_promotion, "pages_graphql_provider_health_observed", false],
  [evidence.anti_promotion, "owner_acceptance_present", false],
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
  "pages_editor_capabilities_for_rollout(",
  "pages_editor_capabilities_for_snapshot(",
]) need(sources.snapshot ?? "", marker, "validated admin snapshot");

// The transport is source-ready, but no current production consumer may activate observed health.
for (const marker of [
  ".flags;",
  "provider_flags: BuilderCapabilityFlags",
  ".with_provider_flags(provider_flags)",
]) need(sources.composition ?? "", marker, "workspace remains rollout-only");
need(sources.builder ?? "", ".map(PageBuilderAdminProviderStatus::unobserved)", "SSR facade remains unobserved");
for (const marker of [
  "pages_editor_capabilities_for_rollout(",
  "&rollout.flags",
]) need(sources.adminMain ?? "", marker, "standalone browser intent remains rollout-only");
for (const source of [sources.composition ?? "", sources.builder ?? "", sources.adminMain ?? ""]) {
  forbid(source, "pages_editor_capabilities_for_snapshot(", "observed health consumer binding must remain absent");
}

for (const marker of [
  "typed-observed-health-transport-source-ready",
  "server-owner-health-binding-blocked",
  "boolean/payload consistency",
  "canonical `ProviderHealthSnapshot::evaluate`",
  "Pages remains `unobserved`",
  "tests were not run",
]) need(sources.overlay ?? "", marker, "health transport actualization");

for (const marker of [
  "provider-health-transport-source-ready",
  "pages-page-builder-provider-health-transport-actualization-2026-08-09.md",
  "typed observed-health transport",
  "Pages remains `unobserved`",
]) need(sources.parity ?? "", marker, "plan parity actualization");

if (evidence.next_cursor?.typed_provider_health_transport !== "source_ready") {
  failures.push("typed provider-health transport must be source-ready");
}
if (evidence.next_cursor?.retained_identity_and_evaluator_runtime_evidence !== "maintainer_execution_pending") {
  failures.push("runtime evidence must remain maintainer-execution pending");
}
if (evidence.next_cursor?.owner_acceptance_and_server_binding !== "blocked_on_retained_runtime_evidence") {
  failures.push("server owner binding must remain blocked on retained runtime evidence");
}

if (failures.length > 0) {
  console.error("[verify-pages-builder-provider-health-transport] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("[verify-pages-builder-provider-health-transport] PASS transport=source_ready server_binding=blocked pages_health=unobserved");
