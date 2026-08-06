#!/usr/bin/env node

import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-index/contracts/evidence/concrete-repair-postgres-execution-contract.json";
const runnerPath = "scripts/evidence/capture-index-repair-postgres.mjs";
const verifierPath = "scripts/verify/verify-index-repair-retained-evidence.mjs";
const environmentTestPath =
  "crates/rustok-index/tests/drift_repair_postgres_environment_test.rs";
const recoveryTestPath =
  "crates/rustok-index/tests/drift_repair_recovery_postgres_test.rs";
const concreteTestPath =
  "crates/rustok-index/tests/drift_repair_concrete_execution_postgres_test.rs";

const expectedEvidencePath =
  "crates/rustok-index/contracts/evidence/concrete-repair-postgres-execution.json";
const expectedStdoutPath =
  "crates/rustok-index/contracts/evidence/concrete-repair-postgres-execution.stdout.log";
const expectedStderrPath =
  "crates/rustok-index/contracts/evidence/concrete-repair-postgres-execution.stderr.log";
const expectedCommands = [
  {
    program: "cargo",
    args: [
      "test",
      "-p",
      "rustok-index",
      "--test",
      "drift_repair_postgres_environment_test",
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
      "rustok-index",
      "--test",
      "drift_repair_recovery_postgres_test",
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
      "rustok-index",
      "--test",
      "drift_repair_concrete_execution_postgres_test",
      "--",
      "--nocapture",
      "--test-threads=1",
    ],
  },
];
const expectedSourceFiles = [
  environmentTestPath,
  recoveryTestPath,
  concreteTestPath,
  "crates/rustok-index/tests/support/drift_repair.rs",
  "crates/rustok-index/src/infrastructure/postgres/drift_repair.rs",
  "crates/rustok-index/src/infrastructure/postgres/drift_repair_recovery.rs",
  "crates/rustok-index/src/infrastructure/postgres/drift_missing_entity_repair.rs",
  "crates/rustok-index/src/infrastructure/postgres/drift_orphan_link_repair.rs",
  "crates/rustok-index/src/infrastructure/postgres/mutation_store.rs",
  "crates/rustok-index/src/infrastructure/postgres/drift_confirmed_candidate_writer.rs",
  "crates/rustok-index/src/infrastructure/postgres/schema_registration.rs",
  "crates/rustok-index/src/migrations/mod.rs",
  "crates/rustok-index/src/migrations/m20260806_000007_add_index_finding_repair_commands.rs",
  "crates/rustok-index/src/migrations/m20260806_000008_add_index_finding_repair_recovery.rs",
];
const expectedMetadata = [
  "git_commit",
  "started_at",
  "completed_at",
  "database_url_source",
  "database_url_class",
  "postgres_server_version",
  "postgres_server_version_num",
  "cargo_version",
  "rustc_version",
  "source_sha256",
  "stdout_sha256",
  "stderr_sha256",
  "final_status",
];
const expectedCases = [
  {
    case: "migrations_recovery_guard_and_concurrent_reservation_are_executable",
    command_index: 1,
    assertions: [
      "one_active_command_per_finding",
      "revision_zero_activation_retained",
      "command_uuid_payload_reuse_rejected",
      "pause_exact_replay_and_stale_revision_retained",
      "paused_completion_trigger_rejected",
      "authorized_resume_allows_completion",
      "completed_command_identity_immutable",
      "repair_and_recovery_migrations_reverse_cleanly",
    ],
  },
  {
    case: "missing_and_orphan_crash_windows_resume_exactly",
    command_index: 2,
    assertions: [
      "missing_owner_commit_survives_pre_receipt_crash",
      "missing_exact_command_retry_converges",
      "orphan_edge_and_inbox_commit_atomically",
      "orphan_exact_command_retry_requires_applied_delivery",
      "source_version_and_unrelated_ordinal_preserved",
      "terminal_replay_returns_already_completed",
    ],
  },
  {
    case: "recovery_admission_fences_side_effect_and_completion",
    command_index: 2,
    assertions: [
      "pause_before_owner_prevents_side_effect",
      "authorized_resume_reuses_exact_command",
      "abandon_after_side_effect_prevents_completion",
      "abandoned_retry_remains_terminally_fenced",
      "database_trigger_rejects_abandoned_completion",
    ],
  },
  {
    case: "orphan_commitments_and_normal_mutations_fail_closed",
    command_index: 2,
    assertions: [
      "source_version_movement_rejected",
      "exact_link_substitution_rejected",
      "target_restoration_rejected",
      "absence_version_movement_rejected",
      "normal_full_mutation_serializes_before_exact_edge_owner",
      "newer_source_and_link_graph_preserved",
    ],
  },
];
const allowedDatabaseClasses = [
  "unix_socket",
  "empty_host",
  "loopback",
  "private_ipv4",
  "public_ipv4",
  "private_ipv6",
  "link_local_ipv6",
  "public_ipv6",
  "dns_name",
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

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function sha256File(relativePath) {
  return sha256(readFileSync(resolve(repoRoot, relativePath)));
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

function requireSha256(name, value) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value)) {
    fail(`${name} must be a lowercase SHA-256 digest`);
  }
}

const contract = readJson(contractPath);
const runner = readText(runnerPath);
const environmentTest = readText(environmentTestPath);
const recoveryTest = readText(recoveryTestPath);
const concreteTest = readText(concreteTestPath);

function verifyContract() {
  if (contract.schema_version !== 1) fail("retained evidence contract schema_version drift");
  if (contract.module !== "rustok-index") fail("retained evidence contract module drift");
  if (contract.packet !== "concrete-repair-postgres-execution-contract") {
    fail("retained evidence contract packet drift");
  }
  if (contract.status !== "runtime_execution_contract_locked") {
    fail("retained evidence contract status drift");
  }
  if (contract.runner !== runnerPath) fail("retained evidence runner path drift");
  if (contract.verifier !== verifierPath) fail("retained evidence verifier path drift");
  if (contract.evidence_path !== expectedEvidencePath) fail("retained evidence path drift");
  if (contract.stdout_path !== expectedStdoutPath) fail("retained stdout path drift");
  if (contract.stderr_path !== expectedStderrPath) fail("retained stderr path drift");
  if (contract.execution_scope !== "opt_in_postgresql_runtime_execution_pending") {
    fail("retained evidence execution scope must remain explicitly pending until execution");
  }
  if (
    contract.promotion_gate !==
    "requires_clean_commit_current_sources_and_all_required_cases_passed"
  ) {
    fail("retained evidence promotion gate drift");
  }
  if (contract.database_url_env !== "RUSTOK_INDEX_TEST_DATABASE_URL") {
    fail("retained evidence database env drift");
  }
  if (contract.database_url_fallback_env !== "DATABASE_URL") {
    fail("retained evidence fallback database env drift");
  }
  if (!sameValue(contract.commands, expectedCommands)) fail("retained evidence commands drift");
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
    fail("retained evidence contract must not claim unexecuted PostgreSQL proof");
  }
}

function verifySourceBoundaries() {
  for (const marker of [
    "validateDatabaseUrl()",
    "classifyDatabaseUrl(value)",
    "ensureCleanCommit()",
    '["status", "--porcelain=v1", "--untracked-files=all"]',
    '["rev-parse", "HEAD"]',
    "const initialSourceSha256 = sourceHashes();",
    "const finalSourceSha256 = sourceHashes();",
    "requireOneMarker(",
    "requirePassedCase(output, requiredCase.case)",
    "sanitizeOutput(result.stdout, databaseUrl.value)",
    "assertSanitizedLog(\"retained stdout\", stdoutLog)",
    "writePacketAndLogs(packet, stdoutLog, stderrLog)",
    "working_tree_clean_before_run: true",
    "source_unchanged_during_run: true",
    'final_status: "pass"',
  ]) {
    requireText("retained evidence runner", runner, marker);
  }
  for (const forbidden of [
    "database_url: databaseUrl.value",
    "connection_string:",
    "raw_database_url:",
    "password: parsed.password",
    "username: parsed.username",
  ]) {
    if (runner.includes(forbidden)) {
      fail(`retained evidence runner contains forbidden persisted field: ${forbidden}`);
    }
  }

  for (const marker of [
    "repair_evidence_environment_reports_postgres_version",
    "current_setting('server_version') AS server_version",
    "current_setting('server_version_num') AS server_version_num",
    "RUSTOK_INDEX_REPAIR_EVIDENCE postgres_server_version=",
    "RUSTOK_INDEX_REPAIR_EVIDENCE postgres_server_version_num=",
  ]) {
    requireText("PostgreSQL environment evidence test", environmentTest, marker);
  }
  requireText(
    "PostgreSQL recovery evidence test",
    recoveryTest,
    "async fn migrations_recovery_guard_and_concurrent_reservation_are_executable()",
  );
  for (const requiredCase of expectedCases.filter((entry) => entry.command_index === 2)) {
    requireText(
      "PostgreSQL concrete repair evidence test",
      concreteTest,
      `async fn ${requiredCase.case}()`,
    );
  }
}

function verifyNoSecrets(name, value) {
  if (/postgres(?:ql)?:\/\//iu.test(value)) {
    fail(`${name} contains a PostgreSQL connection URL`);
  }
  if (/\b(?:password|passwd|pwd)\s*=\s*(?!\[REDACTED\])/iu.test(value)) {
    fail(`${name} contains an unredacted password assignment`);
  }
}

function verifyExecutedEvidence() {
  const existence = {
    evidence: existsSync(resolve(repoRoot, contract.evidence_path)),
    stdout: existsSync(resolve(repoRoot, contract.stdout_path)),
    stderr: existsSync(resolve(repoRoot, contract.stderr_path)),
  };
  const presentCount = Object.values(existence).filter(Boolean).length;
  if (presentCount === 0) {
    console.log(
      "Index concrete repair retained evidence contract verified; PostgreSQL owner execution remains pending.",
    );
    return;
  }
  if (presentCount !== 3) {
    fail("retained evidence packet and both logs must appear atomically as one complete set");
  }

  const evidenceText = readText(contract.evidence_path);
  const stdoutLog = readText(contract.stdout_path);
  const stderrLog = readText(contract.stderr_path);
  const evidence = JSON.parse(evidenceText);

  requireExactKeys("retained evidence packet", evidence, [
    "schema_version",
    "module",
    "packet",
    "status",
    "final_status",
    "generated_from",
    "runner",
    "verifier",
    "git_commit",
    "working_tree_clean_before_run",
    "source_unchanged_during_run",
    "started_at",
    "completed_at",
    "database_url_source",
    "database_url_class",
    "database",
    "toolchain",
    "commands",
    "command_results",
    "source_sha256",
    "logs",
    "executed_cases",
  ]);
  if (evidence.schema_version !== 1) fail("retained evidence schema_version drift");
  if (evidence.module !== contract.module) fail("retained evidence module drift");
  if (evidence.packet !== "concrete-repair-postgres-runtime-evidence") {
    fail("retained evidence packet identity drift");
  }
  if (evidence.status !== "postgres_runtime_executed" || evidence.final_status !== "pass") {
    fail("retained evidence must record successful PostgreSQL execution");
  }
  if (evidence.generated_from !== contractPath) fail("retained evidence contract path drift");
  if (evidence.runner !== contract.runner) fail("retained evidence runner drift");
  if (evidence.verifier !== contract.verifier) fail("retained evidence verifier drift");
  if (!/^[0-9a-f]{40}$/u.test(evidence.git_commit)) fail("retained evidence commit is invalid");
  if (evidence.working_tree_clean_before_run !== true) {
    fail("retained evidence must originate from a clean working tree");
  }
  if (evidence.source_unchanged_during_run !== true) {
    fail("retained evidence must retain unchanged source files during execution");
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
  if (!allowedDatabaseClasses.includes(evidence.database_url_class)) {
    fail("retained evidence database URL class is invalid");
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
  if (!/^cargo\s/u.test(evidence.toolchain.cargo)) fail("retained Cargo version is invalid");
  if (!/^rustc\s/u.test(evidence.toolchain.rustc)) fail("retained Rust version is invalid");
  if (!sameValue(evidence.commands, expectedCommands)) fail("retained evidence command drift");

  if (
    !Array.isArray(evidence.command_results) ||
    evidence.command_results.length !== expectedCommands.length
  ) {
    fail("retained evidence command result count drift");
  }
  for (let index = 0; index < expectedCommands.length; index += 1) {
    const result = evidence.command_results[index];
    requireExactKeys(`retained command result ${index}`, result, [
      "command_index",
      "status",
      "stdout_sha256",
      "stderr_sha256",
      "stdout_bytes",
      "stderr_bytes",
    ]);
    if (result.command_index !== index || result.status !== 0) {
      fail(`retained command result ${index} did not pass exactly`);
    }
    requireSha256(`retained command ${index} stdout`, result.stdout_sha256);
    requireSha256(`retained command ${index} stderr`, result.stderr_sha256);
    for (const field of ["stdout_bytes", "stderr_bytes"]) {
      if (!Number.isSafeInteger(result[field]) || result[field] < 0) {
        fail(`retained command result ${index} ${field} is invalid`);
      }
    }
  }

  requireExactKeys("retained source hashes", evidence.source_sha256, expectedSourceFiles);
  for (const relativePath of expectedSourceFiles) {
    const retainedHash = evidence.source_sha256[relativePath];
    requireSha256(`retained source hash ${relativePath}`, retainedHash);
    if (retainedHash !== sha256File(relativePath)) {
      fail(`retained evidence is stale for source file: ${relativePath}`);
    }
  }

  requireExactKeys("retained logs", evidence.logs, [
    "stdout_path",
    "stderr_path",
    "stdout_sha256",
    "stderr_sha256",
    "stdout_bytes",
    "stderr_bytes",
    "redactions",
  ]);
  if (
    evidence.logs.stdout_path !== contract.stdout_path ||
    evidence.logs.stderr_path !== contract.stderr_path
  ) {
    fail("retained evidence log path drift");
  }
  requireSha256("retained stdout log", evidence.logs.stdout_sha256);
  requireSha256("retained stderr log", evidence.logs.stderr_sha256);
  if (evidence.logs.stdout_sha256 !== sha256(stdoutLog)) fail("retained stdout hash drift");
  if (evidence.logs.stderr_sha256 !== sha256(stderrLog)) fail("retained stderr hash drift");
  if (evidence.logs.stdout_bytes !== Buffer.byteLength(stdoutLog)) {
    fail("retained stdout byte count drift");
  }
  if (evidence.logs.stderr_bytes !== Buffer.byteLength(stderrLog)) {
    fail("retained stderr byte count drift");
  }
  if (
    !sameValue(evidence.logs.redactions, [
      "exact_database_url",
      "postgresql_url_pattern",
      "password_assignment",
    ])
  ) {
    fail("retained evidence redaction contract drift");
  }

  if (!Array.isArray(evidence.executed_cases) || evidence.executed_cases.length !== expectedCases.length) {
    fail("retained evidence executed case count drift");
  }
  for (const requiredCase of expectedCases) {
    const executedCase = evidence.executed_cases.find(
      (candidate) => candidate.case === requiredCase.case,
    );
    requireExactKeys(`retained case ${requiredCase.case}`, executedCase, [
      "case",
      "command_index",
      "result",
      "assertions",
    ]);
    if (
      executedCase.command_index !== requiredCase.command_index ||
      executedCase.result !== "pass" ||
      !sameValue(executedCase.assertions, requiredCase.assertions)
    ) {
      fail(`retained evidence case drift: ${requiredCase.case}`);
    }
  }

  verifyNoSecrets("retained evidence packet", evidenceText);
  verifyNoSecrets("retained stdout", stdoutLog);
  verifyNoSecrets("retained stderr", stderrLog);
  for (let index = 0; index < expectedCommands.length; index += 1) {
    const heading = `=== command ${index}: ${expectedCommands[index].program} ${expectedCommands[index].args.join(" ")} ===`;
    requireText("retained stdout", stdoutLog, heading);
    requireText("retained stderr", stderrLog, heading);
  }

  console.log(
    "Index concrete repair retained PostgreSQL evidence verified: clean commit, bounded metadata, redacted complete logs, current source hashes, and all required cases passed.",
  );
}

try {
  verifyContract();
  verifySourceBoundaries();
  verifyExecutedEvidence();
} catch (error) {
  console.error(`Index repair retained evidence verification failed: ${error.message}`);
  process.exit(1);
}
