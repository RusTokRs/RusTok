#!/usr/bin/env node

import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-iggy/contracts/evidence/contract-poison-external-iggy-dedup-execution-contract.json";
const runnerPath =
  "scripts/evidence/capture-iggy-contract-poison-external-dedup.mjs";
const verifierPath =
  "scripts/verify/verify-iggy-contract-poison-external-dedup-retained-evidence.mjs";
const evidencePath =
  "crates/rustok-iggy/contracts/evidence/contract-poison-external-iggy-dedup-execution.json";
const sourceContractPath =
  "crates/rustok-iggy/contracts/evidence/contract-poison-external-iggy-dedup-source.json";
const testPath =
  "crates/rustok-iggy/tests/contract_poison_external_iggy_dedup.rs";

const contract = readJson(contractPath);
const sourceContract = readJson(sourceContractPath);
const runner = readText(runnerPath);
const test = readText(testPath);

const expectedScenarios = [
  {
    case: "disabled_deduplication_persists_repeated_uuid_twice",
    address_env: "RUSTOK_IGGY_DEDUP_DISABLED_ADDRESS",
    config_path_env: "RUSTOK_IGGY_DEDUP_DISABLED_CONFIG_PATH",
    server_artifact_env: "RUSTOK_IGGY_DEDUP_DISABLED_SERVER_ARTIFACT",
    expected_configuration: {
      enabled: false,
      max_entries: "optional",
      expiry: "optional",
    },
    expected_partition_message_counts: [0, 1, 2],
  },
  {
    case: "enabled_deduplication_suppresses_immediate_repeated_uuid",
    address_env: "RUSTOK_IGGY_DEDUP_ENABLED_ADDRESS",
    config_path_env: "RUSTOK_IGGY_DEDUP_ENABLED_CONFIG_PATH",
    server_artifact_env: "RUSTOK_IGGY_DEDUP_ENABLED_SERVER_ARTIFACT",
    expected_configuration: {
      enabled: true,
      max_entries: "at_least_1",
      expiry: "positive_duration",
    },
    expected_partition_message_counts: [0, 1, 1],
  },
  {
    case: "bounded_deduplication_capacity_eviction_accepts_old_uuid_again",
    address_env: "RUSTOK_IGGY_DEDUP_CAPACITY_ADDRESS",
    config_path_env: "RUSTOK_IGGY_DEDUP_CAPACITY_CONFIG_PATH",
    server_artifact_env: "RUSTOK_IGGY_DEDUP_CAPACITY_SERVER_ARTIFACT",
    expected_configuration: {
      enabled: true,
      max_entries: 1,
      expiry: "positive_duration",
    },
    expected_partition_message_counts: [0, 1, 1, 2, 3],
  },
  {
    case: "expired_deduplication_entry_accepts_same_uuid_after_bounded_wait",
    address_env: "RUSTOK_IGGY_DEDUP_EXPIRY_ADDRESS",
    config_path_env: "RUSTOK_IGGY_DEDUP_EXPIRY_CONFIG_PATH",
    server_artifact_env: "RUSTOK_IGGY_DEDUP_EXPIRY_SERVER_ARTIFACT",
    additional_env: "RUSTOK_IGGY_DEDUP_EXPIRY_WAIT_MS",
    expected_configuration: {
      enabled: true,
      max_entries: "at_least_1",
      expiry: "positive_duration_shorter_than_wait",
    },
    expected_partition_message_counts: [0, 1, 1, 2],
  },
];
const expectedCommandTemplate = {
  program: "cargo",
  args_before_case: [
    "test",
    "-p",
    "rustok-iggy",
    "--features",
    "iggy",
    "--test",
    "contract_poison_external_iggy_dedup",
    "--",
  ],
  args_after_case: ["--exact", "--nocapture", "--test-threads=1"],
};
const expectedSourceFiles = [
  testPath,
  "crates/rustok-iggy/src/contract_decode_failure.rs",
  "crates/rustok-iggy/src/dlq.rs",
  "crates/rustok-iggy/src/dlq_publisher.rs",
  "crates/rustok-iggy/src/partitioning.rs",
  "crates/rustok-iggy/src/transport.rs",
];
const expectedMetadata = [
  "git_commit",
  "started_at",
  "completed_at",
  "cargo_version",
  "rustc_version",
  "source_sha256",
  "combined_test_output_sha256",
  "server_artifact",
  "reviewed_configuration",
  "test_output_sha256",
  "test_output_bytes",
];

function fail(message) {
  throw new Error(message);
}

function readText(relativePath) {
  return readFileSync(resolve(repoRoot, relativePath), "utf8");
}

function readJson(relativePath) {
  return JSON.parse(readText(relativePath));
}

function sameValue(actual, expected) {
  return JSON.stringify(actual) === JSON.stringify(expected);
}

function sameSet(actual, expected) {
  return (
    Array.isArray(actual) &&
    actual.length === expected.length &&
    expected.every((value) => actual.includes(value))
  );
}

function requireText(name, source, marker) {
  if (!source.includes(marker)) {
    fail(`${name} is missing required marker: ${marker}`);
  }
}

function forbidText(name, source, marker) {
  if (source.includes(marker)) {
    fail(`${name} contains forbidden marker: ${marker}`);
  }
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function sha256File(relativePath) {
  return sha256(readFileSync(resolve(repoRoot, relativePath)));
}

function requireExactKeys(name, value, expectedKeys) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    fail(`${name} must be an object`);
  }
  if (!sameSet(Object.keys(value), expectedKeys)) {
    fail(`${name} contains missing or unexpected fields`);
  }
}

function requireIsoTimestamp(name, value) {
  if (typeof value !== "string" || Number.isNaN(Date.parse(value))) {
    fail(`${name} must be an ISO-8601 timestamp`);
  }
}

function scenarioCommand(caseName) {
  return {
    program: expectedCommandTemplate.program,
    args: [
      ...expectedCommandTemplate.args_before_case,
      caseName,
      ...expectedCommandTemplate.args_after_case,
    ],
  };
}

function validateConfiguration(name, configuration, expected, waitMs) {
  requireExactKeys(name, configuration, [
    "section",
    "enabled",
    "max_entries",
    "expiry",
    "expiry_milliseconds",
    "canonical_sha256",
  ]);
  if (configuration.section !== "system.message_deduplication") {
    fail(`${name} section drift`);
  }
  if (configuration.enabled !== expected.enabled) {
    fail(`${name} enabled value drift`);
  }
  if (
    configuration.max_entries !== null &&
    (!Number.isSafeInteger(configuration.max_entries) || configuration.max_entries <= 0)
  ) {
    fail(`${name} max_entries is invalid`);
  }
  if (expected.max_entries === "at_least_1") {
    if (configuration.max_entries === null || configuration.max_entries < 1) {
      fail(`${name} requires max_entries >= 1`);
    }
  } else if (Number.isInteger(expected.max_entries)) {
    if (configuration.max_entries !== expected.max_entries) {
      fail(`${name} max_entries drift`);
    }
  }
  if (configuration.expiry === null) {
    if (configuration.expiry_milliseconds !== null) {
      fail(`${name} has expiry milliseconds without expiry text`);
    }
  } else {
    if (
      typeof configuration.expiry !== "string" ||
      !configuration.expiry ||
      configuration.expiry.length > 128 ||
      !Number.isSafeInteger(configuration.expiry_milliseconds) ||
      configuration.expiry_milliseconds <= 0
    ) {
      fail(`${name} expiry metadata is invalid`);
    }
  }
  if (
    expected.expiry === "positive_duration" &&
    configuration.expiry_milliseconds === null
  ) {
    fail(`${name} requires a positive expiry`);
  }
  if (expected.expiry === "positive_duration_shorter_than_wait") {
    if (
      configuration.expiry_milliseconds === null ||
      configuration.expiry_milliseconds >= waitMs
    ) {
      fail(`${name} expiry must be shorter than retained wait`);
    }
  }
  const canonical = {
    section: configuration.section,
    enabled: configuration.enabled,
    max_entries: configuration.max_entries,
    expiry: configuration.expiry,
    expiry_milliseconds: configuration.expiry_milliseconds,
  };
  if (configuration.canonical_sha256 !== sha256(JSON.stringify(canonical))) {
    fail(`${name} canonical configuration digest drift`);
  }
}

function verifyContract() {
  if (contract.schema_version !== 1) fail("retained dedup contract schema drift");
  if (contract.module !== "iggy") fail("retained dedup contract module drift");
  if (contract.packet !== "contract-poison-external-iggy-dedup-execution-contract") {
    fail("retained dedup contract packet drift");
  }
  if (contract.status !== "runtime_execution_contract_locked") {
    fail("retained dedup contract status drift");
  }
  if (contract.runner !== runnerPath) fail("retained dedup runner path drift");
  if (contract.verifier !== verifierPath) fail("retained dedup verifier path drift");
  if (contract.evidence_path !== evidencePath) fail("retained dedup output path drift");
  if (contract.execution_scope !== "four_external_iggy_dedup_modes_runtime_pending") {
    fail("retained dedup execution scope drift");
  }
  if (
    contract.promotion_gate !==
    "requires_clean_commit_distinct_brokers_reviewed_configs_and_all_cases_passed"
  ) {
    fail("retained dedup promotion gate drift");
  }
  if (contract.test_target !== "contract_poison_external_iggy_dedup") {
    fail("retained dedup target drift");
  }
  if (
    !sameValue(contract.shared_optional_environment, [
      "RUSTOK_IGGY_DEDUP_TEST_USERNAME",
      "RUSTOK_IGGY_DEDUP_TEST_PASSWORD",
    ])
  ) {
    fail("retained dedup credential environment drift");
  }
  if (!sameValue(contract.scenarios, expectedScenarios)) {
    fail("retained dedup scenario contract drift");
  }
  if (!sameValue(contract.command_template, expectedCommandTemplate)) {
    fail("retained dedup command template drift");
  }
  if (
    !sameValue(contract.reviewed_configuration, {
      section: "system.message_deduplication",
      persisted_fields: [
        "enabled",
        "max_entries",
        "expiry",
        "expiry_milliseconds",
        "canonical_sha256",
      ],
      full_config_path_persisted: false,
      full_config_sha256_persisted: false,
      full_config_content_persisted: false,
    })
  ) {
    fail("retained dedup reviewed configuration boundary drift");
  }
  if (!sameValue(contract.source_files, expectedSourceFiles)) {
    fail("retained dedup source file set drift");
  }
  if (!sameSet(contract.required_metadata, expectedMetadata)) {
    fail("retained dedup metadata contract drift");
  }
  if (
    !sameValue(contract.privacy_boundary, {
      forbidden_persisted_values: [
        "broker_address",
        "config_path",
        "username",
        "password",
        "connection_string",
        "raw_test_output",
        "delivery_uuid",
        "payload",
        "source_coordinates",
      ],
    })
  ) {
    fail("retained dedup privacy boundary drift");
  }
  if (contract.evidence_status !== "runtime_execution_pending") {
    fail("retained dedup contract must not claim unexecuted evidence");
  }
  if (sourceContract.execution_status !== "not_run") {
    fail("dedup source contract must remain unexecuted in source-only changes");
  }
}

function verifyRunnerAndSource() {
  for (const marker of [
    "validateContractBoundary()",
    "validateCredentialsPair()",
    "expiryWaitMilliseconds()",
    "absoluteExternalConfigPath(",
    "must point outside the repository",
    "parseDedupConfiguration(",
    'section: "system.message_deduplication"',
    "canonical_sha256: sha256(JSON.stringify(canonical))",
    "retained dedup execution requires four distinct broker addresses",
    "retained dedup execution requires four distinct reviewed config files",
    '["status", "--porcelain=v1", "--untracked-files=all"]',
    '["rev-parse", "HEAD"]',
    "requirePassedCase(output, reviewed.scenario.case)",
    "running 1 test",
    "scenarioCommand(reviewed.scenario.case)",
    "ensureCleanAfterExecution()",
    "writeAtomically({",
    "renameSync(temporaryPath, outputPath)",
    "combined_test_output_sha256: sha256(combinedOutput)",
  ]) {
    requireText("retained dedup runner", runner, marker);
  }
  for (const forbidden of [
    "broker_address:",
    "config_path:",
    "username:",
    "password:",
    "connection_string:",
    "raw_test_output:",
    "delivery_uuid:",
    "payload:",
    "source_coordinates:",
    "full_config_sha256",
    "full_config_content",
  ]) {
    forbidText("retained dedup packet projection", runner, forbidden);
  }
  for (const scenario of expectedScenarios) {
    requireText("dedup source test", test, `async fn ${scenario.case}()`);
  }
}

function verifyExecutedEvidence() {
  const absoluteEvidencePath = resolve(repoRoot, evidencePath);
  if (!existsSync(absoluteEvidencePath)) {
    console.log(
      "External Iggy dedup retained evidence contract verified; four-mode runtime execution remains pending.",
    );
    return;
  }

  const evidenceText = readText(evidencePath);
  const evidence = JSON.parse(evidenceText);
  requireExactKeys("retained dedup evidence", evidence, [
    "schema_version",
    "module",
    "packet",
    "status",
    "generated_from",
    "runner",
    "verifier",
    "git_commit",
    "working_tree_clean_before_run",
    "started_at",
    "completed_at",
    "toolchain",
    "expiry_wait_milliseconds",
    "source_sha256",
    "combined_test_output_sha256",
    "combined_test_output_bytes",
    "executed_scenarios",
  ]);
  if (evidence.schema_version !== 1) fail("retained dedup evidence schema drift");
  if (evidence.module !== "iggy") fail("retained dedup evidence module drift");
  if (evidence.packet !== "contract-poison-external-iggy-dedup-runtime-evidence") {
    fail("retained dedup evidence packet drift");
  }
  if (evidence.status !== "external_iggy_dedup_runtime_executed") {
    fail("retained dedup evidence status drift");
  }
  if (evidence.generated_from !== contractPath) fail("retained dedup generated_from drift");
  if (evidence.runner !== runnerPath) fail("retained dedup runner drift");
  if (evidence.verifier !== verifierPath) fail("retained dedup verifier drift");
  if (!/^[0-9a-f]{40}$/u.test(evidence.git_commit)) {
    fail("retained dedup git commit is invalid");
  }
  if (evidence.working_tree_clean_before_run !== true) {
    fail("retained dedup evidence must originate from a clean tree");
  }
  requireIsoTimestamp("retained dedup started_at", evidence.started_at);
  requireIsoTimestamp("retained dedup completed_at", evidence.completed_at);
  if (Date.parse(evidence.completed_at) < Date.parse(evidence.started_at)) {
    fail("retained dedup completed_at precedes started_at");
  }
  requireExactKeys("retained dedup toolchain", evidence.toolchain, ["cargo", "rustc"]);
  if (!/^cargo\s/u.test(evidence.toolchain.cargo)) fail("retained dedup Cargo version invalid");
  if (!/^rustc\s/u.test(evidence.toolchain.rustc)) fail("retained dedup Rust version invalid");
  if (
    !Number.isSafeInteger(evidence.expiry_wait_milliseconds) ||
    evidence.expiry_wait_milliseconds < 100 ||
    evidence.expiry_wait_milliseconds > 300_000
  ) {
    fail("retained dedup expiry wait is invalid");
  }
  if (!/^[0-9a-f]{64}$/u.test(evidence.combined_test_output_sha256)) {
    fail("retained dedup combined output digest invalid");
  }
  if (
    !Number.isSafeInteger(evidence.combined_test_output_bytes) ||
    evidence.combined_test_output_bytes <= 0
  ) {
    fail("retained dedup combined output byte count invalid");
  }

  requireExactKeys("retained dedup source hashes", evidence.source_sha256, expectedSourceFiles);
  for (const relativePath of expectedSourceFiles) {
    const retainedHash = evidence.source_sha256[relativePath];
    if (!/^[0-9a-f]{64}$/u.test(retainedHash)) {
      fail(`retained dedup source hash invalid: ${relativePath}`);
    }
    if (retainedHash !== sha256File(relativePath)) {
      fail(`retained dedup evidence is stale for source file: ${relativePath}`);
    }
  }

  if (
    !Array.isArray(evidence.executed_scenarios) ||
    evidence.executed_scenarios.length !== expectedScenarios.length
  ) {
    fail("retained dedup executed scenario count drift");
  }
  for (const expected of expectedScenarios) {
    const scenario = evidence.executed_scenarios.find(
      (candidate) => candidate.case === expected.case,
    );
    requireExactKeys(`retained dedup scenario ${expected.case}`, scenario, [
      "case",
      "result",
      "address_source_env",
      "configuration_source_env",
      "server_artifact_source_env",
      "server_artifact",
      "reviewed_configuration",
      "expected_partition_message_counts",
      "command",
      "test_output_sha256",
      "test_output_bytes",
    ]);
    if (scenario.result !== "pass") fail(`retained dedup scenario failed: ${expected.case}`);
    if (scenario.address_source_env !== expected.address_env) {
      fail(`retained dedup address env drift: ${expected.case}`);
    }
    if (scenario.configuration_source_env !== expected.config_path_env) {
      fail(`retained dedup config env drift: ${expected.case}`);
    }
    if (scenario.server_artifact_source_env !== expected.server_artifact_env) {
      fail(`retained dedup artifact env drift: ${expected.case}`);
    }
    if (
      typeof scenario.server_artifact !== "string" ||
      !scenario.server_artifact.trim() ||
      scenario.server_artifact.length > 256 ||
      /[\u0000-\u001f\u007f]/u.test(scenario.server_artifact)
    ) {
      fail(`retained dedup server artifact invalid: ${expected.case}`);
    }
    validateConfiguration(
      `retained dedup configuration ${expected.case}`,
      scenario.reviewed_configuration,
      expected.expected_configuration,
      evidence.expiry_wait_milliseconds,
    );
    if (
      !sameValue(
        scenario.expected_partition_message_counts,
        expected.expected_partition_message_counts,
      )
    ) {
      fail(`retained dedup count sequence drift: ${expected.case}`);
    }
    if (!sameValue(scenario.command, scenarioCommand(expected.case))) {
      fail(`retained dedup command drift: ${expected.case}`);
    }
    if (!/^[0-9a-f]{64}$/u.test(scenario.test_output_sha256)) {
      fail(`retained dedup scenario output digest invalid: ${expected.case}`);
    }
    if (!Number.isSafeInteger(scenario.test_output_bytes) || scenario.test_output_bytes <= 0) {
      fail(`retained dedup scenario output byte count invalid: ${expected.case}`);
    }
  }

  for (const forbiddenKey of contract.privacy_boundary.forbidden_persisted_values) {
    const pattern = new RegExp(`"${forbiddenKey}"\\s*:`, "u");
    if (pattern.test(evidenceText)) {
      fail(`retained dedup evidence contains forbidden persisted field: ${forbiddenKey}`);
    }
  }
  if (/iggy:\/\//iu.test(evidenceText)) {
    fail("retained dedup evidence must not contain an Iggy connection string");
  }
  console.log(
    "External Iggy dedup retained evidence verified: clean commit, reviewed non-secret configuration digests, current source hashes, exact per-case commands, and all four behavior scenarios passed.",
  );
}

try {
  verifyContract();
  verifyRunnerAndSource();
  verifyExecutedEvidence();
} catch (error) {
  console.error(`External Iggy dedup retained evidence verification failed: ${error.message}`);
  process.exit(1);
}
