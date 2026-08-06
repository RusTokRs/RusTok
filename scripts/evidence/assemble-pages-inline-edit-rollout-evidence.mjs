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
const contractPath = path.join(
  repoRoot,
  "crates/rustok-pages/contracts/evidence/pages-inline-edit-rollout-execution-contract.json",
);
const contract = JSON.parse(readFileSync(contractPath, "utf8"));
const sha256Pattern = /^[0-9a-f]{64}$/u;
const digestPattern = /^[^@\s]+@sha256:[0-9a-f]{64}$/u;

function fail(message) {
  throw new Error(`Pages inline edit rollout evidence failed: ${message}`);
}

function parseArguments(argv) {
  const values = {};
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (!argument.startsWith("--")) fail(`unexpected argument ${argument}`);
    const key = argument.slice(2);
    if (!["phase", "browser", "observation", "ffa", "output"].includes(key)) {
      fail(`unknown argument --${key}`);
    }
    const value = argv[index + 1];
    if (!value || value.startsWith("--")) fail(`--${key} requires a value`);
    if (Object.hasOwn(values, key)) fail(`--${key} may only be supplied once`);
    values[key] = value;
    index += 1;
  }
  return values;
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function currentCommit() {
  const value = execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
  }).trim();
  if (!/^[0-9a-f]{40}$/u.test(value)) fail("git HEAD is not a full commit SHA");
  return value;
}

function absolute(value) {
  return path.isAbsolute(value) ? path.resolve(value) : path.resolve(repoRoot, value);
}

function readJson(value, label) {
  const location = absolute(value);
  if (!existsSync(location)) fail(`${label} is missing`);
  const link = lstatSync(location);
  if (link.isSymbolicLink() || !link.isFile()) {
    fail(`${label} must be a regular non-symlink file`);
  }
  const stats = statSync(location);
  if (stats.size <= 0) fail(`${label} must be non-empty`);
  const bytes = readFileSync(location);
  let document;
  try {
    document = JSON.parse(bytes.toString("utf8"));
  } catch (error) {
    fail(`${label} is not valid JSON: ${error.message}`);
  }
  if (!document || typeof document !== "object" || Array.isArray(document)) {
    fail(`${label} must contain a JSON object`);
  }
  return { document, bytes: stats.size, sha256: sha256(bytes) };
}

function object(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
  return value;
}

function boundedString(value, label, maximum = 512) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > maximum ||
    value.trim() !== value ||
    /[\u0000\r\n]/u.test(value)
  ) {
    fail(`${label} must be a bounded trimmed string`);
  }
  return value;
}

function boolean(value, label) {
  if (typeof value !== "boolean") fail(`${label} must be a boolean`);
  return value;
}

function nonNegativeInteger(value, label) {
  if (!Number.isSafeInteger(value) || value < 0) {
    fail(`${label} must be a non-negative safe integer`);
  }
  return value;
}

function shaIdentity(value, label) {
  if (typeof value !== "string" || !sha256Pattern.test(value)) {
    fail(`${label} must be a lowercase SHA-256 identity`);
  }
  return value;
}

function deploymentDigest(value, label) {
  if (typeof value !== "string" || !digestPattern.test(value)) {
    fail(`${label} must be an immutable image RepoDigest`);
  }
  return value;
}

function timestamp(value, label) {
  if (typeof value !== "string" || value.length > 64) {
    fail(`${label} must be a bounded timestamp`);
  }
  const milliseconds = Date.parse(value);
  if (!Number.isFinite(milliseconds) || new Date(milliseconds).toISOString() !== value) {
    fail(`${label} must be an exact ISO-8601 UTC timestamp`);
  }
  return milliseconds;
}

function shaArray(value, label) {
  if (!Array.isArray(value) || value.length === 0 || value.length > 10_000) {
    fail(`${label} must be a non-empty bounded array`);
  }
  const normalized = value.map((entry, index) => shaIdentity(entry, `${label}[${index}]`));
  if (new Set(normalized).size !== normalized.length) fail(`${label} contains duplicates`);
  return [...normalized].sort();
}

function sourceHashes() {
  if (!Array.isArray(contract.required_source_files) || contract.required_source_files.length === 0) {
    fail("rollout contract required_source_files is empty");
  }
  return Object.fromEntries(
    contract.required_source_files.map((relativePath) => {
      const location = path.join(repoRoot, relativePath);
      if (!existsSync(location)) fail(`required source file is missing: ${relativePath}`);
      const link = lstatSync(location);
      if (link.isSymbolicLink() || !link.isFile()) {
        fail(`required source file must be regular and non-symlink: ${relativePath}`);
      }
      return [relativePath, sha256(readFileSync(location))];
    }),
  );
}

function validateBrowser(document, head) {
  if (
    document.format !== contract.browser_input.format ||
    document.status !== contract.browser_input.status ||
    document.source_commit !== head
  ) {
    fail("browser evidence identity, status, or source commit drifted");
  }
  const target = object(document.target, "browser target");
  const digest = deploymentDigest(
    target.deployment_image_digest,
    "browser deployment image digest",
  );
  const boundaries = object(document.boundaries, "browser boundaries");
  if (
    boundaries.tenant_rollout_executed !== false ||
    boundaries.ffa_promoted !== false ||
    boundaries.fba_promoted !== false ||
    boundaries.canonical_source_mutated !== false
  ) {
    fail("browser evidence does not retain an open rollout boundary");
  }
  return { deploymentDigest: digest };
}

function validateCohort(value) {
  const cohort = object(value, "rollout cohort");
  if (cohort.flag_key !== contract.observation_input.flag_key) {
    fail("rollout feature flag key drifted");
  }
  const enabled = shaArray(cohort.enabled_tenant_sha256, "enabled tenant cohort");
  const controls = shaArray(cohort.disabled_control_tenant_sha256, "disabled control cohort");
  const controlSet = new Set(controls);
  if (enabled.some((identity) => controlSet.has(identity))) {
    fail("enabled and disabled control tenant cohorts overlap");
  }
  return {
    flag_key: cohort.flag_key,
    enabled_tenant_sha256: enabled,
    disabled_control_tenant_sha256: controls,
  };
}

function validateAdmission(value) {
  const admission = object(value, "rollout admission");
  const result = {};
  for (const key of contract.observation_input.required_admission_facts) {
    if (admission[key] !== true) fail(`admission.${key} must be true`);
    result[key] = true;
  }
  return result;
}

function validateWindow(value) {
  const window = object(value, "observation window");
  const startedAt = timestamp(window.started_at, "observation window started_at");
  const endedAt = timestamp(window.ended_at, "observation window ended_at");
  if (endedAt <= startedAt) fail("observation window must have positive duration");
  if ((endedAt - startedAt) % 1000 !== 0) {
    fail("observation window duration must resolve to whole seconds");
  }
  const durationSeconds = nonNegativeInteger(
    window.duration_seconds,
    "observation window duration_seconds",
  );
  if (durationSeconds === 0 || durationSeconds !== (endedAt - startedAt) / 1000) {
    fail("observation window duration_seconds does not match its timestamps");
  }
  return {
    started_at: window.started_at,
    ended_at: window.ended_at,
    duration_seconds: durationSeconds,
    startedAt,
    endedAt,
  };
}

function validateMonitoring(value) {
  const monitoring = object(value, "rollout monitoring");
  const result = {};
  for (const series of contract.observation_input.required_monitoring_series) {
    const entry = object(monitoring[series], `monitoring.${series}`);
    const observed = nonNegativeInteger(entry.observed, `monitoring.${series}.observed`);
    const threshold = nonNegativeInteger(entry.threshold, `monitoring.${series}.threshold`);
    if (observed > threshold) {
      fail(`monitoring.${series} exceeds its reviewed threshold`);
    }
    result[series] = { observed, threshold, within_threshold: true };
  }
  const unexpected = Object.keys(monitoring).filter(
    (key) => !contract.observation_input.required_monitoring_series.includes(key),
  );
  if (unexpected.length > 0) fail(`unexpected monitoring series: ${unexpected.join(", ")}`);
  return result;
}

function validateRollback(value, deploymentImageDigest, phase) {
  const rollback = object(value, "rollout rollback");
  const ownerSha256 = shaIdentity(rollback.owner_sha256, "rollback owner identity");
  const imageDigest = deploymentDigest(rollback.image_digest, "rollback image digest");
  if (imageDigest === deploymentImageDigest) {
    fail("rollback image digest must differ from the active deployment image digest");
  }
  const rehearsal = object(rollback.rehearsal, "rollback rehearsal");
  const executed = boolean(rollback.rehearsal.executed, "rollback rehearsal executed");
  const passed = boolean(rollback.rehearsal.passed, "rollback rehearsal passed");
  if (passed && !executed) fail("rollback rehearsal cannot pass without execution");
  if (phase === "fba" && (!executed || !passed)) {
    fail("FBA evidence requires a successful rollback rehearsal");
  }
  if (phase === "ffa" && (executed || passed)) {
    fail("FFA observation must not claim the later FBA rollback rehearsal");
  }
  return {
    owner_sha256: ownerSha256,
    image_digest: imageDigest,
    rehearsal: { executed, passed },
  };
}

function validateApprovals(value, phase) {
  const approvals = object(value, "rollout approvals");
  const required = [
    "browser_evidence_reviewed",
    "configuration_reviewed",
    "monitoring_reviewed",
    "rollout_owner_approved",
  ];
  const result = {};
  for (const key of required) {
    if (approvals[key] !== true) fail(`approvals.${key} must be true`);
    result[key] = true;
  }
  const ffaPacketReviewed = boolean(
    approvals.ffa_packet_reviewed,
    "approvals.ffa_packet_reviewed",
  );
  if (phase === "fba" && !ffaPacketReviewed) {
    fail("FBA evidence requires reviewed FFA evidence");
  }
  if (phase === "ffa" && ffaPacketReviewed) {
    fail("FFA observation cannot claim review of a not-yet-produced FFA packet");
  }
  result.ffa_packet_reviewed = ffaPacketReviewed;
  return result;
}

function validateObservation(document, phase, head, browserDigest) {
  if (
    document.format !== contract.observation_input.format ||
    document.status !== "maintainer_observation_recorded" ||
    document.phase !== phase ||
    document.source_commit !== head
  ) {
    fail("rollout observation identity, phase, status, or source commit drifted");
  }
  const digest = deploymentDigest(
    document.deployment_image_digest,
    "rollout observation deployment image digest",
  );
  if (digest !== browserDigest) {
    fail("rollout observation deployment image digest differs from browser evidence");
  }
  const environment = object(document.environment, "rollout environment");
  const environmentName = boundedString(environment.name, "rollout environment name", 256);
  const configurationProfile = boundedString(
    environment.configuration_profile,
    "rollout configuration profile",
    256,
  );
  return {
    deploymentDigest: digest,
    environmentSha256: sha256(environmentName),
    configurationProfileSha256: sha256(configurationProfile),
    cohort: validateCohort(document.cohort),
    admission: validateAdmission(document.admission),
    window: validateWindow(document.window),
    monitoring: validateMonitoring(document.monitoring),
    rollback: validateRollback(document.rollback, digest, phase),
    approvals: validateApprovals(document.approvals, phase),
  };
}

function validateFfa(document, head, browserDigest, fbaWindow) {
  if (
    document.format !== contract.output.format ||
    document.status !== contract.phases.ffa.output_status ||
    document.phase !== "ffa" ||
    document.source_commit !== head
  ) {
    fail("previous FFA packet identity, status, phase, or source commit drifted");
  }
  const target = object(document.target, "previous FFA target");
  if (target.deployment_image_digest !== browserDigest) {
    fail("previous FFA deployment digest differs from browser evidence");
  }
  const boundaries = object(document.boundaries, "previous FFA boundaries");
  if (
    boundaries.tenant_rollout_executed !== true ||
    boundaries.ffa_promoted !== true ||
    boundaries.fba_promoted !== false ||
    boundaries.canonical_source_mutated !== false
  ) {
    fail("previous FFA packet boundaries drifted");
  }
  const window = validateWindow(document.window);
  if (window.endedAt >= fbaWindow.startedAt) {
    fail("FFA observation window must end before the FBA observation window starts");
  }
  return { window };
}

function outputLocation(value) {
  const location = absolute(value);
  const targetRoot = path.join(repoRoot, "target");
  const relative = path.relative(targetRoot, location);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    fail("rollout evidence output must remain inside repository target/");
  }
  return location;
}

function writeAtomic(location, document) {
  mkdirSync(path.dirname(location), { recursive: true });
  const temporary = `${location}.tmp-${process.pid}`;
  rmSync(temporary, { force: true });
  writeFileSync(temporary, `${JSON.stringify(document, null, 2)}\n`, "utf8");
  renameSync(temporary, location);
}

const options = parseArguments(process.argv.slice(2));
for (const required of ["phase", "browser", "observation", "output"]) {
  if (!options[required]) fail(`--${required} is required`);
}
if (!Object.hasOwn(contract.phases, options.phase)) fail("--phase must be ffa or fba");
if (options.phase === "fba" && !options.ffa) fail("--ffa is required for FBA evidence");
if (options.phase === "ffa" && options.ffa) fail("--ffa is only valid for FBA evidence");

const head = currentCommit();
const browserInput = readJson(options.browser, "browser evidence");
const browser = validateBrowser(browserInput.document, head);
const observationInput = readJson(options.observation, "rollout observation");
const observation = validateObservation(
  observationInput.document,
  options.phase,
  head,
  browser.deploymentDigest,
);
let ffaInput = null;
let previousFfa = null;
if (options.phase === "fba") {
  ffaInput = readJson(options.ffa, "previous FFA evidence");
  previousFfa = validateFfa(
    ffaInput.document,
    head,
    browser.deploymentDigest,
    observation.window,
  );
}

const document = {
  format: contract.output.format,
  status: contract.phases[options.phase].output_status,
  source_commit: head,
  generated_at: new Date().toISOString(),
  phase: options.phase,
  source_sha256: sourceHashes(),
  inputs: {
    browser: { bytes: browserInput.bytes, sha256: browserInput.sha256 },
    observation: { bytes: observationInput.bytes, sha256: observationInput.sha256 },
    previous_ffa:
      ffaInput === null ? null : { bytes: ffaInput.bytes, sha256: ffaInput.sha256 },
  },
  target: {
    deployment_image_digest: browser.deploymentDigest,
    environment_sha256: observation.environmentSha256,
    configuration_profile_sha256: observation.configurationProfileSha256,
  },
  cohort: observation.cohort,
  admission: observation.admission,
  window: {
    started_at: observation.window.started_at,
    ended_at: observation.window.ended_at,
    duration_seconds: observation.window.duration_seconds,
  },
  monitoring: observation.monitoring,
  rollback: observation.rollback,
  approvals: observation.approvals,
  predecessor: {
    browser_evidence_required: true,
    previous_ffa_required: options.phase === "fba",
    previous_ffa_window:
      previousFfa === null
        ? null
        : {
            started_at: previousFfa.window.started_at,
            ended_at: previousFfa.window.ended_at,
            duration_seconds: previousFfa.window.duration_seconds,
          },
  },
  boundaries: {
    tenant_rollout_executed: true,
    ffa_promoted: true,
    fba_promoted: options.phase === "fba",
    canonical_source_mutated: false,
    configuration_mutated_by_assembler: false,
    deployment_mutated_by_assembler: false,
    promotion_performed_by_assembler: false,
    rollback_performed_by_assembler: false,
  },
  privacy: {
    raw_tenant_ids_persisted: false,
    tenant_names_persisted: false,
    raw_environment_or_configuration_profile_persisted: false,
    raw_rollout_owner_persisted: false,
    credentials_or_secrets_persisted: false,
    raw_monitoring_logs_or_alert_payloads_persisted: false,
    raw_browser_html_or_request_response_bodies_persisted: false,
  },
};

const output = outputLocation(options.output);
writeAtomic(output, document);
console.log(
  `[assemble-pages-inline-edit-rollout-evidence] PASS phase=${options.phase} status=${document.status} source_commit=${head} output=${output}`,
);
