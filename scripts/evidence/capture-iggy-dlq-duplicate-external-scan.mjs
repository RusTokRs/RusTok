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
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-execution-contract.json";
const expectedSourceContract =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-runtime-source.json";
const expectedRunner = "scripts/evidence/capture-iggy-dlq-duplicate-external-scan.mjs";
const expectedVerifier =
  "scripts/verify/verify-iggy-dlq-duplicate-external-scan-retained.mjs";
const expectedEvidence =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-execution.json";
const expectedCase =
  "bounded_scan_classifies_duplicates_and_preserves_absent_consumer_offset";
const expectedCommand = {
  program: "cargo",
  args: [
    "test",
    "-p",
    "rustok-iggy",
    "--features",
    "iggy",
    "--test",
    "dlq_duplicate_external_scan",
    "--",
    expectedCase,
    "--exact",
    "--nocapture",
    "--test-threads=1",
  ],
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

function strictOneLine(value, field, maximumLength = 256) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > maximumLength ||
    value.trim() !== value ||
    /[\r\n\u0000-\u001f\u007f]/u.test(value)
  ) {
    fail(`${field} is missing, padded, multiline, or outside the retained boundary`);
  }
  return value;
}

function commandOutputLine(value, field, maximumLength = 256) {
  if (typeof value !== "string") fail(`${field} is missing`);
  const normalized = value.replace(/\r?\n$/u, "");
  if (
    normalized.length === 0 ||
    normalized.length > maximumLength ||
    normalized.trim() !== normalized ||
    /[\r\n\u0000-\u001f\u007f]/u.test(normalized)
  ) {
    fail(`${field} is multiline or outside the retained boundary`);
  }
  return normalized;
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
    contract.module !== "iggy" ||
    contract.packet !== "dlq-duplicate-external-scan-execution-contract" ||
    contract.status !== "runtime_execution_contract_locked" ||
    contract.source_contract !== expectedSourceContract ||
    contract.runner !== expectedRunner ||
    contract.verifier !== expectedVerifier ||
    contract.evidence_path !== expectedEvidence ||
    contract.evidence_status !== "runtime_execution_pending" ||
    contract.case !== expectedCase
  ) {
    fail("external duplicate scan execution contract identity or path drift");
  }
  if (!sameValue(contract.command, expectedCommand)) {
    fail("external duplicate scan exact Cargo command allowlist drift");
  }
  if (
    contract.reviewed_configuration?.section !== "system.message_deduplication" ||
    contract.reviewed_configuration?.required_enabled !== false ||
    contract.reviewed_configuration?.config_path_outside_repository !== true ||
    contract.reviewed_configuration?.full_content_retained !== false ||
    contract.reviewed_configuration?.full_file_sha256_retained !== false
  ) {
    fail("external duplicate scan reviewed configuration boundary drift");
  }
}

function validateAddress(value, field) {
  const address = strictOneLine(value, field, 255);
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
  const port = Number(address.slice(address.lastIndexOf(":") + 1));
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    fail(`${field} port must be between 1 and 65535`);
  }
  return address;
}

function validateCredentialsPair() {
  const [usernameEnvironment, passwordEnvironment] = contract.optional_environment;
  const username = process.env[usernameEnvironment] ?? "";
  const password = process.env[passwordEnvironment] ?? "";
  if ((username.length === 0) !== (password.length === 0)) {
    fail("external duplicate scan username and password must both be set or both be empty");
  }
  for (const [value, field] of [
    [username, usernameEnvironment],
    [password, passwordEnvironment],
  ]) {
    if (value.length === 0) continue;
    strictOneLine(value, field, 191);
    if (value.includes(":") || value.includes("@")) {
      fail(`${field} contains an unsupported connection delimiter`);
    }
  }
}

function reviewedArtifact(value, field) {
  const artifact = strictOneLine(value, field, 256);
  if (
    artifact.includes("://") ||
    artifact.includes("@") ||
    /^\[[0-9a-fA-F:]+\]:\d+$/u.test(artifact) ||
    /^[A-Za-z0-9._-]+:\d+$/u.test(artifact)
  ) {
    fail(`${field} must be an operator-reviewed version or digest label, not an endpoint`);
  }
  return artifact;
}

function externalConfigPath(value, field) {
  const supplied = strictOneLine(value, field, 4096);
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

function parseDedupConfiguration(absolutePath, field) {
  const lines = readFileSync(absolutePath, "utf8").split(/\r?\n/u);
  let inSection = false;
  let sectionFound = false;
  let enabledRaw = null;

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
    if (key !== "enabled") continue;
    if (enabledRaw !== null) fail(`${field} contains duplicate message_deduplication.enabled`);
    enabledRaw = value;
  }

  if (!sectionFound) fail(`${field} is missing [system.message_deduplication]`);
  if (enabledRaw !== "false") {
    fail(`${field} must set message_deduplication.enabled = false`);
  }
  const canonical = {
    section: "system.message_deduplication",
    enabled: false,
  };
  return {
    ...canonical,
    canonical_sha256: sha256(JSON.stringify(canonical)),
  };
}

function requirePassedCase(output) {
  if (!/(?:^|\r?\n)running 1 test(?:\r?\n|$)/u.test(output)) {
    fail("external duplicate scan retained execution did not run exactly one test");
  }
  const marker = new RegExp(
    `(?:^|\\r?\\n)test ${escapeRegExp(expectedCase)} \\.\\.\\. ok(?:\\r?\\n|$)`,
    "u",
  );
  if (!marker.test(output)) {
    fail("external duplicate scan exact case did not report success");
  }
  if (/skipping external Iggy DLQ duplicate scan evidence/iu.test(output)) {
    fail("external duplicate scan exact case reported a skip");
  }
}

function workingTreeStatus() {
  return runChecked("git", ["status", "--porcelain=v1", "--untracked-files=all"]).stdout;
}

function ensureCleanCommit() {
  if (workingTreeStatus().trim()) {
    fail("working tree must be clean before external duplicate scan retained execution");
  }
  const commit = commandOutputLine(
    runChecked("git", ["rev-parse", "HEAD"]).stdout,
    "git_commit",
    40,
  );
  if (!/^[0-9a-f]{40}$/u.test(commit)) {
    fail("git commit must be a full lowercase SHA-1");
  }
  return commit;
}

function ensureOutputInsideRepository() {
  const root = `${resolve(repoRoot)}${sep}`;
  if (!outputPath.startsWith(root)) {
    fail("retained external duplicate scan output must stay inside the repository");
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
  validateContract();

  const addressEnvironment = contract.required_environment.address;
  const configPathEnvironment = contract.required_environment.config_path;
  const serverArtifactEnvironment = contract.required_environment.server_artifact;
  validateAddress(process.env[addressEnvironment] ?? "", addressEnvironment);
  validateCredentialsPair();
  const configPath = externalConfigPath(
    process.env[configPathEnvironment] ?? "",
    configPathEnvironment,
  );
  const reviewedConfiguration = parseDedupConfiguration(
    configPath,
    configPathEnvironment,
  );
  const serverArtifact = reviewedArtifact(
    process.env[serverArtifactEnvironment] ?? "",
    serverArtifactEnvironment,
  );

  const gitCommit = ensureCleanCommit();
  const initialSourceSha256 = sourceHashes();
  const cargoVersion = commandOutputLine(
    runChecked("cargo", ["--version"]).stdout,
    "cargo_version",
  );
  const rustcVersion = commandOutputLine(
    runChecked("rustc", ["--version"]).stdout,
    "rustc_version",
  );
  const startedAt = new Date().toISOString();
  const result = runChecked(contract.command.program, contract.command.args);
  const output = `${result.stdout}\n${result.stderr}`;
  requirePassedCase(output);

  const finalCommit = commandOutputLine(
    runChecked("git", ["rev-parse", "HEAD"]).stdout,
    "final_git_commit",
    40,
  );
  if (finalCommit !== gitCommit) fail("git commit changed during retained execution");
  const finalSourceSha256 = sourceHashes();
  if (!sameValue(finalSourceSha256, initialSourceSha256)) {
    fail("external duplicate scan source files changed during retained execution");
  }
  if (workingTreeStatus().trim()) {
    fail("working tree changed during external duplicate scan retained execution");
  }
  const completedAt = new Date().toISOString();

  writeAtomically({
    schema_version: 1,
    module: contract.module,
    packet: "dlq-duplicate-external-scan-runtime-evidence",
    status: "external_iggy_duplicate_scan_runtime_executed",
    generated_from: contractPath,
    runner: contract.runner,
    verifier: contract.verifier,
    git_commit: gitCommit,
    working_tree_clean_before_run: true,
    started_at: startedAt,
    completed_at: completedAt,
    environment_sources: {
      address_environment: addressEnvironment,
      configuration_path_environment: configPathEnvironment,
      server_artifact_environment: serverArtifactEnvironment,
      username_environment: contract.optional_environment[0],
      password_environment: contract.optional_environment[1],
    },
    reviewed_artifacts: {
      iggy_server: serverArtifact,
    },
    reviewed_configuration: reviewedConfiguration,
    toolchain: {
      cargo: cargoVersion,
      rustc: rustcVersion,
    },
    source_sha256: finalSourceSha256,
    executed_case: {
      name: contract.case,
      result: "pass",
      command: contract.command,
      required_summary: contract.required_summary,
      required_offset_observations: contract.required_offset_observations,
      test_output_sha256: sha256(output),
      test_output_bytes: Buffer.byteLength(output),
    },
  });
  console.log(`Retained external duplicate scan evidence written to ${contract.evidence_path}`);
} catch (error) {
  console.error(`External duplicate scan retained capture failed: ${error.message}`);
  process.exit(1);
}
