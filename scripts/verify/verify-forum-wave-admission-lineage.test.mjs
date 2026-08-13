#!/usr/bin/env node

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";

const repoRoot = path.resolve(".");
const verifier = path.resolve("scripts/verify/verify-forum-wave-admission-lineage.mjs");
const requiredGate = "node scripts/verify/verify-forum-wave-admission-lineage.mjs";
const digest = `example.invalid/rustok-server@sha256:${"a".repeat(64)}`;

function head() {
  const result = spawnSync("git", ["rev-parse", "HEAD"], { cwd: repoRoot, encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout.trim();
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function admissionPacket(sourceCommit, overrides = {}) {
  const base = {
    format: "forum_page_builder_wave_admission_v1",
    status: "forum_wave_inputs_admitted_observed_control_plane_pending",
    generated_at: "2026-08-12T18:00:00.000Z",
    source_commit: sourceCommit,
    deployment: {
      deployment_id: "forum-wave-lineage-test",
      deployment_image_digest: digest,
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
      current_provider_health_asserted: false,
      cryptographic_deployment_binding_claimed: false,
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
  };
  return {
    ...base,
    ...overrides,
    deployment: { ...base.deployment, ...(overrides.deployment ?? {}) },
    admission: { ...base.admission, ...(overrides.admission ?? {}) },
    boundaries: { ...base.boundaries, ...(overrides.boundaries ?? {}) },
    privacy: { ...base.privacy, ...(overrides.privacy ?? {}) },
  };
}

function liveWave(sourceCommit, packetSha, overrides = {}) {
  const base = {
    artifact: "page_builder_wave_evidence_packet",
    module_slug: "forum",
    wave: "1",
    mode: "live",
    provenance: "observed_control_plane",
    execution_status: "maintainer_verified",
    source_commit: sourceCommit,
    deployment_image_digest: digest,
    admission: {
      format: "forum_page_builder_wave_admission_v1",
      status: "forum_wave_inputs_admitted_observed_control_plane_pending",
      source_commit: sourceCommit,
      deployment_image_digest: digest,
      packet_sha256: packetSha,
      pages_reference_consumer_gate_accepted: true,
      exact_source_commit_bound: true,
      exact_deployment_digest_bound: true,
    },
    refresh_history: {
      latest_refresh: {
        no_compile_gates: [requiredGate],
      },
    },
  };
  return {
    ...base,
    ...overrides,
    admission: overrides.admission === null ? null : { ...base.admission, ...(overrides.admission ?? {}) },
    refresh_history: {
      ...base.refresh_history,
      ...(overrides.refresh_history ?? {}),
    },
  };
}

function sourceReadyWave() {
  return {
    artifact: "page_builder_wave_evidence_packet",
    module_slug: "forum",
    wave: "1",
    mode: "source_ready",
    provenance: "synthetic_fixture",
    execution_status: "not_run_by_implementation_agent",
    observed_run: {
      status: "not_run",
      wave_admission: {
        required: true,
        format: "forum_page_builder_wave_admission_v1",
        status: "forum_wave_inputs_admitted_observed_control_plane_pending",
        execution_status: "maintainer_execution_pending",
      },
    },
  };
}

function runCase({ wave, admission, expectedStatus = 0, expectedPattern }) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-forum-wave-lineage-"));
  const wavePath = path.join(root, "wave.json");
  const admissionPath = path.join(root, "admission.json");
  try {
    writeFileSync(wavePath, `${JSON.stringify(wave, null, 2)}\n`);
    const env = { ...process.env, RUSTOK_FORUM_WAVE_EVIDENCE_PATH: wavePath };
    if (admission !== undefined) {
      writeFileSync(admissionPath, `${JSON.stringify(admission, null, 2)}\n`);
      env.RUSTOK_FORUM_WAVE_ADMISSION_PATH = admissionPath;
    }
    const result = spawnSync("node", [verifier], { cwd: repoRoot, env, encoding: "utf8" });
    if (expectedStatus === 0) {
      assert.equal(result.status, 0, result.stderr || result.stdout);
      assert.match(result.stdout, expectedPattern ?? /PASS/);
    } else {
      assert.notEqual(result.status, 0, result.stdout);
      assert.match(result.stderr, expectedPattern);
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

function boundFixture(admissionOverrides = {}, waveOverrides = {}) {
  const sourceCommit = head();
  const admission = admissionPacket(sourceCommit, admissionOverrides);
  const bytes = Buffer.from(`${JSON.stringify(admission, null, 2)}\n`);
  const wave = liveWave(sourceCommit, sha256(bytes), waveOverrides);
  return { admission, wave };
}

test("source-ready Wave keeps admission file pending", () => {
  runCase({ wave: sourceReadyWave(), expectedPattern: /mode=source_ready/ });
});

test("live Wave accepts the exact retained admission packet", () => {
  const fixture = boundFixture();
  runCase({ ...fixture, expectedPattern: /mode=live/ });
});

test("live Wave rejects a missing admission packet path", () => {
  const fixture = boundFixture();
  runCase({ wave: fixture.wave, expectedStatus: 1, expectedPattern: /RUSTOK_FORUM_WAVE_ADMISSION_PATH/ });
});

test("live Wave rejects admission packet hash drift", () => {
  const fixture = boundFixture();
  fixture.wave.admission.packet_sha256 = "b".repeat(64);
  runCase({ ...fixture, expectedStatus: 1, expectedPattern: /packet_sha256 does not match/ });
});

test("live Wave rejects admission source drift", () => {
  const fixture = boundFixture({ source_commit: "0".repeat(40) });
  runCase({ ...fixture, expectedStatus: 1, expectedPattern: /source_commit differs/ });
});

test("live Wave rejects admission RepoDigest drift", () => {
  const fixture = boundFixture({ deployment: { deployment_image_digest: `example.invalid/rustok-server@sha256:${"c".repeat(64)}` } });
  runCase({ ...fixture, expectedStatus: 1, expectedPattern: /RepoDigest differs/ });
});

test("live Wave rejects premature Forum acceptance in admission packet", () => {
  const fixture = boundFixture({ boundaries: { forum_wave_accepted: true } });
  runCase({ ...fixture, expectedStatus: 1, expectedPattern: /forum_wave_accepted must remain false/ });
});

test("live Wave rejects admission privacy overclaim", () => {
  const fixture = boundFixture({ privacy: { raw_http_or_browser_bodies_persisted: true } });
  runCase({ ...fixture, expectedStatus: 1, expectedPattern: /raw_http_or_browser_bodies_persisted must remain false/ });
});

test("live Wave rejects missing retained lineage verifier gate", () => {
  const fixture = boundFixture({}, { refresh_history: { latest_refresh: { no_compile_gates: [] } } });
  runCase({ ...fixture, expectedStatus: 1, expectedPattern: /does not retain the admission-lineage verifier gate/ });
});
