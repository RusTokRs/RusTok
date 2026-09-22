#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const runnerPath = path.join(repoRoot, "scripts/evidence/accept-forum-page-builder-wave.mjs");
const REQUIRED_GATES = [
  "npm run verify:page-builder:consumer:forum",
  "node scripts/verify/verify-forum-page-builder-wave-admission.mjs",
  "npm run verify:forum:wave-evidence-freshness",
  "npm run test:verify:forum:wave-evidence-freshness",
  "node scripts/verify/verify-forum-wave-admission-lineage.mjs",
];
const REQUIRED_SECTIONS = [
  "admission",
  "control_plane.audit_trail",
  "fallback.profiles",
  "observability.metrics",
  "observability.traces",
  "rollback.decision",
  "approvals",
  "waivers",
  "refresh_history.latest_refresh",
];

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function currentCommit() {
  const result = spawnSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout.trim();
}

function iso(offsetMs) {
  return new Date(Date.now() + offsetMs).toISOString();
}

function admissionPacket(head, digest, overrides = {}) {
  const base = {
    format: "forum_page_builder_wave_admission_v1",
    status: "forum_wave_inputs_admitted_observed_control_plane_pending",
    generated_at: iso(-120_000),
    source_commit: head,
    deployment: {
      deployment_id: "synthetic-forum-wave-owner-review",
      deployment_image_digest: digest,
    },
    inputs: {
      pages_gate: { bytes: 1, sha256: "1".repeat(64) },
      forum_browser: { bytes: 1, sha256: "2".repeat(64) },
      forum_runtime_authorization: { bytes: 1, sha256: "3".repeat(64) },
      forum_serverfn_attestation: { bytes: 1, sha256: "4".repeat(64) },
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
    source_files: {},
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

function wavePacket(head, digest, admissionSha, overrides = {}) {
  const createdAt = iso(-60_000);
  const nextDueAt = iso(30 * 24 * 60 * 60 * 1000);
  const base = {
    artifact: "page_builder_wave_evidence_packet",
    module_slug: "forum",
    wave: "1",
    mode: "live",
    provenance: "observed_control_plane",
    execution_status: "maintainer_verified",
    source_commit: head,
    deployment_image_digest: digest,
    created_at: createdAt,
    admission: {
      format: "forum_page_builder_wave_admission_v1",
      status: "forum_wave_inputs_admitted_observed_control_plane_pending",
      source_commit: head,
      deployment_image_digest: digest,
      packet_sha256: admissionSha,
      pages_reference_consumer_gate_accepted: true,
      exact_source_commit_bound: true,
      exact_deployment_digest_bound: true,
    },
    control_plane: { audit_trail: "synthetic-control-plane-audit" },
    fallback: {
      profiles: [
        { name: "all_on", decision: "keep" },
        { name: "publish_off", decision: "keep" },
        { name: "preview_off", decision: "keep" },
        { name: "builder_off", decision: "keep" },
      ],
    },
    observability: {
      metrics: {
        preview_p95_ms: "live_wave1_actual:120",
        publish_p95_ms: "live_wave1_actual:230",
        sanitize_failure_rate: "live_wave1_actual:0.001",
        runtime_error_rate: "live_wave1_actual:0.000",
      },
      traces: {
        builder_write_to_forum_publish: "synthetic-trace-a",
        forum_publish_to_storefront_read: "synthetic-trace-b",
      },
    },
    rollback: { decision: "keep" },
    approvals: {
      platform_on_call: "approved",
      forum_owner: "approved",
      builder_owner: "approved",
      runtime_owner: "approved",
    },
    waivers: [],
    refresh_history: {
      latest_refresh: {
        refreshed_at: createdAt,
        verified_by: "rustok-forum module team",
        no_compile_gates: [...REQUIRED_GATES],
        sections_refreshed: [...REQUIRED_SECTIONS],
      },
    },
    refresh_policy: {
      cadence: "monthly",
      max_age_days: 45,
      next_due_at: nextDueAt,
      required_gate: "npm run verify:page-builder:consumer:forum",
      stale_evidence_action: "block_builder_consumer_rollout_until_refreshed",
      owner: "rustok-forum module team",
      required_sections: [...REQUIRED_SECTIONS],
    },
  };
  return {
    ...base,
    ...overrides,
    admission: { ...base.admission, ...(overrides.admission ?? {}) },
    observability: { ...base.observability, ...(overrides.observability ?? {}) },
    refresh_history: {
      ...base.refresh_history,
      ...(overrides.refresh_history ?? {}),
    },
    refresh_policy: {
      ...base.refresh_policy,
      ...(overrides.refresh_policy ?? {}),
    },
  };
}

function runCase({
  decision = "accept_observed_wave_evidence",
  ownerId = "forum-owner",
  admissionOverrides = {},
  waveOverrides = {},
  mutateAdmissionAfterWave = null,
  dropLineageGate = false,
  outputName = `forum-wave-owner-${process.pid}-${Math.random().toString(16).slice(2)}.json`,
}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-forum-wave-owner-"));
  const head = currentCommit();
  const digest = `example.invalid/rustok-server@sha256:${"a".repeat(64)}`;
  const admissionPath = path.join(root, "admission.json");
  const wavePath = path.join(root, "wave.json");
  const outputPath = path.join(repoRoot, "target", outputName);
  try {
    const admission = admissionPacket(head, digest, admissionOverrides);
    const admissionBytes = Buffer.from(`${JSON.stringify(admission, null, 2)}\n`);
    writeFileSync(admissionPath, admissionBytes);
    const wave = wavePacket(head, digest, sha256(admissionBytes), waveOverrides);
    if (dropLineageGate) {
      wave.refresh_history.latest_refresh.no_compile_gates =
        wave.refresh_history.latest_refresh.no_compile_gates.filter(
          (value) => value !== "node scripts/verify/verify-forum-wave-admission-lineage.mjs",
        );
    }
    writeFileSync(wavePath, `${JSON.stringify(wave, null, 2)}\n`);
    if (mutateAdmissionAfterWave) {
      const mutated = mutateAdmissionAfterWave(admission);
      writeFileSync(admissionPath, `${JSON.stringify(mutated, null, 2)}\n`);
    }
    const result = spawnSync(
      "node",
      [
        runnerPath,
        "--wave-evidence",
        wavePath,
        "--admission",
        admissionPath,
        "--owner-id",
        ownerId,
        "--decision",
        decision,
        "--output",
        outputPath,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: { ...process.env, RUSTOK_VERIFY_NOW: "" },
      },
    );
    const output = result.status === 0 ? JSON.parse(readFileSync(outputPath, "utf8")) : null;
    return { result, output };
  } finally {
    rmSync(root, { recursive: true, force: true });
    rmSync(outputPath, { force: true });
  }
}

test("accepts fresh lineage-verified observed Forum Wave evidence", () => {
  const { result, output } = runCase({});
  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.equal(output.status, "owner_accepted_observed_control_plane_wave_promotion_review_pending");
  assert.equal(output.owner.decision, "accept_observed_wave_evidence");
  assert.equal(output.boundaries.ffa_promoted, false);
  assert.equal(output.boundaries.fba_promoted, false);
});

test("retains explicit reject without promotion", () => {
  const { result, output } = runCase({ decision: "reject" });
  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.equal(output.status, "owner_rejected_observed_control_plane_wave");
  assert.equal(output.owner.decision, "reject");
  assert.equal(output.boundaries.forum_wave_promoted, false);
});

test("rejects source-ready Wave evidence at owner review", () => {
  const { result } = runCase({
    waveOverrides: {
      mode: "source_ready",
      provenance: "synthetic_fixture",
      execution_status: "not_run_by_implementation_agent",
    },
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /requires a maintainer-verified live Forum Wave packet/);
});

test("rejects invalid owner identifier", () => {
  const { result } = runCase({ ownerId: "bad owner id" });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /owner-id/);
});

test("rejects unsupported owner decision", () => {
  const { result } = runCase({ decision: "promote" });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /decision/);
});

test("rejects stale observed Wave evidence", () => {
  const staleCreated = new Date(Date.now() - 60 * 24 * 60 * 60 * 1000).toISOString();
  const staleDue = new Date(Date.now() - 20 * 24 * 60 * 60 * 1000).toISOString();
  const { result } = runCase({
    waveOverrides: {
      created_at: staleCreated,
      refresh_history: {
        latest_refresh: {
          refreshed_at: staleCreated,
          verified_by: "rustok-forum module team",
          no_compile_gates: [...REQUIRED_GATES],
          sections_refreshed: [...REQUIRED_SECTIONS],
        },
      },
      refresh_policy: {
        next_due_at: staleDue,
      },
    },
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /freshness verifier rejected supplied evidence/);
});

test("rejects retained admission hash drift", () => {
  const { result } = runCase({
    mutateAdmissionAfterWave: (admission) => ({
      ...admission,
      generated_at: iso(-30_000),
    }),
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /admission-lineage verifier rejected supplied evidence/);
});

test("rejects admission source-commit drift", () => {
  const { result } = runCase({
    admissionOverrides: { source_commit: "0".repeat(40) },
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /admission-lineage verifier rejected supplied evidence/);
});

test("rejects admission privacy overclaim", () => {
  const { result } = runCase({
    admissionOverrides: {
      privacy: { forum_content_persisted: true },
    },
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /admission-lineage verifier rejected supplied evidence/);
});

test("rejects live Wave that drops the lineage verifier from refresh gates", () => {
  const { result } = runCase({ dropLineageGate: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /admission-lineage verifier rejected supplied evidence/);
});
