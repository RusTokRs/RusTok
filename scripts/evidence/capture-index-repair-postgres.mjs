#!/usr/bin/env node

import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { isIP } from "node:net";
import { dirname, resolve, sep } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-index/contracts/evidence/concrete-repair-postgres-execution-contract.json";
const expectedRunnerPath = "scripts/evidence/capture-index-repair-postgres.mjs";
const expectedVerifierPath = "scripts/verify/verify-index-repair-retained-evidence.mjs";
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
  "crates/rustok-index/tests/drift_repair_postgres_environment_test.rs",
  "crates/rustok-index/tests/drift_repair_recovery_postgres_test.rs",
  "crates/rustok-index/tests/drift_repair_concrete_execution_postgres_test.rs",
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
const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), "utf8"));
const outputPaths = {
  evidence: resolve(repoRoot, contract.evidence_path),
  stdout: resolve(repoRoot, contract.stdout_path),
  stderr: resolve(repoRoot, contract.stderr_path),
};

function fail(message) {
  throw new Error(message);
}

function sameRecord(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function fileSha256(relativePath) {
  return sha256(readFileSync(resolve(repoRoot, relativePath)));
}

function sourceHashes() {
  return Object.fromEntries(
    contract.source_files.map((relativePath) => [relativePath, fileSha256(relativePath)]),
  );
}

function run(program, args, env = process.env) {
  const result = spawnSync(program, args, {
    cwd: repoRoot,
    env,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.error) {
    fail(`${program} could not start: ${result.error.message}`);
  }
  return {
    status: result.status ?? -1,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
  };
}

function runChecked(program, args, env = process.env) {
  const result = run(program, args, env);
  if (result.status !== 0) {
    fail(`${program} exited with status ${result.status}; no retained evidence was written`);
  }
  return result;
}

function oneLine(value, field, maxLength = 256) {
  const line = value.trim().split(/\r?\n/u, 1)[0]?.trim() ?? "";
  if (!line || line.length > maxLength || /[\u0000-\u001f\u007f]/u.test(line)) {
    fail(`${field} is missing or outside the retained evidence boundary`);
  }
  return line;
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

function requirePassedCase(output, caseName) {
  const pattern = new RegExp(
    `(?:^|\\r?\\n)test ${escapeRegExp(caseName)} \\.\\.\\. ok(?:\\r?\\n|$)`,
    "u",
  );
  if (!pattern.test(output)) {
    fail(`required PostgreSQL case did not report success: ${caseName}`);
  }
}

function markerValues(output, marker) {
  const prefix = `RUSTOK_INDEX_REPAIR_EVIDENCE ${marker}=`;
  return output
    .split(/\r?\n/u)
    .filter((line) => line.startsWith(prefix))
    .map((line) => line.slice(prefix.length).trim())
    .filter(Boolean);
}

function requireOneMarker(output, marker) {
  const values = [...new Set(markerValues(output, marker))];
  if (values.length !== 1) {
    fail(`expected exactly one retained evidence marker for ${marker}`);
  }
  return oneLine(values[0], marker, 128);
}

function ensurePathsInsideRepository() {
  const root = resolve(repoRoot) + sep;
  for (const [name, outputPath] of Object.entries(outputPaths)) {
    if (!outputPath.startsWith(root)) {
      fail(`${name} retained evidence path must stay inside the repository`);
    }
  }
}

function validateContractBoundary() {
  if (
    contract.schema_version !== 1 ||
    contract.module !== "rustok-index" ||
    contract.packet !== "concrete-repair-postgres-execution-contract" ||
    contract.status !== "runtime_execution_contract_locked"
  ) {
    fail("concrete repair retained evidence contract identity drift");
  }
  if (
    contract.runner !== expectedRunnerPath ||
    contract.verifier !== expectedVerifierPath ||
    contract.evidence_path !== expectedEvidencePath ||
    contract.stdout_path !== expectedStdoutPath ||
    contract.stderr_path !== expectedStderrPath ||
    contract.evidence_status !== "runtime_execution_pending"
  ) {
    fail("concrete repair retained evidence tooling or output boundary drift");
  }
  if (
    contract.execution_scope !== "opt_in_postgresql_runtime_execution_pending" ||
    contract.promotion_gate !==
      "requires_clean_commit_current_sources_and_all_required_cases_passed"
  ) {
    fail("concrete repair retained evidence promotion boundary drift");
  }
  if (!sameRecord(contract.commands, expectedCommands)) {
    fail("concrete repair retained evidence command allowlist drift");
  }
  if (!sameRecord(contract.source_files, expectedSourceFiles)) {
    fail("concrete repair retained evidence source file allowlist drift");
  }
  if (!sameRecord(contract.required_cases, expectedCases)) {
    fail("concrete repair retained evidence required case allowlist drift");
  }
}

function isPrivateIpv4(host) {
  const octets = host.split(".").map(Number);
  return (
    octets[0] === 10 ||
    (octets[0] === 172 && octets[1] >= 16 && octets[1] <= 31) ||
    (octets[0] === 192 && octets[1] === 168) ||
    (octets[0] === 169 && octets[1] === 254)
  );
}

function classifyDatabaseUrl(value) {
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    fail("the opt-in PostgreSQL URL is invalid");
  }
  if (parsed.protocol !== "postgres:" && parsed.protocol !== "postgresql:") {
    fail("the retained evidence runner accepts PostgreSQL URLs only");
  }
  const socketHost = parsed.searchParams.get("host");
  if (socketHost?.startsWith("/")) return "unix_socket";

  const host = parsed.hostname.replace(/^\[|\]$/gu, "").toLowerCase();
  if (!host) return "empty_host";
  if (host === "localhost" || host === "::1" || host.startsWith("127.")) {
    return "loopback";
  }
  const family = isIP(host);
  if (family === 4) return isPrivateIpv4(host) ? "private_ipv4" : "public_ipv4";
  if (family === 6) {
    if (/^(?:fc|fd)/u.test(host)) return "private_ipv6";
    if (/^fe[89ab]/u.test(host)) return "link_local_ipv6";
    return "public_ipv6";
  }
  return "dns_name";
}

function validateDatabaseUrl() {
  const primary = process.env[contract.database_url_env];
  const fallback = process.env[contract.database_url_fallback_env];
  const value = primary || fallback;
  if (!value) {
    fail(
      `${contract.database_url_env} or ${contract.database_url_fallback_env} must provide an opt-in PostgreSQL URL`,
    );
  }
  return {
    value,
    source: primary ? contract.database_url_env : contract.database_url_fallback_env,
    className: classifyDatabaseUrl(value),
  };
}

function workingTreeStatus() {
  return runChecked("git", ["status", "--porcelain=v1", "--untracked-files=all"]).stdout;
}

function ensureCleanCommit() {
  if (workingTreeStatus().trim()) {
    fail("working tree must be clean before retained evidence execution");
  }
  const commit = oneLine(runChecked("git", ["rev-parse", "HEAD"]).stdout, "git_commit");
  if (!/^[0-9a-f]{40}$/u.test(commit)) {
    fail("git commit must be a full lowercase SHA-1");
  }
  return commit;
}

function ensureCleanAfterExecution() {
  if (workingTreeStatus().trim()) {
    fail("working tree changed during retained evidence execution");
  }
}

function sanitizeOutput(value, databaseUrl) {
  let sanitized = value.split(databaseUrl).join("[REDACTED_POSTGRES_URL]");
  sanitized = sanitized.replace(
    /postgres(?:ql)?:\/\/[^\s"'`<>]+/giu,
    "[REDACTED_POSTGRES_URL]",
  );
  sanitized = sanitized.replace(
    /\b(password|passwd|pwd)\s*=\s*[^\s;]+/giu,
    "$1=[REDACTED]",
  );
  return sanitized;
}

function commandHeading(index, command) {
  return `=== command ${index}: ${command.program} ${command.args.join(" ")} ===`;
}

function retainedLog(results, stream, databaseUrl) {
  return `${results
    .map((result, index) => {
      const value = sanitizeOutput(result[stream], databaseUrl);
      return `${commandHeading(index, expectedCommands[index])}\n${value}`;
    })
    .join("\n\n")}\n`;
}

function assertSanitizedLog(name, value) {
  if (/postgres(?:ql)?:\/\//iu.test(value)) {
    fail(`${name} still contains a PostgreSQL connection URL after redaction`);
  }
  if (/\b(?:password|passwd|pwd)\s*=\s*(?!\[REDACTED\])/iu.test(value)) {
    fail(`${name} still contains an unredacted password assignment`);
  }
}

function temporaryPath(outputPath) {
  return `${outputPath}.tmp-${process.pid}`;
}

function writeFileAtomically(outputPath, content) {
  mkdirSync(dirname(outputPath), { recursive: true });
  const temporary = temporaryPath(outputPath);
  if (existsSync(temporary)) unlinkSync(temporary);
  writeFileSync(temporary, content, "utf8");
  renameSync(temporary, outputPath);
}

function writePacketAndLogs(packet, stdoutLog, stderrLog) {
  ensurePathsInsideRepository();
  writeFileAtomically(outputPaths.stdout, stdoutLog);
  writeFileAtomically(outputPaths.stderr, stderrLog);
  writeFileAtomically(outputPaths.evidence, `${JSON.stringify(packet, null, 2)}\n`);
}

try {
  ensurePathsInsideRepository();
  validateContractBoundary();
  const databaseUrl = validateDatabaseUrl();
  const gitCommit = ensureCleanCommit();
  const initialSourceSha256 = sourceHashes();
  const cargoVersion = oneLine(runChecked("cargo", ["--version"]).stdout, "cargo_version");
  const rustcVersion = oneLine(runChecked("rustc", ["--version"]).stdout, "rustc_version");
  const startedAt = new Date().toISOString();

  const commandResults = expectedCommands.map((command) =>
    runChecked(command.program, command.args),
  );
  const combinedOutputs = commandResults.map(
    (result) => `${result.stdout}\n${result.stderr}`,
  );
  const environmentOutput = combinedOutputs[0];
  const postgresServerVersion = requireOneMarker(
    environmentOutput,
    "postgres_server_version",
  );
  const postgresServerVersionNum = requireOneMarker(
    environmentOutput,
    "postgres_server_version_num",
  );
  if (!/^\d+$/u.test(postgresServerVersionNum)) {
    fail("postgres_server_version_num must contain digits only");
  }

  for (const requiredCase of contract.required_cases) {
    const output = combinedOutputs[requiredCase.command_index];
    if (output === undefined) {
      fail(`required case references an unavailable command: ${requiredCase.case}`);
    }
    requirePassedCase(output, requiredCase.case);
  }
  if (
    combinedOutputs.some(
      (output) =>
        output.includes("is not set to a PostgreSQL URL; skipping") ||
        output.includes("skipping repair_"),
    )
  ) {
    fail("repair evidence tests reported a skip instead of PostgreSQL execution");
  }

  const finalCommit = oneLine(
    runChecked("git", ["rev-parse", "HEAD"]).stdout,
    "final_git_commit",
  );
  if (finalCommit !== gitCommit) {
    fail("git commit changed during retained evidence execution");
  }
  const finalSourceSha256 = sourceHashes();
  if (!sameRecord(finalSourceSha256, initialSourceSha256)) {
    fail("retained evidence source files changed during execution");
  }
  ensureCleanAfterExecution();
  const completedAt = new Date().toISOString();

  const stdoutLog = retainedLog(commandResults, "stdout", databaseUrl.value);
  const stderrLog = retainedLog(commandResults, "stderr", databaseUrl.value);
  assertSanitizedLog("retained stdout", stdoutLog);
  assertSanitizedLog("retained stderr", stderrLog);

  const packet = {
    schema_version: 1,
    module: contract.module,
    packet: "concrete-repair-postgres-runtime-evidence",
    status: "postgres_runtime_executed",
    final_status: "pass",
    generated_from: contractPath,
    runner: contract.runner,
    verifier: contract.verifier,
    git_commit: gitCommit,
    working_tree_clean_before_run: true,
    source_unchanged_during_run: true,
    started_at: startedAt,
    completed_at: completedAt,
    database_url_source: databaseUrl.source,
    database_url_class: databaseUrl.className,
    database: {
      backend: "postgresql",
      server_version: postgresServerVersion,
      server_version_num: postgresServerVersionNum,
    },
    toolchain: {
      cargo: cargoVersion,
      rustc: rustcVersion,
    },
    commands: expectedCommands,
    command_results: commandResults.map((result, index) => ({
      command_index: index,
      status: result.status,
      stdout_sha256: sha256(sanitizeOutput(result.stdout, databaseUrl.value)),
      stderr_sha256: sha256(sanitizeOutput(result.stderr, databaseUrl.value)),
      stdout_bytes: Buffer.byteLength(sanitizeOutput(result.stdout, databaseUrl.value)),
      stderr_bytes: Buffer.byteLength(sanitizeOutput(result.stderr, databaseUrl.value)),
    })),
    source_sha256: finalSourceSha256,
    logs: {
      stdout_path: contract.stdout_path,
      stderr_path: contract.stderr_path,
      stdout_sha256: sha256(stdoutLog),
      stderr_sha256: sha256(stderrLog),
      stdout_bytes: Buffer.byteLength(stdoutLog),
      stderr_bytes: Buffer.byteLength(stderrLog),
      redactions: [
        "exact_database_url",
        "postgresql_url_pattern",
        "password_assignment",
      ],
    },
    executed_cases: contract.required_cases.map((requiredCase) => ({
      case: requiredCase.case,
      command_index: requiredCase.command_index,
      result: "pass",
      assertions: requiredCase.assertions,
    })),
  };

  writePacketAndLogs(packet, stdoutLog, stderrLog);
  console.log(`Retained Index repair PostgreSQL evidence written to ${contract.evidence_path}`);
} catch (error) {
  console.error(`Index repair PostgreSQL evidence capture failed: ${error.message}`);
  process.exit(1);
}
