#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, lstatSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const lineageContractPath = path.join(
  repoRoot,
  "crates/rustok-forum/contracts/evidence/forum-wave-live-admission-lineage-source.json",
);
const defaultWavePath = path.join(
  repoRoot,
  "crates/rustok-forum/contracts/evidence/forum-wave1-rollout-evidence.json",
);
const MAX_PACKET_BYTES = 32 * 1024 * 1024;
const COMMIT_PATTERN = /^[0-9a-f]{40}$/u;
const SHA256_PATTERN = /^[0-9a-f]{64}$/u;
const REPO_DIGEST_PATTERN = /^[^@\s]+@sha256:[0-9a-f]{64}$/u;
const REQUIRED_GATE = "node scripts/verify/verify-forum-wave-admission-lineage.mjs";

function fail(message) {
  throw new Error(`Forum Wave admission lineage verification failed: ${message}`);
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

function resolvePath(value, fallback, label) {
  const candidate = value ?? fallback;
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

function jsonPacket(location, label) {
  if (!existsSync(location)) fail(`${label} is missing`);
  const metadata = lstatSync(location);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    fail(`${label} must be a regular non-symlink file`);
  }
  const size = statSync(location).size;
  if (size <= 0 || size > MAX_PACKET_BYTES) fail(`${label} is outside the bounded size`);
  const bytes = readFileSync(location);
  let document;
  try {
    document = JSON.parse(bytes.toString("utf8"));
  } catch (error) {
    fail(`${label} is invalid JSON: ${error.message}`);
  }
  objectValue(document, label);
  return { document, size, sha256: sha256(bytes) };
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

function requireRepoDigest(value, label) {
  if (typeof value !== "string" || !REPO_DIGEST_PATTERN.test(value)) {
    fail(`${label} must be an immutable REPOSITORY@sha256:<digest>`);
  }
  return value;
}

function validateSourceReady(wave) {
  if (
    wave.provenance !== "synthetic_fixture" ||
    wave.execution_status !== "not_run_by_implementation_agent" ||
    wave.observed_run?.status !== "not_run"
  ) {
    fail("source-ready Wave must remain synthetic and unexecuted");
  }
  const admission = objectValue(wave.observed_run?.wave_admission, "source-ready Wave admission cursor");
  if (
    admission.required !== true ||
    admission.format !== "forum_page_builder_wave_admission_v1" ||
    admission.status !== "forum_wave_inputs_admitted_observed_control_plane_pending" ||
    admission.execution_status !== "maintainer_execution_pending"
  ) {
    fail("source-ready Wave admission cursor drifted");
  }
  console.log("[verify-forum-wave-admission-lineage] PASS mode=source_ready admission_file=pending");
}

function validateAdmissionPacket(packet, wave, head) {
  if (
    packet.format !== "forum_page_builder_wave_admission_v1" ||
    packet.status !== "forum_wave_inputs_admitted_observed_control_plane_pending"
  ) {
    fail("admission packet format/status drifted");
  }
  if (packet.source_commit !== head || packet.source_commit !== wave.source_commit) {
    fail("admission packet source_commit differs from checkout or live Wave");
  }
  const deployment = objectValue(packet.deployment, "admission deployment");
  const admissionDigest = requireRepoDigest(
    deployment.deployment_image_digest,
    "admission deployment image digest",
  );
  if (admissionDigest !== wave.deployment_image_digest) {
    fail("admission packet RepoDigest differs from live Wave");
  }
  const admission = objectValue(packet.admission, "admission result");
  for (const key of [
    "pages_reference_consumer_gate_accepted",
    "exact_source_commit_bound",
    "exact_deployment_digest_bound",
    "forum_browser_execution_passed",
    "forum_runtime_authorization_execution_passed",
    "forum_server_fn_deployment_attestation_passed",
    "observed_control_plane_wave_pending",
  ]) {
    if (admission[key] !== true) fail(`admission result ${key} must be true`);
  }
  const boundaries = objectValue(packet.boundaries, "admission boundaries");
  for (const key of [
    "current_provider_health_asserted",
    "cryptographic_deployment_binding_claimed",
    "observed_control_plane_wave_executed",
    "forum_wave_accepted",
    "ffa_promoted",
    "fba_promoted",
  ]) {
    if (boundaries[key] !== false) fail(`admission boundary ${key} must remain false`);
  }
  const privacy = objectValue(packet.privacy, "admission privacy");
  for (const key of [
    "raw_input_paths_persisted",
    "raw_http_or_browser_bodies_persisted",
    "raw_command_output_persisted",
    "credentials_sessions_or_storage_state_contents_persisted",
    "tenant_or_actor_identifiers_persisted",
    "forum_content_persisted",
  ]) {
    if (privacy[key] !== false) fail(`admission privacy ${key} must remain false`);
  }
}

function validateLive(wave, admissionRecord, head) {
  if (wave.provenance !== "observed_control_plane" || wave.execution_status !== "maintainer_verified") {
    fail("live Wave must be observed_control_plane and maintainer_verified");
  }
  if (wave.source_commit !== head) fail("live Wave source_commit does not equal checkout HEAD");
  requireRepoDigest(wave.deployment_image_digest, "live Wave deployment image digest");

  const retained = objectValue(wave.admission, "live Wave admission lineage");
  if (
    retained.format !== "forum_page_builder_wave_admission_v1" ||
    retained.status !== "forum_wave_inputs_admitted_observed_control_plane_pending" ||
    retained.source_commit !== head ||
    retained.deployment_image_digest !== wave.deployment_image_digest ||
    retained.pages_reference_consumer_gate_accepted !== true ||
    retained.exact_source_commit_bound !== true ||
    retained.exact_deployment_digest_bound !== true ||
    typeof retained.packet_sha256 !== "string" ||
    !SHA256_PATTERN.test(retained.packet_sha256)
  ) {
    fail("live Wave admission lineage fields drifted");
  }
  if (retained.packet_sha256 !== admissionRecord.sha256) {
    fail("live Wave admission packet_sha256 does not match retained admission file");
  }
  validateAdmissionPacket(admissionRecord.document, wave, head);

  const latestRefresh = objectValue(
    wave.refresh_history?.latest_refresh,
    "live Wave latest refresh",
  );
  if (!(latestRefresh.no_compile_gates ?? []).includes(REQUIRED_GATE)) {
    fail("live Wave latest refresh does not retain the admission-lineage verifier gate");
  }
  console.log(
    `[verify-forum-wave-admission-lineage] PASS mode=live source_commit=${head} admission_sha256=${admissionRecord.sha256}`,
  );
}

try {
  const lineageContract = jsonPacket(lineageContractPath, "Forum Wave live admission lineage source contract").document;
  if (
    lineageContract.format !== "forum_wave_live_admission_lineage_source_v1" ||
    lineageContract.status !== "source_ready_maintainer_execution_pending" ||
    lineageContract.runner !== "scripts/verify/verify-forum-wave-admission-lineage.mjs" ||
    lineageContract.required_live_environment?.admission_path !== "RUSTOK_FORUM_WAVE_ADMISSION_PATH" ||
    lineageContract.required_gate !== REQUIRED_GATE
  ) {
    fail("live admission lineage source contract drifted");
  }
  const wavePath = resolvePath(
    process.env.RUSTOK_FORUM_WAVE_EVIDENCE_PATH,
    defaultWavePath,
    "Forum Wave evidence",
  );
  const waveRecord = jsonPacket(wavePath, "Forum Wave evidence");
  const wave = waveRecord.document;
  if (wave.module_slug !== "forum" || wave.wave !== "1") {
    fail("Wave evidence must describe Forum Wave 1");
  }
  if (wave.mode === "source_ready") {
    validateSourceReady(wave);
  } else if (wave.mode === "live") {
    const admissionPath = process.env.RUSTOK_FORUM_WAVE_ADMISSION_PATH;
    if (!admissionPath) fail("live Wave requires RUSTOK_FORUM_WAVE_ADMISSION_PATH");
    const admissionRecord = jsonPacket(
      resolvePath(admissionPath, null, "Forum Wave admission"),
      "Forum Wave admission",
    );
    validateLive(wave, admissionRecord, currentCommit());
  } else {
    fail("Wave evidence mode must be source_ready or live");
  }
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
