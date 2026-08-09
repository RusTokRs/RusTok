#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath = path.join(
  repoRoot,
  "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-owner-acceptance-source.json",
);
const evaluatorContractPath = path.join(
  repoRoot,
  "crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-evaluator-source.json",
);
const MAX_INPUT_BYTES = 8 * 1024 * 1024;
const MAX_SOURCE_BYTES = 8 * 1024 * 1024;
const MINIMUM_SAMPLES_PER_OPERATION = 20;
const COUNT_EPSILON = 1e-6;
const THRESHOLDS = {
  preview_p95_ms: 1500,
  publish_p95_ms: 3000,
  sanitize_failure_rate_max: 0.01,
  runtime_error_rate_max: 0.01,
};
const ACCEPT_DECISION = "accept_for_pages_binding";
const REJECT_DECISION = "reject";
const ROLLBACK_ACTION = "restore_unobserved_provider_health";
const OWNER_ID_PATTERN_SOURCE = "^[A-Za-z0-9._-]{1,64}$";
const OWNER_ID_PATTERN = /^[A-Za-z0-9._-]{1,64}$/;

function fail(message) {
  throw new Error(`Pages Page Builder provider-health owner acceptance failed: ${message}`);
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function parseArguments(argv) {
  const options = {};
  const accepted = new Set([
    "--evaluation",
    "--owner-id",
    "--decision",
    "--rollback-action",
    "--output",
  ]);
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--help" || argument === "-h") {
      console.log(
        "usage: accept-pages-builder-provider-health-deployment.mjs " +
          "--evaluation FILE --owner-id ID --decision accept_for_pages_binding|reject " +
          "[--rollback-action restore_unobserved_provider_health] [--output FILE]",
      );
      process.exit(0);
    }
    if (!accepted.has(argument)) fail(`unknown argument ${argument}`);
    const value = argv[index + 1];
    if (!value) fail(`${argument} requires a value`);
    options[
      argument
        .slice(2)
        .replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())
    ] = value;
    index += 1;
  }
  return options;
}

function currentCommit() {
  const result = spawnSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.status !== 0) fail("git rev-parse HEAD failed");
  const commit = result.stdout.trim().toLowerCase();
  if (!/^[0-9a-f]{40}$/.test(commit)) fail("checkout HEAD is not a canonical Git commit");
  return commit;
}

function regularFile(location, label, maximumBytes = MAX_INPUT_BYTES) {
  if (!existsSync(location)) fail(`${label} is missing`);
  const metadata = lstatSync(location);
  if (!metadata.isFile() || metadata.isSymbolicLink()) fail(`${label} must be a regular non-symlink file`);
  const size = statSync(location).size;
  if (size <= 0 || size > maximumBytes) fail(`${label} is outside the bounded size`);
  return readFileSync(location);
}

function repositoryTargetRoot() {
  const targetRoot = path.resolve(repoRoot, "target");
  if (!existsSync(targetRoot)) fail("repository target/ is missing");
  const metadata = lstatSync(targetRoot);
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) fail("repository target/ must be a real directory");
  return realpathSync(targetRoot);
}

function resolveTargetInput(candidate, label) {
  if (typeof candidate !== "string" || candidate.length === 0 || candidate.length > 16_384) {
    fail(`${label} path is invalid`);
  }
  const absolute = path.isAbsolute(candidate) ? path.resolve(candidate) : path.resolve(repoRoot, candidate);
  if (!existsSync(absolute)) fail(`${label} is missing`);
  const real = realpathSync(absolute);
  const targetRoot = repositoryTargetRoot();
  const relative = path.relative(targetRoot, real);
  if (relative.startsWith("..") || path.isAbsolute(relative) || relative.length === 0) fail(`${label} must reside under repository target/`);
  return real;
}

function jsonDocument(location, label) {
  const bytes = regularFile(location, label);
  try {
    return { document: JSON.parse(bytes.toString("utf8")), bytes };
  } catch (error) {
    fail(`${label} is invalid JSON: ${error.message}`);
  }
}

function canonicalCommit(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{40}$/.test(value)) fail(`${label} must be a lowercase 40-character Git SHA`);
  return value;
}

function canonicalRepoDigest(value) {
  if (
    typeof value !== "string" ||
    value.length > 1024 ||
    !/^[^\s@]+@sha256:[0-9a-f]{64}$/.test(value)
  ) {
    fail("deployment image digest must be REPOSITORY@sha256:<64 lowercase hex>");
  }
  return value;
}

function boundedOwnerId(value) {
  if (typeof value !== "string" || !OWNER_ID_PATTERN.test(value)) {
    fail("--owner-id must be a bounded operator identifier using A-Z a-z 0-9 . _ -");
  }
  return value;
}

function finiteNumber(value, label, minimum = 0, maximum = Number.POSITIVE_INFINITY) {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) {
    fail(`${label} is outside the admitted numeric range`);
  }
  return value;
}

function nonNegativeInteger(value, label) {
  if (!Number.isSafeInteger(value) || value < 0) fail(`${label} must be a non-negative safe integer`);
  return value;
}

function canonicalIsoTimestamp(value, label) {
  if (typeof value !== "string" || value.length === 0 || value.length > 128) fail(`${label} is invalid`);
  const milliseconds = Date.parse(value);
  if (!Number.isFinite(milliseconds) || new Date(milliseconds).toISOString() !== value) fail(`${label} must be canonical ISO-8601 UTC`);
  return milliseconds;
}

function status(pass) {
  return pass ? "pass" : "fail";
}

function evaluateHealth(observed) {
  const degradationReasons = [];
  if (
    observed.preview_p95_ms > THRESHOLDS.preview_p95_ms ||
    observed.runtime_error_rate > THRESHOLDS.runtime_error_rate_max
  ) degradationReasons.push("provider_unhealthy");
  if (observed.sanitize_failure_rate > THRESHOLDS.sanitize_failure_rate_max) {
    degradationReasons.push("sanitize_backpressure");
  }
  if (observed.publish_p95_ms > THRESHOLDS.publish_p95_ms) {
    degradationReasons.push("publish_backlog");
  }
  const state = degradationReasons.length === 0
    ? "ready"
    : observed.runtime_error_rate > THRESHOLDS.runtime_error_rate_max * 2.0
      ? "unavailable"
      : "degraded";
  const sloEvaluation = {
    preview_p95_ms: status(observed.preview_p95_ms <= THRESHOLDS.preview_p95_ms),
    publish_p95_ms: status(observed.publish_p95_ms <= THRESHOLDS.publish_p95_ms),
    sanitize_failure_rate: status(observed.sanitize_failure_rate <= THRESHOLDS.sanitize_failure_rate_max),
    runtime_error_rate: status(observed.runtime_error_rate <= THRESHOLDS.runtime_error_rate_max),
  };
  sloEvaluation.overall = status(Object.values(sloEvaluation).every((value) => value === "pass"));
  return { state, degradationReasons, sloEvaluation };
}

function sameArray(left, right) {
  return Array.isArray(left) && Array.isArray(right) && left.length === right.length && left.every((value, index) => value === right[index]);
}

function verifySourceHashes(evaluation, evaluatorContract) {
  if (!evaluation.source_files || typeof evaluation.source_files !== "object" || Array.isArray(evaluation.source_files)) {
    fail("evaluation source_files must be an object");
  }
  const expected = evaluatorContract.required_source_files;
  if (!Array.isArray(expected) || expected.length === 0) fail("evaluator source contract has no required source files");
  const actualNames = Object.keys(evaluation.source_files).sort();
  const expectedNames = [...expected].sort();
  if (!sameArray(actualNames, expectedNames)) fail("evaluation source file set does not match evaluator source contract");
  for (const relativePath of expectedNames) {
    if (typeof relativePath !== "string" || relativePath.length === 0 || relativePath.length > 4096) fail("evaluator source path is invalid");
    const absolute = path.resolve(repoRoot, relativePath);
    const relative = path.relative(repoRoot, absolute);
    if (relative.startsWith("..") || path.isAbsolute(relative)) fail(`source file ${relativePath} escapes repository root`);
    const bytes = regularFile(absolute, `source file ${relativePath}`, MAX_SOURCE_BYTES);
    const retained = evaluation.source_files[relativePath];
    if (typeof retained !== "string" || !/^[0-9a-f]{64}$/.test(retained)) fail(`evaluation source hash for ${relativePath} is invalid`);
    if (sha256(bytes) !== retained) fail(`evaluation source hash for ${relativePath} does not match checkout`);
  }
}

function withinPopulationTolerance(left, right) {
  const tolerance = COUNT_EPSILON * Math.max(1, Math.abs(left), Math.abs(right));
  return Math.abs(left - right) <= tolerance;
}

function requireEvaluation(evaluation, contract, evaluatorContract, head) {
  if (evaluation.format !== contract.evaluation_input.format) fail("evaluation format drifted");
  if (evaluation.status !== contract.evaluation_input.required_status) fail("evaluation status is not binding-pending");
  if (!evaluation.deployment || typeof evaluation.deployment !== "object") fail("evaluation deployment is missing");
  const sourceCommit = canonicalCommit(evaluation.deployment.source_commit, "evaluation deployment source_commit");
  if (sourceCommit !== head) fail("evaluation source commit does not equal checkout HEAD");
  const deploymentImageDigest = canonicalRepoDigest(evaluation.deployment.deployment_image_digest);
  const deploymentId = evaluation.deployment.deployment_id;
  if (typeof deploymentId !== "string" || deploymentId.length === 0 || deploymentId.length > 256) fail("evaluation deployment id is invalid");
  const expectedTargets = nonNegativeInteger(evaluation.deployment.expected_target_count, "expected target count");
  const verifiedTargets = nonNegativeInteger(evaluation.deployment.verified_backend_target_count, "verified target count");
  if (expectedTargets < 1 || expectedTargets > 64 || expectedTargets !== verifiedTargets) fail("evaluation target counts are incomplete");

  const minimumWindow = nonNegativeInteger(evaluatorContract.backend_query?.query_window_seconds_minimum, "evaluator minimum query window");
  const maximumWindow = nonNegativeInteger(evaluatorContract.backend_query?.query_window_seconds_maximum, "evaluator maximum query window");
  const minimumFreshness = nonNegativeInteger(evaluatorContract.backend_query?.freshness_seconds_minimum, "evaluator minimum freshness window");
  const maximumIdentityAge = nonNegativeInteger(evaluatorContract.backend_query?.identity_capture_maximum_age_seconds, "evaluator maximum identity age");
  if (minimumWindow === 0 || maximumWindow < minimumWindow || minimumFreshness === 0 || maximumIdentityAge < minimumWindow) {
    fail("deployment evaluator bounds are invalid");
  }
  const queryWindow = nonNegativeInteger(evaluation.deployment.query_window_seconds, "query window");
  const freshnessWindow = nonNegativeInteger(evaluation.deployment.freshness_seconds, "freshness window");
  if (queryWindow < minimumWindow || queryWindow > maximumWindow) fail("evaluation query window is outside evaluator contract bounds");
  if (freshnessWindow < minimumFreshness || freshnessWindow > queryWindow) fail("evaluation freshness window is outside evaluator contract bounds");
  const identityAge = finiteNumber(evaluation.deployment.identity_age_seconds, "identity age");
  if (identityAge < queryWindow || identityAge > maximumIdentityAge) fail("evaluation identity age is outside admitted bounds");
  const evaluatedAtMs = canonicalIsoTimestamp(evaluation.evaluated_at, "evaluation evaluated_at");
  const identityCapturedAtMs = canonicalIsoTimestamp(evaluation.deployment.identity_captured_at, "evaluation identity_captured_at");
  const timestampAge = (evaluatedAtMs - identityCapturedAtMs) / 1000;
  if (timestampAge < 0 || Math.abs(timestampAge - identityAge) > 1) fail("evaluation identity age does not match retained timestamps");

  if (evaluation.backend?.target_mapping_complete !== true) fail("evaluation target mapping is not complete");
  if (evaluation.backend?.raw_prometheus_url_persisted !== false || evaluation.backend?.raw_promql_persisted !== false || evaluation.backend?.raw_backend_responses_persisted !== false || evaluation.backend?.raw_matcher_values_persisted !== false) {
    fail("evaluation retained raw backend material");
  }
  if (!Array.isArray(evaluation.targets) || evaluation.targets.length !== expectedTargets) fail("evaluation retained target set is incomplete");
  const targetIds = new Set();
  for (const target of evaluation.targets) {
    if (!target || typeof target !== "object") fail("evaluation target entry is invalid");
    if (typeof target.target_id !== "string" || target.target_id.length === 0 || target.target_id.length > 256) fail("evaluation target id is invalid");
    if (targetIds.has(target.target_id)) fail("evaluation target ids are duplicated");
    targetIds.add(target.target_id);
    if (target.current_source_commit_verified !== true || target.unexpected_source_in_window !== false) fail(`target ${target.target_id} source admission is incomplete`);
    finiteNumber(target.preview_freshness_age_seconds, `${target.target_id} preview freshness age`, 0, freshnessWindow);
    finiteNumber(target.publish_freshness_age_seconds, `${target.target_id} publish freshness age`, 0, freshnessWindow);
  }

  const previewSamples = finiteNumber(evaluation.samples?.preview, "preview samples", MINIMUM_SAMPLES_PER_OPERATION);
  const publishSamples = finiteNumber(evaluation.samples?.publish, "publish samples", MINIMUM_SAMPLES_PER_OPERATION);
  const previewHistogram = finiteNumber(evaluation.samples?.preview_histogram, "preview histogram population", MINIMUM_SAMPLES_PER_OPERATION);
  const publishHistogram = finiteNumber(evaluation.samples?.publish_histogram, "publish histogram population", MINIMUM_SAMPLES_PER_OPERATION);
  if (!withinPopulationTolerance(previewSamples, previewHistogram) || !withinPopulationTolerance(publishSamples, publishHistogram)) {
    fail("evaluation histogram populations do not match terminal completion populations");
  }
  if (evaluation.samples?.minimum_per_operation !== MINIMUM_SAMPLES_PER_OPERATION) fail("evaluation minimum sample floor drifted");

  if (!evaluation.snapshot || typeof evaluation.snapshot !== "object") fail("evaluation provider-health snapshot is missing");
  const thresholds = evaluation.snapshot.thresholds;
  for (const [key, expected] of Object.entries(THRESHOLDS)) {
    if (thresholds?.[key] !== expected) fail(`evaluation threshold ${key} drifted`);
  }
  const observed = {
    preview_p95_ms: nonNegativeInteger(evaluation.snapshot.observed?.preview_p95_ms, "preview p95"),
    publish_p95_ms: nonNegativeInteger(evaluation.snapshot.observed?.publish_p95_ms, "publish p95"),
    sanitize_failure_rate: finiteNumber(evaluation.snapshot.observed?.sanitize_failure_rate, "sanitize failure rate", 0, 1),
    runtime_error_rate: finiteNumber(evaluation.snapshot.observed?.runtime_error_rate, "runtime error rate", 0, 1),
  };
  const canonical = evaluateHealth(observed);
  if (evaluation.snapshot.state !== canonical.state) fail("evaluation provider-health state does not match canonical policy");
  if (!sameArray(evaluation.snapshot.degradation_reasons, canonical.degradationReasons)) fail("evaluation degradation reasons do not match canonical policy");
  const retainedSlo = evaluation.slo_evaluation;
  for (const [key, expected] of Object.entries(canonical.sloEvaluation)) {
    if (retainedSlo?.[key] !== expected) fail(`evaluation SLO result ${key} does not match canonical policy`);
  }

  if (
    evaluation.pages_provider_health_observed !== false ||
    evaluation.pages_reference_consumer_gate_accepted !== false ||
    evaluation.forum_wave_accepted !== false ||
    evaluation.ffa_promoted !== false ||
    evaluation.fba_promoted !== false
  ) {
    fail("evaluation contains a forbidden promotion claim");
  }

  verifySourceHashes(evaluation, evaluatorContract);
  return {
    sourceCommit,
    deploymentImageDigest,
    deploymentId,
    expectedTargets,
    queryWindow,
    freshnessWindow,
    identityAge,
    previewSamples,
    publishSamples,
    observed,
    state: canonical.state,
    degradationReasons: canonical.degradationReasons,
    sloEvaluation: canonical.sloEvaluation,
  };
}

function regularSourceHash(relativePath) {
  const absolute = path.resolve(repoRoot, relativePath);
  const relative = path.relative(repoRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) fail(`source file ${relativePath} escapes repository root`);
  return sha256(regularFile(absolute, `source file ${relativePath}`, MAX_SOURCE_BYTES));
}

function sourceHashes(contract) {
  return Object.fromEntries(contract.required_source_files.map((relativePath) => [relativePath, regularSourceHash(relativePath)]));
}

function outputPath(contract, requested) {
  const candidate = requested ?? contract.output.default_path;
  if (typeof candidate !== "string" || candidate.length === 0 || candidate.length > 16_384) fail("acceptance output path is invalid");
  const absolute = path.isAbsolute(candidate) ? path.resolve(candidate) : path.resolve(repoRoot, candidate);
  const targetRoot = repositoryTargetRoot();
  if (path.dirname(absolute) !== targetRoot || path.basename(absolute).length === 0) fail("acceptance output must be a direct file inside repository target/");
  if (existsSync(absolute) && lstatSync(absolute).isSymbolicLink()) fail("acceptance output must not be a symlink");
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
  for (const required of ["evaluation", "ownerId", "decision"]) {
    if (!options[required]) fail(`--${required.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)} is required`);
  }
  const contract = JSON.parse(regularFile(contractPath, "owner acceptance source contract").toString("utf8"));
  const evaluatorContract = JSON.parse(regularFile(evaluatorContractPath, "deployment evaluator source contract").toString("utf8"));
  if (contract.format !== "pages_builder_provider_health_owner_acceptance_source_v1" || contract.status !== "source_ready_maintainer_execution_pending") {
    fail("owner acceptance source contract identity drifted");
  }
  if (
    contract.owner_decision?.owner_id_pattern !== OWNER_ID_PATTERN_SOURCE ||
    contract.owner_decision?.accepted_rollback_action !== ROLLBACK_ACTION ||
    !contract.owner_decision?.decisions?.includes(ACCEPT_DECISION) ||
    !contract.owner_decision?.decisions?.includes(REJECT_DECISION) ||
    contract.output?.format !== "pages_builder_provider_health_owner_acceptance_v1" ||
    contract.output?.accepted_status !== "owner_accepted_server_binding_pending" ||
    contract.output?.rejected_status !== "owner_rejected_observed_health_binding"
  ) {
    fail("owner acceptance source contract policy drifted");
  }
  if (evaluatorContract.status !== "source_ready_execution_pending") fail("deployment evaluator source contract drifted");

  const decision = options.decision;
  if (decision !== ACCEPT_DECISION && decision !== REJECT_DECISION) fail("--decision must be accept_for_pages_binding or reject");
  if (decision === ACCEPT_DECISION && options.rollbackAction !== ROLLBACK_ACTION) {
    fail(`accepted decision requires --rollback-action ${ROLLBACK_ACTION}`);
  }
  if (decision === REJECT_DECISION && options.rollbackAction) fail("rejected decision must not carry a rollback action");
  const ownerId = boundedOwnerId(options.ownerId);
  const head = currentCommit();
  const evaluationPath = resolveTargetInput(options.evaluation, "evaluation packet");
  const { document: evaluation, bytes: evaluationBytes } = jsonDocument(evaluationPath, "evaluation packet");
  const admitted = requireEvaluation(evaluation, contract, evaluatorContract, head);
  const output = outputPath(contract, options.output);
  if (path.resolve(output) === path.resolve(evaluationPath)) fail("acceptance output must not overwrite evaluation input");
  rmSync(output, { force: true });

  const accepted = decision === ACCEPT_DECISION;
  writeAtomic(output, {
    format: contract.output.format,
    status: accepted ? contract.output.accepted_status : contract.output.rejected_status,
    decided_at: new Date().toISOString(),
    decision: {
      value: decision,
      owner_id: ownerId,
      owner_identity_is_operator_assertion: true,
      cryptographic_signature_present: false,
      rollback_action: accepted ? ROLLBACK_ACTION : null,
      free_text_reason_retained: false,
    },
    deployment: {
      deployment_id: admitted.deploymentId,
      deployment_image_digest: admitted.deploymentImageDigest,
      source_commit: admitted.sourceCommit,
      expected_target_count: admitted.expectedTargets,
      verified_backend_target_count: admitted.expectedTargets,
      query_window_seconds: admitted.queryWindow,
      freshness_seconds: admitted.freshnessWindow,
      identity_age_seconds: admitted.identityAge,
    },
    evaluation: {
      format: evaluation.format,
      status: evaluation.status,
      evaluated_at: evaluation.evaluated_at,
      evaluation_sha256: sha256(evaluationBytes),
      raw_evaluation_path_persisted: false,
      source_hashes_verified_against_checkout: true,
      samples: {
        preview: admitted.previewSamples,
        publish: admitted.publishSamples,
      },
      snapshot: {
        state: admitted.state,
        degradation_reasons: admitted.degradationReasons,
        thresholds: THRESHOLDS,
        observed: admitted.observed,
      },
      slo_evaluation: admitted.sloEvaluation,
    },
    binding: {
      server_binding_authorized: accepted,
      server_binding_performed: false,
      required_live_source_commit: admitted.sourceCommit,
      required_deployment_image_digest: admitted.deploymentImageDigest,
      failure_action: ROLLBACK_ACTION,
    },
    source_files: sourceHashes(contract),
    pages_provider_health_observed: false,
    pages_ui_provider_health_bound: false,
    pages_ssr_provider_health_bound: false,
    standalone_browser_intent_provider_health_bound: false,
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
