#!/usr/bin/env node

import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-iggy-connector/contracts/evidence/consumer-poison-postgres-execution-contract.json";
const runnerPath = "scripts/evidence/capture-iggy-consumer-poison-postgres.mjs";
const environmentTestPath =
  "crates/rustok-iggy-connector/tests/consumer_poison_receipt_postgres_environment.rs";
const scenarioTestPath =
  "crates/rustok-iggy-connector/tests/consumer_poison_receipt_postgres.rs";

const contract = readJson(contractPath);
const runner = readText(runnerPath);
const environmentTest = readText(environmentTestPath);
const scenarioTest = readText(scenarioTestPath);

const expectedCommands = [
  {
    program: "cargo",
    args: [
      "test",
      "-p",
      "rustok-iggy-connector",
      "--features",
      "migrations",
      "--test",
      "consumer_poison_receipt_postgres_environment",
      "--",
      "--nocapture",
      "--test-threads=1",
    ],
  },
  {
    program: "cargo",
    args: [
      "test",
      "-p",
      "rustok-iggy-connector",
      "--features",
      "migrations",
      "--test",
      "consumer_poison_receipt_postgres",
      "--",
      "--nocapture",
      "--test-threads=1",
    ],
  },
];
const expectedSourceFiles = [
  environmentTestPath,
  scenarioTestPath,
  "crates/rustok-iggy-connector/src/consumer_poison_receipt.rs",
  "crates/rustok-iggy-connector/src/consumer_poison_inspection.rs",
  "crates/rustok-iggy-connector/src/migrations.rs",
];
const expectedCases = [
  {
    case: "concurrent_publishers_have_one_claim_owner",
    assertions: [
      "exactly_one_publisher_claimed",
      "other_publisher_busy",
      "first_error_code_and_attempt_retained_as_one_atomic_pair",
    ],
  },
  {
    case: "expired_lease_is_reclaimed_and_fences_the_previous_publisher",
    assertions: [
      "empty_exact_payload_accepted",
      "expired_publication_lease_reclaimed",
      "previous_publisher_fenced_with_claim_lost",
      "first_diagnostics_unchanged_after_reclaim",
    ],
  },
  {
    case: "conflicts_roll_back_without_overwriting_original_identity",
    assertions: [
      "delivery_uuid_source_collision_rejected",
      "source_coordinate_payload_collision_rejected",
      "original_receipt_unchanged",
      "failed_conflicts_leave_one_receipt",
    ],
  },
  {
    case: "terminal_states_and_aggregate_inspection_remain_consistent",
    assertions: [
      "reserved_published_acknowledged_counts_consistent",
      "expired_publishing_subset_consistent",
      "published_redelivery_does_not_reopen_publication",
      "acknowledged_redelivery_does_not_reopen_publication",
    ],
  },
];
const expectedMetadata = [
  "git_commit",
  "started_at",
  "completed_at",
  "postgres_server_version",
  "postgres_server_version_num",
  "cargo_version",
  "rustc_version",
  "test_output_sha256",
  "source_sha256",
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

function sha256File(relativePath) {
  return createHash("sha256")
    .update(readFileSync(resolve(repoRoot, relativePath)))
    .digest("hex");
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

function verifyContract() {
  if (contract.schema_version !== 1) fail("retained evidence contract schema_version drift");
  if (contract.module !== "iggy-connector") fail("retained evidence contract module drift");
  if (contract.packet !== "consumer-poison-postgres-execution-contract") {
    fail("retained evidence contract packet drift");
  }
  if (contract.status !== "runtime_execution_contract_locked") {
    fail("retained evidence contract status drift");
  }
  if (contract.runner !== runnerPath) fail("retained evidence runner path drift");
  if (contract.verifier !== "scripts/verify/verify-iggy-consumer-poison-retained-evidence.mjs") {
    fail("retained evidence verifier path drift");
  }
  if (
    contract.evidence_path !==
    "crates/rustok-iggy-connector/contracts/evidence/consumer-poison-postgres-execution.json"
  ) {
    fail("retained evidence output path drift");
  }
  if (contract.execution_scope !== "opt_in_postgresql_runtime_execution_pending") {
    fail("retained evidence execution scope must remain explicitly pending until execution");
  }
  if (contract.promotion_gate !== "requires_clean_commit_and_all_required_cases_passed") {
    fail("retained evidence promotion gate drift");
  }
  if (contract.database_url_env !== "RUSTOK_IGGY_CONNECTOR_TEST_DATABASE_URL") {
    fail("retained evidence database env drift");
  }
  if (contract.database_url_fallback_env !== "DATABASE_URL") {
    fail("retained evidence fallback database env drift");
  }
  if (!sameValue(contract.commands, expectedCommands)) {
    fail("retained evidence commands drift");
  }
  if (!sameValue(contract.source_files, expectedSourceFiles)) {
    fail("retained evidence source file set drift");
  }
  if (!sameSet(contract.required_metadata, expectedMetadata)) {
    fail("retained evidence metadata contract drift");
  }
  if (!sameValue(contract.required_cases, expectedCases)) {
    fail("retained evidence required case contract drift");
  }
  if (contract.evidence_status !== "runtime_execution_pending") {
    fail("retained evidence contract must not claim unexecuted runtime proof");
  }
}

function verifySourceBoundaries() {
  for (const marker of [
    'const contractPath =',
    'validateDatabaseUrl()',
    'ensureCleanCommit()',
    '["status", "--porcelain=v1", "--untracked-files=all"]',
    '["rev-parse", "HEAD"]',
    'const initialSourceSha256 = sourceHashes();',
    'const finalSourceSha256 = sourceHashes();',
    'requireOneMarker(',
    'requirePassedCase(scenarioOutput, requiredCase.case)',
    'test_output_sha256: sha256(combinedOutput)',
    'writeAtomically(packet)',
    'renameSync(temporaryPath, outputPath)',
    'working_tree_clean_before_run: true',
  ]) {
    requireText("retained evidence runner", runner, marker);
  }
  for (const forbidden of [
    "database_url: value",
    "connection_string:",
    "raw_log:",
    "stdout: environmentOutput",
    "stderr: scenarioOutput",
    "payload:",
    "delivery_id:",
    "source_offset:",
  ]) {
    if (runner.includes(forbidden)) {
      fail(`retained evidence runner contains forbidden persisted field: ${forbidden}`);
    }
  }

  for (const marker of [
    '#![cfg(feature = "migrations")]',
    'RUSTOK_IGGY_CONNECTOR_TEST_DATABASE_URL',
    "current_setting('server_version') AS server_version",
    "current_setting('server_version_num') AS server_version_num",
    'RUSTOK_IGGY_POISON_EVIDENCE postgres_server_version=',
    'RUSTOK_IGGY_POISON_EVIDENCE postgres_server_version_num=',
    '.max_connections(1)',
    'sqlx_logging(false)',
  ]) {
    requireText("PostgreSQL environment evidence test", environmentTest, marker);
  }
  for (const requiredCase of expectedCases) {
    requireText(
      "PostgreSQL scenario evidence test",
      scenarioTest,
      `async fn ${requiredCase.case}()`,
    );
  }
}

function verifyExecutedEvidence() {
  const evidencePath = contract.evidence_path;
  const absoluteEvidencePath = resolve(repoRoot, evidencePath);
  if (!existsSync(absoluteEvidencePath)) {
    console.log(
      "Iggy consumer poison retained evidence contract verified; PostgreSQL runtime execution remains pending.",
    );
    return;
  }

  const evidenceText = readText(evidencePath);
  const evidence = JSON.parse(evidenceText);
  requireExactKeys("retained evidence packet", evidence, [
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
    "database_url_source",
    "database",
    "toolchain",
    "commands",
    "source_sha256",
    "test_output_sha256",
    "test_output_bytes",
    "executed_cases",
  ]);
  if (evidence.schema_version !== 1) fail("retained evidence schema_version drift");
  if (evidence.module !== contract.module) fail("retained evidence module drift");
  if (evidence.packet !== "consumer-poison-postgres-runtime-evidence") {
    fail("retained evidence packet identity drift");
  }
  if (evidence.status !== "postgres_runtime_executed") {
    fail("retained evidence must record successful PostgreSQL runtime execution");
  }
  if (evidence.generated_from !== contractPath) fail("retained evidence source contract drift");
  if (evidence.runner !== contract.runner) fail("retained evidence runner drift");
  if (evidence.verifier !== contract.verifier) fail("retained evidence verifier drift");
  if (!/^[0-9a-f]{40}$/u.test(evidence.git_commit)) fail("retained evidence git commit is invalid");
  if (evidence.working_tree_clean_before_run !== true) {
    fail("retained evidence must come from a clean working tree");
  }
  requireIsoTimestamp("retained evidence started_at", evidence.started_at);
  requireIsoTimestamp("retained evidence completed_at", evidence.completed_at);
  if (Date.parse(evidence.completed_at) < Date.parse(evidence.started_at)) {
    fail("retained evidence completed_at precedes started_at");
  }
  if (
    evidence.database_url_source !== contract.database_url_env &&
    evidence.database_url_source !== contract.database_url_fallback_env
  ) {
    fail("retained evidence database URL source drift");
  }

  requireExactKeys("retained evidence database", evidence.database, [
    "backend",
    "server_version",
    "server_version_num",
  ]);
  if (evidence.database.backend !== "postgresql") fail("retained evidence backend drift");
  if (
    typeof evidence.database.server_version !== "string" ||
    !evidence.database.server_version.trim() ||
    evidence.database.server_version.length > 128
  ) {
    fail("retained evidence PostgreSQL server version is invalid");
  }
  if (!/^\d+$/u.test(evidence.database.server_version_num)) {
    fail("retained evidence PostgreSQL numeric version is invalid");
  }

  requireExactKeys("retained evidence toolchain", evidence.toolchain, ["cargo", "rustc"]);
  if (!/^cargo\s/u.test(evidence.toolchain.cargo)) fail("retained evidence Cargo version is invalid");
  if (!/^rustc\s/u.test(evidence.toolchain.rustc)) fail("retained evidence Rust version is invalid");
  if (!sameValue(evidence.commands, expectedCommands)) fail("retained evidence command drift");
  if (!/^[0-9a-f]{64}$/u.test(evidence.test_output_sha256)) {
    fail("retained evidence output SHA-256 is invalid");
  }
  if (!Number.isSafeInteger(evidence.test_output_bytes) || evidence.test_output_bytes <= 0) {
    fail("retained evidence output byte count is invalid");
  }

  requireExactKeys("retained evidence source hashes", evidence.source_sha256, expectedSourceFiles);
  for (const relativePath of expectedSourceFiles) {
    const retainedHash = evidence.source_sha256[relativePath];
    if (!/^[0-9a-f]{64}$/u.test(retainedHash)) {
      fail(`retained evidence source hash is invalid: ${relativePath}`);
    }
    if (retainedHash !== sha256File(relativePath)) {
      fail(`retained evidence is stale for source file: ${relativePath}`);
    }
  }

  if (!Array.isArray(evidence.executed_cases) || evidence.executed_cases.length !== expectedCases.length) {
    fail("retained evidence executed case count drift");
  }
  for (const requiredCase of expectedCases) {
    const executedCase = evidence.executed_cases.find(
      (candidate) => candidate.case === requiredCase.case,
    );
    requireExactKeys(`retained evidence case ${requiredCase.case}`, executedCase, [
      "case",
      "result",
      "assertions",
    ]);
    if (executedCase.result !== "pass") {
      fail(`retained evidence case did not pass: ${requiredCase.case}`);
    }
    if (!sameValue(executedCase.assertions, requiredCase.assertions)) {
      fail(`retained evidence assertions drift: ${requiredCase.case}`);
    }
  }

  if (/postgres(?:ql)?:\/\//iu.test(evidenceText)) {
    fail("retained evidence must not contain a PostgreSQL connection URL");
  }
  console.log(
    "Iggy consumer poison retained PostgreSQL evidence verified: clean commit, bounded metadata, current source hashes, and all required cases passed.",
  );
}

try {
  verifyContract();
  verifySourceBoundaries();
  verifyExecutedEvidence();
} catch (error) {
  console.error(`Iggy consumer poison retained evidence verification failed: ${error.message}`);
  process.exit(1);
}
