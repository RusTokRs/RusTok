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
const contractPath = path.join(repoRoot, "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-observed-acceptance-source.json");
const runtimeContractPath = path.join(repoRoot, "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-runtime-execution-contract.json");
const identitySourcePath = path.join(repoRoot, "crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-identity-source.json");
const evaluatorSourcePath = path.join(repoRoot, "crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-evaluator-source.json");
const bindingSourcePath = path.join(repoRoot, "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-owner-acceptance-source.json");
const MAX_INPUT_BYTES = 8 * 1024 * 1024;
const MAX_SOURCE_BYTES = 8 * 1024 * 1024;
const CLOCK_SKEW_MS = 5_000;
const OWNER_ID_PATTERN = /^[A-Za-z0-9._-]{1,64}$/u;
const ACCEPT_DECISION = "accept_observed_runtime_evidence";
const REJECT_DECISION = "reject";
const BINDING_DECISION = "accept_for_pages_binding";
const BINDING_ROLLBACK = "restore_unobserved_provider_health";

function fail(message) {
  throw new Error(`Pages Page Builder observed-health owner acceptance failed: ${message}`);
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
function parseArguments(argv) {
  const options = {};
  const accepted = new Set(["--runtime-evidence", "--identity", "--evaluation", "--binding-acceptance", "--owner-id", "--decision", "--output"]);
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--help" || argument === "-h") {
      console.log(
        "usage: accept-pages-builder-provider-health-runtime.mjs " +
          "--runtime-evidence FILE --identity FILE --evaluation FILE --binding-acceptance FILE " +
          "--owner-id ID --decision accept_observed_runtime_evidence|reject [--output FILE]",
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
function currentCommit() {
  const result = spawnSync("git", ["rev-parse", "HEAD"], { cwd: repoRoot, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
  if (result.status !== 0) fail("git rev-parse HEAD failed");
  const commit = result.stdout.trim().toLowerCase();
  if (!/^[0-9a-f]{40}$/u.test(commit)) fail("checkout HEAD is not a canonical Git commit");
  return commit;
}
function resolveInput(candidate, label) {
  if (typeof candidate !== "string" || candidate.length === 0 || candidate.length > 16_384) fail(`${label} path is invalid`);
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
    return JSON.parse(record.bytes.toString("utf8"));
  } catch (error) {
    fail(`${label} is invalid JSON: ${error.message}`);
  }
}
function canonicalCommit(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{40}$/u.test(value)) fail(`${label} must be a lowercase 40-character Git SHA`);
  return value;
}
function canonicalRepoDigest(value, label) {
  if (typeof value !== "string" || value.length > 1024 || !/^[^\s@]+@sha256:[0-9a-f]{64}$/u.test(value)) fail(`${label} must be REPOSITORY@sha256:<64 lowercase hex>`);
  return value;
}
function canonicalIso(value, label) {
  if (typeof value !== "string" || value.length === 0 || value.length > 128) fail(`${label} is invalid`);
  const milliseconds = Date.parse(value);
  if (!Number.isFinite(milliseconds) || new Date(milliseconds).toISOString() !== value) fail(`${label} must be canonical ISO-8601 UTC`);
  return milliseconds;
}
function requireSha256(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value)) fail(`${label} must be a lowercase SHA-256`);
  return value;
}
function requirePositiveInteger(value, label) {
  if (!Number.isSafeInteger(value) || value <= 0) fail(`${label} must be a positive safe integer`);
  return value;
}
function boundedOwnerId(value) {
  if (typeof value !== "string" || !OWNER_ID_PATTERN.test(value)) fail("--owner-id must match ^[A-Za-z0-9._-]{1,64}$");
  return value;
}
function regularSourceHash(relativePath) {
  if (typeof relativePath !== "string" || relativePath.length === 0 || relativePath.length > 4096) fail("source path is invalid");
  const absolute = path.resolve(repoRoot, relativePath);
  const relative = path.relative(repoRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) fail(`source file ${relativePath} escapes repository root`);
  return sha256(regularFile(absolute, `source file ${relativePath}`, MAX_SOURCE_BYTES).bytes);
}
function expectedSourceFiles(sourceContract, label) {
  const expected = sourceContract.required_source_files;
  if (!Array.isArray(expected) || expected.length === 0 || expected.length > 64) fail(`${label} required_source_files is invalid`);
  return [...expected].sort();
}
function verifyRetainedSourceHashes(document, sourceContract, field, label) {
  const retained = objectValue(document[field], `${label}.${field}`);
  const expectedNames = expectedSourceFiles(sourceContract, label);
  const actualNames = Object.keys(retained).sort();
  if (canonicalJson(actualNames) !== canonicalJson(expectedNames)) fail(`${label} source SHA set differs from source contract`);
  for (const relativePath of expectedNames) {
    if (requireSha256(retained[relativePath], `${label} source SHA ${relativePath}`) !== regularSourceHash(relativePath)) {
      fail(`${label} source SHA for ${relativePath} does not match checkout`);
    }
  }
}
function sourceHashes(contract) {
  return Object.fromEntries(expectedSourceFiles(contract, "observed acceptance source contract").map((relativePath) => [relativePath, regularSourceHash(relativePath)]));
}
function requirePacketHash(record, retained, label) {
  const node = objectValue(retained, label);
  if (requirePositiveInteger(node.bytes, `${label}.bytes`) !== record.size) fail(`${label} byte length does not match supplied packet`);
  if (requireSha256(node.sha256, `${label}.sha256`) !== record.sha256) fail(`${label} SHA-256 does not match supplied packet`);
}
function requireHttpRecord(value, label, exactStatus = null) {
  const record = objectValue(value, label);
  if (exactStatus === null) {
    if (!Number.isSafeInteger(record.status) || record.status < 200 || record.status >= 400) fail(`${label} status must be successful`);
  } else if (record.status !== exactStatus) {
    fail(`${label} status must equal ${exactStatus}`);
  }
  requirePositiveInteger(record.response_body_bytes, `${label} response bytes`);
  requireSha256(record.response_body_sha256, `${label} response SHA-256`);
  return record;
}
function requireCapability(value, capability, expected) {
  const result = objectValue(value, `${capability} preflight observation`);
  if (result.capability !== capability) fail(`${capability} preflight capability drifted`);
  if (expected === "allowed") {
    if (result.allowed !== true || result.error_kind !== null || result.error_code !== null) fail(`${capability} preflight must be allowed`);
  } else if (result.allowed !== false || result.error_kind !== "feature-disabled" || result.error_code !== "FEATURE_DISABLED") {
    fail(`${capability} preflight must be feature-disabled / FEATURE_DISABLED`);
  }
}
function expectedForHealth(state) {
  if (state === "ready") return { preview: "allowed", properties: "allowed", publish: "allowed" };
  if (state === "degraded") return { preview: "allowed", properties: "allowed", publish: "feature_disabled" };
  if (state === "unavailable") return { preview: "feature_disabled", properties: "feature_disabled", publish: "feature_disabled" };
  fail(`unsupported accepted provider health state ${state}`);
}
function requireWorkspaceState(value, expected, state) {
  const workspace = objectValue(value, "workspace observation");
  if (workspace.provider_control_state !== state || workspace.provider_health !== state) fail("workspace provider state does not match accepted health");
  if (workspace.preview_enabled !== (expected.preview === "allowed")) fail("workspace preview state does not match accepted health");
  const requireFieldset = (actual, allowed, label) => {
    if (allowed && actual !== "enabled") fail(`${label} must be enabled`);
    if (!allowed && !["disabled", "hidden"].includes(actual)) fail(`${label} must be disabled or hidden`);
  };
  requireFieldset(workspace.properties, expected.properties === "allowed", "workspace properties");
  requireFieldset(workspace.publish, expected.publish === "allowed", "workspace publish");
}
function requireSsrState(value, state) {
  const ssr = objectValue(value, "authoritative SSR preview observation");
  if (state === "unavailable") {
    if (ssr.request_attempted !== false || ssr.ui_blocked !== true || ssr.mutation_possible !== false) fail("unavailable health must block SSR Preview before request dispatch");
    return;
  }
  if (ssr.request_attempted !== true || ssr.capability_disabled !== false || ssr.mutation_possible !== false || ssr.raw_request_or_response_persisted !== false) {
    fail("ready/degraded health SSR Preview observation drifted");
  }
  requireHttpRecord(ssr, "SSR Preview observation");
}
function requireBrowserDenial(value, intent, capability) {
  const denial = requireHttpRecord(value, `${intent} browser-intent denial`, 403);
  if (
    denial.code !== "FLY_CAPABILITY_DENIED" ||
    denial.intent !== intent ||
    denial.capability !== capability ||
    denial.mismatch_page_id_used_as_non_mutating_fallback !== true ||
    denial.raw_request_or_response_persisted !== false
  ) fail(`${intent} browser-intent denial drifted`);
}
function requireBrowserState(value, state) {
  if (!Array.isArray(value)) fail("standalone browser-intent observation must be an array");
  if (state === "ready") {
    if (value.length !== 0) fail("ready health must not require a browser-intent denial probe");
    return;
  }
  if (state === "degraded") {
    if (value.length !== 1) fail("degraded health must retain exactly one browser-intent denial");
    requireBrowserDenial(value[0], "save", "publish");
    return;
  }
  if (value.length !== 2) fail("unavailable health must retain two browser-intent denials");
  requireBrowserDenial(value[0], "save", "publish");
  requireBrowserDenial(value[1], "rename_page", "properties");
}
function requireRuntimeEvidence(records, contracts, contract, head) {
  const { runtimeRecord, identityRecord, evaluationRecord, bindingRecord } = records;
  const { runtimeContract, identitySource, evaluatorSource, bindingSource } = contracts;
  const runtime = runtimeRecord.document;
  if (runtime.format !== contract.runtime_evidence_input.format || runtime.status !== contract.runtime_evidence_input.required_status) fail("runtime evidence format/status drifted");
  const sourceCommit = canonicalCommit(runtime.source_commit, "runtime source_commit");
  if (sourceCommit !== head) fail("runtime source commit does not equal checkout HEAD");
  verifyRetainedSourceHashes(runtime, runtimeContract, "source_sha256", "runtime evidence");

  const runtimeDeployment = objectValue(runtime.deployment, "runtime deployment");
  const deploymentId = runtimeDeployment.deployment_id;
  if (typeof deploymentId !== "string" || deploymentId.length === 0 || deploymentId.length > 256) fail("runtime deployment id is invalid");
  const deploymentDigest = canonicalRepoDigest(runtimeDeployment.deployment_image_digest, "runtime deployment image digest");

  const identity = identityRecord.document;
  const evaluation = evaluationRecord.document;
  const binding = bindingRecord.document;
  const identitySpec = contract.supplied_predecessor_packets.deployment_identity;
  const evaluationSpec = contract.supplied_predecessor_packets.deployment_evaluation;
  const bindingSpec = contract.supplied_predecessor_packets.binding_owner_acceptance;
  if (identity.format !== identitySpec.format || identity.status !== identitySpec.status) fail("identity packet format/status drifted");
  if (evaluation.format !== evaluationSpec.format || evaluation.status !== evaluationSpec.status) fail("evaluation packet format/status drifted");
  if (binding.format !== bindingSpec.format || binding.status !== bindingSpec.status) fail("binding owner-acceptance packet format/status drifted");
  verifyRetainedSourceHashes(identity, identitySource, "source_files", "identity packet");
  verifyRetainedSourceHashes(evaluation, evaluatorSource, "source_files", "evaluation packet");
  verifyRetainedSourceHashes(binding, bindingSource, "source_files", "binding owner-acceptance packet");

  for (const [document, label] of [[identity, "identity"], [evaluation, "evaluation"], [binding, "binding acceptance"]]) {
    const deployment = objectValue(document.deployment, `${label} deployment`);
    if (deployment.source_commit !== sourceCommit || deployment.deployment_id !== deploymentId || deployment.deployment_image_digest !== deploymentDigest) fail(`${label} deployment identity differs from runtime evidence`);
  }

  const inputPackets = objectValue(runtime.input_packets, "runtime input_packets");
  if (inputPackets.raw_paths_persisted !== false) fail("runtime evidence retained raw predecessor paths");
  requirePacketHash(identityRecord, inputPackets.deployment_identity, "runtime identity packet");
  requirePacketHash(evaluationRecord, inputPackets.deployment_evaluation, "runtime evaluation packet");
  requirePacketHash(bindingRecord, inputPackets.owner_acceptance, "runtime binding acceptance packet");

  const bindingDecision = objectValue(binding.decision, "binding owner decision");
  if (
    bindingDecision.value !== bindingSpec.decision ||
    bindingDecision.rollback_action !== bindingSpec.rollback_action ||
    bindingDecision.owner_identity_is_operator_assertion !== true ||
    bindingDecision.cryptographic_signature_present !== false ||
    bindingDecision.free_text_reason_retained !== false
  ) fail("binding owner decision contract drifted");
  const bindingEvaluation = objectValue(binding.evaluation, "binding acceptance evaluation");
  if (bindingEvaluation.evaluation_sha256 !== evaluationRecord.sha256) fail("binding owner acceptance is not bound to supplied evaluation packet");
  const bindingBoundary = objectValue(binding.binding, "binding acceptance boundary");
  if (
    bindingBoundary.server_binding_authorized !== true ||
    bindingBoundary.server_binding_performed !== false ||
    bindingBoundary.required_live_source_commit !== sourceCommit ||
    bindingBoundary.required_deployment_image_digest !== deploymentDigest ||
    bindingBoundary.failure_action !== BINDING_ROLLBACK
  ) fail("binding owner acceptance boundary drifted");

  const acceptedHealth = objectValue(runtime.accepted_health, "runtime accepted_health");
  const healthValidUntil = acceptedHealth.health_valid_until;
  const healthValidUntilMs = canonicalIso(healthValidUntil, "runtime health_valid_until");
  if (bindingEvaluation.health_valid_until !== healthValidUntil) fail("runtime health_valid_until differs from binding acceptance");
  if (canonicalJson(acceptedHealth.snapshot) !== canonicalJson(bindingEvaluation.snapshot)) fail("runtime accepted health snapshot differs from binding acceptance");
  if (canonicalJson(acceptedHealth.slo_evaluation) !== canonicalJson(bindingEvaluation.slo_evaluation)) fail("runtime accepted SLO evaluation differs from binding acceptance");
  const generatedAtMs = canonicalIso(runtime.generated_at, "runtime generated_at");
  if (generatedAtMs > healthValidUntilMs + CLOCK_SKEW_MS) fail("runtime evidence was generated after its admitted health lease deadline");

  const state = objectValue(acceptedHealth.snapshot, "accepted health snapshot").state;
  const expected = expectedForHealth(state);
  const observations = objectValue(runtime.observations, "runtime observations");
  const graphql = requireHttpRecord(observations.graphql, "runtime GraphQL observation", 200);
  if (graphql.configured_rollout_all_on !== true || graphql.provider_health_observed !== true || graphql.provider_state !== state || graphql.raw_request_or_response_persisted !== false) {
    fail("runtime GraphQL provider-health observation drifted");
  }
  requireCapability(graphql.preview, "preview", expected.preview);
  requireCapability(graphql.properties, "properties", expected.properties);
  requireCapability(graphql.publish, "publish", expected.publish);
  requireWorkspaceState(observations.workspace, expected, state);
  requireSsrState(observations.authoritative_ssr_preview, state);
  requireBrowserState(observations.standalone_browser_intent, state);

  const graphqlAfter = requireHttpRecord(observations.graphql_after_consumers, "post-consumer GraphQL observation", 200);
  if (graphqlAfter.provider_health_still_observed !== true) fail("provider health was not observed after consumer probes");

  const boundaries = objectValue(runtime.boundaries, "runtime boundaries");
  for (const key of ["exact_identity_evaluator_acceptance_chain_verified", "accepted_packet_runtime_observed", "configured_rollout_all_on", "mismatched_page_id_protects_browser_intent_probe_if_health_revoked"]) {
    if (boundaries[key] !== true) fail(`runtime boundary ${key} must be true`);
  }
  for (const key of ["rollout_settings_mutated", "publish_mutation_executed", "owner_observed_health_acceptance", "pages_reference_consumer_gate_accepted", "forum_wave_accepted", "ffa_promoted", "fba_promoted", "canonical_source_mutated"]) {
    if (boundaries[key] !== false) fail(`runtime boundary ${key} must be false`);
  }
  const privacy = objectValue(runtime.privacy, "runtime privacy");
  for (const [key, value] of Object.entries(privacy)) if (value !== false) fail(`runtime privacy flag ${key} must be false`);

  return { sourceCommit, deploymentId, deploymentDigest, generatedAt: runtime.generated_at, healthValidUntil, snapshot: acceptedHealth.snapshot, sloEvaluation: acceptedHealth.slo_evaluation };
}
function outputPath(contract, requested) {
  const candidate = requested ?? contract.output.default_path;
  if (typeof candidate !== "string" || candidate.length === 0 || candidate.length > 16_384) fail("output path is invalid");
  const absolute = path.isAbsolute(candidate) ? path.resolve(candidate) : path.resolve(repoRoot, candidate);
  const targetRoot = path.resolve(repoRoot, "target");
  const relative = path.relative(targetRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) fail("observed-health owner acceptance output must remain under repository target/");
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
  for (const required of ["runtimeEvidence", "identity", "evaluation", "bindingAcceptance", "ownerId", "decision"]) {
    if (!options[required]) fail(`--${required.replace(/[A-Z]/gu, (letter) => `-${letter.toLowerCase()}`)} is required`);
  }
  if (![ACCEPT_DECISION, REJECT_DECISION].includes(options.decision)) fail(`--decision must be ${ACCEPT_DECISION} or ${REJECT_DECISION}`);
  const ownerId = boundedOwnerId(options.ownerId);
  const contract = jsonSource(contractPath, "observed-health acceptance source contract");
  const runtimeContract = jsonSource(runtimeContractPath, "runtime execution contract");
  const identitySource = jsonSource(identitySourcePath, "identity source contract");
  const evaluatorSource = jsonSource(evaluatorSourcePath, "evaluator source contract");
  const bindingSource = jsonSource(bindingSourcePath, "binding owner-acceptance source contract");
  if (contract.status !== "source_ready_maintainer_execution_pending") fail("observed-health acceptance source contract must remain execution-pending before owner decision");
  if (runtimeContract.status !== "source_ready_maintainer_execution_pending") fail("runtime evidence execution contract drifted");
  if (
    !contract.owner_decision?.decisions?.includes(ACCEPT_DECISION) ||
    !contract.owner_decision?.decisions?.includes(REJECT_DECISION) ||
    contract.supplied_predecessor_packets?.binding_owner_acceptance?.decision !== BINDING_DECISION ||
    contract.supplied_predecessor_packets?.binding_owner_acceptance?.rollback_action !== BINDING_ROLLBACK
  ) fail("observed-health owner-decision policy drifted");

  const records = {
    runtimeRecord: jsonInput(options.runtimeEvidence, "runtime evidence"),
    identityRecord: jsonInput(options.identity, "deployment identity evidence"),
    evaluationRecord: jsonInput(options.evaluation, "deployment evaluation evidence"),
    bindingRecord: jsonInput(options.bindingAcceptance, "binding owner acceptance evidence"),
  };
  const admitted = requireRuntimeEvidence(records, { runtimeContract, identitySource, evaluatorSource, bindingSource }, contract, currentCommit());
  const decidedAt = new Date();
  if (decidedAt.getTime() < canonicalIso(admitted.generatedAt, "runtime generated_at")) fail("owner decision predates runtime evidence generation");

  const accepted = options.decision === ACCEPT_DECISION;
  const output = outputPath(contract, options.output);
  rmSync(output, { force: true });
  writeAtomic(output, {
    format: contract.output.format,
    status: accepted ? contract.output.accepted_status : contract.output.rejected_status,
    decided_at: decidedAt.toISOString(),
    decision: {
      value: options.decision,
      owner_id: ownerId,
      owner_identity_is_operator_assertion: true,
      cryptographic_signature_present: false,
      free_text_reason_retained: false,
      acceptance_meaning: contract.owner_decision.acceptance_meaning,
    },
    source_commit: admitted.sourceCommit,
    deployment: { deployment_id: admitted.deploymentId, deployment_image_digest: admitted.deploymentDigest },
    runtime_evidence: {
      format: records.runtimeRecord.document.format,
      status: records.runtimeRecord.document.status,
      generated_at: admitted.generatedAt,
      runtime_evidence_sha256: records.runtimeRecord.sha256,
      deployment_identity_sha256: records.identityRecord.sha256,
      deployment_evaluation_sha256: records.evaluationRecord.sha256,
      binding_owner_acceptance_sha256: records.bindingRecord.sha256,
      health_valid_until: admitted.healthValidUntil,
      source_hashes_verified_against_checkout: true,
      predecessor_source_hashes_verified_against_checkout: true,
      raw_input_paths_persisted: false,
    },
    observed_health: {
      snapshot: admitted.snapshot,
      slo_evaluation: admitted.sloEvaluation,
      historical_lease_deadline_only: true,
      current_provider_health_asserted: false,
    },
    binding_lineage: {
      predecessor_decision: BINDING_DECISION,
      predecessor_rollback_action: BINDING_ROLLBACK,
      live_binding_action: "unchanged",
      server_binding_authorized_by_this_packet: false,
      health_lease_extended: false,
    },
    gate: {
      eligible_for_pages_gate_review: accepted,
      pages_reference_consumer_gate_accepted: false,
      automatic_gate_acceptance: false,
      reference_gate_owner_signoff_satisfied: false,
      reference_gate_rollback_decision_satisfied: false,
    },
    source_files: sourceHashes(contract),
    raw_input_paths_persisted: false,
    pages_reference_consumer_gate_accepted: false,
    forum_wave_accepted: false,
    ffa_promoted: false,
    fba_promoted: false,
  });
}

try {
  main();
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
