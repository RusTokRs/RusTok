#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  contract: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-runtime-execution-contract.json",
  source: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-runtime-harness-source.json",
  config: "apps/next-admin/playwright.pages-builder-provider-health-runtime.config.ts",
  spec: "apps/next-admin/tests/pages-builder-provider-health-runtime/runtime.spec.ts",
  identity: "crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-identity-source.json",
  evaluator: "crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-evaluator-source.json",
  acceptance: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-owner-acceptance-source.json",
  observedAcceptance: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-observed-acceptance-source.json",
  serverBinding: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-server-binding-source.json",
  consumerBinding: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-consumer-binding-source.json",
  preflight: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-capability-preflight-source.json",
  browserIntent: "crates/rustok-pages/admin/src/contribution_browser_intent.rs",
  adminMain: "apps/admin/src/main.rs",
  overlay: "docs/modules/pages-page-builder-provider-health-runtime-evidence-harness-actualization-2026-08-09.md",
  parity: "docs/modules/pages-page-builder-plan-parity-actualization-2026-08-08.md",
};
const abs = (value) => path.join(repoRoot, value);
const read = (value) => fs.readFileSync(abs(value), "utf8");
const need = (source, marker, label) => { if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`); };
const forbid = (source, marker, label) => { if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`); };

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(abs(relativePath))) { failures.push(`${label}: missing ${relativePath}`); continue; }
  const stats = fs.lstatSync(abs(relativePath));
  if (!stats.isFile() || stats.isSymbolicLink()) failures.push(`${label}: ${relativePath} must be a regular non-symlink file`);
}
if (failures.length) {
  console.error("[verify-pages-builder-provider-health-runtime-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const sources = Object.fromEntries(Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]));
const contract = JSON.parse(sources.contract);
const source = JSON.parse(sources.source);
const identity = JSON.parse(sources.identity);
const evaluator = JSON.parse(sources.evaluator);
const acceptance = JSON.parse(sources.acceptance);
const observedAcceptance = JSON.parse(sources.observedAcceptance);
const serverBinding = JSON.parse(sources.serverBinding);
const consumerBinding = JSON.parse(sources.consumerBinding);
const preflight = JSON.parse(sources.preflight);

if (contract.schema_version !== 1 || contract.module !== "pages" || contract.packet !== "pages-builder-provider-health-runtime-evidence" || contract.status !== "source_ready_maintainer_execution_pending") failures.push("runtime execution contract identity drifted");
if (contract.output?.format !== "pages_builder_provider_health_runtime_evidence_v1" || contract.output?.status !== "observed_runtime_evidence_owner_review_pending") failures.push("runtime evidence output contract drifted");
if (contract.fixtures?.page_id_format !== "uuid") failures.push("runtime evidence page id must remain UUID-bound");
if (source.format !== "pages_builder_provider_health_runtime_harness_source_v1" || source.status !== "source_ready_maintainer_execution_pending") failures.push("runtime harness source packet identity drifted");
if (!Array.isArray(source.execution) || source.execution.length !== 0) failures.push("runtime harness source execution must remain empty");
for (const value of Object.values(source.validation ?? {})) if (value !== false) failures.push("runtime harness validation fields must remain false");

if (identity.format !== "page_builder_provider_health_deployment_identity_source_v1") failures.push("identity predecessor source drifted");
if (evaluator.format !== "page_builder_provider_health_deployment_evaluator_source_v1") failures.push("evaluator predecessor source drifted");
if (acceptance.format !== "pages_builder_provider_health_owner_acceptance_source_v1") failures.push("owner acceptance predecessor source drifted");
if (observedAcceptance.format !== "pages_builder_provider_health_observed_acceptance_source_v1" || observedAcceptance.status !== "source_ready_maintainer_execution_pending") failures.push("observed-health owner acceptance continuation drifted");
if (serverBinding.format !== "pages_builder_provider_health_server_binding_source_v1") failures.push("server binding predecessor source drifted");
if (consumerBinding.format !== "pages_builder_provider_health_consumer_binding_source_v1") failures.push("consumer binding predecessor source drifted");
if (preflight.format !== "pages_builder_provider_health_capability_preflight_source_v1") failures.push("capability-preflight predecessor source drifted");

for (const [object, key, expected] of [
  [contract.admission, "checkout_head_must_equal_identity_evaluation_and_acceptance_source_commit", true],
  [contract.admission, "acceptance_evaluation_sha256_must_match_supplied_evaluation", true],
  [contract.admission, "accepted_health_snapshot_must_match_evaluation_snapshot", true],
  [contract.admission, "configured_rollout_must_be_all_on", true],
  [contract.admission, "graphql_snapshot_must_observe_health", true],
  [contract.admission, "browser_intent_mismatch_sentinel_is_non_uuid", true],
  [contract.admission, "browser_intent_probe_cannot_target_fixture_page", true],
  [contract.observations, "graphql_non_mutating_capability_preflight", true],
  [contract.observations, "authoritative_ssr_preview_when_ui_allows", true],
  [contract.observations, "standalone_browser_intent_denial_when_health_narrows", true],
  [contract.observations, "publish_mutation_executed", false],
  [contract.observations, "rollout_settings_mutated", false],
  [source.source_contract, "runtime_page_id_fixture_must_be_uuid", true],
  [source.source_contract, "browser_intent_mismatch_sentinel_is_non_uuid", true],
  [source.source_contract, "browser_intent_probe_uses_mismatched_page_id_as_non_mutating_fallback", true],
  [source.source_contract, "publish_mutation_is_never_executed", true],
  [source.source_contract, "automatic_owner_acceptance", false],
  [source.source_contract, "automatic_pages_gate_acceptance", false],
  [source.observed_owner_acceptance, "source_ready", true],
  [source.observed_owner_acceptance, "execution_pending", true],
  [source.observed_owner_acceptance, "health_lease_extended", false],
  [source.observed_owner_acceptance, "automatic_pages_gate_acceptance", false],
]) if (object?.[key] !== expected) failures.push(`${key} must equal ${JSON.stringify(expected)}`);

for (const marker of [
  "fullyParallel: false", "workers: 1", "retries: 0", 'trace: "off"', 'screenshot: "off"', 'video: "off"',
  'name: "pages-builder-provider-health-runtime-chromium"',
]) need(sources.config, marker, "Playwright config");

for (const marker of [
  "type PredecessorSpec = {",
  "decision?: string",
  "rollback_action?: string",
  "runtime evidence page id must be a UUID",
  "runtime evidence page id collides with non-mutating mismatch sentinel",
  "validateEvidenceChain(",
  "identityDeployment.source_commit",
  "acceptanceEvaluation.evaluation_sha256 !== evaluation.record.sha256",
  "accepted provider health is expired before runtime observation begins",
  "runtime evidence requires configured all_on rollout flags",
  "runtime snapshot did not observe provider health",
  "runtime GraphQL health differs from accepted provider snapshot",
  'result.errorKind !== "feature-disabled"',
  'result.errorCode !== "FEATURE_DISABLED"',
  "data-fly-provider-control-state",
  "data-fly-provider-health",
  "data-page-builder-provider-preview",
  "safeSsrPreviewObservation(",
  "safeBrowserIntentDenial(",
  "page_id: mismatchPageId",
  "mismatched page id prevents mutation if health was revoked",
  "rollout_settings_mutated: false",
  "publish_mutation_executed: false",
  "owner_observed_health_acceptance: false",
  "pages_reference_consumer_gate_accepted: false",
  "raw_evidence_paths_persisted: false",
  "writeAtomic(output",
]) need(sources.spec, marker, "runtime evidence spec");

for (const marker of [
  "updateModuleSettings", "writePagesSettings(", "PublishPageBuilderInput", "save_project(",
  "owner_observed_health_acceptance: true", "pages_reference_consumer_gate_accepted: true",
  'trace: "on"', 'screenshot: "on"', 'video: "on"',
]) forbid(sources.spec, marker, "runtime evidence spec");

for (const marker of [
  "let envelope = preflight_pages_intent(envelope, capabilities)?;",
  "validate_browser_capability_access(&envelope, capabilities)?;",
]) need(sources.browserIntent, marker, "browser-intent capability-before-dispatch safety");
for (const marker of [
  "pages_editor_capabilities_for_snapshot(role_capabilities, &rollout)",
  "dispatch_pages_browser_intent_with_capabilities(snapshot, envelope, editor_capabilities)",
]) need(sources.adminMain, marker, "standalone browser-intent health binding");

for (const relativePath of contract.required_source_files ?? []) {
  if (!fs.existsSync(abs(relativePath))) failures.push(`required source file missing: ${relativePath}`);
}
if (!(contract.required_source_files ?? []).includes("apps/next-admin/tests/pages-builder-provider-health-runtime/runtime.spec.ts")) failures.push("runtime spec is absent from required source hashes");

if (preflight.next_cursor?.observed_health_runtime_evidence_harness !== "source_ready_maintainer_execution_pending") failures.push("capability-preflight cursor must point to source-ready runtime harness");
if (consumerBinding.next_cursor?.observed_health_runtime_evidence_harness !== "source_ready_maintainer_execution_pending") failures.push("consumer-binding cursor must point to source-ready runtime harness");
if (source.next_cursor?.observed_health_runtime_evidence_harness !== "source_ready_maintainer_execution_pending") failures.push("runtime harness cursor drifted");
if (source.next_cursor?.observed_health_owner_acceptance !== "source_ready_maintainer_execution_pending") failures.push("observed-health owner acceptance must be source-ready/execution-pending");
if (observedAcceptance.next_cursor?.observed_health_owner_acceptance !== "source_ready_maintainer_execution_pending") failures.push("observed-health owner acceptance source cursor drifted");

for (const marker of [
  "provider-health-runtime-evidence-harness-source-ready",
  "mismatched envelope page id",
  "all_on",
  "observed_runtime_evidence_owner_review_pending",
  "No tests, Node verifiers, Cargo commands",
]) need(sources.overlay, marker, "runtime harness actualization");
for (const marker of [
  "provider-health-runtime-evidence-harness-source-ready",
  "provider-health-observed-acceptance-source-ready",
  "pages-page-builder-provider-health-runtime-evidence-harness-actualization-2026-08-09.md",
  "observed-health runtime evidence harness [source-ready / maintainer execution pending]",
  "observed-health owner acceptance [source-ready / maintainer execution pending]",
]) need(sources.parity, marker, "plan parity actualization");

if (failures.length) {
  console.error("[verify-pages-builder-provider-health-runtime-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-builder-provider-health-runtime-harness] PASS source_ready=true execution=pending owner_acceptance=source_ready_execution_pending");
