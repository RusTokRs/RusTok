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
const contractPath = path.join(
  repoRoot,
  "crates/rustok-forum/contracts/evidence/forum-page-builder-ffa-fba-promotion-review-source.json",
);
const MAX_INPUT_BYTES = 1024 * 1024;
const OWNER_ID_PATTERN = /^[A-Za-z0-9._-]{1,64}$/u;
const APPROVE_DECISION = "approve_ffa_fba_promotion_review";
const REJECT_DECISION = "reject";
const COMMIT_PATTERN = /^[0-9a-f]{40}$/u;
const SHA256_PATTERN = /^[0-9a-f]{64}$/u;
const REPO_DIGEST_PATTERN = /^[^@\s]+@sha256:[0-9a-f]{64}$/u;
const CLOCK_SKEW_MS = 5 * 60 * 1000;

function fail(message) {
  throw new Error(`Forum FFA/FBA promotion review failed: ${message}`);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function objectValue(value, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
  return value;
}

function parseArguments(argv) {
  const options = {};
  const accepted = new Set(["--observed-acceptance", "--owner-id", "--decision", "--output"]);
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--help" || argument === "-h") {
      console.log(
        "usage: review-forum-page-builder-ffa-fba-promotion.mjs " +
          "--observed-acceptance FILE --owner-id ID " +
          "--decision approve_ffa_fba_promotion_review|reject [--output FILE]",
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
  if (
    typeof candidate !== "string" ||
    candidate.length === 0 ||
    candidate.length > 16_384 ||
    /[\u0000\r\n]/u.test(candidate)
  ) {
    fail(`${label} path is invalid`);
  }
  return path.isAbsolute(candidate) ? path.resolve(candidate) : path.resolve(repoRoot, candidate);
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

function jsonInput(candidate, label) {
  const location = resolveInput(candidate, label);
  const record = regularFile(location, label);
  try {
    const document = JSON.parse(record.bytes.toString("utf8"));
    objectValue(document, label);
    return { location, document, ...record };
  } catch (error) {
    fail(`${label} is invalid JSON: ${error.message}`);
  }
}

function currentCommit() {
  const result = spawnSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.status !== 0) fail("git rev-parse HEAD failed");
  const commit = result.stdout.trim().toLowerCase();
  if (!COMMIT_PATTERN.test(commit)) fail("checkout HEAD is not a canonical Git commit");
  return commit;
}

function canonicalCommit(value, label) {
  if (typeof value !== "string" || !COMMIT_PATTERN.test(value)) {
    fail(`${label} must be a lowercase 40-character Git SHA`);
  }
  return value;
}

function canonicalRepoDigest(value, label) {
  if (typeof value !== "string" || value.length > 1024 || !REPO_DIGEST_PATTERN.test(value)) {
    fail(`${label} must be REPOSITORY@sha256:<64 lowercase hex>`);
  }
  return value;
}

function canonicalSha256(value, label) {
  if (typeof value !== "string" || !SHA256_PATTERN.test(value)) {
    fail(`${label} must be 64 lowercase hex characters`);
  }
  return value;
}

function canonicalSize(value, label) {
  if (!Number.isSafeInteger(value) || value <= 0 || value > 32 * 1024 * 1024) {
    fail(`${label} must be a positive bounded byte count`);
  }
  return value;
}

function canonicalIso(value, label) {
  if (typeof value !== "string" || value.length === 0 || value.length > 128) {
    fail(`${label} is invalid`);
  }
  const milliseconds = Date.parse(value);
  if (!Number.isFinite(milliseconds) || new Date(milliseconds).toISOString() !== value) {
    fail(`${label} must be canonical ISO-8601 UTC`);
  }
  return { value, milliseconds };
}

function outputPath(contract, requested) {
  const value = requested ?? contract.output?.default_path;
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
    fail("promotion review output must remain inside repository target/");
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

function requireFalse(record, key, label) {
  if (record[key] !== false) fail(`${label}.${key} must remain false`);
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  for (const required of ["observedAcceptance", "ownerId", "decision"]) {
    if (!options[required]) fail(`missing required ${required} input`);
  }
  if (!OWNER_ID_PATTERN.test(options.ownerId)) {
    fail("--owner-id must match ^[A-Za-z0-9._-]{1,64}$");
  }
  if (![APPROVE_DECISION, REJECT_DECISION].includes(options.decision)) {
    fail("--decision must be approve_ffa_fba_promotion_review or reject");
  }

  const contractRecord = jsonInput(contractPath, "Forum FFA/FBA promotion review source contract");
  const contract = contractRecord.document;
  if (
    contract.format !== "forum_page_builder_ffa_fba_promotion_review_source_v1" ||
    contract.status !== "source_ready_maintainer_execution_pending" ||
    contract.promotion_review?.runner !==
      "scripts/evidence/review-forum-page-builder-ffa-fba-promotion.mjs"
  ) {
    fail("promotion review source contract identity drifted");
  }

  const acceptance = jsonInput(options.observedAcceptance, "Forum observed Wave owner acceptance");
  const document = acceptance.document;
  const head = currentCommit();
  if (
    document.format !== contract.predecessor?.format ||
    document.status !== contract.predecessor?.accepted_status
  ) {
    fail("promotion review requires an accepted observed-Wave owner packet");
  }
  if (canonicalCommit(document.source_commit, "observed acceptance source_commit") !== head) {
    fail("observed acceptance source_commit does not equal checkout HEAD");
  }
  const deploymentDigest = canonicalRepoDigest(
    document.deployment_image_digest,
    "observed acceptance deployment_image_digest",
  );
  const reviewedAt = canonicalIso(document.reviewed_at, "observed acceptance reviewed_at");
  const wave = objectValue(document.wave, "observed acceptance wave");
  const admission = objectValue(document.admission, "observed acceptance admission");
  const waveCreatedAt = canonicalIso(wave.created_at, "observed acceptance wave.created_at");
  const waveNextDueAt = canonicalIso(wave.next_due_at, "observed acceptance wave.next_due_at");
  canonicalSize(wave.bytes, "observed acceptance wave.bytes");
  canonicalSha256(wave.sha256, "observed acceptance wave.sha256");
  canonicalSize(admission.bytes, "observed acceptance admission.bytes");
  canonicalSha256(admission.sha256, "observed acceptance admission.sha256");

  const now = Date.now();
  if (reviewedAt.milliseconds > now + CLOCK_SKEW_MS) {
    fail("observed acceptance reviewed_at is implausibly in the future");
  }
  if (waveCreatedAt.milliseconds > reviewedAt.milliseconds) {
    fail("observed Wave created_at is after owner acceptance reviewed_at");
  }
  if (waveNextDueAt.milliseconds <= now) {
    fail("observed Wave evidence is stale at promotion review time");
  }
  if (wave.freshness_verifier_passed_at_review !== true) {
    fail("observed acceptance did not retain a passing freshness verifier");
  }
  if (wave.admission_lineage_verifier_passed_at_review !== true) {
    fail("observed acceptance did not retain a passing admission-lineage verifier");
  }

  const priorOwner = objectValue(document.owner, "observed acceptance owner");
  if (
    priorOwner.decision !== contract.predecessor?.owner_decision_must_equal ||
    priorOwner.identity_is_operator_assertion !== true ||
    priorOwner.cryptographic_signature_verified !== false
  ) {
    fail("observed acceptance owner decision boundary drifted");
  }

  const priorBoundaries = objectValue(document.boundaries, "observed acceptance boundaries");
  if (priorBoundaries.retrospective_evidence_review_only !== true) {
    fail("observed acceptance must remain retrospective evidence review only");
  }
  for (const key of [
    "control_plane_or_rollout_mutated",
    "current_provider_health_asserted",
    "cryptographic_origin_to_repo_digest_binding_claimed",
    "forum_wave_promoted",
    "ffa_promoted",
    "fba_promoted",
  ]) requireFalse(priorBoundaries, key, "observed acceptance boundaries");

  const privacy = objectValue(document.privacy, "observed acceptance privacy");
  for (const key of [
    "raw_input_paths_persisted",
    "raw_metrics_or_trace_values_persisted",
    "forum_content_persisted",
    "tenant_or_actor_identifiers_persisted",
    "free_text_reason_persisted",
  ]) requireFalse(privacy, key, "observed acceptance privacy");

  const output = outputPath(contract, options.output);
  rmSync(output, { force: true });
  const approved = options.decision === APPROVE_DECISION;
  writeAtomic(output, {
    format: contract.output.format,
    status: approved ? contract.output.approved_status : contract.output.rejected_status,
    reviewed_at: new Date().toISOString(),
    source_commit: head,
    deployment_image_digest: deploymentDigest,
    observed_acceptance: {
      bytes: acceptance.size,
      sha256: acceptance.sha256,
      reviewed_at: reviewedAt.value,
      wave_created_at: waveCreatedAt.value,
      wave_next_due_at: waveNextDueAt.value,
      prior_owner_decision: priorOwner.decision,
      freshness_verifier_passed_at_prior_review: true,
      admission_lineage_verifier_passed_at_prior_review: true,
    },
    promotion_review: {
      owner_id: options.ownerId,
      decision: options.decision,
      targets: ["ffa", "fba"],
      identity_is_operator_assertion: true,
      cryptographic_signature_verified: false,
    },
    boundaries: {
      review_only: true,
      approval_is_not_control_plane_execution: true,
      control_plane_or_rollout_mutated: false,
      pages_or_forum_persistence_mutated: false,
      current_provider_health_asserted: false,
      cryptographic_origin_to_repo_digest_binding_claimed: false,
      forum_wave_promoted: false,
      ffa_promoted: false,
      fba_promoted: false,
      separate_control_plane_execution_required: approved,
    },
    privacy: {
      raw_input_path_persisted: false,
      raw_metrics_or_trace_values_persisted: false,
      forum_content_persisted: false,
      tenant_or_actor_identifiers_persisted: false,
      free_text_reason_persisted: false,
    },
  });
}

try {
  main();
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
