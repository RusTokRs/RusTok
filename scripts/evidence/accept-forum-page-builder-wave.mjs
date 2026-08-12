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
  "crates/rustok-forum/contracts/evidence/forum-page-builder-wave-observed-acceptance-source.json",
);
const freshnessVerifierPath = path.join(
  repoRoot,
  "scripts/verify/verify-forum-wave-evidence-freshness.mjs",
);
const lineageVerifierPath = path.join(
  repoRoot,
  "scripts/verify/verify-forum-wave-admission-lineage.mjs",
);
const MAX_INPUT_BYTES = 32 * 1024 * 1024;
const OWNER_ID_PATTERN = /^[A-Za-z0-9._-]{1,64}$/u;
const ACCEPT_DECISION = "accept_observed_wave_evidence";
const REJECT_DECISION = "reject";
const COMMIT_PATTERN = /^[0-9a-f]{40}$/u;
const REPO_DIGEST_PATTERN = /^[^@\s]+@sha256:[0-9a-f]{64}$/u;

function fail(message) {
  throw new Error(`Forum Wave observed owner acceptance failed: ${message}`);
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
  const accepted = new Set([
    "--wave-evidence",
    "--admission",
    "--owner-id",
    "--decision",
    "--output",
  ]);
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--help" || argument === "-h") {
      console.log(
        "usage: accept-forum-page-builder-wave.mjs " +
          "--wave-evidence FILE --admission FILE --owner-id ID " +
          "--decision accept_observed_wave_evidence|reject [--output FILE]",
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

function runVerifier(program, env, label) {
  const result = spawnSync("node", [program], {
    cwd: repoRoot,
    env: {
      ...process.env,
      RUSTOK_VERIFY_NOW: "",
      ...env,
    },
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    maxBuffer: 8 * 1024 * 1024,
  });
  if (result.error) fail(`${label} could not start: ${result.error.message}`);
  if (result.status !== 0) {
    const detail = (result.stderr || result.stdout || "").trim();
    fail(`${label} rejected supplied evidence${detail ? `: ${detail}` : ""}`);
  }
}

function canonicalCommit(value, label) {
  if (typeof value !== "string" || !COMMIT_PATTERN.test(value)) {
    fail(`${label} must be a lowercase 40-character Git SHA`);
  }
  return value;
}

function canonicalRepoDigest(value, label) {
  if (
    typeof value !== "string" ||
    value.length > 1024 ||
    !REPO_DIGEST_PATTERN.test(value)
  ) {
    fail(`${label} must be REPOSITORY@sha256:<64 lowercase hex>`);
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
  return value;
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
    fail("owner acceptance output must remain inside repository target/");
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
  for (const required of ["waveEvidence", "admission", "ownerId", "decision"]) {
    if (!options[required]) fail(`missing required ${required} input`);
  }
  if (!OWNER_ID_PATTERN.test(options.ownerId)) {
    fail("--owner-id must match ^[A-Za-z0-9._-]{1,64}$");
  }
  if (![ACCEPT_DECISION, REJECT_DECISION].includes(options.decision)) {
    fail("--decision must be accept_observed_wave_evidence or reject");
  }

  const contractRecord = jsonInput(contractPath, "Forum Wave observed acceptance source contract");
  const contract = contractRecord.document;
  if (
    contract.format !== "forum_page_builder_wave_observed_acceptance_source_v1" ||
    contract.status !== "source_ready_maintainer_execution_pending" ||
    contract.owner_decision?.runner !== "scripts/evidence/accept-forum-page-builder-wave.mjs"
  ) {
    fail("observed acceptance source contract identity drifted");
  }

  const wave = jsonInput(options.waveEvidence, "Forum Wave live evidence");
  const admission = jsonInput(options.admission, "Forum Wave retained admission");
  const head = currentCommit();

  if (
    wave.document.artifact !== "page_builder_wave_evidence_packet" ||
    wave.document.module_slug !== "forum" ||
    wave.document.wave !== "1" ||
    wave.document.mode !== "live" ||
    wave.document.provenance !== "observed_control_plane" ||
    wave.document.execution_status !== "maintainer_verified"
  ) {
    fail("owner review requires a maintainer-verified live Forum Wave packet");
  }
  if (canonicalCommit(wave.document.source_commit, "Wave source_commit") !== head) {
    fail("Wave source_commit does not equal checkout HEAD");
  }
  const deploymentDigest = canonicalRepoDigest(
    wave.document.deployment_image_digest,
    "Wave deployment_image_digest",
  );
  const createdAt = canonicalIso(wave.document.created_at, "Wave created_at");
  const nextDueAt = canonicalIso(
    wave.document.refresh_policy?.next_due_at,
    "Wave refresh_policy.next_due_at",
  );

  runVerifier(
    freshnessVerifierPath,
    { RUSTOK_FORUM_WAVE_EVIDENCE_PATH: wave.location },
    "Forum Wave freshness verifier",
  );
  runVerifier(
    lineageVerifierPath,
    {
      RUSTOK_FORUM_WAVE_EVIDENCE_PATH: wave.location,
      RUSTOK_FORUM_WAVE_ADMISSION_PATH: admission.location,
    },
    "Forum Wave admission-lineage verifier",
  );

  const retainedAdmission = objectValue(wave.document.admission, "Wave admission lineage");
  if (retainedAdmission.packet_sha256 !== admission.sha256) {
    fail("Wave admission.packet_sha256 does not match supplied retained admission");
  }
  if (
    admission.document.format !== contract.admission_input?.format ||
    admission.document.status !== contract.admission_input?.status ||
    admission.document.source_commit !== head ||
    admission.document.deployment?.deployment_image_digest !== deploymentDigest
  ) {
    fail("retained admission packet identity differs from reviewed Wave");
  }

  const output = outputPath(contract, options.output);
  rmSync(output, { force: true });
  const accepted = options.decision === ACCEPT_DECISION;
  writeAtomic(output, {
    format: contract.output.format,
    status: accepted ? contract.output.accepted_status : contract.output.rejected_status,
    reviewed_at: new Date().toISOString(),
    source_commit: head,
    deployment_image_digest: deploymentDigest,
    wave: {
      bytes: wave.size,
      sha256: wave.sha256,
      created_at: createdAt,
      next_due_at: nextDueAt,
      freshness_verifier_passed_at_review: true,
      admission_lineage_verifier_passed_at_review: true,
    },
    admission: {
      bytes: admission.size,
      sha256: admission.sha256,
    },
    owner: {
      owner_id: options.ownerId,
      decision: options.decision,
      identity_is_operator_assertion: true,
      cryptographic_signature_verified: false,
    },
    boundaries: {
      retrospective_evidence_review_only: true,
      control_plane_or_rollout_mutated: false,
      current_provider_health_asserted: false,
      cryptographic_origin_to_repo_digest_binding_claimed: false,
      forum_wave_promoted: false,
      ffa_promoted: false,
      fba_promoted: false,
    },
    privacy: {
      raw_input_paths_persisted: false,
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
