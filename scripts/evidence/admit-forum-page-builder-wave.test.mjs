#!/usr/bin/env node

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const runnerPath = path.join(repoRoot, "scripts/evidence/admit-forum-page-builder-wave.mjs");
const targetRoot = path.join(repoRoot, "target");
const admissionContractPath = path.join(
  repoRoot,
  "crates/rustok-forum/contracts/evidence/forum-page-builder-wave-admission-source.json",
);
const gateContractPath = path.join(
  repoRoot,
  "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-acceptance-source.json",
);
const browserContractPath = path.join(
  repoRoot,
  "crates/rustok-forum/contracts/evidence/forum-page-builder-browser-execution-contract.json",
);
const runtimeContractPath = path.join(
  repoRoot,
  "crates/rustok-forum/contracts/evidence/forum-page-builder-runtime-authorization-execution-contract.json",
);
const serverfnContractPath = path.join(
  repoRoot,
  "crates/rustok-forum/contracts/evidence/forum-page-builder-serverfn-deployment-attestation-contract.json",
);

const sha256 = (value) => createHash("sha256").update(value).digest("hex");
const readJson = (location) => JSON.parse(readFileSync(location, "utf8"));
const admissionContract = readJson(admissionContractPath);
const gateContract = readJson(gateContractPath);
const browserContract = readJson(browserContractPath);
const runtimeContract = readJson(runtimeContractPath);
const serverfnContract = readJson(serverfnContractPath);
const EMPTY_HASH = sha256(Buffer.alloc(0));
const FIXTURE_HASH = sha256(Buffer.from("forum-wave-admission-runner-fixture", "utf8"));
const DEPLOYMENT_DIGEST = `ghcr.io/rustok/server@sha256:${"1".repeat(64)}`;
const OTHER_DEPLOYMENT_DIGEST = `ghcr.io/rustok/server@sha256:${"2".repeat(64)}`;
const ISO_TIME = "2026-08-12T00:00:00.000Z";

function currentCommit() {
  const result = spawnSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
    shell: false,
  });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout.trim();
}

const HEAD = currentCommit();

function sourceHashes(contract) {
  return Object.fromEntries(
    [...contract.required_source_files]
      .sort()
      .map((relativePath) => [
        relativePath,
        sha256(readFileSync(path.join(repoRoot, relativePath))),
      ]),
  );
}

function browserObservation(facts) {
  return {
    passed: true,
    criticalFailures: 0,
    facts,
  };
}

function buildPagesGate() {
  return {
    format: admissionContract.pages_gate_input.format,
    status: admissionContract.pages_gate_input.required_status,
    decided_at: ISO_TIME,
    source_commit: HEAD,
    deployment: {
      deployment_id: "synthetic-forum-wave-test",
      deployment_image_digest: DEPLOYMENT_DIGEST,
    },
    decision: {
      value: admissionContract.pages_gate_input.required_decision,
      owner_id: "synthetic-owner",
      owner_identity_is_operator_assertion: true,
      cryptographic_signature_present: false,
      free_text_reason_retained: false,
    },
    rollback_decision: {
      value: admissionContract.pages_gate_input.required_rollback_decision,
      rollback_action_performed: false,
    },
    gate: {
      id: "pages_reference_consumer_gate",
      accepted: true,
      owner_signoff_satisfied: true,
      rollback_decision_satisfied: true,
      exact_source_commit_bound: true,
      exact_deployment_digest_bound: true,
      candidate_and_observed_health_chain_bound: true,
    },
    boundaries: {
      canonical_source_mutated: false,
      rollback_action_executed: false,
      forum_wave_accepted: false,
      ffa_promoted: false,
      fba_promoted: false,
      automatic_downstream_promotion: false,
    },
    source_files: sourceHashes(gateContract),
  };
}

function buildBrowserEvidence() {
  return {
    format: admissionContract.forum_browser_input.format,
    status: admissionContract.forum_browser_input.required_status,
    source_commit: HEAD,
    deployment_digest: DEPLOYMENT_DIGEST,
    executed_at: ISO_TIME,
    source_files: sourceHashes(browserContract),
    observations: {
      full: browserObservation({
        topic_list_admitted: true,
        invalid_owner_props_rejected: true,
        owner_normalization_observed: true,
        fly_undo_observed: true,
        fly_redo_observed: true,
        owner_preview_ready: true,
        pages_save_completed: true,
      }),
      preview_off: browserObservation({
        topic_list_admitted: true,
        owner_properties_actionable: true,
        owner_preview_not_admitted: true,
      }),
      properties_off: browserObservation({
        topic_list_not_admitted: true,
        owner_properties_not_actionable: true,
      }),
      forum_disabled: browserObservation({
        topic_list_absent: true,
        owner_property_panel_absent: true,
        owner_preview_panel_absent: true,
      }),
      no_read: browserObservation({
        topic_list_not_admitted: true,
        owner_properties_not_actionable: true,
      }),
    },
    retained_secrets: false,
    browser_execution_only: true,
    runtime_authorization_evidence_pending: true,
    observed_page_builder_wave_pending: true,
    input_records: {
      editor_storage_state: { bytes: 16, sha256: FIXTURE_HASH },
      no_read_storage_state: { bytes: 16, sha256: FIXTURE_HASH },
      profile_url_sha256: Object.fromEntries(
        browserContract.profiles.map((profile) => [profile, FIXTURE_HASH]),
      ),
    },
  };
}

function buildRuntimeEvidence() {
  return {
    format: admissionContract.forum_runtime_authorization_input.format,
    status: admissionContract.forum_runtime_authorization_input.required_status,
    source_commit: HEAD,
    executed_at: ISO_TIME,
    source_files: sourceHashes(runtimeContract),
    commands: runtimeContract.commands.map((command) => ({
      id: command.id,
      program: command.program,
      args: [...command.args],
      status: 0,
      stdout: { bytes: 0, sha256: EMPTY_HASH },
      stderr: { bytes: 0, sha256: EMPTY_HASH },
    })),
    retained_raw_command_output: false,
    runtime_authorization_execution_only: true,
    deployed_server_fn_attestation_not_claimed: true,
    browser_execution_not_claimed: true,
    provider_slo_health_not_claimed: true,
    observed_page_builder_wave_pending: true,
  };
}

function buildServerfnAttestation() {
  return {
    format: admissionContract.forum_serverfn_attestation_input.format,
    status: admissionContract.forum_serverfn_attestation_input.required_status,
    source_commit: HEAD,
    live_server_source_commit_verified_equal_checkout: true,
    captured_at: ISO_TIME,
    target: {
      deployment_image_digest: DEPLOYMENT_DIGEST,
      origin_sha256: FIXTURE_HASH,
      origin_bytes: 32,
      raw_origin_persisted: false,
      origin_to_repo_digest_binding: "maintainer_reviewed_external_fact",
      cryptographic_origin_to_repo_digest_binding: false,
    },
    source_files: sourceHashes(serverfnContract),
    scenarios: serverfnContract.scenarios.map((scenario) => ({
      id: scenario.id,
      status: scenario.expected_status ?? 403,
      credential_values_persisted: false,
      raw_body_persisted: false,
      body_bytes: 0,
      body_sha256: EMPTY_HASH,
    })),
    privacy: {
      credential_environment_names_only: true,
      credential_values_persisted: false,
      common_header_values_persisted: false,
      raw_response_bodies_persisted: false,
      tenant_or_actor_identifiers_persisted: false,
      forum_content_persisted: false,
    },
    browser_execution_not_claimed: true,
    runtime_authorization_execution_not_claimed: true,
    provider_slo_health_not_claimed: true,
    observed_page_builder_wave_pending: true,
  };
}

function buildFixture() {
  return {
    gate: buildPagesGate(),
    browser: buildBrowserEvidence(),
    runtime: buildRuntimeEvidence(),
    serverfn: buildServerfnAttestation(),
  };
}

function writeJson(location, value) {
  writeFileSync(location, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function runCase(name, mutate, expectedFailure) {
  const fixture = buildFixture();
  if (mutate) mutate(fixture);
  mkdirSync(targetRoot, { recursive: true });
  const directory = mkdtempSync(path.join(targetRoot, `forum-wave-admission-runner-${name}-`));
  const gatePath = path.join(directory, "pages-gate.json");
  const browserPath = path.join(directory, "forum-browser.json");
  const runtimePath = path.join(directory, "forum-runtime.json");
  const serverfnPath = path.join(directory, "forum-serverfn.json");
  const outputPath = path.join(directory, "admission.json");
  try {
    writeJson(gatePath, fixture.gate);
    writeJson(browserPath, fixture.browser);
    writeJson(runtimePath, fixture.runtime);
    writeJson(serverfnPath, fixture.serverfn);
    const result = spawnSync(
      process.execPath,
      [
        runnerPath,
        "--pages-gate",
        gatePath,
        "--browser-evidence",
        browserPath,
        "--runtime-evidence",
        runtimePath,
        "--serverfn-attestation",
        serverfnPath,
        "--output",
        outputPath,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        shell: false,
        maxBuffer: 8 * 1024 * 1024,
      },
    );

    if (expectedFailure) {
      assert.notEqual(result.status, 0, `${name}: runner unexpectedly accepted fixture`);
      assert.ok(
        result.stderr.includes(expectedFailure),
        `${name}: expected failure containing ${JSON.stringify(expectedFailure)}, got ${JSON.stringify(result.stderr)}`,
      );
      return null;
    }

    assert.equal(result.status, 0, `${name}: ${result.stderr}`);
    const output = readJson(outputPath);
    assert.equal(output.format, admissionContract.output.format);
    assert.equal(output.status, admissionContract.output.status);
    assert.equal(output.source_commit, HEAD);
    assert.equal(output.deployment.deployment_id, fixture.gate.deployment.deployment_id);
    assert.equal(output.deployment.deployment_image_digest, DEPLOYMENT_DIGEST);
    assert.equal(output.admission.pages_reference_consumer_gate_accepted, true);
    assert.equal(output.admission.exact_source_commit_bound, true);
    assert.equal(output.admission.exact_deployment_digest_bound, true);
    assert.equal(output.admission.forum_browser_execution_passed, true);
    assert.equal(output.admission.forum_runtime_authorization_execution_passed, true);
    assert.equal(output.admission.forum_server_fn_deployment_attestation_passed, true);
    assert.equal(output.admission.observed_control_plane_wave_pending, true);
    assert.equal(output.boundaries.observed_control_plane_wave_executed, false);
    assert.equal(output.boundaries.forum_wave_accepted, false);
    assert.equal(output.boundaries.current_provider_health_asserted, false);
    assert.equal(output.boundaries.cryptographic_deployment_binding_claimed, false);
    assert.equal(output.privacy.raw_input_paths_persisted, false);
    assert.equal(output.privacy.raw_http_or_browser_bodies_persisted, false);
    assert.equal(output.privacy.raw_command_output_persisted, false);
    assert.deepEqual(output.source_files, sourceHashes(admissionContract));
    assert.equal(output.inputs.pages_gate.sha256, sha256(readFileSync(gatePath)));
    assert.equal(output.inputs.forum_browser.sha256, sha256(readFileSync(browserPath)));
    assert.equal(output.inputs.forum_runtime_authorization.sha256, sha256(readFileSync(runtimePath)));
    assert.equal(output.inputs.forum_serverfn_attestation.sha256, sha256(readFileSync(serverfnPath)));
    assert.equal(output.inputs.raw_input_paths_persisted, false);
    return output;
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

runCase("valid", null, null);
runCase(
  "gate-not-accepted",
  ({ gate }) => {
    gate.gate.accepted = false;
  },
  "Pages reference-consumer gate is not fully accepted and bound",
);
runCase(
  "gate-promotion-overclaim",
  ({ gate }) => {
    gate.boundaries.forum_wave_accepted = true;
  },
  "Pages gate boundary forum_wave_accepted must remain false",
);
runCase(
  "browser-digest-mismatch",
  ({ browser }) => {
    browser.deployment_digest = OTHER_DEPLOYMENT_DIGEST;
  },
  "Forum browser deployment digest differs from accepted Pages gate",
);
runCase(
  "browser-fact-missing",
  ({ browser }) => {
    browser.observations.full.facts.pages_save_completed = false;
  },
  "Forum browser fact pages_save_completed must be true",
);
runCase(
  "runtime-command-drift",
  ({ runtime }) => {
    runtime.commands[0].args = [...runtime.commands[0].args, "--synthetic-drift"];
  },
  "Forum runtime-authorization commands[0] id/program/argv/status drifted",
);
runCase(
  "runtime-source-hash-tamper",
  ({ runtime }) => {
    const firstPath = [...runtimeContract.required_source_files].sort()[0];
    runtime.source_files[firstPath] = "0".repeat(64);
  },
  "does not match checkout",
);
runCase(
  "serverfn-live-commit-unverified",
  ({ serverfn }) => {
    serverfn.live_server_source_commit_verified_equal_checkout = false;
  },
  "Forum server-function packet did not verify live source commit against checkout",
);
runCase(
  "serverfn-privacy-overclaim",
  ({ serverfn }) => {
    serverfn.privacy.credential_values_persisted = true;
  },
  "Forum server-function privacy boundary drifted",
);
runCase(
  "serverfn-cryptographic-overclaim",
  ({ serverfn }) => {
    serverfn.target.cryptographic_origin_to_repo_digest_binding = true;
  },
  "Forum server-function target identity/privacy boundary drifted",
);

console.log(
  "Forum Page Builder Wave admission runner tests passed: valid admission plus gate, digest, browser-fact, runtime-command/source-hash, server-fn live-commit/privacy/provenance rejection cases.",
);
