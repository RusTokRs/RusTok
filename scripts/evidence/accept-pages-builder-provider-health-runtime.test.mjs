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
const runner = path.join(repoRoot, "scripts/evidence/accept-pages-builder-provider-health-runtime.mjs");
const runtimeContract = readJson(
  "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-runtime-execution-contract.json",
);
const identitySource = readJson(
  "crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-identity-source.json",
);
const evaluatorSource = readJson(
  "crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-evaluator-source.json",
);
const bindingSource = readJson(
  "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-owner-acceptance-source.json",
);
const testRoot = path.join(repoRoot, "target/pages-builder-provider-health-owner-runner-tests");
const deploymentId = "synthetic-provider-health-owner-test";
const deploymentDigest = `ghcr.io/rustok/page-builder@sha256:${"a".repeat(64)}`;
const bodySha = "b".repeat(64);
const postBodySha = "c".repeat(64);
const ssrBodySha = "d".repeat(64);

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

function packetRecord(location) {
  const bytes = readFileSync(location);
  return { bytes: statSync(location).size, sha256: sha256(bytes) };
}

function allowedCapability(capability) {
  return { capability, allowed: true, error_kind: null, error_code: null };
}

function readyRuntimeObservations() {
  return {
    graphql: {
      status: 200,
      response_body_bytes: 64,
      response_body_sha256: bodySha,
      configured_rollout_all_on: true,
      provider_health_observed: true,
      provider_state: "ready",
      preview: allowedCapability("preview"),
      properties: allowedCapability("properties"),
      publish: allowedCapability("publish"),
      raw_request_or_response_persisted: false,
    },
    workspace: {
      provider_control_state: "ready",
      provider_health: "ready",
      preview_enabled: true,
      properties: "enabled",
      publish: "enabled",
    },
    authoritative_ssr_preview: {
      request_attempted: true,
      status: 200,
      response_body_bytes: 32,
      response_body_sha256: ssrBodySha,
      capability_disabled: false,
      mutation_possible: false,
      raw_request_or_response_persisted: false,
    },
    standalone_browser_intent: [],
    graphql_after_consumers: {
      status: 200,
      response_body_bytes: 64,
      response_body_sha256: postBodySha,
      provider_health_still_observed: true,
    },
  };
}

function runtimeBoundaries() {
  return {
    exact_identity_evaluator_acceptance_chain_verified: true,
    accepted_packet_runtime_observed: true,
    configured_rollout_all_on: true,
    mismatched_page_id_protects_browser_intent_probe_if_health_revoked: true,
    rollout_settings_mutated: false,
    publish_mutation_executed: false,
    owner_observed_health_acceptance: false,
    pages_reference_consumer_gate_accepted: false,
    forum_wave_accepted: false,
    ffa_promoted: false,
    fba_promoted: false,
    canonical_source_mutated: false,
  };
}

function runtimePrivacy() {
  return {
    tenant_slug_or_id_persisted: false,
    page_id_persisted: false,
    authorization_or_cookie_values_persisted: false,
    storage_state_contents_persisted: false,
    tokens_or_session_ids_persisted: false,
    raw_graphql_bodies_persisted: false,
    raw_server_function_bodies_persisted: false,
    raw_evidence_paths_persisted: false,
    screenshots_persisted: false,
    videos_persisted: false,
    traces_persisted: false,
  };
}

function createFixture(name) {
  const root = path.join(testRoot, name);
  rmSync(root, { recursive: true, force: true });
  mkdirSync(root, { recursive: true });

  const sourceCommit = currentCommit();
  assert.match(sourceCommit, /^[0-9a-f]{40}$/u);
  const generatedAt = new Date(Date.now() - 1_000).toISOString();
  const healthValidUntil = new Date(Date.now() + 60 * 60 * 1_000).toISOString();
  const snapshot = {
    state: "ready",
    degradation_reasons: [],
    thresholds: {
      preview_p95_ms_max: 1500,
      publish_p95_ms_max: 3000,
      sanitize_failure_rate_max: 0.01,
      runtime_error_rate_max: 0.01,
    },
    observed: {
      preview_p95_ms: 120,
      publish_p95_ms: 240,
      sanitize_failure_rate: 0,
      runtime_error_rate: 0,
    },
  };
  const sloEvaluation = {
    ready: true,
    degradation_reasons: [],
    sample_floor_satisfied: true,
  };
  const deployment = {
    source_commit: sourceCommit,
    deployment_id: deploymentId,
    deployment_image_digest: deploymentDigest,
  };

  const identityPath = path.join(root, "identity.json");
  writeJson(identityPath, {
    format: "page_builder_provider_health_deployment_identity_v1",
    status: "deployment_identity_verified_health_evaluation_pending",
    deployment,
    source_files: sourceHashes(identitySource),
  });

  const evaluationPath = path.join(root, "evaluation.json");
  writeJson(evaluationPath, {
    format: "page_builder_provider_health_deployment_evaluation_v1",
    status: "deployment_health_evaluated_pages_binding_pending",
    deployment,
    snapshot,
    slo_evaluation: sloEvaluation,
    source_files: sourceHashes(evaluatorSource),
  });
  const evaluationRecord = packetRecord(evaluationPath);

  const bindingPath = path.join(root, "binding.json");
  writeJson(bindingPath, {
    format: "pages_builder_provider_health_owner_acceptance_v1",
    status: "owner_accepted_server_binding_pending",
    deployment,
    decision: {
      value: "accept_for_pages_binding",
      rollback_action: "restore_unobserved_provider_health",
      owner_identity_is_operator_assertion: true,
      cryptographic_signature_present: false,
      free_text_reason_retained: false,
    },
    evaluation: {
      evaluation_sha256: evaluationRecord.sha256,
      health_valid_until: healthValidUntil,
      snapshot,
      slo_evaluation: sloEvaluation,
    },
    binding: {
      server_binding_authorized: true,
      server_binding_performed: false,
      required_live_source_commit: sourceCommit,
      required_deployment_image_digest: deploymentDigest,
      failure_action: "restore_unobserved_provider_health",
    },
    source_files: sourceHashes(bindingSource),
  });

  const identityRecord = packetRecord(identityPath);
  const bindingRecord = packetRecord(bindingPath);
  const runtimePath = path.join(root, "runtime.json");
  writeJson(runtimePath, {
    format: "pages_builder_provider_health_runtime_evidence_v1",
    status: "observed_runtime_evidence_owner_review_pending",
    generated_at: generatedAt,
    source_commit: sourceCommit,
    deployment: {
      deployment_id: deploymentId,
      deployment_image_digest: deploymentDigest,
    },
    input_packets: {
      deployment_identity: identityRecord,
      deployment_evaluation: evaluationRecord,
      owner_acceptance: bindingRecord,
      raw_paths_persisted: false,
    },
    source_sha256: sourceHashes(runtimeContract),
    accepted_health: {
      health_valid_until: healthValidUntil,
      snapshot,
      slo_evaluation: sloEvaluation,
    },
    observations: readyRuntimeObservations(),
    boundaries: runtimeBoundaries(),
    privacy: runtimePrivacy(),
  });

  return {
    root,
    identityPath,
    evaluationPath,
    bindingPath,
    runtimePath,
    outputPath: path.join(root, "decision.json"),
    sourceCommit,
    healthValidUntil,
  };
}

function runFixture(fixture, decision = "accept_observed_runtime_evidence") {
  return spawnSync(
    process.execPath,
    [
      runner,
      "--runtime-evidence",
      fixture.runtimePath,
      "--identity",
      fixture.identityPath,
      "--evaluation",
      fixture.evaluationPath,
      "--binding-acceptance",
      fixture.bindingPath,
      "--owner-id",
      "synthetic-owner",
      "--decision",
      decision,
      "--output",
      fixture.outputPath,
    ],
    { cwd: repoRoot, encoding: "utf8" },
  );
}

function expectSuccess(name, decision, expectedStatus, eligibleForGate) {
  const fixture = createFixture(name);
  const result = runFixture(fixture, decision);
  assert.equal(result.status, 0, result.stderr);
  const output = JSON.parse(readFileSync(fixture.outputPath, "utf8"));
  assert.equal(output.format, "pages_builder_provider_health_observed_acceptance_v1");
  assert.equal(output.status, expectedStatus);
  assert.equal(output.source_commit, fixture.sourceCommit);
  assert.equal(output.deployment.deployment_image_digest, deploymentDigest);
  assert.equal(output.decision.value, decision);
  assert.equal(output.gate.eligible_for_pages_gate_review, eligibleForGate);
  assert.equal(output.gate.pages_reference_consumer_gate_accepted, false);
  assert.equal(output.observed_health.current_provider_health_asserted, false);
  assert.equal(output.binding_lineage.live_binding_action, "unchanged");
  assert.equal(output.binding_lineage.health_lease_extended, false);
  assert.equal(output.raw_input_paths_persisted, false);
}

function expectFailure(name, mutate, expectedMessage) {
  const fixture = createFixture(name);
  mutate(fixture);
  const result = runFixture(fixture);
  assert.notEqual(result.status, 0, `${name} unexpectedly succeeded`);
  assert.match(result.stderr, expectedMessage);
  assert.equal(
    (() => {
      try {
        readFileSync(fixture.outputPath);
        return true;
      } catch {
        return false;
      }
    })(),
    false,
    `${name} retained an acceptance output after failure`,
  );
}

function rewriteRuntime(fixture, mutate) {
  const document = JSON.parse(readFileSync(fixture.runtimePath, "utf8"));
  mutate(document);
  writeJson(fixture.runtimePath, document);
}

function rewriteBindingAndRefreshRuntimeHash(fixture, mutate) {
  const binding = JSON.parse(readFileSync(fixture.bindingPath, "utf8"));
  mutate(binding);
  const bindingRecord = writeJson(fixture.bindingPath, binding);
  rewriteRuntime(fixture, (runtime) => {
    runtime.input_packets.owner_acceptance = bindingRecord;
  });
}

rmSync(testRoot, { recursive: true, force: true });
mkdirSync(testRoot, { recursive: true });

try {
  expectSuccess(
    "accept-valid",
    "accept_observed_runtime_evidence",
    "owner_accepted_observed_runtime_evidence_gate_review_pending",
    true,
  );
  expectSuccess(
    "reject-valid",
    "reject",
    "owner_rejected_observed_runtime_evidence",
    false,
  );

  expectFailure(
    "runtime-source-hash-tamper",
    (fixture) =>
      rewriteRuntime(fixture, (runtime) => {
        const [firstPath] = Object.keys(runtime.source_sha256);
        runtime.source_sha256[firstPath] = "0".repeat(64);
      }),
    /runtime evidence source SHA .* does not match checkout/u,
  );

  expectFailure(
    "runtime-after-health-deadline",
    (fixture) =>
      rewriteRuntime(fixture, (runtime) => {
        runtime.generated_at = new Date(
          Date.parse(runtime.accepted_health.health_valid_until) + 10_000,
        ).toISOString();
      }),
    /runtime evidence was generated after its admitted health lease deadline/u,
  );

  expectFailure(
    "gate-overclaim",
    (fixture) =>
      rewriteRuntime(fixture, (runtime) => {
        runtime.boundaries.pages_reference_consumer_gate_accepted = true;
      }),
    /runtime boundary pages_reference_consumer_gate_accepted must be false/u,
  );

  expectFailure(
    "privacy-overclaim",
    (fixture) =>
      rewriteRuntime(fixture, (runtime) => {
        runtime.privacy.raw_evidence_paths_persisted = true;
      }),
    /runtime privacy flag raw_evidence_paths_persisted must be false/u,
  );

  expectFailure(
    "binding-repodigest-mismatch",
    (fixture) =>
      rewriteBindingAndRefreshRuntimeHash(fixture, (binding) => {
        binding.deployment.deployment_image_digest = `ghcr.io/rustok/page-builder@sha256:${"e".repeat(64)}`;
      }),
    /binding acceptance deployment identity differs from runtime evidence/u,
  );

  console.log(
    "Pages Page Builder observed-health owner acceptance runner tests passed: accept, reject, source-hash tamper, expired runtime, gate overclaim, privacy overclaim, RepoDigest mismatch.",
  );
} finally {
  rmSync(testRoot, { recursive: true, force: true });
}
