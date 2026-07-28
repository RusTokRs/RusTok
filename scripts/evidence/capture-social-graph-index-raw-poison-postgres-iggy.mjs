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
import { dirname, resolve, sep } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-social-graph/contracts/evidence/index-raw-poison-postgres-iggy-execution-contract.json";
const expectedRunner =
  "scripts/evidence/capture-social-graph-index-raw-poison-postgres-iggy.mjs";
const expectedVerifier =
  "scripts/verify/verify-social-graph-index-raw-poison-postgres-iggy-retained.mjs";
const expectedEvidence =
  "crates/rustok-social-graph/contracts/evidence/index-raw-poison-postgres-iggy-execution.json";
const expectedCases = [
  "raw_poison_persists_published_before_source_acknowledgement",
  "published_redelivery_is_acknowledgement_only_without_republication",
];
const expectedCommandTemplate = {
  program: "cargo",
  args_before_case: [
    "test",
    "-p",
    "rustok-social-graph",
    "--features",
    "index-consumer",
    "--test",
    "index_raw_poison_postgres_iggy",
    "--",
  ],
  args_after_case: ["--exact", "--nocapture", "--test-threads=1"],
};
const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), "utf8"));
const outputPath = resolve(repoRoot, contract.evidence_path);

function fail(message) {
  throw new Error(message);
}

function run(program, args, env = process.env) {
  const result = spawnSync(program, args, {
    cwd: repoRoot,
    env,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.error) fail(`${program} could not start: ${result.error.message}`);
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

function oneLine(value, field, maximumLength = 256) {
  const line = value.trim().split(/\r?\n/, 1)[0]?.trim() ?? "";
  if (!line || line.length > maximumLength || /[\u0000-\u001f\u007f]/u.test(line)) {
    fail(`${field} is missing or outside the retained evidence boundary`);
  }
  return line;
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

function sameValue(actual, expected) {
  return JSON.stringify(actual) === JSON.stringify(expected);
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

function validateContract() {
  if (
    contract.schema_version !== 1 ||
    contract.module !== "social-graph" ||
    contract.packet !== "index-raw-poison-postgres-iggy-execution-contract" ||
    contract.status !== "runtime_execution_contract_locked"
  ) {
    fail("combined poison execution contract identity drift");
  }
  if (
    contract.runner !== expectedRunner ||
    contract.verifier !== expectedVerifier ||
    contract.evidence_path !== expectedEvidence ||
    contract.evidence_status !== "runtime_execution_pending"
  ) {
    fail("combined poison retained tooling boundary drift");
  }
  if (!sameValue(contract.command_template, expectedCommandTemplate)) {
    fail("combined poison Cargo command allowlist drift");
  }
  const cases = contract.required_cases?.map((entry) => entry.case);
  if (!sameValue(cases, expectedCases)) fail("combined poison exact case allowlist drift");
}

function validateDatabaseUrl() {
  const envName = "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_DATABASE_URL";
  const value = process.env[envName];
  if (!value) fail(`${envName} is required for retained execution`);
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    fail(`${envName} is invalid`);
  }
  if (parsed.protocol !== "postgres:" && parsed.protocol !== "postgresql:") {
    fail(`${envName} must use PostgreSQL`);
  }
  return envName;
}

function validateIggyAddress() {
  const envName = "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_ADDRESS";
  const address = oneLine(process.env[envName] ?? "", envName, 255);
  if (
    address.includes("://") ||
    address.includes("@") ||
    address.includes("?") ||
    address.includes("#")
  ) {
    fail(`${envName} must be host:port without credentials or URL delimiters`);
  }
  if (!/^\[[0-9a-f:]+\]:\d+$|^[A-Za-z0-9._-]+:\d+$/u.test(address)) {
    fail(`${envName} must be a bounded host:port address`);
  }
  return envName;
}

function validateCredentialsPair() {
  const usernameEnv = "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_USERNAME";
  const passwordEnv = "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_PASSWORD";
  const username = process.env[usernameEnv] ?? "";
  const password = process.env[passwordEnv] ?? "";
  if (username.trim() !== username || password.trim() !== password) {
    fail("Iggy credentials must not have surrounding whitespace");
  }
  if (username.length > 191 || password.length > 191) {
    fail("Iggy credentials exceed the retained execution boundary");
  }
  if (username.includes(":") || username.includes("@")) {
    fail("Iggy username contains an unsupported delimiter");
  }
  if (password.includes(":") || password.includes("@")) {
    fail("Iggy password contains an unsupported delimiter");
  }
  if ((username.length === 0) !== (password.length === 0)) {
    fail("Iggy username and password must both be set or both be empty");
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

function requirePassedCase(output, caseName) {
  if (!/(?:^|\r?\n)running 1 test(?:\r?\n|$)/u.test(output)) {
    fail(`combined poison scenario did not execute exactly one test: ${caseName}`);
  }
  const marker = new RegExp(
    `(?:^|\\r?\\n)test ${escapeRegExp(caseName)} \\.\\.\\. ok(?:\\r?\\n|$)`,
    "u",
  );
  if (!marker.test(output)) fail(`combined poison case did not report success: ${caseName}`);
  if (/skipping Social Graph raw poison PostgreSQL\/Iggy evidence/iu.test(output)) {
    fail(`combined poison case reported a skip: ${caseName}`);
  }
}

function workingTreeStatus() {
  return runChecked("git", ["status", "--porcelain=v1", "--untracked-files=all"]).stdout;
}

function ensureCleanCommit() {
  if (workingTreeStatus().trim()) fail("working tree must be clean before retained execution");
  const commit = oneLine(runChecked("git", ["rev-parse", "HEAD"]).stdout, "git_commit");
  if (!/^[0-9a-f]{40}$/u.test(commit)) fail("git commit must be a full lowercase SHA-1");
  return commit;
}

function ensureOutputInsideRepository() {
  const prefix = resolve(repoRoot) + sep;
  if (!outputPath.startsWith(prefix)) fail("retained evidence output must stay inside repository");
}

function writeAtomically(packet) {
  ensureOutputInsideRepository();
  mkdirSync(dirname(outputPath), { recursive: true });
  const temporaryPath = `${outputPath}.tmp-${process.pid}`;
  if (existsSync(temporaryPath)) unlinkSync(temporaryPath);
  writeFileSync(temporaryPath, `${JSON.stringify(packet, null, 2)}\n`, "utf8");
  renameSync(temporaryPath, outputPath);
}

try {
  ensureOutputInsideRepository();
  validateContract();
  const databaseUrlSource = validateDatabaseUrl();
  const iggyAddressSource = validateIggyAddress();
  validateCredentialsPair();
  const postgresArtifact = oneLine(
    process.env.RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_POSTGRES_ARTIFACT ?? "",
    "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_POSTGRES_ARTIFACT",
  );
  const iggyServerArtifact = oneLine(
    process.env.RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_SERVER_ARTIFACT ?? "",
    "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_SERVER_ARTIFACT",
  );

  const gitCommit = ensureCleanCommit();
  const initialSourceSha256 = sourceHashes();
  const cargoVersion = oneLine(runChecked("cargo", ["--version"]).stdout, "cargo_version");
  const rustcVersion = oneLine(runChecked("rustc", ["--version"]).stdout, "rustc_version");
  const startedAt = new Date().toISOString();
  const executedCases = [];
  const combinedOutputs = [];

  for (const requiredCase of contract.required_cases) {
    const command = scenarioCommand(requiredCase.case);
    const result = runChecked(command.program, command.args);
    const output = `${result.stdout}\n${result.stderr}`;
    requirePassedCase(output, requiredCase.case);
    combinedOutputs.push(`--- ${requiredCase.case} ---\n${output}`);
    executedCases.push({
      case: requiredCase.case,
      result: "pass",
      assertions: requiredCase.assertions,
      command,
      test_output_sha256: sha256(output),
      test_output_bytes: Buffer.byteLength(output),
    });
  }

  const finalCommit = oneLine(
    runChecked("git", ["rev-parse", "HEAD"]).stdout,
    "final_git_commit",
  );
  if (finalCommit !== gitCommit) fail("git commit changed during retained execution");
  const finalSourceSha256 = sourceHashes();
  if (!sameValue(finalSourceSha256, initialSourceSha256)) {
    fail("combined poison source files changed during retained execution");
  }
  if (workingTreeStatus().trim()) fail("working tree changed during retained execution");

  const completedAt = new Date().toISOString();
  const combinedOutput = combinedOutputs.join("\n");
  writeAtomically({
    schema_version: 1,
    module: contract.module,
    packet: "index-raw-poison-postgres-iggy-runtime-evidence",
    status: "postgres_iggy_runtime_executed",
    generated_from: contractPath,
    runner: contract.runner,
    verifier: contract.verifier,
    git_commit: gitCommit,
    working_tree_clean_before_run: true,
    started_at: startedAt,
    completed_at: completedAt,
    environment_sources: {
      database_url: databaseUrlSource,
      iggy_address: iggyAddressSource,
    },
    reviewed_artifacts: {
      postgresql: postgresArtifact,
      iggy_server: iggyServerArtifact,
    },
    toolchain: {
      cargo: cargoVersion,
      rustc: rustcVersion,
    },
    source_sha256: finalSourceSha256,
    combined_test_output_sha256: sha256(combinedOutput),
    combined_test_output_bytes: Buffer.byteLength(combinedOutput),
    executed_cases: executedCases,
  });
  console.log(`Retained Social Graph PostgreSQL/Iggy evidence written to ${contract.evidence_path}`);
} catch (error) {
  console.error(`Social Graph PostgreSQL/Iggy evidence capture failed: ${error.message}`);
  process.exit(1);
}
