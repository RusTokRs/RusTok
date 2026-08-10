#!/usr/bin/env node

import { spawnSync } from "node:child_process";
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

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const admissionContractPath = path.join(
  repoRoot,
  "crates/rustok-forum/contracts/evidence/forum-page-builder-wave-admission-source.json",
);
const gateContractPath = path.join(
  repoRoot,
  "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-acceptance-source.json",
);
const browserContractPath = path.join(
  repoRoot,
  "crates/rustok-forum/contracts/evidence/forum-page-builder-browser-execution-contract.json",
);
const runtimeContractPath = path.join(
  repoRoot,
  "crates/rustok-forum/contracts/evidence/forum-page-builder-runtime-authorization-execution-contract.json",
);
const serverfnContractPath = path.join(
  repoRoot,
  "crates/rustok-forum/contracts/evidence/forum-page-builder-serverfn-deployment-attestation-contract.json",
);

const MAX_INPUT_BYTES = 32 * 1024 * 1024;
const MAX_SOURCE_BYTES = 8 * 1024 * 1024;
const COMMIT_PATTERN = /^[0-9a-f]{40}$/u;
const SHA256_PATTERN = /^[0-9a-f]{64}$/u;
const REPO_DIGEST_PATTERN = /^[^@\s]+@sha256:[0-9a-f]{64}$/u;

function fail(message) {
  throw new Error(`Forum Page Builder Wave admission failed: ${message}`);
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function objectValue(value, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
  return value;
}

function canonicalJson(value) {
  const normalize = (input) => {
    if (Array.isArray(input)) return input.map(normalize);
    if (input !== null && typeof input === "object") {
      return Object.fromEntries(
        Object.entries(input)
          .sort(([left], [right]) => left.localeCompare(right))
          .map(([key, nested]) => [key, normalize(nested)]),
      );
    }
    return input;
  };
  return JSON.stringify(normalize(value));
}

function currentCommit() {
  const result = spawnSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
    shell: false,
    maxBuffer: 1024 * 1024,
  });
  if (result.error) fail(`git HEAD lookup failed: ${result.error.message}`);
  if (result.status !== 0) fail("git HEAD lookup returned a non-zero status");
  const value = result.stdout.trim();
  if (!COMMIT_PATTERN.test(value)) fail("git HEAD is not a full lowercase SHA");
  return value;
}

function parseArguments(argv) {
  const options = {};
  const allowed = new Set([
    "--pages-gate",
    "--browser-evidence",
    "--runtime-evidence",
    "--serverfn-attestation",
    "--output",
  ]);
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--help" || argument === "-h") {
      console.log(
        "usage: admit-forum-page-builder-wave.mjs " +
          "--pages-gate FILE --browser-evidence FILE --runtime-evidence FILE " +
          "--serverfn-attestation FILE [--output FILE]",
      );
      process.exit(0);
    }
    if (!allowed.has(argument)) fail(`unknown argument ${argument}`);
    const value = argv[index + 1];
    if (!value) fail(`${argument} requires a value`);
    const key = argument
      .slice(2)
      .replace(/-([a-z])/gu, (_, letter) => letter.toUpperCase());
    options[key] = value;
    index += 1;
  }
  return options;
}

function resolveInput(value, label) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > 16_384 ||
    /[\u0000\r\n]/u.test(value)
  ) {
    fail(`${label} path is invalid`);
  }
  return path.isAbsolute(value) ? path.resolve(value) : path.resolve(repoRoot, value);
}

function regularFile(location, label, maximumBytes = MAX_INPUT_BYTES) {
  if (!existsSync(location)) fail(`${label} is missing`);
  const metadata = lstatSync(location);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    fail(`${label} must be a regular non-symlink file`);
  }
  const size = statSync(location).size;
  if (size <= 0 || size > maximumBytes) fail(`${label} is outside the bounded size`);
  const bytes = readFileSync(location);
  return { bytes, size, sha256: sha256(bytes) };
}

function jsonInput(value, label) {
  const record = regularFile(resolveInput(value, label), label);
  try {
    const document = JSON.parse(record.bytes.toString("utf8"));
    objectValue(document, label);
    return { document, record };
  } catch (error) {
    fail(`${label} is invalid JSON: ${error.message}`);
  }
}

function jsonSource(location, label) {
  const record = regularFile(location, label, MAX_SOURCE_BYTES);
  try {
    const document = JSON.parse(record.bytes.toString("utf8"));
    objectValue(document, label);
    return document;
  } catch (error) {
    fail(`${label} is invalid JSON: ${error.message}`);
  }
}

function requireSha256(value, label) {
  if (typeof value !== "string" || !SHA256_PATTERN.test(value)) {
    fail(`${label} must be a lowercase SHA-256`);
  }
  return value;
}

function requireRepoDigest(value, label) {
  if (typeof value !== "string" || value.length > 1024 || !REPO_DIGEST_PATTERN.test(value)) {
    fail(`${label} must be an immutable REPOSITORY@sha256:<digest>`);
  }
  return value;
}

function requireCanonicalIso(value, label) {
  if (typeof value !== "string") fail(`${label} must be a canonical ISO timestamp`);
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp) || new Date(timestamp).toISOString() !== value) {
    fail(`${label} must be a canonical ISO timestamp`);
  }
  return value;
}

function expectedSourceFiles(contract, label) {
  const files = contract.required_source_files;
  if (!Array.isArray(files) || files.length === 0 || files.length > 128) {
    fail(`${label} required_source_files is invalid`);
  }
  if (new Set(files).size !== files.length) fail(`${label} required_source_files contains duplicates`);
  return [...files].sort();
}

function sourceHash(relativePath) {
  if (
    typeof relativePath !== "string" ||
    relativePath.length === 0 ||
    relativePath.length > 4096 ||
    relativePath.includes("\0")
  ) {
    fail("source path is invalid");
  }
  const absolute = path.resolve(repoRoot, relativePath);
  const relative = path.relative(repoRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    fail(`source path escapes repository: ${relativePath}`);
  }
  return sha256(regularFile(absolute, `source file ${relativePath}`, MAX_SOURCE_BYTES).bytes);
}

function verifyRetainedSourceHashes(document, sourceContract, field, label) {
  const retained = objectValue(document[field], `${label}.${field}`);
  const expectedNames = expectedSourceFiles(sourceContract, label);
  const actualNames = Object.keys(retained).sort();
  if (canonicalJson(actualNames) !== canonicalJson(expectedNames)) {
    fail(`${label} source hash set differs from its source contract`);
  }
  for (const relativePath of expectedNames) {
    if (requireSha256(retained[relativePath], `${label} source hash ${relativePath}`) !== sourceHash(relativePath)) {
      fail(`${label} source hash for ${relativePath} does not match checkout`);
    }
  }
}

function sourceHashes(contract) {
  return Object.fromEntries(
    expectedSourceFiles(contract, "Wave admission contract").map((relativePath) => [
      relativePath,
      sourceHash(relativePath),
    ]),
  );
}

function requirePacketRecord(value, label) {
  const record = objectValue(value, label);
  if (!Number.isSafeInteger(record.bytes) || record.bytes <= 0 || record.bytes > MAX_INPUT_BYTES) {
    fail(`${label}.bytes is invalid`);
  }
  requireSha256(record.sha256, `${label}.sha256`);
}

function validatePagesGate(input, admissionContract, gateContract, head) {
  const document = input.document;
  const specification = admissionContract.pages_gate_input;
  if (document.format !== specification?.format || document.status !== specification?.required_status) {
    fail("Pages gate packet format/status drifted");
  }
  if (document.source_commit !== head) fail("Pages gate source_commit does not equal checkout HEAD");
  requireCanonicalIso(document.decided_at, "Pages gate decided_at");
  const deployment = objectValue(document.deployment, "Pages gate deployment");
  const deploymentId = deployment.deployment_id;
  if (typeof deploymentId !== "string" || deploymentId.length === 0 || deploymentId.length > 256) {
    fail("Pages gate deployment_id is invalid");
  }
  const deploymentDigest = requireRepoDigest(
    deployment.deployment_image_digest,
    "Pages gate deployment image digest",
  );
  verifyRetainedSourceHashes(document, gateContract, "source_files", "Pages gate packet");

  const decision = objectValue(document.decision, "Pages gate decision");
  if (
    decision.value !== specification.required_decision ||
    decision.owner_identity_is_operator_assertion !== true ||
    decision.cryptographic_signature_present !== false ||
    decision.free_text_reason_retained !== false
  ) {
    fail("Pages gate owner decision contract drifted");
  }
  const rollback = objectValue(document.rollback_decision, "Pages gate rollback decision");
  if (
    rollback.value !== specification.required_rollback_decision ||
    rollback.rollback_action_performed !== false
  ) {
    fail("Pages gate rollback disposition drifted");
  }
  const gate = objectValue(document.gate, "Pages gate result");
  if (
    gate.id !== "pages_reference_consumer_gate" ||
    gate.accepted !== true ||
    gate.owner_signoff_satisfied !== true ||
    gate.rollback_decision_satisfied !== true ||
    gate.exact_source_commit_bound !== true ||
    gate.exact_deployment_digest_bound !== true ||
    gate.candidate_and_observed_health_chain_bound !== true
  ) {
    fail("Pages reference-consumer gate is not fully accepted and bound");
  }
  const boundaries = objectValue(document.boundaries, "Pages gate boundaries");
  for (const key of [
    "canonical_source_mutated",
    "rollback_action_executed",
    "forum_wave_accepted",
    "ffa_promoted",
    "fba_promoted",
    "automatic_downstream_promotion",
  ]) {
    if (boundaries[key] !== false) fail(`Pages gate boundary ${key} must remain false`);
  }
  return { sourceCommit: document.source_commit, deploymentId, deploymentDigest };
}

function requireBrowserFacts(profile, requiredFacts) {
  const observation = objectValue(profile, "browser profile observation");
  if (observation.passed !== true || observation.criticalFailures !== 0) {
    fail("Forum browser profile did not pass cleanly");
  }
  const facts = objectValue(observation.facts, "browser profile facts");
  for (const fact of requiredFacts) {
    if (facts[fact] !== true) fail(`Forum browser fact ${fact} must be true`);
  }
}

function validateBrowser(input, admissionContract, browserContract, head, gate) {
  const document = input.document;
  const specification = admissionContract.forum_browser_input;
  if (document.format !== specification?.format || document.status !== specification?.required_status) {
    fail("Forum browser packet format/status drifted");
  }
  if (document.source_commit !== head || document.source_commit !== gate.sourceCommit) {
    fail("Forum browser source_commit differs from gate or checkout");
  }
  if (requireRepoDigest(document.deployment_digest, "Forum browser deployment digest") !== gate.deploymentDigest) {
    fail("Forum browser deployment digest differs from accepted Pages gate");
  }
  requireCanonicalIso(document.executed_at, "Forum browser executed_at");
  verifyRetainedSourceHashes(document, browserContract, "source_files", "Forum browser packet");
  if (canonicalJson(browserContract.profiles) !== canonicalJson(["full", "preview_off", "properties_off", "forum_disabled", "no_read"])) {
    fail("Forum browser execution contract profile set drifted");
  }
  const observations = objectValue(document.observations, "Forum browser observations");
  if (canonicalJson(Object.keys(observations).sort()) !== canonicalJson([...browserContract.profiles].sort())) {
    fail("Forum browser packet profile set drifted");
  }
  requireBrowserFacts(observations.full, [
    "topic_list_admitted",
    "invalid_owner_props_rejected",
    "owner_normalization_observed",
    "fly_undo_observed",
    "fly_redo_observed",
    "owner_preview_ready",
    "pages_save_completed",
  ]);
  requireBrowserFacts(observations.preview_off, [
    "topic_list_admitted",
    "owner_properties_actionable",
    "owner_preview_not_admitted",
  ]);
  requireBrowserFacts(observations.properties_off, [
    "topic_list_not_admitted",
    "owner_properties_not_actionable",
  ]);
  requireBrowserFacts(observations.forum_disabled, [
    "topic_list_absent",
    "owner_property_panel_absent",
    "owner_preview_panel_absent",
  ]);
  requireBrowserFacts(observations.no_read, [
    "topic_list_not_admitted",
    "owner_properties_not_actionable",
  ]);
  if (
    document.retained_secrets !== false ||
    document.browser_execution_only !== true ||
    document.runtime_authorization_evidence_pending !== true ||
    document.observed_page_builder_wave_pending !== true
  ) {
    fail("Forum browser execution/non-promotion boundary drifted");
  }
  const records = objectValue(document.input_records, "Forum browser input records");
  requirePacketRecord(records.editor_storage_state, "Forum browser editor storage record");
  requirePacketRecord(records.no_read_storage_state, "Forum browser no-read storage record");
  const routeHashes = objectValue(records.profile_url_sha256, "Forum browser profile URL hashes");
  if (canonicalJson(Object.keys(routeHashes).sort()) !== canonicalJson([...browserContract.profiles].sort())) {
    fail("Forum browser profile URL hash set drifted");
  }
  for (const profile of browserContract.profiles) {
    requireSha256(routeHashes[profile], `Forum browser ${profile} URL hash`);
  }
}

function validateCommandResults(actual, expected, label) {
  if (!Array.isArray(actual) || !Array.isArray(expected) || actual.length !== expected.length || actual.length === 0) {
    fail(`${label} command set differs from execution contract`);
  }
  for (let index = 0; index < expected.length; index += 1) {
    const record = objectValue(actual[index], `${label}[${index}]`);
    const command = objectValue(expected[index], `${label} expected[${index}]`);
    if (
      record.id !== command.id ||
      record.program !== command.program ||
      canonicalJson(record.args) !== canonicalJson(command.args) ||
      record.status !== 0
    ) {
      fail(`${label}[${index}] id/program/argv/status drifted`);
    }
    for (const streamName of ["stdout", "stderr"]) {
      const stream = objectValue(record[streamName], `${label}[${index}].${streamName}`);
      if (!Number.isSafeInteger(stream.bytes) || stream.bytes < 0 || stream.bytes > MAX_INPUT_BYTES) {
        fail(`${label}[${index}].${streamName}.bytes is invalid`);
      }
      requireSha256(stream.sha256, `${label}[${index}].${streamName}.sha256`);
    }
  }
}

function validateRuntime(input, admissionContract, runtimeContract, head, gate) {
  const document = input.document;
  const specification = admissionContract.forum_runtime_authorization_input;
  if (document.format !== specification?.format || document.status !== specification?.required_status) {
    fail("Forum runtime-authorization packet format/status drifted");
  }
  if (document.source_commit !== head || document.source_commit !== gate.sourceCommit) {
    fail("Forum runtime-authorization source_commit differs from gate or checkout");
  }
  requireCanonicalIso(document.executed_at, "Forum runtime-authorization executed_at");
  verifyRetainedSourceHashes(document, runtimeContract, "source_files", "Forum runtime-authorization packet");
  validateCommandResults(document.commands, runtimeContract.commands, "Forum runtime-authorization commands");
  if (
    document.retained_raw_command_output !== false ||
    document.runtime_authorization_execution_only !== true ||
    document.deployed_server_fn_attestation_not_claimed !== true ||
    document.browser_execution_not_claimed !== true ||
    document.provider_slo_health_not_claimed !== true ||
    document.observed_page_builder_wave_pending !== true
  ) {
    fail("Forum runtime-authorization execution/non-promotion boundary drifted");
  }
}

function validateServerfn(input, admissionContract, serverfnContract, head, gate) {
  const document = input.document;
  const specification = admissionContract.forum_serverfn_attestation_input;
  if (document.format !== specification?.format || document.status !== specification?.required_status) {
    fail("Forum server-function attestation format/status drifted");
  }
  if (document.source_commit !== head || document.source_commit !== gate.sourceCommit) {
    fail("Forum server-function source_commit differs from gate or checkout");
  }
  if (document.live_server_source_commit_verified_equal_checkout !== true) {
    fail("Forum server-function packet did not verify live source commit against checkout");
  }
  requireCanonicalIso(document.captured_at, "Forum server-function captured_at");
  const target = objectValue(document.target, "Forum server-function target");
  if (requireRepoDigest(target.deployment_image_digest, "Forum server-function deployment digest") !== gate.deploymentDigest) {
    fail("Forum server-function deployment digest differs from accepted Pages gate");
  }
  requireSha256(target.origin_sha256, "Forum server-function origin hash");
  if (
    !Number.isSafeInteger(target.origin_bytes) ||
    target.origin_bytes <= 0 ||
    target.origin_bytes > 16_384 ||
    target.raw_origin_persisted !== false ||
    target.origin_to_repo_digest_binding !== "maintainer_reviewed_external_fact" ||
    target.cryptographic_origin_to_repo_digest_binding !== false
  ) {
    fail("Forum server-function target identity/privacy boundary drifted");
  }
  verifyRetainedSourceHashes(document, serverfnContract, "source_files", "Forum server-function packet");
  const expectedScenarios = serverfnContract.scenarios;
  const scenarios = document.scenarios;
  if (!Array.isArray(scenarios) || canonicalJson(scenarios.map((value) => value?.id)) !== canonicalJson(expectedScenarios.map((value) => value.id))) {
    fail("Forum server-function scenario set/order differs from contract");
  }
  for (let index = 0; index < expectedScenarios.length; index += 1) {
    const expected = expectedScenarios[index];
    const record = objectValue(scenarios[index], `Forum server-function scenario ${expected.id}`);
    if (expected.expected_status !== undefined && record.status !== expected.expected_status) {
      fail(`Forum server-function scenario ${expected.id} status drifted`);
    }
    if (!Number.isInteger(record.status) || record.status < 100 || record.status > 599) {
      fail(`Forum server-function scenario ${expected.id} status is invalid`);
    }
    if (record.credential_values_persisted !== false || record.raw_body_persisted !== false) {
      fail(`Forum server-function scenario ${expected.id} retained credential values or raw body`);
    }
    if (!Number.isSafeInteger(record.body_bytes) || record.body_bytes < 0 || record.body_bytes > MAX_INPUT_BYTES) {
      fail(`Forum server-function scenario ${expected.id} body size is invalid`);
    }
    requireSha256(record.body_sha256, `Forum server-function scenario ${expected.id} body hash`);
  }
  const privacy = objectValue(document.privacy, "Forum server-function privacy");
  if (
    privacy.credential_environment_names_only !== true ||
    privacy.credential_values_persisted !== false ||
    privacy.common_header_values_persisted !== false ||
    privacy.raw_response_bodies_persisted !== false ||
    privacy.tenant_or_actor_identifiers_persisted !== false ||
    privacy.forum_content_persisted !== false
  ) {
    fail("Forum server-function privacy boundary drifted");
  }
  if (
    document.browser_execution_not_claimed !== true ||
    document.runtime_authorization_execution_not_claimed !== true ||
    document.provider_slo_health_not_claimed !== true ||
    document.observed_page_builder_wave_pending !== true
  ) {
    fail("Forum server-function non-promotion boundary drifted");
  }
}

function outputPath(contract, requested) {
  const value = requested ?? contract.output.default_path;
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > 16_384 ||
    /[\u0000\r\n]/u.test(value)
  ) {
    fail("output path is invalid");
  }
  const absolute = path.isAbsolute(value) ? path.resolve(value) : path.resolve(repoRoot, value);
  const targetRoot = path.resolve(repoRoot, "target");
  const relative = path.relative(targetRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    fail("Wave admission output must remain inside repository target/");
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

function main() {
  const options = parseArguments(process.argv.slice(2));
  for (const required of ["pagesGate", "browserEvidence", "runtimeEvidence", "serverfnAttestation"]) {
    if (!options[required]) fail(`missing required ${required} input`);
  }

  const admissionContract = jsonSource(admissionContractPath, "Wave admission source contract");
  const gateContract = jsonSource(gateContractPath, "Pages gate acceptance source contract");
  const browserContract = jsonSource(browserContractPath, "Forum browser execution contract");
  const runtimeContract = jsonSource(runtimeContractPath, "Forum runtime-authorization execution contract");
  const serverfnContract = jsonSource(serverfnContractPath, "Forum server-function attestation contract");
  if (
    admissionContract.format !== "forum_page_builder_wave_admission_source_v1" ||
    admissionContract.status !== "source_ready_maintainer_execution_pending" ||
    admissionContract.module !== "forum" ||
    admissionContract.wave !== "1"
  ) {
    fail("Wave admission source contract identity drifted");
  }
  if (
    gateContract.status !== "source_ready_maintainer_execution_pending" ||
    browserContract.status !== "source_ready_maintainer_execution_pending" ||
    runtimeContract.status !== "source_ready_maintainer_execution_pending" ||
    serverfnContract.status !== "source_ready_maintainer_execution_pending"
  ) {
    fail("Wave admission predecessor source contract drifted");
  }

  const head = currentCommit();
  const gateInput = jsonInput(options.pagesGate, "accepted Pages gate evidence");
  const browserInput = jsonInput(options.browserEvidence, "Forum browser evidence");
  const runtimeInput = jsonInput(options.runtimeEvidence, "Forum runtime-authorization evidence");
  const serverfnInput = jsonInput(options.serverfnAttestation, "Forum server-function attestation");

  const gate = validatePagesGate(gateInput, admissionContract, gateContract, head);
  validateBrowser(browserInput, admissionContract, browserContract, head, gate);
  validateRuntime(runtimeInput, admissionContract, runtimeContract, head, gate);
  validateServerfn(serverfnInput, admissionContract, serverfnContract, head, gate);

  const output = outputPath(admissionContract, options.output);
  rmSync(output, { force: true });
  writeAtomic(output, {
    format: admissionContract.output.format,
    status: admissionContract.output.status,
    generated_at: new Date().toISOString(),
    source_commit: head,
    deployment: {
      deployment_id: gate.deploymentId,
      deployment_image_digest: gate.deploymentDigest,
    },
    inputs: {
      pages_gate: { bytes: gateInput.record.size, sha256: gateInput.record.sha256 },
      forum_browser: { bytes: browserInput.record.size, sha256: browserInput.record.sha256 },
      forum_runtime_authorization: { bytes: runtimeInput.record.size, sha256: runtimeInput.record.sha256 },
      forum_serverfn_attestation: { bytes: serverfnInput.record.size, sha256: serverfnInput.record.sha256 },
      raw_input_paths_persisted: false,
    },
    admission: {
      pages_reference_consumer_gate_accepted: true,
      exact_source_commit_bound: true,
      exact_deployment_digest_bound: true,
      forum_browser_execution_passed: true,
      forum_runtime_authorization_execution_passed: true,
      forum_server_fn_deployment_attestation_passed: true,
      observed_control_plane_wave_pending: true,
    },
    boundaries: {
      canonical_forum_wave_packet_mutated: false,
      control_plane_audit_trail_observed: false,
      observability_metrics_observed: false,
      observability_traces_observed: false,
      rollback_decision_observed: false,
      approvals_observed: false,
      observed_control_plane_wave_executed: false,
      forum_wave_accepted: false,
      ffa_promoted: false,
      fba_promoted: false,
    },
    privacy: {
      raw_input_paths_persisted: false,
      raw_http_or_browser_bodies_persisted: false,
      raw_command_output_persisted: false,
      credentials_sessions_or_storage_state_contents_persisted: false,
      tenant_or_actor_identifiers_persisted: false,
      forum_content_persisted: false,
    },
    source_files: sourceHashes(admissionContract),
  });
}

try {
  main();
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
