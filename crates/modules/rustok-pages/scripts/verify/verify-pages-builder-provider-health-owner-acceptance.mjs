#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  contract: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-owner-acceptance-source.json",
  runner: "scripts/evidence/accept-pages-builder-provider-health-deployment.mjs",
  evaluator: "scripts/evidence/evaluate-page-builder-provider-health-deployment.mjs",
  server: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-server-binding-source.json",
  consumer: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-consumer-binding-source.json",
  graphql: "crates/rustok-pages/src/graphql/builder_rollout.rs",
  parity: "docs/modules/pages-page-builder-plan-parity-actualization-2026-08-08.md",
};
const read = (name) => {
  const file = path.join(root, files[name]);
  if (!fs.existsSync(file) || !fs.lstatSync(file).isFile() || fs.lstatSync(file).isSymbolicLink()) {
    failures.push(`${name}: missing regular source file`);
    return "";
  }
  return fs.readFileSync(file, "utf8");
};
const source = Object.fromEntries(Object.keys(files).map((name) => [name, read(name)]));
const need = (text, marker, label) => { if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`); };
const forbid = (text, marker, label) => { if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`); };
let contract = {};
let server = {};
let consumer = {};
try { contract = JSON.parse(source.contract); } catch (error) { failures.push(`contract JSON: ${error.message}`); }
try { server = JSON.parse(source.server); } catch (error) { failures.push(`server JSON: ${error.message}`); }
try { consumer = JSON.parse(source.consumer); } catch (error) { failures.push(`consumer JSON: ${error.message}`); }

if (contract.format !== "pages_builder_provider_health_owner_acceptance_source_v1") failures.push("owner acceptance format drifted");
if (contract.status !== "source_ready_maintainer_execution_pending") failures.push("owner acceptance execution cursor drifted");
for (const [object, key, expected] of [
  [contract.evaluation_input, "source_commit_must_equal_checkout_head", true],
  [contract.evaluation_input, "source_hashes_must_match_checkout", true],
  [contract.evaluation_input, "maximum_target_operation_freshness_age_recomputed", true],
  [contract.evaluation_input, "health_valid_until_derived_from_remaining_freshness", true],
  [contract.evaluation_input, "minimum_preview_samples", 20],
  [contract.evaluation_input, "minimum_publish_samples", 20],
  [contract.evaluation_input, "canonical_health_snapshot_recomputed", true],
  [contract.evaluation_input, "canonical_slo_evaluation_recomputed", true],
  [contract.owner_decision, "accepted_rollback_action", "restore_unobserved_provider_health"],
  [contract.owner_decision, "accept_requires_unexpired_health_valid_until", true],
  [contract.owner_decision, "acceptance_clock_skew_tolerance_seconds", 5],
  [contract.output, "maximum_target_operation_freshness_age_seconds_retained", true],
  [contract.output, "health_valid_until_retained", true],
  [contract.binding_boundary, "server_binding_source_ready", true],
  [contract.binding_boundary, "consumer_binding_source_ready", true],
  [contract.binding_boundary, "server_binding_must_enforce_health_valid_until", true],
  [contract.non_claims, "owner_acceptance_executed", false],
  [contract.non_claims, "accepted_packet_installed", false],
  [contract.non_claims, "server_binding_runtime_activated", false],
  [contract.non_claims, "pages_reference_consumer_gate_accepted", false],
]) if (object?.[key] !== expected) failures.push(`${key} must equal ${JSON.stringify(expected)}`);

for (const marker of [
  'const ACCEPT_DECISION = "accept_for_pages_binding"',
  'const ROLLBACK_ACTION = "restore_unobserved_provider_health"',
  "maximumFreshnessAge",
  "remainingFreshnessSeconds",
  "healthValidUntilMs",
  "accepted decision is outside the retained health freshness deadline",
  "health_valid_until: admitted.healthValidUntil",
  "max_target_operation_freshness_age_seconds: admitted.maximumFreshnessAge",
  "server_binding_authorized: accepted",
  "server_binding_performed: false",
  "pages_provider_health_observed: false",
]) need(source.runner, marker, "acceptance runner");
for (const marker of ["source_files: sourceHashes(contract)", "pages_provider_health_observed: false"])
  need(source.evaluator, marker, "evaluator predecessor");

if (server.format !== "pages_builder_provider_health_server_binding_source_v1") failures.push("server continuation missing");
if (consumer.format !== "pages_builder_provider_health_consumer_binding_source_v1") failures.push("consumer continuation missing");
need(source.graphql, "PagesGraphqlRuntimeData::provider_health_snapshot", "GraphQL typed authority");
forbid(source.graphql, "pages_builder_provider_health_owner_acceptance_v1", "GraphQL direct packet parsing");
for (const marker of [
  "provider-health-owner-acceptance-source-ready",
  "provider-health-server-binding-source-ready",
  "provider-health-consumer-binding-source-ready",
]) need(source.parity, marker, "parity continuation");

if (contract.next_cursor?.owner_acceptance_packet !== "source_ready_maintainer_execution_pending") failures.push("owner acceptance cursor drifted");
if (contract.next_cursor?.server_owner_health_binding !== "source_ready_maintainer_activation_pending") failures.push("server source cursor drifted");
if (contract.next_cursor?.ui_ssr_browser_intent_health_binding !== "source_ready_runtime_activation_pending") failures.push("consumer source cursor drifted");
if (contract.next_cursor?.retained_identity_evaluator_owner_acceptance !== "maintainer_execution_pending") failures.push("runtime evidence cursor drifted");
if (contract.next_cursor?.observed_health_acceptance !== "pending") failures.push("observed acceptance must remain pending");

if (failures.length) {
  console.error("[verify-pages-builder-provider-health-owner-acceptance] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-builder-provider-health-owner-acceptance] PASS acceptance=source_ready execution=pending server=source_ready consumers=source_ready");
