#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  contract: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-owner-acceptance-source.json",
  runner: "scripts/evidence/accept-pages-builder-provider-health-deployment.mjs",
  evaluatorContract: "crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-evaluator-source.json",
  evaluator: "scripts/evidence/evaluate-page-builder-provider-health-deployment.mjs",
  transportEvidence: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-transport-source.json",
  owner: "crates/rustok-pages/src/graphql/builder_rollout.rs",
  transport: "crates/rustok-pages/admin/src/transport/builder_rollout_adapter.rs",
  snapshot: "crates/rustok-pages/admin/src/builder_rollout_settings.rs",
  composition: "crates/rustok-pages/admin/src/composition.rs",
  builder: "crates/rustok-pages/admin/src/builder.rs",
  adminMain: "apps/admin/src/main.rs",
  overlay: "docs/modules/pages-page-builder-provider-health-owner-acceptance-actualization-2026-08-09.md",
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
let evaluatorContract = {};
let transportEvidence = {};
if (failures.length === 0) {
  for (const [label, target] of [
    ["contract", "contract"],
    ["evaluatorContract", "evaluatorContract"],
    ["transportEvidence", "transportEvidence"],
  ]) {
    try {
      const parsed = JSON.parse(sources[label]);
      if (target === "contract") contract = parsed;
      if (target === "evaluatorContract") evaluatorContract = parsed;
      if (target === "transportEvidence") transportEvidence = parsed;
    } catch (error) {
      failures.push(`${label}: invalid JSON: ${error.message}`);
    }
  }
}

if (contract.format !== "pages_builder_provider_health_owner_acceptance_source_v1") failures.push("owner acceptance contract format drifted");
if (contract.status !== "source_ready_maintainer_execution_pending") failures.push("owner acceptance contract must remain execution-pending");
if (evaluatorContract.output?.format !== "page_builder_provider_health_deployment_evaluation_v1") failures.push("evaluator output format drifted");
if (evaluatorContract.output?.status !== "deployment_health_evaluated_pages_binding_pending") failures.push("evaluator output status drifted");
if (transportEvidence.graphql_owner?.current_server_observed_value !== false) failures.push("transport evidence must keep server health unobserved");
if (transportEvidence.graphql_owner?.owner_binding_to_retained_evaluator_packet_present !== false) failures.push("transport evidence must not claim server binding");

for (const [object, key, expected] of [
  [contract.evaluation_input, "must_reside_under_repository_target", true],
  [contract.evaluation_input, "source_commit_must_equal_checkout_head", true],
  [contract.evaluation_input, "source_hashes_must_match_checkout", true],
  [contract.evaluation_input, "expected_target_count_must_equal_verified_backend_target_count", true],
  [contract.evaluation_input, "target_mapping_complete_must_be_true", true],
  [contract.evaluation_input, "query_window_revalidated_against_evaluator_bounds", true],
  [contract.evaluation_input, "freshness_window_revalidated_against_evaluator_bounds", true],
  [contract.evaluation_input, "identity_age_revalidated_against_query_window_and_evaluator_maximum", true],
  [contract.evaluation_input, "evaluation_timestamps_must_be_canonical_iso", true],
  [contract.evaluation_input, "target_freshness_age_must_not_exceed_retained_freshness_window", true],
  [contract.evaluation_input, "minimum_preview_samples", 20],
  [contract.evaluation_input, "minimum_publish_samples", 20],
  [contract.evaluation_input, "histogram_completion_population_must_match", true],
  [contract.evaluation_input, "canonical_health_snapshot_recomputed", true],
  [contract.evaluation_input, "canonical_slo_evaluation_recomputed", true],
  [contract.owner_decision, "accept_requires_explicit_rollback_action", true],
  [contract.owner_decision, "accepted_rollback_action", "restore_unobserved_provider_health"],
  [contract.owner_decision, "cryptographic_signature_required", false],
  [contract.owner_decision, "owner_identity_is_operator_assertion", true],
  [contract.binding_boundary, "accepted_packet_can_authorize_future_server_binding", true],
  [contract.binding_boundary, "accepted_packet_does_not_perform_server_binding", true],
  [contract.binding_boundary, "server_binding_must_revalidate_exact_source_commit", true],
  [contract.binding_boundary, "server_binding_must_revalidate_exact_deployment_image_digest", true],
  [contract.binding_boundary, "server_binding_failure_action", "restore_unobserved_provider_health"],
  [contract.non_claims, "owner_acceptance_executed", false],
  [contract.non_claims, "server_binding_performed", false],
  [contract.non_claims, "pages_provider_health_observed", false],
  [contract.non_claims, "pages_reference_consumer_gate_accepted", false],
  [contract.non_claims, "forum_wave_accepted", false],
]) {
  if (object?.[key] !== expected) failures.push(`${key} must equal ${JSON.stringify(expected)}`);
}
if (!contract.owner_decision?.decisions?.includes("accept_for_pages_binding") || !contract.owner_decision?.decisions?.includes("reject")) {
  failures.push("owner acceptance decisions must include accept_for_pages_binding and reject");
}

for (const marker of [
  "--evaluation",
  "--owner-id",
  "--decision",
  "--rollback-action",
  'const ACCEPT_DECISION = "accept_for_pages_binding"',
  'const REJECT_DECISION = "reject"',
  'const ROLLBACK_ACTION = "restore_unobserved_provider_health"',
  "evaluation packet must reside under repository target/",
  "evaluation source commit does not equal checkout HEAD",
  "evaluation source file set does not match evaluator source contract",
  "evaluation source hash for ${relativePath} does not match checkout",
  "evaluation target counts are incomplete",
  "evaluation query window is outside evaluator contract bounds",
  "evaluation freshness window is outside evaluator contract bounds",
  "evaluation identity age is outside admitted bounds",
  "evaluation identity age does not match retained timestamps",
  "canonicalIsoTimestamp",
  "evaluation target mapping is not complete",
  "preview freshness age`, 0, freshnessWindow",
  "publish freshness age`, 0, freshnessWindow",
  "evaluation histogram populations do not match terminal completion populations",
  "evaluation provider-health state does not match canonical policy",
  "evaluation degradation reasons do not match canonical policy",
  "evaluation contains a forbidden promotion claim",
  "accepted decision requires --rollback-action",
  "server_binding_authorized: accepted",
  "server_binding_performed: false",
  "required_live_source_commit: admitted.sourceCommit",
  "required_deployment_image_digest: admitted.deploymentImageDigest",
  "pages_provider_health_observed: false",
  "pages_reference_consumer_gate_accepted: false",
]) need(sources.runner ?? "", marker, "owner acceptance runner");

for (const marker of [
  "preview_p95_ms: 1500",
  "publish_p95_ms: 3000",
  "sanitize_failure_rate_max: 0.01",
  "runtime_error_rate_max: 0.01",
  "MINIMUM_SAMPLES_PER_OPERATION = 20",
]) need(sources.runner ?? "", marker, "acceptance health policy parity");

for (const marker of [
  "format: contract.output.format",
  "status: contract.output.status",
  "source_files: sourceHashes(contract)",
  "pages_provider_health_observed: false",
]) need(sources.evaluator ?? "", marker, "deployment evaluator predecessor");

// Source readiness must not activate any production Pages consumer.
for (const marker of [
  "provider_health_observed: false",
  "provider_health: None",
]) need(sources.owner ?? "", marker, "Pages GraphQL remains unobserved");
need(sources.builder ?? "", ".map(PageBuilderAdminProviderStatus::unobserved)", "Pages SSR facade remains unobserved");
for (const marker of [".flags;", "provider_flags: BuilderCapabilityFlags", ".with_provider_flags(provider_flags)"])
  need(sources.composition ?? "", marker, "workspace remains rollout-only");
for (const marker of ["pages_editor_capabilities_for_rollout(", "&rollout.flags"])
  need(sources.adminMain ?? "", marker, "browser intent remains rollout-only");
for (const source of [sources.owner ?? "", sources.composition ?? "", sources.builder ?? "", sources.adminMain ?? ""]) {
  forbid(source, "pages_builder_provider_health_owner_acceptance_v1", "production consumer must not load owner acceptance packet yet");
}

for (const marker of [
  "owner-acceptance-packet-source-ready",
  "maintainer-execution-pending",
  "operator assertion",
  "restore_unobserved_provider_health",
  "does not perform server binding",
  "Pages remains `unobserved`",
  "tests were not run",
]) need(sources.overlay ?? "", marker, "owner acceptance actualization");
for (const marker of [
  "provider-health-owner-acceptance-source-ready",
  "pages-page-builder-provider-health-owner-acceptance-actualization-2026-08-09.md",
  "owner acceptance packet",
  "Pages remains `unobserved`",
]) need(sources.parity ?? "", marker, "plan parity actualization");

if (contract.next_cursor?.owner_acceptance_packet !== "source_ready_maintainer_execution_pending") failures.push("owner acceptance packet cursor drifted");
if (contract.next_cursor?.server_owner_health_binding !== "blocked_on_accepted_owner_packet") failures.push("server binding must remain blocked on accepted owner packet");
if (contract.next_cursor?.observed_health_acceptance !== "pending") failures.push("observed health acceptance must remain pending");

if (failures.length > 0) {
  console.error("[verify-pages-builder-provider-health-owner-acceptance] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("[verify-pages-builder-provider-health-owner-acceptance] PASS acceptance=source_ready execution=pending server_binding=blocked pages_health=unobserved");
