#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const failures = [];
const files = {
  contract:
    "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-observed-acceptance-runner-test-source.json",
  runner: "scripts/evidence/accept-pages-builder-provider-health-runtime.mjs",
  test: "scripts/evidence/accept-pages-builder-provider-health-runtime.test.mjs",
  workflow: ".github/workflows/pages-page-builder-provider-health.yml",
  overlay:
    "docs/modules/pages-page-builder-provider-health-owner-runner-tests-actualization-2026-08-12.md",
};

function absolute(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  const location = absolute(relativePath);
  if (!fs.existsSync(location)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  const metadata = fs.lstatSync(location);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    failures.push(`${relativePath}: must be a regular non-symlink file`);
    return "";
  }
  return fs.readFileSync(location, "utf8");
}

function need(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing '${marker}'`);
}

function forbid(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden '${marker}'`);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
let contract = {};
try {
  contract = JSON.parse(sources.contract);
} catch (error) {
  failures.push(`contract: invalid JSON: ${error.message}`);
}

if (
  contract.schema_version !== 1 ||
  contract.module !== "pages" ||
  contract.packet !== "pages-builder-provider-health-observed-acceptance-runner-test-source" ||
  contract.status !== "source_ready_ci_execution_pending"
) {
  failures.push("runner-test source identity drifted");
}
if (contract.production_runner !== files.runner || contract.test_runner !== files.test) {
  failures.push("runner-test source paths drifted");
}
if (contract.production_output_format !== "pages_builder_provider_health_observed_acceptance_v1") {
  failures.push("runner-test production output format drifted");
}
if (!Array.isArray(contract.cases) || contract.cases.length !== 7) {
  failures.push("runner-test contract must retain exactly seven synthetic cases");
}

for (const [key, expected] of [
  ["uses_current_checkout_head", true],
  ["recomputes_required_source_hashes_from_current_checkout", true],
  ["uses_synthetic_ready_health_snapshot", true],
  ["uses_synthetic_packet_files_under_repository_target", true],
  ["spawns_production_runner_as_child_process", true],
  ["does_not_import_or_reimplement_production_runner_decision_logic", true],
  ["cleans_test_packets_after_execution", true],
]) {
  if (contract.fixture_policy?.[key] !== expected) {
    failures.push(`fixture_policy.${key} must equal ${expected}`);
  }
}
for (const key of [
  "deployment_identity_capture_executed",
  "prometheus_query_executed",
  "deployment_evaluator_executed",
  "server_binding_activated",
  "graphql_or_http_executed",
  "browser_executed",
  "observed_runtime_evidence_from_live_deployment_claimed",
  "owner_observed_health_decision_from_live_deployment_claimed",
  "current_provider_health_asserted",
  "pages_reference_consumer_gate_accepted",
  "forum_wave_accepted",
  "ffa_promoted",
  "fba_promoted",
]) {
  if (contract.boundaries?.[key] !== false) failures.push(`boundaries.${key} must remain false`);
}

const requiredFiles = new Set(contract.required_source_files ?? []);
for (const relativePath of Object.values(files)) {
  if (!requiredFiles.has(relativePath)) {
    failures.push(`contract.required_source_files missing ${relativePath}`);
  }
}

for (const marker of [
  'execFileSync("git", ["rev-parse", "HEAD"]',
  "function sourceHashes(contract)",
  "contract.required_source_files.map",
  "spawnSync(",
  "accept_observed_runtime_evidence",
  "owner_accepted_observed_runtime_evidence_gate_review_pending",
  "owner_rejected_observed_runtime_evidence",
  "runtime-source-hash-tamper",
  "runtime-after-health-deadline",
  "gate-overclaim",
  "privacy-overclaim",
  "binding-repodigest-mismatch",
  "runtime evidence source SHA .* does not match checkout",
  "runtime evidence was generated after its admitted health lease deadline",
  "runtime boundary pages_reference_consumer_gate_accepted must be false",
  "runtime privacy flag raw_evidence_paths_persisted must be false",
  "binding acceptance deployment identity differs from runtime evidence",
  "rmSync(testRoot, { recursive: true, force: true })",
]) {
  need(sources.test, marker, "runner synthetic test");
}
for (const marker of [
  "@playwright/test",
  "fetch(",
  "http.request",
  "https.request",
  "prom-client",
]) {
  forbid(sources.test, marker, "runner synthetic test");
}

for (const marker of [
  'const ACCEPT_DECISION = "accept_observed_runtime_evidence"',
  'const REJECT_DECISION = "reject"',
  'spawnSync("git", ["rev-parse", "HEAD"]',
  'live_binding_action: "unchanged"',
  "health_lease_extended: false",
  "current_provider_health_asserted: false",
  "pages_reference_consumer_gate_accepted: false",
]) {
  need(sources.runner, marker, "production owner-acceptance runner");
}

for (const command of contract.ci?.commands ?? []) {
  need(sources.workflow, command, "focused provider-health workflow");
}
for (const marker of [
  "permissions:",
  "contents: read",
  "Observed-health owner acceptance runner synthetic tests",
]) {
  need(sources.workflow, marker, "focused provider-health workflow");
}

for (const marker of [
  "provider-health-observed-acceptance-runner-tested",
  "seven synthetic fail-closed cases",
  files.test,
  files.workflow,
  "Live provider-health execution remains pending",
  "Pages reference-consumer gate remains unaccepted",
]) {
  need(sources.overlay, marker, "runner-test actualization");
}

if (
  contract.next_cursor?.observed_health_owner_acceptance_runner !==
  "synthetic_fail_closed_ci_coverage_source_ready"
) {
  failures.push("runner-test next cursor drifted");
}
if (
  contract.next_cursor?.live_identity_evaluator_binding_acceptance_runtime_evidence !==
  "maintainer_execution_pending"
) {
  failures.push("live provider-health execution cursor must remain maintainer-owned");
}
if (
  contract.next_cursor?.pages_reference_consumer_gate_acceptance !==
  "blocked_on_retained_live_execution_and_owner_decision"
) {
  failures.push("Pages gate cursor must remain blocked on live evidence and owner decision");
}

if (failures.length > 0) {
  console.error("[verify-pages-builder-provider-health-owner-runner-tests] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "[verify-pages-builder-provider-health-owner-runner-tests] PASS synthetic_runner_coverage=source_ready live_execution=pending pages_gate=unaccepted",
);
