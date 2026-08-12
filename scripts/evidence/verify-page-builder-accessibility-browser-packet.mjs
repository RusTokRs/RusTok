#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const verifierContractPath =
  "crates/rustok-page-builder/contracts/evidence/page-builder-generic-accessibility-browser-packet-verifier-source.json";
const verifierContract = JSON.parse(
  readFileSync(path.join(repoRoot, verifierContractPath), "utf8"),
);
const executionContractPath = verifierContract.input.execution_contract;
const executionContract = JSON.parse(
  readFileSync(path.join(repoRoot, executionContractPath), "utf8"),
);

function fail(message) {
  throw new Error(`Page Builder accessibility browser packet verification failed: ${message}`);
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function requireSha256(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value)) {
    fail(`${label} must be a lowercase SHA-256 hex digest`);
  }
  return value;
}

function requireCommit(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{40}$/u.test(value)) {
    fail(`${label} must be a full lowercase Git SHA`);
  }
  return value;
}

function requireDeploymentDigest(value, label) {
  if (
    typeof value !== "string" ||
    value.length > 1024 ||
    !/^[^@\s]+@sha256:[0-9a-f]{64}$/u.test(value)
  ) {
    fail(`${label} must be a bounded immutable image RepoDigest`);
  }
  return value;
}

function requireBoundedString(value, label, maximumLength = 256) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > maximumLength ||
    /[\u0000\r\n]/u.test(value)
  ) {
    fail(`${label} must be a bounded non-empty single-line string`);
  }
  return value;
}

function requireExactKeys(value, expected, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    fail(`${label} keys drifted: expected ${wanted.join(", ")}, got ${actual.join(", ")}`);
  }
}

function regularFile(relativeOrAbsolute, label, maximumBytes = 1024 * 1024) {
  const absolute = path.isAbsolute(relativeOrAbsolute)
    ? path.resolve(relativeOrAbsolute)
    : path.resolve(process.cwd(), relativeOrAbsolute);
  if (!existsSync(absolute)) fail(`${label} is missing`);
  const link = lstatSync(absolute);
  if (link.isSymbolicLink() || !link.isFile()) fail(`${label} must be a regular non-symlink file`);
  const stats = statSync(absolute);
  if (stats.size <= 0 || stats.size > maximumBytes) {
    fail(`${label} must be a bounded non-empty file`);
  }
  const bytes = readFileSync(absolute);
  return { absolute, bytes, size: stats.size, sha256: sha256(bytes) };
}

function repoFile(relativePath, label, maximumBytes = 8 * 1024 * 1024) {
  const absolute = path.resolve(repoRoot, relativePath);
  const relative = path.relative(repoRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    fail(`${label} must remain inside the repository`);
  }
  if (!existsSync(absolute)) fail(`${label} is missing`);
  const link = lstatSync(absolute);
  if (link.isSymbolicLink() || !link.isFile()) fail(`${label} must be a regular non-symlink file`);
  const stats = statSync(absolute);
  if (stats.size <= 0 || stats.size > maximumBytes) {
    fail(`${label} must be a bounded non-empty file`);
  }
  const bytes = readFileSync(absolute);
  return { absolute, bytes, size: stats.size, sha256: sha256(bytes) };
}

function currentCommit() {
  return requireCommit(
    execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: repoRoot,
      encoding: "utf8",
    }).trim(),
    "checkout HEAD",
  );
}

function parseArgs(argv) {
  const result = {};
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (!["--packet", "--expected-source", "--expected-deployment-digest", "--output"].includes(token)) {
      fail(`unknown argument ${token}`);
    }
    const value = argv[index + 1];
    if (value === undefined || value.startsWith("--")) fail(`${token} requires a value`);
    if (result[token] !== undefined) fail(`${token} must be supplied at most once`);
    result[token] = value;
    index += 1;
  }
  for (const required of ["--packet", "--expected-source", "--expected-deployment-digest"]) {
    if (result[required] === undefined) fail(`${required} is required`);
  }
  return result;
}

function outputPath(raw) {
  const requested = raw ?? verifierContract.output.default_path;
  const absolute = path.isAbsolute(requested)
    ? path.resolve(requested)
    : path.resolve(repoRoot, requested);
  const targetRoot = path.resolve(repoRoot, "target");
  const relative = path.relative(targetRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    fail("verification output must remain inside repository target/");
  }
  return absolute;
}

function writeAtomic(location, document) {
  mkdirSync(path.dirname(location), { recursive: true });
  const temporary = `${location}.tmp-${process.pid}`;
  rmSync(temporary, { force: true });
  writeFileSync(temporary, `${JSON.stringify(document, null, 2)}\n`, "utf8");
  renameSync(temporary, location);
}

function verifySourceHashes(packet) {
  const required = executionContract.required_source_files;
  if (!Array.isArray(required) || required.length === 0) {
    fail("execution contract required_source_files is empty");
  }
  requireExactKeys(packet.source_files, required, "packet source_files");
  for (const relativePath of required) {
    const actual = repoFile(relativePath, `required source ${relativePath}`).sha256;
    const retained = requireSha256(packet.source_files[relativePath], `source_files.${relativePath}`);
    if (retained !== actual) fail(`retained source hash does not match checkout for ${relativePath}`);
  }
}

function verifyInputRecords(packet) {
  requireExactKeys(packet.input_records, ["editor_storage_state", "profile_url_sha256"], "input_records");
  requireExactKeys(packet.input_records.editor_storage_state, ["bytes", "sha256"], "editor_storage_state");
  const bytes = packet.input_records.editor_storage_state.bytes;
  if (!Number.isSafeInteger(bytes) || bytes <= 0 || bytes > 8 * 1024 * 1024) {
    fail("editor storage-state byte size is outside the retained bound");
  }
  requireSha256(packet.input_records.editor_storage_state.sha256, "editor storage-state hash");
  requireExactKeys(packet.input_records.profile_url_sha256, executionContract.profiles, "profile URL hashes");
  for (const profile of executionContract.profiles) {
    requireSha256(packet.input_records.profile_url_sha256[profile], `${profile} URL hash`);
  }
}

function verifyObservation(profile, observation) {
  const specification = verifierContract.profiles[profile];
  if (!specification) fail(`verifier contract has no profile specification for ${profile}`);
  requireExactKeys(observation, ["passed", "criticalFailures", "facts"], `${profile} observation`);
  if (observation.passed !== true) fail(`${profile} observation did not pass`);
  if (observation.criticalFailures !== 0) fail(`${profile} observation retained critical failures`);
  const expectedFactKeys = ["pageCount", ...specification.required_boolean_facts];
  requireExactKeys(observation.facts, expectedFactKeys, `${profile} facts`);
  if (
    !Number.isSafeInteger(observation.facts.pageCount) ||
    observation.facts.pageCount < specification.minimum_page_count
  ) {
    fail(`${profile} pageCount is below the required minimum`);
  }
  for (const fact of specification.required_boolean_facts) {
    if (observation.facts[fact] !== true) fail(`${profile} required fact ${fact} is not true`);
  }
}

function verifyPacket(packet, expectedSource, expectedDeploymentDigest) {
  requireExactKeys(
    packet,
    [
      "format",
      "status",
      "source_commit",
      "deployment_digest",
      "node_version",
      "playwright_version",
      "source_files",
      "input_records",
      "observations",
      "retained_secrets",
      "raw_dom_retained",
      "aria_snapshot_text_retained",
      "screen_reader_execution_pending",
      "wcag_conformance_not_claimed",
      "executed_at",
    ],
    "browser packet",
  );
  if (packet.format !== verifierContract.input.required_format) fail("browser packet format drifted");
  if (packet.status !== verifierContract.input.required_status) fail("browser packet status drifted");

  const packetSource = requireCommit(packet.source_commit, "browser packet source_commit");
  if (packetSource !== expectedSource) fail("browser packet source_commit does not match expected source");
  const head = currentCommit();
  if (head !== expectedSource) fail(`expected source ${expectedSource} does not match checkout HEAD ${head}`);

  const packetDigest = requireDeploymentDigest(packet.deployment_digest, "browser packet deployment_digest");
  if (packetDigest !== expectedDeploymentDigest) {
    fail("browser packet deployment_digest does not match the separately supplied expected RepoDigest");
  }

  requireBoundedString(packet.node_version, "node_version", 64);
  requireBoundedString(packet.playwright_version, "playwright_version", 64);
  const executedAt = requireBoundedString(packet.executed_at, "executed_at", 64);
  const executedTimestamp = Date.parse(executedAt);
  if (!Number.isFinite(executedTimestamp) || new Date(executedTimestamp).toISOString() !== executedAt) {
    fail("executed_at must be a canonical ISO-8601 timestamp");
  }

  verifySourceHashes(packet);
  verifyInputRecords(packet);
  requireExactKeys(packet.observations, executionContract.profiles, "observations");
  for (const profile of executionContract.profiles) verifyObservation(profile, packet.observations[profile]);

  for (const [flag, expected] of Object.entries(verifierContract.required_input_flags)) {
    if (packet[flag] !== expected) fail(`${flag} must remain ${expected}`);
  }
}

function main() {
  if (
    verifierContract.format !== "page_builder_generic_accessibility_browser_packet_verifier_source_v1" ||
    verifierContract.status !== "source_ready_maintainer_execution_pending"
  ) {
    fail("packet verifier source contract drifted");
  }
  if (
    executionContract.output?.format !== verifierContract.input.required_format ||
    executionContract.output?.status !== verifierContract.input.required_status
  ) {
    fail("execution contract does not match packet verifier input identity");
  }

  const args = parseArgs(process.argv.slice(2));
  const expectedSource = requireCommit(args["--expected-source"], "expected source");
  const expectedDeploymentDigest = requireDeploymentDigest(
    args["--expected-deployment-digest"],
    "expected deployment digest",
  );
  const packetRecord = regularFile(args["--packet"], "browser packet");
  const packet = JSON.parse(packetRecord.bytes.toString("utf8"));
  verifyPacket(packet, expectedSource, expectedDeploymentDigest);

  const executionContractRecord = repoFile(executionContractPath, "execution contract");
  const verifierContractRecord = repoFile(verifierContractPath, "verifier contract");
  const output = {
    format: verifierContract.output.format,
    status: verifierContract.output.status,
    source_commit: expectedSource,
    deployment_digest: expectedDeploymentDigest,
    input_packet_sha256: packetRecord.sha256,
    execution_contract_sha256: executionContractRecord.sha256,
    verifier_contract_sha256: verifierContractRecord.sha256,
    source_files: packet.source_files,
    profiles: Object.fromEntries(
      executionContract.profiles.map((profile) => [
        profile,
        {
          passed: true,
          critical_failures: 0,
          page_count: packet.observations[profile].facts.pageCount,
        },
      ]),
    ),
    browser_executed_at: packet.executed_at,
    verified_at: new Date().toISOString(),
    owner_review_required: true,
    deployment_provenance_verified_by_this_packet: false,
    cryptographic_origin_to_repo_digest_binding_claimed: false,
    screen_reader_execution_pending: true,
    wcag_conformance_not_claimed: true,
    provider_slo_health_not_claimed: true,
    pages_gate_acceptance_not_claimed: true,
    forum_wave_admission_not_claimed: true,
    tenant_rollout_not_claimed: true,
  };
  writeAtomic(outputPath(args["--output"]), output);
  console.log(
    `[verify-page-builder-accessibility-browser-packet] PASS source=${expectedSource} profiles=${executionContract.profiles.join(",")} owner_review=pending screen_reader=pending`,
  );
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
