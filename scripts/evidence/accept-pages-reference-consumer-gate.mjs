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
const contractPath = path.join(repoRoot, "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-acceptance-source.json");
const candidateContractPath = path.join(repoRoot, "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-execution-contract.json");
const observedAcceptanceSourcePath = path.join(repoRoot, "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-observed-acceptance-source.json");
const sourceGatePath = path.join(repoRoot, "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json");
const MAX_INPUT_BYTES = 32 * 1024 * 1024;
const MAX_SOURCE_BYTES = 8 * 1024 * 1024;
const OWNER_ID_PATTERN = /^[A-Za-z0-9._-]{1,64}$/u;
const COMMIT_PATTERN = /^[0-9a-f]{40}$/u;
const SHA256_PATTERN = /^[0-9a-f]{64}$/u;
const REPO_DIGEST_PATTERN = /^[^\s@]+@sha256:[0-9a-f]{64}$/u;
const ACCEPT_DECISION = "accept_pages_reference_consumer_gate";
const REJECT_DECISION = "reject";
const RETAIN_DECISION = "retain_reference_consumer_candidate";
const ROLLBACK_DECISION = "rollback_reference_consumer_candidate";
const OBSERVED_ACCEPT_DECISION = "accept_observed_runtime_evidence";

function fail(message) {
  throw new Error(`Pages reference-consumer gate acceptance failed: ${message}`);
}
function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}
function objectValue(value, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) fail(`${label} must be an object`);
  return value;
}
function canonicalJson(value) {
  const normalize = (input) => {
    if (Array.isArray(input)) return input.map(normalize);
    if (input !== null && typeof input === "object") {
      return Object.fromEntries(Object.entries(input).sort(([a], [b]) => a.localeCompare(b)).map(([key, nested]) => [key, normalize(nested)]));
    }
    return input;
  };
  return JSON.stringify(normalize(value));
}
function currentCommit() {
  const result = spawnSync("git", ["rev-parse", "HEAD"], { cwd: repoRoot, encoding: "utf8", shell: false, stdio: ["ignore", "pipe", "pipe"] });
  if (result.status !== 0) fail("git rev-parse HEAD failed");
  const commit = result.stdout.trim().toLowerCase();
  if (!COMMIT_PATTERN.test(commit)) fail("checkout HEAD is not a canonical lowercase Git SHA");
  return commit;
}
function parseArguments(argv) {
  const options = {};
  const accepted = new Set(["--candidate", "--observed-health-acceptance", "--owner-id", "--decision", "--rollback-decision", "--output"]);
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--help" || argument === "-h") {
      console.log(
        "usage: accept-pages-reference-consumer-gate.mjs " +
          "--candidate FILE --observed-health-acceptance FILE --owner-id ID " +
          "--decision accept_pages_reference_consumer_gate|reject " +
          "--rollback-decision retain_reference_consumer_candidate|rollback_reference_consumer_candidate [--output FILE]",
      );
      process.exit(0);
    }
    if (!accepted.has(argument)) fail(`unknown argument ${argument}`);
    const value = argv[index + 1];
    if (!value) fail(`${argument} requires a value`);
    options[argument.slice(2).replace(/-([a-z])/gu, (_, letter) => letter.toUpperCase())] = value;
    index += 1;
  }
  return options;
}
function resolveInput(candidate, label) {
  if (typeof candidate !== "string" || candidate.length === 0 || candidate.length > 16_384 || /[\u0000\r\n]/u.test(candidate)) fail(`${label} path is invalid`);
  return path.isAbsolute(candidate) ? path.resolve(candidate) : path.resolve(repoRoot, candidate);
}
function regularFile(location, label, maximumBytes = MAX_INPUT_BYTES) {
  if (!existsSync(location)) fail(`${label} is missing`);
  const metadata = lstatSync(location);
  if (!metadata.isFile() || metadata.isSymbolicLink()) fail(`${label} must be a regular non-symlink file`);
  const size = statSync(location).size;
  if (size <= 0 || size > maximumBytes) fail(`${label} is outside the bounded size`);
  const bytes = readFileSync(location);
  return { bytes, size, sha256: sha256(bytes) };
}
function jsonInput(candidate, label) {
  const record = regularFile(resolveInput(candidate, label), label);
  try {
    const document = JSON.parse(record.bytes.toString("utf8"));
    objectValue(document, label);
    return { document, ...record };
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
  if (typeof value !== "string" || !SHA256_PATTERN.test(value)) fail(`${label} must be a lowercase SHA-256`);
  return value;
}
function requireRepoDigest(value, label) {
  if (typeof value !== "string" || value.length > 1024 || !REPO_DIGEST_PATTERN.test(value)) fail(`${label} must be an immutable REPOSITORY@sha256:<digest>`);
  return value;
}
function requireOwnerId(value) {
  if (typeof value !== "string" || !OWNER_ID_PATTERN.test(value)) fail("--owner-id must match ^[A-Za-z0-9._-]{1,64}$");
  return value;
}
function expectedSourceFiles(sourceContract, label) {
  const files = sourceContract.required_source_files;
  if (!Array.isArray(files) || files.length === 0 || files.length > 128) fail(`${label} required_source_files is invalid`);
  if (new Set(files).size !== files.length) fail(`${label} required_source_files contains duplicates`);
  return [...files].sort();
}
function sourceHash(relativePath) {
  if (typeof relativePath !== "string" || relativePath.length === 0 || relativePath.length > 4096 || relativePath.includes("\0")) fail("source path is invalid");
  const absolute = path.resolve(repoRoot, relativePath);
  const relative = path.relative(repoRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) fail(`source path escapes repository: ${relativePath}`);
  return sha256(regularFile(absolute, `source file ${relativePath}`, MAX_SOURCE_BYTES).bytes);
}
function verifyRetainedSourceHashes(document, sourceContract, field, label) {
  const retained = objectValue(document[field], `${label}.${field}`);
  const expectedNames = expectedSourceFiles(sourceContract, label);
  const actualNames = Object.keys(retained).sort();
  if (canonicalJson(actualNames) !== canonicalJson(expectedNames)) fail(`${label} source hash set differs from source contract`);
  for (const relativePath of expectedNames) {
    if (requireSha256(retained[relativePath], `${label} source hash ${relativePath}`) !== sourceHash(relativePath)) {
      fail(`${label} source hash for ${relativePath} does not match checkout`);
    }
  }
}
function sourceHashes(contract) {
  return Object.fromEntries(expectedSourceFiles(contract, "gate acceptance contract").map((relativePath) => [relativePath, sourceHash(relativePath)]));
}
function requireAllFalse(value, label) {
  const record = objectValue(value, label);
  for (const [key, actual] of Object.entries(record)) if (actual !== false) fail(`${label}.${key} must remain false`);
}
function requirePacketRecord(value, label) {
  const record = objectValue(value, label);
  if (!Number.isSafeInteger(record.bytes) || record.bytes <= 0 || record.bytes > MAX_INPUT_BYTES) fail(`${label}.bytes is outside the admitted range`);
  requireSha256(record.sha256, `${label}.sha256`);
}
function requireCommandResults(value, expectedCommands, label) {
  if (!Array.isArray(value) || !Array.isArray(expectedCommands) || value.length !== expectedCommands.length || value.length === 0 || value.length > 64) {
    fail(`${label} must match the exact bounded execution-contract command set`);
  }
  for (let index = 0; index < expectedCommands.length; index += 1) {
    const record = objectValue(value[index], `${label}[${index}]`);
    const expected = objectValue(expectedCommands[index], `${label} expected[${index}]`);
    if (record.id !== expected.id || record.program !== expected.program || canonicalJson(record.args) !== canonicalJson(expected.args)) {
      fail(`${label}[${index}] id/program/argv differs from execution contract`);
    }
    if (record.status !== 0) fail(`${label}[${index}] contains a non-zero command status`);
    for (const streamName of ["stdout", "stderr"]) {
      const stream = objectValue(record[streamName], `${label}[${index}].${streamName}`);
      if (!Number.isSafeInteger(stream.bytes) || stream.bytes < 0 || stream.bytes > MAX_INPUT_BYTES) fail(`${label}[${index}].${streamName}.bytes is invalid`);
      requireSha256(stream.sha256, `${label}[${index}].${streamName}.sha256`);
    }
  }
}
function validateSourceGate(sourceGate) {
  if (
    sourceGate.artifact !== "pages_reference_consumer_gate_source" ||
    sourceGate.mode !== "source_ready" ||
    sourceGate.accepted !== false ||
    sourceGate.current_boundary?.execution_gate !== "pending" ||
    sourceGate.current_boundary?.provider_health !== "unobserved" ||
    sourceGate.current_boundary?.forum_wave_blocker !== "pages_reference_consumer_gate"
  ) {
    fail("committed Pages reference-consumer source gate must remain fail-closed before owner decision");
  }
  if (!sourceGate.forbidden_claims?.includes("observed provider health")) {
    fail("source gate must continue forbidding fabricated observed provider health");
  }
}
function validateCandidate(input, contract, candidateContract, head) {
  const document = input.document;
  if (document.format !== contract.candidate_input.format || document.status !== contract.candidate_input.required_status) fail("reference candidate format/status drifted");
  if (document.source_commit !== head) fail("reference candidate source_commit does not equal checkout HEAD");
  const deploymentDigest = requireRepoDigest(document.deployment_image_digest, "reference candidate deployment image digest");
  verifyRetainedSourceHashes(document, candidateContract, "source_sha256", "reference candidate");
  const inputs = objectValue(document.inputs, "reference candidate inputs");
  const expectedInputNames = ["artifact_http", "browser", "rollout_matrix", "rollout_feature_preflight"];
  if (canonicalJson(Object.keys(inputs).sort()) !== canonicalJson([...expectedInputNames].sort())) fail("reference candidate input hash set drifted");
  for (const inputName of expectedInputNames) requirePacketRecord(inputs[inputName], `reference candidate input ${inputName}`);
  requireCommandResults(document.source_guards, candidateContract.source_guards, "reference candidate source guards");
  requireCommandResults(document.focused_tests, candidateContract.focused_tests, "reference candidate focused tests");

  const candidate = objectValue(document.candidate, "reference candidate result");
  for (const key of [
    "all_source_guards_passed", "all_focused_tests_passed", "exact_source_commit_bound", "exact_deployment_digest_bound",
    "artifact_http_browser_chain_bound", "rollout_matrix_browser_chain_bound", "rollout_matrix_profiles_passed",
    "rollout_matrix_settings_restored", "rollout_feature_preflight_chain_bound", "rollout_feature_preflight_profiles_passed",
    "rollout_feature_preflight_settings_restored", "canonical_feature_disabled_catalog_passed", "browser_intent_denial_kept_separate",
  ]) if (candidate[key] !== true) fail(`reference candidate.${key} must be true`);
  if (candidate.provider_health !== "unobserved") fail("reference candidate provider_health must remain unobserved");
  if (candidate.owner_signoff !== "pending" || candidate.rollback_decision !== "pending" || candidate.gate_acceptance !== "pending") {
    fail("reference candidate owner/gate decisions must remain pending before acceptance");
  }
  const boundaries = objectValue(document.boundaries, "reference candidate boundaries");
  for (const key of ["canonical_source_mutated", "gate_accepted", "forum_wave_accepted", "ffa_promoted", "fba_promoted"]) {
    if (boundaries[key] !== false) fail(`reference candidate boundary ${key} must remain false`);
  }
  requireAllFalse(document.privacy, "reference candidate privacy");
  return { deploymentDigest, sourceCommit: document.source_commit };
}
function validateObservedHealthAcceptance(input, contract, observedSource, head, candidate) {
  const document = input.document;
  if (document.format !== contract.observed_health_input.format || document.status !== contract.observed_health_input.required_status) {
    fail("observed-health acceptance format/status drifted");
  }
  if (document.source_commit !== head || document.source_commit !== candidate.sourceCommit) fail("observed-health acceptance source_commit differs from candidate or checkout");
  const deployment = objectValue(document.deployment, "observed-health acceptance deployment");
  const deploymentId = deployment.deployment_id;
  if (typeof deploymentId !== "string" || deploymentId.length === 0 || deploymentId.length > 256) fail("observed-health deployment id is invalid");
  const digest = requireRepoDigest(deployment.deployment_image_digest, "observed-health deployment image digest");
  if (digest !== candidate.deploymentDigest) fail("observed-health acceptance deployment digest differs from reference candidate");
  verifyRetainedSourceHashes(document, observedSource, "source_files", "observed-health acceptance");

  const decision = objectValue(document.decision, "observed-health acceptance decision");
  if (decision.value !== OBSERVED_ACCEPT_DECISION || decision.owner_identity_is_operator_assertion !== true || decision.cryptographic_signature_present !== false || decision.free_text_reason_retained !== false) {
    fail("observed-health acceptance decision contract drifted");
  }
  const observed = objectValue(document.observed_health, "observed-health historical evidence");
  if (observed.historical_lease_deadline_only !== true || observed.current_provider_health_asserted !== false) fail("observed-health acceptance must remain retrospective");
  objectValue(observed.snapshot, "observed-health snapshot");
  objectValue(observed.slo_evaluation, "observed-health SLO evaluation");
  const binding = objectValue(document.binding_lineage, "observed-health binding lineage");
  if (binding.live_binding_action !== "unchanged" || binding.server_binding_authorized_by_this_packet !== false || binding.health_lease_extended !== false) {
    fail("observed-health acceptance must not alter live binding or extend health lease");
  }
  const gate = objectValue(document.gate, "observed-health gate boundary");
  if (
    gate.eligible_for_pages_gate_review !== true || gate.pages_reference_consumer_gate_accepted !== false ||
    gate.automatic_gate_acceptance !== false || gate.reference_gate_owner_signoff_satisfied !== false ||
    gate.reference_gate_rollback_decision_satisfied !== false
  ) fail("observed-health acceptance gate boundary drifted");
  for (const key of ["pages_reference_consumer_gate_accepted", "forum_wave_accepted", "ffa_promoted", "fba_promoted"]) {
    if (document[key] !== false) fail(`observed-health acceptance ${key} must remain false`);
  }
  return { snapshot: observed.snapshot, sloEvaluation: observed.slo_evaluation, deploymentId };
}
function outputPath(contract, requested) {
  const candidate = requested ?? contract.output.default_path;
  if (typeof candidate !== "string" || candidate.length === 0 || candidate.length > 16_384 || /[\u0000\r\n]/u.test(candidate)) fail("output path is invalid");
  const absolute = path.isAbsolute(candidate) ? path.resolve(candidate) : path.resolve(repoRoot, candidate);
  const targetRoot = path.resolve(repoRoot, "target");
  const relative = path.relative(targetRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) fail("gate acceptance output must remain inside repository target/");
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
  for (const required of ["candidate", "observedHealthAcceptance", "ownerId", "decision", "rollbackDecision"]) {
    if (!options[required]) fail(`missing required --${required.replace(/[A-Z]/gu, (letter) => `-${letter.toLowerCase()}`)}`);
  }
  if (![ACCEPT_DECISION, REJECT_DECISION].includes(options.decision)) fail(`--decision must be ${ACCEPT_DECISION} or ${REJECT_DECISION}`);
  if (![RETAIN_DECISION, ROLLBACK_DECISION].includes(options.rollbackDecision)) fail(`--rollback-decision must be ${RETAIN_DECISION} or ${ROLLBACK_DECISION}`);
  if (options.decision === ACCEPT_DECISION && options.rollbackDecision !== RETAIN_DECISION) fail("accepted Pages gate requires retain_reference_consumer_candidate rollback decision");
  if (options.decision === REJECT_DECISION && options.rollbackDecision !== ROLLBACK_DECISION) fail("rejected Pages gate requires rollback_reference_consumer_candidate rollback decision");
  const ownerId = requireOwnerId(options.ownerId);

  const contract = jsonSource(contractPath, "gate acceptance source contract");
  const candidateContract = jsonSource(candidateContractPath, "reference candidate execution contract");
  const observedSource = jsonSource(observedAcceptanceSourcePath, "observed-health acceptance source contract");
  const sourceGate = jsonSource(sourceGatePath, "Pages reference-consumer source gate");
  if (contract.status !== "source_ready_maintainer_execution_pending") fail("gate acceptance source contract must remain execution-pending before owner decision");
  if (candidateContract.status !== "source_ready_maintainer_execution_pending") fail("reference candidate execution contract drifted");
  if (observedSource.status !== "source_ready_maintainer_execution_pending") fail("observed-health acceptance source contract drifted");
  if (
    !contract.owner_decision?.decisions?.includes(ACCEPT_DECISION) ||
    !contract.owner_decision?.decisions?.includes(REJECT_DECISION) ||
    contract.owner_decision?.accepted_gate_requires_rollback_decision !== RETAIN_DECISION ||
    contract.owner_decision?.rejected_gate_requires_rollback_decision !== ROLLBACK_DECISION
  ) fail("gate owner-decision source policy drifted");
  validateSourceGate(sourceGate);

  const head = currentCommit();
  const candidateInput = jsonInput(options.candidate, "reference candidate evidence");
  const observedInput = jsonInput(options.observedHealthAcceptance, "observed-health owner acceptance evidence");
  const candidate = validateCandidate(candidateInput, contract, candidateContract, head);
  const observed = validateObservedHealthAcceptance(observedInput, contract, observedSource, head, candidate);
  const accepted = options.decision === ACCEPT_DECISION;
  const output = outputPath(contract, options.output);
  rmSync(output, { force: true });

  writeAtomic(output, {
    format: contract.output.format,
    status: accepted ? contract.output.accepted_status : contract.output.rejected_status,
    decided_at: new Date().toISOString(),
    source_commit: head,
    deployment: {
      deployment_id: observed.deploymentId,
      deployment_image_digest: candidate.deploymentDigest,
    },
    inputs: {
      reference_candidate: { bytes: candidateInput.size, sha256: candidateInput.sha256 },
      observed_health_acceptance: { bytes: observedInput.size, sha256: observedInput.sha256 },
      raw_input_paths_persisted: false,
    },
    decision: {
      value: options.decision,
      owner_id: ownerId,
      owner_identity_is_operator_assertion: true,
      cryptographic_signature_present: false,
      free_text_reason_retained: false,
    },
    rollback_decision: {
      value: options.rollbackDecision,
      rollback_action_performed: false,
    },
    evidence: {
      candidate_provider_health: "unobserved",
      observed_health_snapshot: observed.snapshot,
      observed_health_slo_evaluation: observed.sloEvaluation,
      observed_health_is_historical_evidence: true,
      current_provider_health_asserted: false,
      provider_health_lease_extended: false,
    },
    gate: {
      id: "pages_reference_consumer_gate",
      accepted,
      owner_signoff_satisfied: true,
      rollback_decision_satisfied: true,
      exact_source_commit_bound: true,
      exact_deployment_digest_bound: true,
      candidate_and_observed_health_chain_bound: true,
    },
    boundaries: {
      canonical_source_mutated: false,
      rollback_action_executed: false,
      forum_wave_accepted: false,
      ffa_promoted: false,
      fba_promoted: false,
      automatic_downstream_promotion: false,
    },
    source_files: sourceHashes(contract),
  });
}

try {
  main();
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
