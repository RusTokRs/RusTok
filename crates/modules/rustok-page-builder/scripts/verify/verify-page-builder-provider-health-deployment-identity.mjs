#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  telemetryMetrics: "crates/libs/rustok-telemetry/src/page_builder_provider_metrics.rs",
  releaseDockerfile: "apps/server/Dockerfile.release",
  releaseWorkflow: ".github/workflows/release.yml",
  capture: "scripts/evidence/capture-page-builder-provider-health-deployment-identity.mjs",
  evidence: "crates/modules/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-identity-source.json",
  pagesGraphql: "crates/modules/rustok-pages/src/graphql/builder_rollout.rs",
  pagesFacade: "crates/modules/rustok-pages/admin/src/builder_rollout_settings.rs",
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
if (failures.length > 0) {
  console.error("[verify-page-builder-provider-health-deployment-identity] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);

if (evidence.format !== "page_builder_provider_health_deployment_identity_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "source_ready_execution_pending") failures.push("evidence status drifted");

for (const [key, expected] of Object.entries({
  runtime_environment: "RUSTOK_SOURCE_COMMIT",
  metric: "rustok_page_builder_provider_build_info",
  metric_label: "source_commit",
  canonical_git_sha_length: 40,
  missing_or_invalid_metric_identity: "fail_closed",
  release_source: "OCI_REVISION",
  canonical_release_value: "github.sha",
})) {
  if (evidence.source_identity?.[key] !== expected) {
    failures.push(`source_identity.${key} must equal ${JSON.stringify(expected)}`);
  }
}

const inventory = evidence.expected_target_inventory ?? {};
for (const [key, expected] of Object.entries({
  authority: "maintainer_supplied_complete_inventory",
  inventory_complete_must_be_true: true,
  minimum_targets: 1,
  maximum_targets: 64,
  target_ids_unique: true,
  metrics_urls_unique: true,
  metrics_url_credentials_forbidden: true,
  metrics_url_query_forbidden: true,
  metrics_url_fragment_forbidden: true,
  redirects_followed: false,
  raw_metrics_urls_retained: false,
})) {
  if (inventory[key] !== expected) {
    failures.push(`expected_target_inventory.${key} must equal ${JSON.stringify(expected)}`);
  }
}

for (const [key, expected] of Object.entries({
  runtime_capture_executed: false,
  deployment_image_digest_cryptographically_proved_by_target: false,
  prometheus_backend_query_executed: false,
  provider_health_snapshot_evaluated: false,
  pages_provider_health_observed: false,
  pages_reference_consumer_gate_accepted: false,
  forum_wave_accepted: false,
  ffa_promoted: false,
  fba_promoted: false,
})) {
  if (evidence.non_claims?.[key] !== expected) {
    failures.push(`non_claims.${key} must equal ${JSON.stringify(expected)}`);
  }
}

, marker, "deployment identity overlay");

, marker, "plan parity actualization");

need(sources.pagesGraphql, "provider_health_observed: false", "Pages GraphQL remains unobserved");
forbid(sources.pagesGraphql, "provider_health_observed: true", "Pages GraphQL health promotion");
need(sources.pagesFacade, "PageBuilderAdminProviderStatus::unobserved", "Pages admin remains unobserved");
forbid(sources.pagesGraphql, "page_builder_provider_build_info", "Pages GraphQL must not bind raw identity metric");
forbid(sources.pagesFacade, "page_builder_provider_build_info", "Pages admin must not bind raw identity metric");

if (evidence.next_cursor?.deployment_identity_capture !== "maintainer_execution_pending") {
  failures.push("deployment identity capture must remain maintainer execution pending");
}
if (evidence.next_cursor?.deployment_health_backend_evaluator !== "open") {
  failures.push("deployment health backend evaluator must remain open");
}
if (
  evidence.next_cursor?.pages_provider_status_binding !==
  "blocked_on_deployment_evaluator_and_runtime_evidence"
) {
  failures.push("Pages provider status binding must remain blocked on evaluator and runtime evidence");
}

if (failures.length > 0) {
  console.error("[verify-page-builder-provider-health-deployment-identity] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "[verify-page-builder-provider-health-deployment-identity] PASS source_ready=true execution=pending evaluator=open pages_health=unobserved",
);
