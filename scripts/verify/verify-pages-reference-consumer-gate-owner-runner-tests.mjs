#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const failures = [];
const files = {
  contract:
    "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-acceptance-runner-test-source.json",
  acceptance:
    "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-acceptance-source.json",
  runner: "scripts/evidence/accept-pages-reference-consumer-gate.mjs",
  test: "scripts/evidence/accept-pages-reference-consumer-gate.test.mjs",
  workflow: ".github/workflows/pages-page-builder-provider-health.yml",
  overlay:
    "docs/modules/pages-page-builder-reference-consumer-gate-owner-runner-tests-actualization-2026-08-12.md",
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
let acceptance = {};
try {
  contract = JSON.parse(sources.contract);
} catch (error) {
  failures.push(`contract: invalid JSON: ${error.message}`);
}
try {
  acceptance = JSON.parse(sources.acceptance);
} catch (error) {
  failures.push(`acceptance source: invalid JSON: ${error.message}`);
}

if (
  contract.schema_version !== 1 ||
  contract.module !== "pages" ||
  contract.packet !==
    "pages-reference-consumer-gate-acceptance-runner-test-source" ||
  contract.status !== "source_ready_ci_execution_pending"
) {
  failures.push("gate owner-runner test source identity drifted");
}
if (
  contract.production_runner !== files.runner ||
  contract.test_runner !== files.test ||
  contract.production_output_format !==
    "pages_reference_consumer_gate_acceptance_v1"
) {
  failures.push("gate owner-runner source paths/output format drifted");
}
if (!Array.isArray(contract.cases) || contract.cases.length !== 9) {
  failures.push("gate owner-runner contract must retain exactly nine synthetic cases");
}

for (const [key, expected] of [
  ["uses_current_checkout_head", true],
  ["recomputes_candidate_and_observed_source_hashes_from_current_checkout", true],
  ["uses_synthetic_candidate_and_retrospective_observed_health_packets", true],
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
  "reference_candidate_live_execution_claimed",
  "observed_health_live_execution_claimed",
  "owner_gate_decision_from_live_evidence_claimed",
  "rollback_action_executed": false,
  "canonical_source_mutated",
  "current_provider_health_asserted",
  "pages_reference_consumer_gate_accepted_in_source",
  "forum_wave_accepted",
  "ffa_promoted",
  "fba_promoted",
]) {
  if (contract.boundaries?.[key] !== false) {
    failures.push(`boundaries.${key} must remain false`);
  }
}

const requiredFiles = new Set(contract.required_source_files ?? []);
for (const relativePath of Object.values(files)) {
  if (!requiredFiles.has(relativePath)) {
    failures.push(`contract.required_source_files missing ${relativePath}`);
  }
}

if (
  acceptance.owner_decision?.runner !== files.runner ||
  acceptance.output?.format !== "pages_reference_consumer_gate_acceptance_v1" ||
  acceptance.output?.accepted_status !==
    "owner_accepted_pages_reference_consumer_gate" ||
  acceptance.output?.rejected_status !==
    "owner_rejected_pages_reference_consumer_gate"
) {
  failures.push("gate acceptance source/runner output contract drifted");
}

for (const marker of [
  'execFileSync("git", ["rev-parse", "HEAD"]',
  "function sourceHashes(contract)",
  "sourceHashes(candidateContract)",
  "sourceHashes(observedSource)",
  "spawnSync(",
  "accept_pages_reference_consumer_gate",
  "owner_accepted_pages_reference_consumer_gate",
  "owner_rejected_pages_reference_consumer_gate",
  "accept-rollback-mismatch",
  "candidate-source-hash-tamper",
  "candidate-command-drift",
  "candidate-provider-health-overclaim",
  "observed-deployment-digest-mismatch",
  "observed-current-health-overclaim",
  "observed-gate-eligibility-revoked",
  "reference candidate source hash .* does not match checkout",
  "id\\/program\\/argv differs from execution contract",
  "reference candidate provider_health must remain unobserved",
  "observed-health acceptance deployment digest differs from reference candidate",
  "observed-health acceptance must remain retrospective",
  "observed-health aceptance gate boundary drifted",
  "rmSync(testRoot, { recursive: true, force: true })",
]) {
  need(sources.test, marker, "gate owner-runner synthetic test");
}

for (const marker of [
  "@playwright/test",
  "fetch(",
  "http.request",
  "https.request",
  "prom-client",
  "cargo test",
]) {
  forbid(sources.test, marker, "gate owner-runner synthetic test");
}

for (const marker of [
  'const ACCEPT_DECISION = "accept_pages_reference_consumer_gate"',
  'const REJECT_DECISION = "reject"',
  'const RETAIN_DECISION = "retain_reference_consumer_candidate"',
  'const ROLLBACK_DECISION = "rollback_reference_consumer_candidate"',
  'spawnSync("git", ["rev-parse", "HEAD"]',
  'candidate.provider_health !== "unobserved"',
  'observed.current_provider_health_asserted !== false',
  'binding.health_lease_extended !== false',
  "accepted Pages gate requires retain_reference_consumer_candidate rollback decision",
  "rejected Pages gate requires rollback_reference_consumer_candidate rollback decision",
  "rollback_action_performed: false",
  "canonical_source_mutated: false",
  "forum_wave_accepted: false",
  "automatic_downstream_promotion: false",
]) {
  need(sources.runner, marker, "production gate acceptance runner");
}

for (const command of contract.ci?.commands ?? []) {
  need(sources.workflow, command, "focused provider-health/gate workflow");
}
for (const marker of [
  "permissions:",
  "contents: read",
  "concurrency:",
  "cancel-in-progress: true",
  "Pages gate owner acceptance runner synthetic tests",
]) {
  need(sources.workflow, marker, "focused provider-health/gate workflow");
}

for (const marker of [
  "reference-consumer-gate-owner-runner-tested",
  "nine synthetic fail-closed cases",
  files.test,
  files.workflow,
  "live candidate and observed-health execution remain pending",
  "Forum Wave remains unaccepted",
  "does not mutate the source gate",
]) {
  need(sources.overlay, marker, "gate owner-runner actualization");
}

if (
  contract.next_cursor?.reference_consumer_gate_acceptance_runner !==
  "synthetic_fail_closed_ci_coverage_source_ready"
) {
  failures.push("gate owner-runner next cursor drifted");
}
if (
  contract.next_cursor?.candidate_and_observed_health_owner_accepted_packets !==
  "maintainer_execution_pending"
) {
  failures.push("live Pages gate predecessor execution must remain maintainer-owned");
}
if (
  contract.next_cursor?.accepted_pages_gate_packet !==
  "blocked_on_maintainer_execution_and_decision"
) {
  failures.push("accepted Pages gate packet must remain blocked on live evidence");
}
if (
  contract.next_cursor?.forum_wave_admission !==
  "source_ready_maintainer_execution_pending"
) {
  failures.push("Forum Wave admission cursor drifted");
}

if (failures.length > 0) {
  console.error("[verify-pages-reference-consumer-gate-owner-runner-tests] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "[verify-pages-reference-consumer-gate-owner-runner-tests] PASS synthetic_runner_coverage=source_ready live_gate_execution=pending forum_wave=unaccepted",
);
