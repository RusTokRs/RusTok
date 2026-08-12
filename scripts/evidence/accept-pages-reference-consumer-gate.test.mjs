#!/usr/bin/env node

import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const runner = path.join(
  repoRoot,
  "scripts/evidence/accept-pages-reference-consumer-gate.mjs",
);
const candidateContract = readJson(
  "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-execution-contract.json",
);
const observedSource = readJson(
  "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-observed-acceptance-source.json",
);
const testRoot = path.join(
  repoRoot,
  "target/pages-reference-consumer-gate-owner-runner-tests",
);
const deploymentDigest = `ghcr.io/rustok/page-builder@sha256:${"a".repeat(64)}`;
const deploymentId = "synthetic-pages-gate-owner-test";
const emptySha = sha256(Buffer.alloc(0));
const packetSha = "b".repeat(64);

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function readJson(relativePath) {
  return JSON.parse(readFileSync(path.join(repoRoot, relativePath), "utf8"));
}

function currentCommit() {
  return execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
  }).trim();
}

function sourceHashes(contract) {
  assert.ok(Array.isArray(contract.required_source_files));
  assert.ok(contract.required_source_files.length > 0);
  return Object.fromEntries(
    contract.required_source_files.map((relativePath) => [
      relativePath,
      sha256(readFileSync(path.join(repoRoot, relativePath))),
    ]),
  );
}

function writeJson(location, document) {
  mkdirSync(path.dirname(location), { recursive: true });
  writeFileSync(location, `${JSON.stringify(document, null, 2)}\n`, "utf8");
  const bytes = readFileSync(location);
  return { bytes: statSync(location).size, sha256: sha256(bytes) };
}

function packetRecord() {
  return { bytes: 1, sha256: packetSha };
}

function commandResults(commands) {
  return commands.map((command) => ({
    id: command.id,
    program: command.program,
    args: [...command.args],
    status: 0,
    stdout: { bytes: 0, sha256: emptySha },
    stderr: { bytes: 0, sha256: emptySha },
  }));
}

function candidateResult() {
  return {
    all_source_guards_passed: true,
    all_focused_tests_passed: true,
    exact_source_commit_bound: true,
    exact_deployment_digest_bound: true,
    artifact_http_browser_chain_bound: true,
    rollout_matrix_browser_chain_bound: true,
    rollout_matrix_profiles_passed: true,
    rollout_matrix_settings_restored: true,
    rollout_feature_preflight_chain_bound: true,
    rollout_feature_preflight_profiles_passed: true,
    rollout_feature_preflight_settings_restored: true,
    canonical_feature_disabled_catalog_passed: true,
    browser_intent_denial_kept_separate: true,
    provider_health: "unobserved",
    owner_signoff: "pending",
    rollback_decision: "pending",
    gate_acceptance: "pending",
  };
}

function createFixture(name) {
  const root = path.join(testRoot, name);
  rmSync(root, { recursive: true, force: true });
  mkdirSync(root, { recursive: true });
  const sourceCommit = currentCommit();
  assert.match(sourceCommit, /^[0-9a-f]{40}$/u);

  const candidatePath = path.join(root, "candidate.json");
  writeJson(candidatePath, {
    format: "pages_reference_consumer_gate_candidate_v1",
    status: "component_execution_passed_owner_review_pending",
    source_commit: sourceCommit,
    deployment_image_digest: deploymentDigest,
    source_sha256: sourceHashes(candidateContract),
    inputs: {
      artifact_http: packetRecord(),
      browser: packetRecord(),
      rollout_matrix: packetRecord(),
      rollout_feature_preflight: packetRecord(),
    },
    source_guards: commandResults(candidateContract.source_guards),
    focused_tests: commandResults(candidateContract.focused_tests),
    candidate: candidateResult(),
    boundaries: {
      canonical_source_mutated: false,
      gate_accepted: false,
      forum_wave_accepted: false,
      ffa_promoted: false,
      fba_promoted: false,
    },
    privacy: {
      tenant_id_persisted: false,
      actor_id_persisted: false,
      raw_stdout_persisted: false,
      raw_stderr_persisted: false,
    },
  });

  const observedPath = path.join(root, "observed-health-acceptance.json");
  writeJson(observedPath, {
    format: "pages_builder_provider_health_observed_acceptance_v1",
    status: "owner_accepted_observed_runtime_evidence_gate_review_pending",
    source_commit: sourceCommit,
    deployment: {
      deployment_id: deploymentId,
      deployment_image_digest: deploymentDigest,
    },
    source_files: sourceHashes(observedSource),
    decision: {
      value: "accept_observed_runtime_evidence",
      owner_id: "synthetic-provider-health-owner",
      owner_identity_is_operator_assertion: true,
      cryptographic_signature_present: false,
      free_text_reason_retained: false,
    },
    observed_health: {
      historical_lease_deadline_only: true,
      current_provider_health_asserted: false,
      snapshot: {
        state: "ready",
        degradation_reasons: [],
      },
      slo_evaluation: {
        ready: true,
        degradation_reasons: [],
        sample_floor_satisfied: true,
      },
    },
    binding_lineage: {
      live_binding_action: "unchanged",
      server_binding_authorized_by_this_packet: false,
      health_lease_extended: false,
    },
    gate: {
      eligible_for_pages_gate_review: true,
      pages_reference_consumer_gate_accepted: false,
      automatic_gate_acceptance: false,
      reference_gate_owner_signoff_satisfied: false,
      reference_gate_rollback_decision_satisfied: false,
    },
    pages_reference_consumer_gate_accepted: false,
    forum_wave_accepted: false,
    ffa_promoted: false,
    fba_promoted: false,
  });

  return {
    root,
    candidatePath,
    observedPath,
    outputPath: path.join(root, "gate-decision.json"),
    sourceCommit,
  };
}

function runFixture(
  fixture,
  decision = "accept_pages_reference_consumer_gate",
  rollbackDecision = "retain_reference_consumer_candidate",
) {
  return spawnSync(
    process.execPath,
    [
      runner,
      "--candidate",
      fixture.candidatePath,
      "--observed-health-acceptance",
      fixture.observedPath,
      "--owner-id",
      "synthetic-pages-gate-owner",
      "--decision",
      decision,
      "--rollback-decision",
      rollbackDecision,
      "--output",
      fixture.outputPath,
    ],
    { cwd: repoRoot, encoding: "utf8" },
  );
}

function expectSuccess(name, decision, rollbackDecision, expectedStatus, accepted) {
  const fixture = createFixture(name);
  const result = runFixture(fixture, decision, rollbackDecision);
  assert.equal(result.status, 0, result.stderr);
  const output = JSON.parse(readFileSync(fixture.outputPath, "utf8"));
  assert.equal(output.format, "pages_reference_consumer_gate_acceptance_v1");
  assert.equal(output.status, expectedStatus);
  assert.equal(output.source_commit, fixture.sourceCommit);
  assert.equal(output.deployment.deployment_image_digest, deploymentDigest);
  assert.equal(output.decision.value, decision);
  assert.equal(output.rollback_decision.value, rollbackDecision);
  assert.equal(output.rollback_decision.rollback_action_performed, false);
  assert.equal(output.evidence.candidate_provider_health, "unobserved");
  assert.equal(output.evidence.current_provider_health_asserted, false);
  assert.equal(output.evidence.provider_health_lease_extended, false);
  assert.equal(output.gate.accepted, accepted);
  assert.equal(output.boundaries.canonical_source_mutated, false);
  assert.equal(output.boundaries.rollback_action_executed, false);
  assert.equal(output.boundaries.forum_wave_accepted, false);
  assert.equal(output.boundaries.automatic_downstream_promotion, false);
}

function expectFailure(name, mutate, expectedMessage, decision, rollbackDecision) {
  const fixture = createFixture(name);
  mutate(fixture);
  const result = runFixture(
    fixture,
    decision ?? "accept_pages_reference_consumer_gate",
    rollbackDecision ?? "retain_reference_consumer_candidate",
  );
  assert.notEqual(result.status, 0, `${name} unexpectedly succeeded`);
  assert.match(result.stderr, expectedMessage);
  let outputExists = true;
  try {
    readFileSync(fixture.outputPath);
  } catch {
    outputExists = false;
  }
  assert.equal(outputExists, false, `${name} retained a decision output after failure`);
}

function rewriteCandidate(fixture, mutate) {
  const document = JSON.parse(readFileSync(fixture.candidatePath, "utf8"));
  mutate(document);
  writeJson(fixture.candidatePath, document);
}

function rewriteObserved(fixture, mutate) {
  const document = JSON.parse(readFileSync(fixture.observedPath, "utf8"));
  mutate(document);
  writeJson(fixture.observedPath, document);
}

rmSync(testRoot, { recursive: true, force: true });
mkdirSync(testRoot, { recursive: true });

try {
  expectSuccess(
    "accept-valid",
    "accept_pages_reference_consumer_gate",
    "retain_reference_consumer_candidate",
    "owner_accepted_pages_reference_consumer_gate",
    true,
  );
  expectSuccess(
    "reject-valid",
    "reject",
    "rollback_reference_consumer_candidate",
    "owner_rejected_pages_reference_consumer_gate",
    false,
  );

  expectFailure(
    "accept-rollback-mismatch",
    () => {},
    /accepted Pages gate requires retain_reference_consumer_candidate rollback decision/u,
    "accept_pages_reference_consumer_gate",
    "rollback_reference_consumer_candidate",
  );

  expectFailure(
    "candidate-source-hash-tamper",
    (fixture) =>
      rewriteCandidate(fixture, (candidate) => {
        const [firstPath] = Object.keys(candidate.source_sha256);
        candidate.source_sha256[firstPath] = "0".repeat(64);
      }),
    /reference candidate source hash .* does not match checkout/u,
  );

  expectFailure(
    "candidate-command-drift",
    (fixture) =>
      rewriteCandidate(fixture, (candidate) => {
        candidate.source_guards[0].args = ["unexpected.mjs"];
      }),
    /id\/program\/argv differs from execution contract/u,
  );

  expectFailure(
    "candidate-provider-health-overclaim",
    (fixture) =>
      rewriteCandidate(fixture, (candidate) => {
        candidate.candidate.provider_health = "ready";
      }),
    /reference candidate provider_health must remain unobserved/u,
  );

  expectFailure(
    "observed-deployment-digest-mismatch",
    (fixture) =>
      rewriteObserved(fixture, (observed) => {
        observed.deployment.deployment_image_digest =
          `ghcr.io/rustok/page-builder@sha256:${"e".repeat(64)}`;
      }),
    /observed-health acceptance deployment digest differs from reference candidate/u,
  );

  expectFailure(
    "observed-current-health-overclaim",
    (fixture) =>
      rewriteObserved(fixture, (observed) => {
        observed.observed_health.current_provider_health_asserted = true;
      }),
    /observed-health acceptance must remain retrospective/u,
  );

  expectFailure(
    "observed-gate-eligibility-revoked",
    (fixture) =>
      rewriteObserved(fixture, (observed) => {
        observed.gate.eligible_for_pages_gate_review = false;
      }),
    /observed-health acceptance gate boundary drifted/u,
  );

  console.log(
    "Pages reference-consumer gate owner runner tests passed: accept, reject, rollback mismatch, source-hash tamper, command drift, candidate health overclaim, RepoDigest mismatch, current-health overclaim, gate eligibility revoke.",
  );
} finally {
  rmSync(testRoot, { recursive: true, force: true });
}
