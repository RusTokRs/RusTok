#!/usr/bin/env node

import { createHash } from "node:crypto";
import {
  existsSync,
  linkSync,
  mkdirSync,
  readFileSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, isAbsolute, resolve, sep } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-iggy/contracts/evidence/dedup-recovery-window-calibration-execution-contract.json";
const expectedSourceContract =
  "crates/rustok-iggy/contracts/evidence/dedup-recovery-window-policy-source.json";
const expectedRunner =
  "scripts/evidence/capture-iggy-dedup-recovery-window-calibration.mjs";
const expectedVerifier =
  "scripts/verify/verify-iggy-dedup-recovery-window-retained.mjs";
const expectedSourceVerifier =
  "scripts/verify/verify-iggy-dedup-recovery-window-policy.mjs";
const expectedEvidence =
  "crates/rustok-iggy/contracts/evidence/dedup-recovery-window-calibration-execution.json";
const expectedCase = "reviewed_configuration_covers_recovery_window";
const expectedStatus = "iggy.dedup_recovery.sufficient";
const testEnvironment = {
  publicationLease: "RUSTOK_IGGY_DEDUP_RECOVERY_PUBLICATION_LEASE_MS",
  processRestart: "RUSTOK_IGGY_DEDUP_RECOVERY_PROCESS_RESTART_MS",
  transportReconnect: "RUSTOK_IGGY_DEDUP_RECOVERY_TRANSPORT_RECONNECT_MS",
  operatorRecovery: "RUSTOK_IGGY_DEDUP_RECOVERY_OPERATOR_RECOVERY_MS",
  requiredMaxEntries:
    "RUSTOK_IGGY_DEDUP_RECOVERY_REQUIRED_MAX_ENTRIES_PER_PARTITION",
  configuredMaxEntries: "RUSTOK_IGGY_DEDUP_RECOVERY_CONFIGURED_MAX_ENTRIES",
  configuredExpiry: "RUSTOK_IGGY_DEDUP_RECOVERY_CONFIGURED_EXPIRY_MS",
};
const skipMarker =
  "skipping Iggy dedup recovery-window retained calibration: required environment is absent";

const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), "utf8"));
const outputPath = resolve(repoRoot, contract.evidence_path);

function fail(message) {
  throw new Error(message);
}

function same(actual, expected) {
  return JSON.stringify(actual) === JSON.stringify(expected);
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
  return strictOneLine(value.replace(/\r?\n$/u, ""), field, maximumLength);
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

function validateContract() {
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
    contract.source_contract !== expectedSourceContract ||
    contract.runner !== expectedRunner ||
    contract.verifier !== expectedVerifier ||
    contract.source_verifier !== expectedSourceVerifier ||
    contract.evidence_path !== expectedEvidence ||
    contract.evidence_status !== "runtime_calibration_pending" ||
    contract.case !== expectedCase ||
    !same(contract.command, expectedCommand) ||
    contract.expected_assessment?.status !== expectedStatus
  ) {
    fail("dedup recovery-window calibration execution contract drift");
  }
  if (
    contract.bounds_contract?.schema_version !== 1 ||
    contract.bounds_contract?.path_outside_repository !== true ||
    contract.bounds_contract?.full_content_retained !== false ||
    contract.bounds_contract?.full_file_sha256_retained !== false ||
    contract.reviewed_configuration?.section !== "system.message_deduplication" ||
    contract.reviewed_configuration?.required_enabled !== true ||
    contract.reviewed_configuration?.config_path_outside_repository !== true ||
    contract.reviewed_configuration?.full_content_retained !== false ||
    contract.reviewed_configuration?.full_file_sha256_retained !== false ||
    contract.runner_requirements?.no_clobber_atomic_write_after_pass !== true
  ) {
    fail("dedup recovery-window calibration retained boundary drift");
  }
}

function externalFile(value, field) {
  const supplied = strictOneLine(value, field, 4096);
  if (!isAbsolute(supplied)) fail(`${field} must be an absolute path outside the repository`);
  const absolutePath = resolve(supplied);
  const repository = resolve(repoRoot);
  if (absolutePath === repository || absolutePath.startsWith(`${repository}${sep}`)) {
    fail(`${field} must point outside the repository`);
  }
  if (!existsSync(absolutePath) || !statSync(absolutePath).isFile()) {
    fail(`${field} must point to an existing external file`);
  }
  return absolutePath;
}

function reviewedArtifact(value, field) {
  const artifact = strictOneLine(value, field, 256);
  if (
    artifact.includes("://") ||
    artifact.includes("@") ||
    /^\[[0-9a-fA-F:]+\]:\d+$/u.test(artifact) ||
    /^[A-Za-z0-9._-]+:\d+$/u.test(artifact)
  ) {
    fail(`${field} must be a reviewed version or digest label, not an endpoint`);
  }
  return artifact;
}

function integer(value, field, allowZero) {
  if (!Number.isSafeInteger(value) || value < 0 || (!allowZero && value === 0)) {
    fail(`${field} must be a ${allowZero ? "non-negative" : "positive"} safe integer`);
  }
  return value;
}

function reviewedBounds(absolutePath) {
  let raw;
  try {
    raw = JSON.parse(readFileSync(absolutePath, "utf8"));
  } catch (error) {
    fail(`reviewed bounds JSON is invalid: ${error.message}`);
  }
  const expectedKeys = [
    "schema_version",
    "publication_lease_milliseconds",
    "process_restart_milliseconds",
    "transport_reconnect_milliseconds",
    "operator_recovery_milliseconds",
    "required_max_entries_per_partition",
    "capacity_basis",
  ];
  if (
    raw === null ||
    Array.isArray(raw) ||
    typeof raw !== "object" ||
    !same(Object.keys(raw).sort(), [...expectedKeys].sort()) ||
    raw.schema_version !== 1
  ) {
    fail("reviewed recovery bounds must use the exact versioned field allowlist");
  }
  const canonical = {
    schema_version: 1,
    publication_lease_milliseconds: integer(
      raw.publication_lease_milliseconds,
      "publication_lease_milliseconds",
      false,
    ),
    process_restart_milliseconds: integer(
      raw.process_restart_milliseconds,
      "process_restart_milliseconds",
      true,
    ),
    transport_reconnect_milliseconds: integer(
      raw.transport_reconnect_milliseconds,
      "transport_reconnect_milliseconds",
      true,
    ),
    operator_recovery_milliseconds: integer(
      raw.operator_recovery_milliseconds,
      "operator_recovery_milliseconds",
      true,
    ),
    required_max_entries_per_partition: integer(
      raw.required_max_entries_per_partition,
      "required_max_entries_per_partition",
      false,
    ),
    capacity_basis: reviewedArtifact(raw.capacity_basis, "capacity_basis"),
  };
  const requiredExpiry =
    canonical.publication_lease_milliseconds +
    canonical.process_restart_milliseconds +
    canonical.transport_reconnect_milliseconds +
    canonical.operator_recovery_milliseconds;
  if (!Number.isSafeInteger(requiredExpiry) || requiredExpiry <= 0) {
    fail("reviewed recovery horizon overflows the retained JavaScript boundary");
  }
  return {
    ...canonical,
    required_expiry_milliseconds: requiredExpiry,
    canonical_sha256: sha256(JSON.stringify(canonical)),
  };
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
      return strictOneLine(JSON.parse(value), field, 128);
    } catch {
      fail(`${field} contains an invalid quoted TOML string`);
    }
  }
  if (value.startsWith("'") && value.endsWith("'")) {
    return strictOneLine(value.slice(1, -1), field, 128);
  }
  return strictOneLine(value, field, 128);
}

function durationMilliseconds(value, field) {
  const match = /^(\d+)(ms|s|m|h|d)$/u.exec(value);
  if (!match) fail(`${field} must be a duration such as 500ms, 10s, 5m, 1h, or 1d`);
  const multipliers = { ms: 1, s: 1_000, m: 60_000, h: 3_600_000, d: 86_400_000 };
  const milliseconds = Number(match[1]) * multipliers[match[2]];
  return integer(milliseconds, `${field} milliseconds`, false);
}

function reviewedConfiguration(absolutePath) {
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
    if (separator <= 0) fail("reviewed Iggy config contains an invalid assignment");
    const key = line.slice(0, separator).trim();
    const value = line.slice(separator + 1).trim();
    if (!["enabled", "max_entries", "expiry"].includes(key)) continue;
    if (values.has(key)) fail(`reviewed Iggy config contains duplicate key: ${key}`);
    values.set(key, value);
  }
  if (!sectionFound) fail("reviewed Iggy config is missing [system.message_deduplication]");
  if (values.get("enabled") !== "true") {
    fail("reviewed Iggy config must set message_deduplication.enabled = true");
  }
  const maxEntriesRaw = values.get("max_entries") ?? "";
  if (!/^\d+$/u.test(maxEntriesRaw)) fail("reviewed max_entries must be an integer");
  const maxEntries = integer(Number(maxEntriesRaw), "reviewed max_entries", false);
  const expiry = parseTomlString(values.get("expiry") ?? "", "reviewed expiry");
  const expiryMilliseconds = durationMilliseconds(expiry, "reviewed expiry");
  const canonical = {
    section: "system.message_deduplication",
    enabled: true,
    max_entries: maxEntries,
    expiry,
    expiry_milliseconds: expiryMilliseconds,
  };
  return { ...canonical, canonical_sha256: sha256(JSON.stringify(canonical)) };
}

function workingTreeStatus() {
  return runChecked("git", ["status", "--porcelain=v1", "--untracked-files=all"]).stdout;
}

function ensureCleanCommit() {
  if (workingTreeStatus().trim()) {
    fail("working tree must be clean before retained recovery-window calibration");
  }
  const commit = commandOutputLine(
    runChecked("git", ["rev-parse", "HEAD"]).stdout,
    "git_commit",
    40,
  );
  if (!/^[0-9a-f]{40}$/u.test(commit)) fail("git commit must be a full lowercase SHA-1");
  return commit;
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

function parsePassedAssessment(output, bounds, configuration) {
  if (!/(?:^|\r?\n)running 1 test(?:\r?\n|$)/u.test(output)) {
    fail("retained recovery-window calibration did not run exactly one test");
  }
  const passed = new RegExp(
    `(?:^|\\r?\\n)test ${escapeRegExp(expectedCase)} \\.\\.\\. ok(?:\\r?\\n|$)`,
    "u",
  );
  if (!passed.test(output)) fail("retained recovery-window calibration case did not pass");
  if (output.includes(skipMarker)) fail("retained recovery-window calibration reported a skip");
  const marker =
    /RUSTOK_DEDUP_RECOVERY_CALIBRATION status=(\S+) required_expiry_ms=(\d+) configured_expiry_ms=(\d+) required_max_entries_per_partition=(\d+) configured_max_entries=(\d+)/u.exec(
      output,
    );
  if (!marker) fail("retained recovery-window calibration output marker is missing");
  const assessment = {
    status: marker[1],
    required_expiry_milliseconds: Number(marker[2]),
    configured_expiry_milliseconds: Number(marker[3]),
    required_max_entries_per_partition: Number(marker[4]),
    configured_max_entries: Number(marker[5]),
  };
  if (
    assessment.status !== expectedStatus ||
    assessment.required_expiry_milliseconds !== bounds.required_expiry_milliseconds ||
    assessment.configured_expiry_milliseconds !== configuration.expiry_milliseconds ||
    assessment.required_max_entries_per_partition !==
      bounds.required_max_entries_per_partition ||
    assessment.configured_max_entries !== configuration.max_entries
  ) {
    fail("Rust recovery-window assessment does not match reviewed inputs");
  }
  return assessment;
}

function ensureOutputInsideRepository() {
  const repository = `${resolve(repoRoot)}${sep}`;
  if (!outputPath.startsWith(repository)) {
    fail("retained recovery-window output must stay inside the repository");
  }
}

function writeNoClobber(packet) {
  ensureOutputInsideRepository();
  mkdirSync(dirname(outputPath), { recursive: true });
  if (existsSync(outputPath)) {
    fail("retained recovery-window evidence already exists and will not be replaced");
  }
  const temporaryPath = `${outputPath}.tmp-${process.pid}`;
  try {
    writeFileSync(temporaryPath, `${JSON.stringify(packet, null, 2)}\n`, {
      encoding: "utf8",
      flag: "wx",
    });
    linkSync(temporaryPath, outputPath);
  } finally {
    if (existsSync(temporaryPath)) unlinkSync(temporaryPath);
  }
}

try {
  validateContract();
  ensureOutputInsideRepository();

  const boundsEnvironment = contract.required_environment.bounds_path;
  const configEnvironment = contract.required_environment.config_path;
  const artifactEnvironment = contract.required_environment.server_artifact;
  const bounds = reviewedBounds(
    externalFile(process.env[boundsEnvironment] ?? "", boundsEnvironment),
  );
  const configuration = reviewedConfiguration(
    externalFile(process.env[configEnvironment] ?? "", configEnvironment),
  );
  const serverArtifact = reviewedArtifact(
    process.env[artifactEnvironment] ?? "",
    artifactEnvironment,
  );

  if (
    configuration.expiry_milliseconds < bounds.required_expiry_milliseconds ||
    configuration.max_entries < bounds.required_max_entries_per_partition
  ) {
    fail("reviewed Iggy configuration does not cover the supplied recovery-window bounds");
  }

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

  const testEnv = {
    ...process.env,
    [testEnvironment.publicationLease]: String(bounds.publication_lease_milliseconds),
    [testEnvironment.processRestart]: String(bounds.process_restart_milliseconds),
    [testEnvironment.transportReconnect]: String(bounds.transport_reconnect_milliseconds),
    [testEnvironment.operatorRecovery]: String(bounds.operator_recovery_milliseconds),
    [testEnvironment.requiredMaxEntries]: String(
      bounds.required_max_entries_per_partition,
    ),
    [testEnvironment.configuredMaxEntries]: String(configuration.max_entries),
    [testEnvironment.configuredExpiry]: String(configuration.expiry_milliseconds),
  };
  const result = runChecked(contract.command.program, contract.command.args, testEnv);
  const output = `${result.stdout}\n${result.stderr}`;
  const assessment = parsePassedAssessment(output, bounds, configuration);

  const finalCommit = commandOutputLine(
    runChecked("git", ["rev-parse", "HEAD"]).stdout,
    "final_git_commit",
    40,
  );
  if (finalCommit !== gitCommit) fail("git commit changed during retained calibration");
  const finalSourceSha256 = sourceHashes();
  if (!same(finalSourceSha256, initialSourceSha256)) {
    fail("bound recovery-window sources changed during retained calibration");
  }
  if (workingTreeStatus().trim()) {
    fail("working tree changed during retained recovery-window calibration");
  }

  writeNoClobber({
    schema_version: 1,
    module: "iggy",
    packet: "dedup-recovery-window-calibration-runtime-evidence",
    status: "reviewed_recovery_window_sufficient",
    generated_from: contractPath,
    runner: expectedRunner,
    verifier: expectedVerifier,
    source_verifier: expectedSourceVerifier,
    git_commit: gitCommit,
    working_tree_clean_before_run: true,
    working_tree_clean_after_run: true,
    started_at: startedAt,
    completed_at: new Date().toISOString(),
    toolchain: { cargo: cargoVersion, rustc: rustcVersion },
    input_environment_names: {
      bounds_path: boundsEnvironment,
      config_path: configEnvironment,
      server_artifact: artifactEnvironment,
    },
    reviewed_bounds: bounds,
    reviewed_configuration: configuration,
    server_artifact: serverArtifact,
    assessment,
    command: contract.command,
    case: expectedCase,
    result: "pass",
    source_sha256: finalSourceSha256,
    test_output_sha256: sha256(output),
    test_output_bytes: Buffer.byteLength(output),
  });
  console.log(
    `Retained Iggy dedup recovery-window calibration written to ${contract.evidence_path}`,
  );
} catch (error) {
  console.error(`Iggy dedup recovery-window calibration capture failed: ${error.message}`);
  process.exit(1);
}
