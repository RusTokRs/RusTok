#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
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
const runnerPath = path.join(
  repoRoot,
  "scripts/evidence/review-forum-page-builder-ffa-fba-promotion.mjs",
);

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

function acceptancePacket(head, overrides = {}) {
  const base = {
    format: "forum_page_builder_wave_observed_acceptance_v1",
    status: "owner_accepted_observed_control_plane_wave_promotion_review_pending",
    reviewed_at: iso(-60_000),
    source_commit: head,
    deployment_image_digest: `example.invalid/rustok-server@sha256:${"a".repeat(64)}`,
    wave: {
      bytes: 4096,
      sha256: "1".repeat(64),
      created_at: iso(-120_000),
      next_due_at: iso(30 * 24 * 60 * 60 * 1000),
      freshness_verifier_passed_at_review: true,
      admission_lineage_verifier_passed_at_review: true,
    },
    admission: {
      bytes: 2048,
      sha256: "2".repeat(64),
    },
    owner: {
      owner_id: "forum-owner",
      decision: "accept_observed_wave_evidence",
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
  };
  return {
    ...base,
    ...overrides,
    wave: { ...base.wave, ...(overrides.wave ?? {}) },
    admission: { ...base.admission, ...(overrides.admission ?? {}) },
    owner: { ...base.owner, ...(overrides.owner ?? {}) },
    boundaries: { ...base.boundaries, ...(overrides.boundaries ?? {}) },
    privacy: { ...base.privacy, ...(overrides.privacy ?? {}) },
  };
}

function runCase({
  decision = "approve_ffa_fba_promotion_review",
  ownerId = "release-owner",
  acceptanceOverrides = {},
  outputName = `forum-promotion-review-${process.pid}-${Math.random().toString(16).slice(2)}.json`,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-forum-promotion-review-"));
  const acceptancePath = path.join(root, "observed-acceptance.json");
  const outputPath = path.join(repoRoot, "target", outputName);
  try {
    const acceptance = acceptancePacket(currentCommit(), acceptanceOverrides);
    writeFileSync(acceptancePath, `${JSON.stringify(acceptance, null, 2)}\n`);
    const result = spawnSync(
      "node",
      [
        runnerPath,
        "--observed-acceptance",
        acceptancePath,
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
      },
    );
    const output = result.status === 0 ? JSON.parse(readFileSync(outputPath, "utf8")) : null;
    return { result, output };
  } finally {
    rmSync(root, { recursive: true, force: true });
    rmSync(outputPath, { force: true });
  }
}

test("approves promotion review without promoting FFA or FBA", () => {
  const { result, output } = runCase();
  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.equal(output.status, "owner_approved_ffa_fba_promotion_review_execution_pending");
  assert.equal(output.promotion_review.decision, "approve_ffa_fba_promotion_review");
  assert.deepEqual(output.promotion_review.targets, ["ffa", "fba"]);
  assert.equal(output.boundaries.control_plane_or_rollout_mutated, false);
  assert.equal(output.boundaries.ffa_promoted, false);
  assert.equal(output.boundaries.fba_promoted, false);
  assert.equal(output.boundaries.separate_control_plane_execution_required, true);
});

test("retains explicit promotion review reject without rollout mutation", () => {
  const { result, output } = runCase({ decision: "reject" });
  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.equal(output.status, "owner_rejected_ffa_fba_promotion_review");
  assert.equal(output.promotion_review.decision, "reject");
  assert.equal(output.boundaries.separate_control_plane_execution_required, false);
  assert.equal(output.boundaries.ffa_promoted, false);
  assert.equal(output.boundaries.fba_promoted, false);
});

test("rejects non-accepted observed Wave owner packet", () => {
  const { result } = runCase({
    acceptanceOverrides: { status: "owner_rejected_observed_control_plane_wave" },
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /requires an accepted observed-Wave owner packet/);
});

test("rejects observed acceptance source-commit drift", () => {
  const { result } = runCase({
    acceptanceOverrides: { source_commit: "0".repeat(40) },
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /source_commit does not equal checkout HEAD/);
});

test("rejects stale observed Wave evidence at promotion review time", () => {
  const { result } = runCase({
    acceptanceOverrides: {
      wave: { next_due_at: iso(-1_000) },
    },
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /stale at promotion review time/);
});

test("rejects prior owner decision drift", () => {
  const { result } = runCase({
    acceptanceOverrides: { owner: { decision: "reject" } },
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /owner decision boundary drifted/);
});

test("rejects missing retained freshness verifier success", () => {
  const { result } = runCase({
    acceptanceOverrides: { wave: { freshness_verifier_passed_at_review: false } },
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /passing freshness verifier/);
});

test("rejects missing retained admission-lineage verifier success", () => {
  const { result } = runCase({
    acceptanceOverrides: { wave: { admission_lineage_verifier_passed_at_review: false } },
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /passing admission-lineage verifier/);
});

test("rejects prior rollout mutation overclaim", () => {
  const { result } = runCase({
    acceptanceOverrides: { boundaries: { control_plane_or_rollout_mutated: true } },
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /control_plane_or_rollout_mutated must remain false/);
});

test("rejects prior FFA promotion overclaim", () => {
  const { result } = runCase({
    acceptanceOverrides: { boundaries: { ffa_promoted: true } },
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /ffa_promoted must remain false/);
});

test("rejects retained privacy overclaim", () => {
  const { result } = runCase({
    acceptanceOverrides: { privacy: { forum_content_persisted: true } },
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /forum_content_persisted must remain false/);
});

test("rejects invalid promotion-review owner identifier", () => {
  const { result } = runCase({ ownerId: "bad owner id" });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /owner-id/);
});

test("rejects unsupported promotion-review decision", () => {
  const { result } = runCase({ decision: "promote_now" });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /decision/);
});
