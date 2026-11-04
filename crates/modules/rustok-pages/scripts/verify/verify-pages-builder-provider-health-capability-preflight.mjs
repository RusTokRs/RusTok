#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  contract: "crates/modules/rustok-pages/contracts/evidence/pages-builder-provider-health-capability-preflight-source.json",
  coreRollout: "crates/modules/rustok-page-builder/src/rollout.rs",
  providerStatus: "crates/modules/rustok-page-builder/admin/src/provider_status.rs",
  owner: "crates/modules/rustok-pages/src/graphql/builder_rollout.rs",
  runtimeData: "crates/modules/rustok-pages/src/graphql/runtime_data.rs",
  pagesSnapshot: "crates/modules/rustok-pages/admin/src/builder_rollout_settings.rs",
  pagesFacade: "crates/modules/rustok-pages/admin/src/builder.rs",
  rolloutHarnessContract: "crates/modules/rustok-pages/contracts/evidence/pages-builder-rollout-feature-preflight-execution-contract.json",
  rolloutHarnessEvidence: "crates/modules/rustok-pages/contracts/evidence/pages-builder-rollout-feature-preflight-harness-source.json",
  rolloutHarnessSpec: "apps/next-admin/tests/pages-builder-rollout-feature-preflight/feature-preflight.spec.ts",
  runtimeHarness: "crates/modules/rustok-pages/contracts/evidence/pages-builder-provider-health-runtime-harness-source.json",
  consumerBinding: "crates/modules/rustok-pages/contracts/evidence/pages-builder-provider-health-consumer-binding-source.json",
  serverBinding: "crates/modules/rustok-pages/contracts/evidence/pages-builder-provider-health-server-binding-source.json",
  gate: "crates/modules/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
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
  console.error("[verify-pages-builder-provider-health-capability-preflight] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const sources = Object.fromEntries(Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]));
const contract = JSON.parse(sources.contract);
const rolloutContract = JSON.parse(sources.rolloutHarnessContract);
const rolloutEvidence = JSON.parse(sources.rolloutHarnessEvidence);
const runtimeHarness = JSON.parse(sources.runtimeHarness);
const consumerBinding = JSON.parse(sources.consumerBinding);
const serverBinding = JSON.parse(sources.serverBinding);
const gate = JSON.parse(sources.gate);

if (contract.format !== "pages_builder_provider_health_capability_preflight_source_v1" || contract.status !== "source_ready_runtime_execution_pending") failures.push("provider-health capability-preflight source identity drifted");
for (const [object, key, expected] of [
  [contract.shared_runtime_policy, "owner", "rustok_page_builder::rollout::effective_provider_runtime_flags"],
  [contract.shared_runtime_policy, "configured_rollout_flags_remain_authoritative", true],
  [contract.shared_runtime_policy, "provider_health_may_only_narrow", true],
  [contract.graphql_preflight, "operation", "pageBuilderCapabilityPreflight"],
  [contract.graphql_preflight, "non_mutating", true],
  [contract.graphql_preflight, "provider_health_source", "PagesGraphqlRuntimeData"],
  [contract.graphql_preflight, "uses_shared_runtime_policy", true],
  [contract.graphql_preflight, "disabled_error_kind", "feature-disabled"],
  [contract.graphql_preflight, "disabled_error_code", "FEATURE_DISABLED"],
  [contract.graphql_preflight, "snapshot_continues_to_return_configured_flags_separately_from_health", true],
  [contract.rollout_feature_preflight_isolation, "existing_four_profile_harness_remains_rollout_only", true],
  [contract.rollout_feature_preflight_isolation, "existing_harness_fails_closed_if_health_is_observed", true],
  [contract.rollout_feature_preflight_isolation, "existing_harness_retains_raw_health_payload", false],
  [contract.anti_promotion, "observed_graphql_preflight_executed", false],
  [contract.anti_promotion, "pages_reference_consumer_gate_accepted", false],
]) if (object?.[key] !== expected) failures.push(`${key} must equal ${JSON.stringify(expected)}`);

, marker, "capability-preflight actualization");
, marker, "plan parity actualization");

if (failures.length) {
  console.error("[verify-pages-builder-provider-health-capability-preflight] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-builder-provider-health-capability-preflight] PASS source_ready=true runtime_harness=source_ready execution=pending");
