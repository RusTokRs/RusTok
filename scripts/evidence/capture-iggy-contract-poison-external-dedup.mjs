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
import { dirname, resolve, sep } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-iggy/contracts/evidence/contract-poison-external-iggy-dedup-execution-contract.json";
const expectedRunnerPath =
  "scripts/evidence/capture-iggy-contract-poison-external-dedup.mjs";
const expectedVerifierPath =
  "scripts/verify/verify-iggy-contract-poison-external-dedup-retained-evidence.mjs";
const expectedEvidencePath =
  "crates/rustok-iggy/contracts/evidence/contract-poison-external-iggy-dedup-execution.json";
const expectedCaseNames = [
  "disabled_deduplication_persists_repeated_uuid_twice",
  "enabled_deduplication_suppresses_immediate_repeated_uuid",
  "bounded_deduplication_capacity_eviction_accepts_old_uuid_again",
  "expired_deduplication_entry_accepts_same_uuid_after_bounded_wait",
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
const expiryWaitEnv = "RUSTOK_IGGY_DEDUP_EXPIRY_WAIT_MS";
const minimumExpiryWaitMs = 100;
const maximumExpiryWaitMs = 300_000;
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

function oneLine(value, field, maximumLength = 256) {
  const line = value.trim().split(/\r?\n/, 1)[0]?.trim() ?? "";
  if (
    !line ||
    line.length > maximumLength ||
    /[\u0000-\u001f\u007f]/u.test(line)
  ) {
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

function sameRecord(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

function requirePassedCase(output, caseName) {
  if (!/(?:^|\r?\n)running 1 test(?:\r?\n|$)/u.test(output)) {
    fail(`dedup scenario did not execute exactly one test: ${caseName}`);
  }
  const pattern = new RegExp(
    `(?:^|\\r?\\n)test ${escapeRegExp(caseName)} \\.\\.\\. ok(?:\\r?\\n|$)`,
    "u",
  );
  if (!pattern.test(output)) {
    fail(`required external Iggy dedup case did not report success: ${caseName}`);
  }
  if (/skipping .*Iggy deduplication evidence/iu.test(output)) {
    fail(`dedup scenario reported a skip instead of execution: ${caseName}`);
  }
}

function ensureOutputInsideRepository() {
  const root = resolve(repoRoot) + sep;
  if (!outputPath.startsWith(root)) {
    fail("retained dedup evidence output path must stay inside the repository");
  }
}

function validateContractBoundary() {
  if (
    contract.schema_version !== 1 ||
    contract.module !== "iggy" ||
    contract.packet !== "contract-poison-external-iggy-dedup-execution-contract" ||
    contract.status !== "runtime_execution_contract_locked"
  ) {
    fail("retained dedup execution contract identity drift");
  }
  if (
    contract.runner !== expectedRunnerPath ||
    contract.verifier !== expectedVerifierPath ||
    contract.evidence_path !== expectedEvidencePath ||
    contract.evidence_status !== "runtime_execution_pending"
  ) {
    fail("retained dedup tooling or output boundary drift");
  }
  if (!sameRecord(contract.command_template, expectedCommandTemplate)) {
    fail("retained dedup Cargo command allowlist drift");
  }
  const caseNames = contract.scenarios?.map((scenario) => scenario.case);
  if (!sameRecord(caseNames, expectedCaseNames)) {
    fail("retained dedup case allowlist drift");
  }
}

function validateAddress(value, field) {
  const address = oneLine(value, field, 255);
  if (
    address.includes("://") ||
    address.includes("@") ||
    address.includes("?") ||
    address.includes("#")
  ) {
    fail(`${field} must be host:port without scheme, credentials, query, or fragment`);
  }
  if (!/^\[[0-9a-f:]+\]:\d+$|^[A-Za-z0-9._-]+:\d+$/u.test(address)) {
    fail(`${field} must be a bounded host:port address`);
  }
  return address;
}

function validateCredentialsPair() {
  const [usernameEnv, passwordEnv] = contract.shared_optional_environment;
  const username = process.env[usernameEnv] ?? "";
  const password = process.env[passwordEnv] ?? "";
  if (username.trim() !== username || password.trim() !== password) {
    fail("dedup evidence credentials must not have surrounding whitespace");
  }
  if (username.length > 191 || password.length > 191) {
    fail("dedup evidence credentials exceed the test boundary");
  }
  if (username.includes(":") || username.includes("@")) {
    fail("dedup evidence username contains an unsupported connection delimiter");
  }
  if (password.includes(":") || password.includes("@")) {
    fail("dedup evidence password contains an unsupported connection delimiter");
  }
  if (username.length === 0 !== (password.length === 0)) {
    fail("dedup evidence username and password must both be set or both be empty");
  }
}

function absoluteExternalConfigPath(value, field) {
  const pathValue = oneLine(value, field, 4096);
  const absolutePath = resolve(pathValue);
  const repositoryPrefix = resolve(repoRoot) + sep;
  if (absolutePath === resolve(repoRoot) || absolutePath.startsWith(repositoryPrefix)) {
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
    if (quote === null && character === "#") {
      return line.slice(0, index);
    }
  }
  return line;
}

function parseTomlString(value, field) {
  if (value.startsWith('"') && value.endsWith('"')) {
    try {
      const parsed = JSON.parse(value);
      return oneLine(parsed, field, 128);
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
  if (!match) {
    fail(`${field} must be a positive duration such as 500ms, 10s, 5m, 1h, or 1d`);
  }
  const amount = Number(match[1]);
  const multipliers = {
    ms: 1,
    s: 1_000,
    m: 60_000,
    h: 3_600_000,
    d: 86_400_000,
  };
  const milliseconds = amount * multipliers[match[2]];
  if (!Number.isSafeInteger(milliseconds) || milliseconds <= 0) {
    fail(`${field} is outside the retained evidence duration boundary`);
  }
  return milliseconds;
}

function parseDedupConfiguration(absolutePath, field) {
  const text = readFileSync(absolutePath, "utf8");
  const lines = text.split(/\r?\n/u);
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
    if (separator <= 0) {
      fail(`${field} contains an invalid message_deduplication assignment`);
    }
    const key = line.slice(0, separator).trim();
    const value = line.slice(separator + 1).trim();
    if (!["enabled", "max_entries", "expiry"].includes(key)) continue;
    if (values.has(key)) {
      fail(`${field} contains duplicate message_deduplication key: ${key}`);
    }
    values.set(key, value);
  }

  if (!sectionFound) {
    fail(`${field} is missing [system.message_deduplication]`);
  }
  const enabledRaw = values.get("enabled");
  if (enabledRaw !== "true" && enabledRaw !== "false") {
    fail(`${field} must set message_deduplication.enabled to true or false`);
  }
  const enabled = enabledRaw === "true";

  let maxEntries = null;
  if (values.has("max_entries")) {
    const raw = values.get("max_entries");
    if (!/^\d+$/u.test(raw)) {
      fail(`${field} message_deduplication.max_entries must be an integer`);
    }
    maxEntries = Number(raw);
    if (!Number.isSafeInteger(maxEntries) || maxEntries <= 0) {
      fail(`${field} message_deduplication.max_entries must be positive`);
    }
  }

  let expiry = null;
  let expiryMs = null;
  if (values.has("expiry")) {
    expiry = parseTomlString(values.get("expiry"), `${field} expiry`);
    expiryMs = durationMilliseconds(expiry, `${field} expiry`);
  }

  return {
    section: "system.message_deduplication",
    enabled,
    max_entries: maxEntries,
    expiry,
    expiry_milliseconds: expiryMs,
  };
}

function expiryWaitMilliseconds() {
  const value = oneLine(process.env[expiryWaitEnv] ?? "", expiryWaitEnv, 16);
  if (!/^\d+$/u.test(value)) {
    fail(`${expiryWaitEnv} must be an integer`);
  }
  const milliseconds = Number(value);
  if (
    !Number.isSafeInteger(milliseconds) ||
    milliseconds < minimumExpiryWaitMs ||
    milliseconds > maximumExpiryWaitMs
  ) {
    fail(
      `${expiryWaitEnv} must be between ${minimumExpiryWaitMs} and ${maximumExpiryWaitMs}`,
    );
  }
  return milliseconds;
}

function validateExpectedConfiguration(scenario, configuration, waitMs) {
  const expected = scenario.expected_configuration;
  if (configuration.enabled !== expected.enabled) {
    fail(`${scenario.case} reviewed configuration has unexpected enabled value`);
  }
  if (expected.max_entries === "at_least_1") {
    if (configuration.max_entries === null || configuration.max_entries < 1) {
      fail(`${scenario.case} reviewed configuration requires max_entries >= 1`);
    }
  } else if (Number.isInteger(expected.max_entries)) {
    if (configuration.max_entries !== expected.max_entries) {
      fail(`${scenario.case} reviewed configuration requires max_entries=${expected.max_entries}`);
    }
  }
  if (expected.expiry === "positive_duration") {
    if (configuration.expiry_milliseconds === null) {
      fail(`${scenario.case} reviewed configuration requires a positive expiry`);
    }
  }
  if (expected.expiry === "positive_duration_shorter_than_wait") {
    if (
      configuration.expiry_milliseconds === null ||
      configuration.expiry_milliseconds >= waitMs
    ) {
      fail(`${scenario.case} reviewed expiry must be shorter than ${expiryWaitEnv}`);
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

function workingTreeStatus() {
  return runChecked("git", ["status", "--porcelain=v1", "--untracked-files=all"])
    .stdout;
}

function ensureCleanCommit() {
  if (workingTreeStatus().trim()) {
    fail("working tree must be clean before retained dedup execution");
  }
  const commit = oneLine(
    runChecked("git", ["rev-parse", "HEAD"]).stdout,
    "git_commit",
  );
  if (!/^[0-9a-f]{40}$/u.test(commit)) {
    fail("git commit must be a full lowercase SHA-1");
  }
  return commit;
}

function ensureCleanAfterExecution() {
  if (workingTreeStatus().trim()) {
    fail("working tree changed during retained dedup execution");
  }
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
  validateContractBoundary();
  validateCredentialsPair();
  const waitMs = expiryWaitMilliseconds();

  const addresses = new Set();
  const configPaths = new Set();
  const reviewedScenarios = contract.scenarios.map((scenario) => {
    const address = validateAddress(
      process.env[scenario.address_env] ?? "",
      scenario.address_env,
    );
    if (addresses.has(address)) {
      fail("retained dedup execution requires four distinct broker addresses");
    }
    addresses.add(address);

    const configPath = absoluteExternalConfigPath(
      process.env[scenario.config_path_env] ?? "",
      scenario.config_path_env,
    );
    if (configPaths.has(configPath)) {
      fail("retained dedup execution requires four distinct reviewed config files");
    }
    configPaths.add(configPath);

    const serverArtifact = oneLine(
      process.env[scenario.server_artifact_env] ?? "",
      scenario.server_artifact_env,
      256,
    );
    const configuration = validateExpectedConfiguration(
      scenario,
      parseDedupConfiguration(configPath, scenario.config_path_env),
      waitMs,
    );
    return {
      scenario,
      serverArtifact,
      configuration,
    };
  });

  const gitCommit = ensureCleanCommit();
  const initialSourceSha256 = sourceHashes();
  const cargoVersion = oneLine(runChecked("cargo", ["--version"]).stdout, "cargo_version");
  const rustcVersion = oneLine(runChecked("rustc", ["--version"]).stdout, "rustc_version");
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
      reviewed_configuration: reviewed.configuration,
      expected_partition_message_counts:
        reviewed.scenario.expected_partition_message_counts,
      command,
      test_output_sha256: sha256(output),
      test_output_bytes: Buffer.byteLength(output),
    });
  }

  const finalCommit = oneLine(
    runChecked("git", ["rev-parse", "HEAD"]).stdout,
    "final_git_commit",
  );
  if (finalCommit !== gitCommit) {
    fail("git commit changed during retained dedup execution");
  }
  const finalSourceSha256 = sourceHashes();
  if (!sameRecord(finalSourceSha256, initialSourceSha256)) {
    fail("retained dedup source files changed during execution");
  }
  ensureCleanAfterExecution();
  const completedAt = new Date().toISOString();
  const combinedOutput = combinedOutputs.join("\n");

  writeAtomically({
    schema_version: 1,
    module: contract.module,
    packet: "contract-poison-external-iggy-dedup-runtime-evidence",
    status: "external_iggy_dedup_runtime_executed",
    generated_from: contractPath,
    runner: contract.runner,
    verifier: contract.verifier,
    git_commit: gitCommit,
    working_tree_clean_before_run: true,
    started_at: startedAt,
    completed_at: completedAt,
    toolchain: {
      cargo: cargoVersion,
      rustc: rustcVersion,
    },
    expiry_wait_milliseconds: waitMs,
    source_sha256: finalSourceSha256,
    combined_test_output_sha256: sha256(combinedOutput),
    combined_test_output_bytes: Buffer.byteLength(combinedOutput),
    executed_scenarios: executedScenarios,
  });
  console.log(`Retained external Iggy dedup evidence written to ${contract.evidence_path}`);
} catch (error) {
  console.error(`External Iggy dedup evidence capture failed: ${error.message}`);
  process.exit(1);
}
