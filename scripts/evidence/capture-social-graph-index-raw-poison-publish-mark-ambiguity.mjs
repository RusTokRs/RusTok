#!/usr/bin/env node

import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, isAbsolute, resolve, sep } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-social-graph/contracts/evidence/index-raw-poison-publish-mark-ambiguity-execution-contract.json";
const expectedRunner =
  "scripts/evidence/capture-social-graph-index-raw-poison-publish-mark-ambiguity.mjs";
const expectedVerifier =
  "scripts/verify/verify-social-graph-index-raw-poison-publish-mark-ambiguity-retained.mjs";
const expectedEvidence =
  "crates/rustok-social-graph/contracts/evidence/index-raw-poison-publish-mark-ambiguity-execution.json";
const expectedCases = [
  "dedup_enabled_closes_publish_mark_ambiguity_without_physical_duplicate",
  "dedup_disabled_exposes_publish_mark_ambiguity_as_physical_duplicate",
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
    "index_raw_poison_publish_mark_ambiguity",
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
  if (
    value.trim() !== value ||
    value.length === 0 ||
    value.length > maximumLength ||
    /[\r\n]/u.test(value) ||
    /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    fail(`${field} is missing, padded, multiline, or outside the retained evidence boundary`);
  }
  return value;
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function fileSha256(relativePath) {
  const absolutePath = resolve(repoRoot, relativePath);
  if (!existsSync(absolutePath) || !statSync(absolutePath).isFile()) {
    fail(`source file is missing: ${relativePath}`);
  }
  return sha256(readFileSync(absolutePath));
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
    contract.packet !== "index-raw-poison-publish-mark-ambiguity-execution-contract" ||
    contract.status !== "runtime_execution_contract_locked"
  ) {
    fail("publish/mark ambiguity execution contract identity drift");
  }
  if (
    contract.runner !== expectedRunner ||
    contract.verifier !== expectedVerifier ||
    contract.evidence_path !== expectedEvidence ||
    contract.evidence_status !== "runtime_execution_pending"
  ) {
    fail("publish/mark ambiguity retained tooling boundary drift");
  }
  if (!sameValue(contract.command_template, expectedCommandTemplate)) {
    fail("publish/mark ambiguity Cargo command allowlist drift");
  }
  if (!sameValue(contract.scenarios?.map((scenario) => scenario.case), expectedCases)) {
    fail("publish/mark ambiguity exact case allowlist drift");
  }
  if (contract.lease_reclaim_wait_milliseconds !== 1500) {
    fail("publish/mark ambiguity lease reclaim wait drift");
  }
}

function validateDatabaseUrl() {
  const value = oneLine(
    process.env[contract.database_environment] ?? "",
    contract.database_environment,
    4096,
  );
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    fail(`${contract.database_environment} is invalid`);
  }
  if (parsed.protocol !== "postgres:" && parsed.protocol !== "postgresql:") {
    fail(`${contract.database_environment} must use PostgreSQL`);
  }
  if (!parsed.hostname) fail(`${contract.database_environment} must include a host`);
}

function validateAddress(value, field) {
  const address = oneLine(value, field, 255);
  if (
    address.includes("://") ||
    address.includes("@") ||
    address.includes("?") ||
    address.includes("#")
  ) {
    fail(`${field} must be host:port without URL or credential delimiters`);
  }
  if (!/^\[[0-9a-fA-F:]+\]:\d+$|^[A-Za-z0-9._-]+:\d+$/u.test(address)) {
    fail(`${field} must be a bounded host:port address`);
  }
  const portText = address.slice(address.lastIndexOf(":") + 1);
  const port = Number(portText);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    fail(`${field} port must be between 1 and 65535`);
  }
  return address;
}

function validateCredentialsPair() {
  const [usernameEnv, passwordEnv] = contract.shared_optional_environment;
  const username = process.env[usernameEnv] ?? "";
  const password = process.env[passwordEnv] ?? "";
  if (username.trim() !== username || password.trim() !== password) {
    fail("Iggy credentials must not have surrounding whitespace");
  }
  if (username.length > 191 || password.length > 191) {
    fail("Iggy credentials exceed the source harness boundary");
  }
  if (/[\r\n\u0000-\u001f\u007f]/u.test(username + password)) {
    fail("Iggy credentials contain control characters");
  }
  if (username.includes(":") || username.includes("@")) {
    fail("Iggy username contains an unsupported connection delimiter");
  }
  if (password.includes(":") || password.includes("@")) {
    fail("Iggy password contains an unsupported connection delimiter");
  }
  if ((username.length === 0) !== (password.length === 0)) {
    fail("Iggy username and password must both be set or both be empty");
  }
}

function reviewedArtifact(value, field) {
  const artifact = oneLine(value, field, 256);
  if (
    artifact.includes("://") ||
    artifact.includes("@") ||
    /^\[[0-9a-fA-F:]+\]:\d+$/u.test(artifact) ||
    /^[A-Za-z0-9._-]+:\d+$/u.test(artifact)
  ) {
    fail(`${field} must be a version, digest, or operator-reviewed artifact label, not an endpoint`);
  }
  return artifact;
}

function externalConfigPath(value, field) {
  const supplied = oneLine(value, field, 4096);
  if (!isAbsolute(supplied)) fail(`${field} must be an absolute path outside the repository`);
  const absolutePath = resolve(supplied);
  const root = resolve(repoRoot);
  const prefix = `${root}${sep}`;
  if (absolutePath === root || absolutePath.startsWith(prefix)) {
    fail(`${field} must point outside the repository`);
  }
  if (!existsSync(absolutePath) || !statSync(absolutePath).isFile()) {
    fail(`${field} must point to an existing external configuration file`);
  }
  return absolutePath;
}

function stripTomlComment(line) {
  let quote = null;
  let escaped = false;
  for (let index = 0; index < line.length; index += 1) {
    const character = line[index];
    if (quote === '"' && escaped) {
      escaped = false;
      continue;
    }
    if (quote === '"' && character === "\\") {
      escaped = true;
      continue;
    }
    if (quote !== null && character === quote) {
      quote = null;
      continue;
    }
    if (quote === null && (character === '"' || character === "'")) {
      quote = character;
      continue;
    }
    if (quote === null && character === "#") return line.slice(0, index);
  }
  return line;
}

function parseTomlString(value, field) {
  if (value.startsWith('"') && value.endsWith('"')) {
    try {
      return oneLine(JSON.parse(value), field, 128);
    } catch {
      fail(`${field} contains an invalid quoted TOML string`);
    }
  }
  if (value.startsWith("'") && value.endsWith("'")) {
    return oneLine(value.slice(1, -1), field, 128);
  }
  return oneLine(value, field, 128);
}

function durationMilliseconds(value, field) {
  const match = /^(\d+)(ms|s|m|h|d)$/u.exec(value);
  if (!match) fail(`${field} must be a positive duration such as 500ms, 10s, 5m, 1h, or 1d`);
  const multipliers = { ms: 1, s: 1000, m: 60_000, h: 3_600_000, d: 86_400_000 };
  const milliseconds = Number(match[1]) * multipliers[match[2]];
  if (!Number.isSafeInteger(milliseconds) || milliseconds <= 0) {
    fail(`${field} is outside the retained duration boundary`);
  }
  return milliseconds;
}

function parseDedupConfiguration(absolutePath, field) {
  const lines = readFileSync(absolutePath, "utf8").split(/\r?\n/u);
  let inSection = false;
  let sectionFound = false;
  const values = new Map();

  for (const rawLine of lines) {
    const line = stripTomlComment(rawLine).trim();
    if (!line) continue;
    const section = /^\[([^\]]+)\]$/u.exec(line);
    if (section) {
      if (inSection) break;
      inSection = section[1].trim() === "system.message_deduplication";
      sectionFound ||= inSection;
      continue;
    }
    if (!inSection) continue;
    const separator = line.indexOf("=");
    if (separator <= 0) fail(`${field} contains an invalid message_deduplication assignment`);
    const key = line.slice(0, separator).trim();
    const value = line.slice(separator + 1).trim();
    if (!["enabled", "max_entries", "expiry"].includes(key)) continue;
    if (values.has(key)) fail(`${field} contains duplicate key: ${key}`);
    values.set(key, value);
  }

  if (!sectionFound) fail(`${field} is missing [system.message_deduplication]`);
  const enabledRaw = values.get("enabled");
  if (enabledRaw !== "true" && enabledRaw !== "false") {
    fail(`${field} must set message_deduplication.enabled to true or false`);
  }
  const enabled = enabledRaw === "true";

  let maxEntries = null;
  if (values.has("max_entries")) {
    const raw = values.get("max_entries");
    if (!/^\d+$/u.test(raw)) fail(`${field} max_entries must be an integer`);
    maxEntries = Number(raw);
    if (!Number.isSafeInteger(maxEntries) || maxEntries <= 0) {
      fail(`${field} max_entries must be positive`);
    }
  }

  let expiry = null;
  let expiryMilliseconds = null;
  if (values.has("expiry")) {
    expiry = parseTomlString(values.get("expiry"), `${field} expiry`);
    expiryMilliseconds = durationMilliseconds(expiry, `${field} expiry`);
  }

  return {
    section: "system.message_deduplication",
    enabled,
    max_entries: maxEntries,
    expiry,
    expiry_milliseconds: expiryMilliseconds,
  };
}

function validateScenarioConfiguration(scenario, configuration) {
  if (configuration.enabled !== scenario.expected_configuration.enabled) {
    fail(`${scenario.case} reviewed configuration has an unexpected enabled value`);
  }
  if (scenario.expected_configuration.max_entries === "at_least_1") {
    if (configuration.max_entries === null || configuration.max_entries < 1) {
      fail(`${scenario.case} requires reviewed max_entries >= 1`);
    }
  }
  if (scenario.expected_configuration.expiry === "longer_than_lease_reclaim_wait") {
    if (
      configuration.expiry_milliseconds === null ||
      configuration.expiry_milliseconds <= contract.lease_reclaim_wait_milliseconds
    ) {
      fail(`${scenario.case} reviewed expiry must exceed the 1500 ms recovery wait`);
    }
  }
  const canonical = {
    section: configuration.section,
    enabled: configuration.enabled,
    max_entries: configuration.max_entries,
    expiry: configuration.expiry,
    expiry_milliseconds: configuration.expiry_milliseconds,
  };
  return {
    ...canonical,
    canonical_sha256: sha256(JSON.stringify(canonical)),
  };
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
    fail(`publish/mark ambiguity case did not execute exactly one test: ${caseName}`);
  }
  const marker = new RegExp(
    `(?:^|\\r?\\n)test ${escapeRegExp(caseName)} \\.\\.\\. ok(?:\\r?\\n|$)`,
    "u",
  );
  if (!marker.test(output)) fail(`publish/mark ambiguity case did not report success: ${caseName}`);
  if (/skipping Social Graph publish\/mark ambiguity evidence/iu.test(output)) {
    fail(`publish/mark ambiguity case reported a skip: ${caseName}`);
  }
}

function workingTreeStatus() {
  return runChecked("git", ["status", "--porcelain=v1", "--untracked-files=all"]).stdout;
}

function ensureCleanCommit() {
  if (workingTreeStatus().trim()) fail("working tree must be clean before retained execution");
  const commit = oneLine(runChecked("git", ["rev-parse", "HEAD"]).stdout.trim(), "git_commit");
  if (!/^[0-9a-f]{40}$/u.test(commit)) fail("git commit must be a full lowercase SHA-1");
  return commit;
}

function ensureOutputInsideRepository() {
  const root = `${resolve(repoRoot)}${sep}`;
  if (!outputPath.startsWith(root)) fail("retained evidence output must stay inside repository");
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
  validateDatabaseUrl();
  validateCredentialsPair();

  const postgresArtifact = reviewedArtifact(
    process.env[contract.postgres_artifact_environment] ?? "",
    contract.postgres_artifact_environment,
  );
  const addresses = new Set();
  const configPaths = new Set();
  const reviewedScenarios = contract.scenarios.map((scenario) => {
    const address = validateAddress(process.env[scenario.address_env] ?? "", scenario.address_env);
    if (addresses.has(address)) fail("ambiguity execution requires two distinct broker addresses");
    addresses.add(address);

    const configPath = externalConfigPath(
      process.env[scenario.config_path_env] ?? "",
      scenario.config_path_env,
    );
    if (configPaths.has(configPath)) {
      fail("ambiguity execution requires two distinct reviewed config files");
    }
    configPaths.add(configPath);

    const serverArtifact = reviewedArtifact(
      process.env[scenario.server_artifact_env] ?? "",
      scenario.server_artifact_env,
    );
    const reviewedConfiguration = validateScenarioConfiguration(
      scenario,
      parseDedupConfiguration(configPath, scenario.config_path_env),
    );
    return { scenario, serverArtifact, reviewedConfiguration };
  });

  const gitCommit = ensureCleanCommit();
  const initialSourceSha256 = sourceHashes();
  const cargoVersion = oneLine(runChecked("cargo", ["--version"]).stdout.trim(), "cargo_version");
  const rustcVersion = oneLine(runChecked("rustc", ["--version"]).stdout.trim(), "rustc_version");
  const startedAt = new Date().toISOString();
  const executedScenarios = [];
  const combinedOutputs = [];

  for (const reviewed of reviewedScenarios) {
    const command = scenarioCommand(reviewed.scenario.case);
    const result = runChecked(command.program, command.args);
    const output = `${result.stdout}\n${result.stderr}`;
    requirePassedCase(output, reviewed.scenario.case);
    combinedOutputs.push(`--- ${reviewed.scenario.case} ---\n${output}`);
    executedScenarios.push({
      case: reviewed.scenario.case,
      result: "pass",
      address_source_env: reviewed.scenario.address_env,
      configuration_source_env: reviewed.scenario.config_path_env,
      server_artifact_source_env: reviewed.scenario.server_artifact_env,
      server_artifact: reviewed.serverArtifact,
      reviewed_configuration: reviewed.reviewedConfiguration,
      expected_partition_message_counts: reviewed.scenario.expected_partition_message_counts,
      command,
      test_output_sha256: sha256(output),
      test_output_bytes: Buffer.byteLength(output),
    });
  }

  const finalCommit = oneLine(
    runChecked("git", ["rev-parse", "HEAD"]).stdout.trim(),
    "final_git_commit",
  );
  if (finalCommit !== gitCommit) fail("git commit changed during retained execution");
  const finalSourceSha256 = sourceHashes();
  if (!sameValue(finalSourceSha256, initialSourceSha256)) {
    fail("publish/mark ambiguity source files changed during retained execution");
  }
  if (workingTreeStatus().trim()) fail("working tree changed during retained execution");

  const completedAt = new Date().toISOString();
  const combinedOutput = combinedOutputs.join("\n");
  writeAtomically({
    schema_version: 1,
    module: contract.module,
    packet: "index-raw-poison-publish-mark-ambiguity-runtime-evidence",
    status: "postgres_iggy_ambiguity_runtime_executed",
    generated_from: contractPath,
    runner: contract.runner,
    verifier: contract.verifier,
    git_commit: gitCommit,
    working_tree_clean_before_run: true,
    started_at: startedAt,
    completed_at: completedAt,
    lease_reclaim_wait_milliseconds: contract.lease_reclaim_wait_milliseconds,
    environment_sources: {
      database_url: contract.database_environment,
      postgresql_artifact: contract.postgres_artifact_environment,
    },
    reviewed_artifacts: {
      postgresql: postgresArtifact,
    },
    toolchain: {
      cargo: cargoVersion,
      rustc: rustcVersion,
    },
    source_sha256: finalSourceSha256,
    combined_test_output_sha256: sha256(combinedOutput),
    combined_test_output_bytes: Buffer.byteLength(combinedOutput),
    executed_scenarios: executedScenarios,
  });
  console.log(`Retained publish/mark ambiguity evidence written to ${contract.evidence_path}`);
} catch (error) {
  console.error(`Publish/mark ambiguity evidence capture failed: ${error.message}`);
  process.exit(1);
}
