#!/usr/bin/env node

import { createHash } from "node:crypto";
import { existsSync, readFileSync, statSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const sourceContractPath =
  "crates/rustok-iggy/contracts/evidence/dedup-recovery-window-policy-source.json";
const executionContractPath =
  "crates/rustok-iggy/contracts/evidence/dedup-recovery-window-calibration-execution-contract.json";
const evidencePath =
  "crates/rustok-iggy/contracts/evidence/dedup-recovery-window-calibration-execution.json";
const runnerPath =
  "scripts/evidence/capture-iggy-dedup-recovery-window-calibration.mjs";
const verifierPath =
  "scripts/verify/verify-iggy-dedup-recovery-window-retained.mjs";
const sourceVerifierPath =
  "scripts/verify/verify-iggy-dedup-recovery-window-policy.mjs";
const testPath = "crates/rustok-iggy/tests/dedup_recovery_window_calibration.rs";
const expectedCase = "reviewed_configuration_covers_recovery_window";
const expectedStatus = "iggy.dedup_recovery.sufficient";

const sourceContract = JSON.parse(
  readFileSync(resolve(repoRoot, sourceContractPath), "utf8"),
);
const contract = JSON.parse(
  readFileSync(resolve(repoRoot, executionContractPath), "utf8"),
);
const runner = readFileSync(resolve(repoRoot, runnerPath), "utf8");
const test = readFileSync(resolve(repoRoot, testPath), "utf8");
const failures = [];

function fail(message) {
  failures.push(message);
}

function same(actual, expected) {
  return JSON.stringify(actual) === JSON.stringify(expected);
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function fileSha256(relativePath) {
  const absolutePath = resolve(repoRoot, relativePath);
  if (!existsSync(absolutePath) || !statSync(absolutePath).isFile()) {
    fail(`bound source file is missing: ${relativePath}`);
    return null;
  }
  return sha256(readFileSync(absolutePath));
}

function requireText(name, text, marker) {
  if (!text.includes(marker)) fail(`${name} is missing required marker: ${marker}`);
}

function forbidText(name, text, marker) {
  if (text.includes(marker)) fail(`${name} contains forbidden marker: ${marker}`);
}

function boundedLine(value, field, maximumLength = 256) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > maximumLength ||
    value.trim() !== value ||
    /[\r\n\u0000-\u001f\u007f]/u.test(value)
  ) {
    fail(`${field} is outside the retained one-line boundary`);
    return false;
  }
  return true;
}

function safeInteger(value, field, allowZero = false) {
  if (!Number.isSafeInteger(value) || value < 0 || (!allowZero && value === 0)) {
    fail(`${field} must be a ${allowZero ? "non-negative" : "positive"} safe integer`);
    return false;
  }
  return true;
}

const expectedCommand = {
  program: "cargo",
  args: [
    "test",
    "-p",
    "rustok-iggy",
    "--test",
    "dedup_recovery_window_calibration",
    "--",
    expectedCase,
    "--exact",
    "--nocapture",
    "--test-threads=1",
  ],
};

if (
  contract.schema_version !== 1 ||
  contract.module !== "iggy" ||
  contract.packet !== "dedup-recovery-window-calibration-execution-contract" ||
  contract.status !== "runtime_execution_contract_locked" ||
  contract.source_contract !== sourceContractPath ||
  contract.test_target !== "dedup_recovery_window_calibration" ||
  contract.case !== expectedCase ||
  contract.runner !== runnerPath ||
  contract.verifier !== verifierPath ||
  contract.source_verifier !== sourceVerifierPath ||
  contract.evidence_path !== evidencePath ||
  contract.evidence_status !== "runtime_calibration_pending" ||
  !same(contract.command, expectedCommand)
) {
  fail("dedup recovery-window retained execution contract identity drift");
}

if (
  sourceContract.schema_version !== 2 ||
  sourceContract.status !== "source_complete_retained_calibration_pending" ||
  sourceContract.retained_calibration?.contract !== executionContractPath ||
  sourceContract.retained_calibration?.test !== testPath ||
  sourceContract.retained_calibration?.runner !== runnerPath ||
  sourceContract.retained_calibration?.verifier !== verifierPath ||
  sourceContract.retained_calibration?.evidence_path !== evidencePath ||
  sourceContract.retained_calibration?.canonical_packet_present !== false ||
  sourceContract.retained_calibration?.no_clobber_write !== true
) {
  fail("dedup recovery-window source/retained relationship drift");
}

if (
  contract.expected_assessment?.status !== expectedStatus ||
  contract.expected_assessment?.configured_expiry_not_below_required !== true ||
  contract.expected_assessment?.configured_max_entries_not_below_required !== true ||
  contract.expected_assessment?.stronger_guarantee !== false
) {
  fail("dedup recovery-window expected assessment drift");
}
if (
  contract.bounds_contract?.schema_version !== 1 ||
  contract.bounds_contract?.path_outside_repository !== true ||
  contract.bounds_contract?.full_content_retained !== false ||
  contract.bounds_contract?.full_file_sha256_retained !== false ||
  contract.bounds_contract?.canonical_projection_retained !== true ||
  contract.reviewed_configuration?.section !== "system.message_deduplication" ||
  contract.reviewed_configuration?.required_enabled !== true ||
  contract.reviewed_configuration?.config_path_outside_repository !== true ||
  contract.reviewed_configuration?.full_content_retained !== false ||
  contract.reviewed_configuration?.full_file_sha256_retained !== false ||
  contract.reviewed_configuration?.canonical_projection_retained !== true
) {
  fail("dedup recovery-window reviewed input boundary drift");
}
for (const requirement of Object.values(contract.runner_requirements ?? {})) {
  if (requirement !== true) fail("dedup recovery-window runner requirement weakened");
}
for (const retained of Object.values(contract.retained_packet ?? {})) {
  if (retained !== true && retained !== "pass") {
    fail("dedup recovery-window retained packet requirement weakened");
  }
}

for (const marker of [
  "const SKIP_MESSAGE",
  "reviewed_configuration_covers_recovery_window",
  "IggyDedupRecoveryWindowPolicy::new(",
  "IggyDeduplicationConfiguration::enabled(",
  "IggyDedupRecoveryWindowStatus::Sufficient",
  "RUSTOK_DEDUP_RECOVERY_CALIBRATION status={}",
  "required_expiry_ms={}",
  "configured_expiry_ms={}",
  "required_max_entries_per_partition={}",
  "configured_max_entries={}",
]) {
  requireText("dedup recovery-window calibration test", test, marker);
}
for (const marker of [
  "IggyClient",
  "IggyTransport",
  ".connect(",
  ".poll_messages(",
  ".move_to_dlq(",
  ".acknowledge(",
  ".reserve_and_claim(",
  ".mark_published(",
  ".delete(",
  ".purge(",
]) {
  forbidText("dedup recovery-window calibration test", test, marker);
}

for (const marker of [
  "function reviewedBounds(",
  "function reviewedConfiguration(",
  "function ensureCleanCommit(",
  "function parsePassedAssessment(",
  "function writeNoClobber(",
  "linkSync(temporaryPath, outputPath)",
  "sourceHashes()",
  "working tree must be clean",
  "reported a skip",
  "reviewed Iggy configuration does not cover",
]) {
  requireText("dedup recovery-window capture runner", runner, marker);
}
for (const marker of [
  "broker_address:",
  "username:",
  "password:",
  "raw_test_output:",
  "payload:",
  "delivery_uuid:",
  "ack_token:",
]) {
  forbidText("dedup recovery-window capture packet", runner, marker);
}

const expectedSourceFiles = [
  "crates/rustok-iggy/src/dedup_recovery_window_policy.rs",
  "crates/rustok-iggy/src/lib.rs",
  testPath,
  sourceContractPath,
  executionContractPath,
  runnerPath,
  sourceVerifierPath,
  verifierPath,
];
if (!same(contract.source_files, expectedSourceFiles)) {
  fail("dedup recovery-window retained source hash allowlist drift");
}

const absoluteEvidence = resolve(repoRoot, evidencePath);
if (!existsSync(absoluteEvidence)) {
  if (failures.length > 0) {
    console.error("Iggy dedup recovery-window retained verification failed:");
    for (const failure of failures) console.error(`- ${failure}`);
    process.exit(1);
  }
  console.log(
    "Iggy dedup recovery-window retained source verified: execution contract, exact environment-driven Rust calibration, reviewed bounds/config projections, clean-commit source binding, skip rejection, sufficient-only gate, privacy exclusions, and no-clobber publication are locked; canonical runtime packet is pending.",
  );
  process.exit(0);
}

let packet;
try {
  packet = JSON.parse(readFileSync(absoluteEvidence, "utf8"));
} catch (error) {
  fail(`canonical recovery-window packet is invalid JSON: ${error.message}`);
}

if (packet) {
  if (
    packet.schema_version !== 1 ||
    packet.module !== "iggy" ||
    packet.packet !== "dedup-recovery-window-calibration-runtime-evidence" ||
    packet.status !== "reviewed_recovery_window_sufficient" ||
    packet.generated_from !== executionContractPath ||
    packet.runner !== runnerPath ||
    packet.verifier !== verifierPath ||
    packet.source_verifier !== sourceVerifierPath ||
    packet.case !== expectedCase ||
    packet.result !== "pass" ||
    !same(packet.command, expectedCommand)
  ) {
    fail("canonical recovery-window packet identity drift");
  }

  if (
    typeof packet.git_commit !== "string" ||
    !/^[0-9a-f]{40}$/u.test(packet.git_commit) ||
    packet.working_tree_clean_before_run !== true ||
    packet.working_tree_clean_after_run !== true
  ) {
    fail("canonical recovery-window commit/clean-tree evidence drift");
  }
  if (
    Number.isNaN(Date.parse(packet.started_at)) ||
    Number.isNaN(Date.parse(packet.completed_at)) ||
    Date.parse(packet.completed_at) < Date.parse(packet.started_at)
  ) {
    fail("canonical recovery-window timestamps are invalid");
  }
  boundedLine(packet.toolchain?.cargo, "toolchain.cargo");
  boundedLine(packet.toolchain?.rustc, "toolchain.rustc");
  boundedLine(packet.server_artifact, "server_artifact");

  const bounds = packet.reviewed_bounds;
  if (
    bounds?.schema_version !== 1 ||
    !safeInteger(bounds?.publication_lease_milliseconds, "publication lease") ||
    !safeInteger(bounds?.process_restart_milliseconds, "process restart", true) ||
    !safeInteger(bounds?.transport_reconnect_milliseconds, "transport reconnect", true) ||
    !safeInteger(bounds?.operator_recovery_milliseconds, "operator recovery", true) ||
    !safeInteger(
      bounds?.required_max_entries_per_partition,
      "required max entries per partition",
    ) ||
    !safeInteger(bounds?.required_expiry_milliseconds, "required expiry") ||
    !boundedLine(bounds?.capacity_basis, "capacity_basis") ||
    typeof bounds?.canonical_sha256 !== "string" ||
    !/^[0-9a-f]{64}$/u.test(bounds.canonical_sha256)
  ) {
    fail("canonical reviewed recovery bounds are invalid");
  } else {
    const canonicalBounds = {
      schema_version: 1,
      publication_lease_milliseconds: bounds.publication_lease_milliseconds,
      process_restart_milliseconds: bounds.process_restart_milliseconds,
      transport_reconnect_milliseconds: bounds.transport_reconnect_milliseconds,
      operator_recovery_milliseconds: bounds.operator_recovery_milliseconds,
      required_max_entries_per_partition: bounds.required_max_entries_per_partition,
      capacity_basis: bounds.capacity_basis,
    };
    const requiredExpiry =
      bounds.publication_lease_milliseconds +
      bounds.process_restart_milliseconds +
      bounds.transport_reconnect_milliseconds +
      bounds.operator_recovery_milliseconds;
    if (
      bounds.required_expiry_milliseconds !== requiredExpiry ||
      bounds.canonical_sha256 !== sha256(JSON.stringify(canonicalBounds))
    ) {
      fail("canonical reviewed recovery bounds digest or checked sum drift");
    }
  }

  const configuration = packet.reviewed_configuration;
  if (
    configuration?.section !== "system.message_deduplication" ||
    configuration?.enabled !== true ||
    !safeInteger(configuration?.max_entries, "configured max_entries") ||
    !boundedLine(configuration?.expiry, "configured expiry", 128) ||
    !safeInteger(configuration?.expiry_milliseconds, "configured expiry milliseconds") ||
    typeof configuration?.canonical_sha256 !== "string" ||
    !/^[0-9a-f]{64}$/u.test(configuration.canonical_sha256)
  ) {
    fail("canonical reviewed Iggy configuration is invalid");
  } else {
    const canonicalConfiguration = {
      section: "system.message_deduplication",
      enabled: true,
      max_entries: configuration.max_entries,
      expiry: configuration.expiry,
      expiry_milliseconds: configuration.expiry_milliseconds,
    };
    if (
      configuration.canonical_sha256 !==
      sha256(JSON.stringify(canonicalConfiguration))
    ) {
      fail("canonical reviewed Iggy configuration digest drift");
    }
  }

  if (
    packet.assessment?.status !== expectedStatus ||
    packet.assessment?.required_expiry_milliseconds !==
      packet.reviewed_bounds?.required_expiry_milliseconds ||
    packet.assessment?.configured_expiry_milliseconds !==
      packet.reviewed_configuration?.expiry_milliseconds ||
    packet.assessment?.required_max_entries_per_partition !==
      packet.reviewed_bounds?.required_max_entries_per_partition ||
    packet.assessment?.configured_max_entries !==
      packet.reviewed_configuration?.max_entries ||
    packet.reviewed_configuration?.expiry_milliseconds <
      packet.reviewed_bounds?.required_expiry_milliseconds ||
    packet.reviewed_configuration?.max_entries <
      packet.reviewed_bounds?.required_max_entries_per_partition
  ) {
    fail("canonical sufficient recovery-window assessment drift");
  }

  if (
    typeof packet.test_output_sha256 !== "string" ||
    !/^[0-9a-f]{64}$/u.test(packet.test_output_sha256) ||
    !safeInteger(packet.test_output_bytes, "test_output_bytes") ||
    packet.input_environment_names?.bounds_path !==
      contract.required_environment.bounds_path ||
    packet.input_environment_names?.config_path !==
      contract.required_environment.config_path ||
    packet.input_environment_names?.server_artifact !==
      contract.required_environment.server_artifact
  ) {
    fail("canonical recovery-window provenance drift");
  }

  if (
    packet.source_sha256 === null ||
    Array.isArray(packet.source_sha256) ||
    typeof packet.source_sha256 !== "object" ||
    !same(Object.keys(packet.source_sha256), contract.source_files)
  ) {
    fail("canonical recovery-window source hash map drift");
  } else {
    for (const relativePath of contract.source_files) {
      const retained = packet.source_sha256[relativePath];
      const current = fileSha256(relativePath);
      if (
        typeof retained !== "string" ||
        !/^[0-9a-f]{64}$/u.test(retained) ||
        retained !== current
      ) {
        fail(`canonical recovery-window source hash is stale: ${relativePath}`);
      }
    }
  }

  const forbiddenKeys = new Set(contract.privacy_exclusions);
  function inspect(value) {
    if (Array.isArray(value)) {
      for (const item of value) inspect(item);
      return;
    }
    if (value === null || typeof value !== "object") return;
    for (const [key, nested] of Object.entries(value)) {
      if (forbiddenKeys.has(key)) fail(`canonical packet contains forbidden field: ${key}`);
      inspect(nested);
    }
  }
  inspect(packet);
}

if (failures.length > 0) {
  console.error("Iggy dedup recovery-window retained verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Iggy dedup recovery-window retained evidence verified: one clean commit binds reviewed recovery bounds, per-partition capacity basis, reviewed enabled Iggy configuration, exact sufficient Rust assessment, current source hashes, bounded toolchain/output digests, privacy exclusions, and no-clobber publication without claiming active readback, failover, multi-replica behavior, a database/broker transaction, exactly-once, or Profiles authorization.",
);
