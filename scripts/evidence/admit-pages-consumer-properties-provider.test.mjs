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
const runnerPath = path.join(repoRoot, "scripts/evidence/admit-pages-consumer-properties-provider.mjs");
const contractPath = path.join(
  repoRoot,
  "crates/rustok-pages/contracts/evidence/pages-consumer-properties-provider-admission-source.json",
);
const targetRoot = path.join(repoRoot, "target");
const readJson = (location) => JSON.parse(readFileSync(location, "utf8"));
const sha256 = (value) => createHash("sha256").update(value).digest("hex");
const admissionContract = readJson(contractPath);
const rustContract = readJson(path.join(repoRoot, admissionContract.rust_receipt_input.source_contract));
const browserContract = readJson(path.join(repoRoot, admissionContract.browser_input.source_contract));
const deploymentContract = readJson(
  path.join(repoRoot, admissionContract.deployment_identity_input.source_contract),
);
const DEPLOYMENT_DIGEST = `ghcr.io/rustok/server@sha256:${"1".repeat(64)}`;
const OTHER_DEPLOYMENT_DIGEST = `ghcr.io/rustok/server@sha256:${"2".repeat(64)}`;
const FIXTURE_HASH = sha256("pages-consumer-properties-provider-admission-fixture");
const ISO_TIME = "2026-08-18T00:00:00.000Z";

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

function targetRecord(specification) {
  const absolute = path.join(repoRoot, specification.path);
  return {
    path: specification.path,
    status_before: specification.path.endsWith("page-builder-fba-registry.json")
      ? "boundary_ready"
      : "metadata_surface_cutover_complete",
    sha256: sha256(readFileSync(absolute)),
    json_pointer: specification.executed_evidence_json_pointer ?? specification.json_pointer,
    before: specification.required_before_value ?? specification.required_before,
  };
}

function buildRustReceipt() {
  return {
    format: admissionContract.rust_receipt_input.format,
    status: admissionContract.rust_receipt_input.required_status,
    generated_at: ISO_TIME,
    source_commit: HEAD,
    provenance: {
      repository: admissionContract.rust_receipt_input.canonical_repository,
      workflow: admissionContract.rust_receipt_input.canonical_workflow,
      run_id: "1",
      run_attempt: "1",
      event_name: admissionContract.rust_receipt_input.required_event,
      head_branch: admissionContract.rust_receipt_input.required_branch,
      github_actions: true,
      cryptographic_ci_attestation_claimed: false,
    },
    targets: {
      consumer_contract: targetRecord(rustContract.consumer_contract),
      fba_registry: targetRecord(rustContract.fba_registry),
    },
    execution: {
      test_list_command: rustContract.execution.test_list_command,
      required_test_name_fragments: rustContract.execution.required_test_name_fragments,
      verifier_commands: rustContract.execution.verifier_commands,
      test_commands: rustContract.execution.test_commands,
      check_command: rustContract.execution.check_command,
      all_commands_passed: true,
      packet_generated_only_after_test_and_check_steps: true,
      network_runtime_under_test: false,
      database_used: false,
      browser_used: false,
      browser_evidence_pending: true,
    },
    source_sha256: sourceHashes(rustContract),
    governance: {
      consumer_contract_mutated: false,
      fba_registry_mutated: false,
      executed_evidence_cleared: false,
      browser_execution_claimed: false,
      deployment_provenance_verified: false,
      terminal_inventory_complete_claimed: false,
      owner_approval_claimed: false,
      platform_approval_claimed: false,
      pages_ffa_promoted: false,
      page_builder_fba_promoted: false,
      later_admission_must_bind_rust_browser_and_source_lineage: true,
    },
    privacy: {
      raw_test_logs_embedded: false,
      tenant_identity_retained: false,
      credentials_or_cookies_retained: false,
      raw_graphql_or_browser_payload_retained: false,
    },
  };
}

function browserObservation(facts) {
  return { passed: true, criticalFailures: 0, facts };
}

function buildBrowserEvidence() {
  return {
    format: admissionContract.browser_input.format,
    status: admissionContract.browser_input.required_status,
    source_commit: HEAD,
    deployment_digest: DEPLOYMENT_DIGEST,
    node_version: process.version,
    playwright_version: "synthetic",
    source_files: sourceHashes(browserContract),
    input_records: {
      editor_storage_state: { bytes: 16, sha256: FIXTURE_HASH },
      profile_url_sha256: Object.fromEntries(
        browserContract.profiles.map((profile) => [profile, FIXTURE_HASH]),
      ),
    },
    observations: {
      published: browserObservation({
        registered_surface_visible: true,
        published_only_admission: true,
        fly_canvas_unmounted: true,
        document_authoring_unmounted: true,
        registered_runtime_present: true,
        owner_port_persistence_declared: true,
        registered_property_panel_ready: true,
        save_action_available_without_mutation: true,
      }),
      draft: browserObservation({
        registered_published_surface_absent: true,
        metadata_surface_error_absent: true,
      }),
      archived: browserObservation({
        registered_published_surface_absent: true,
        metadata_surface_error_absent: true,
      }),
      missing: browserObservation({
        registered_published_surface_absent: true,
        metadata_surface_error_absent: true,
      }),
    },
    retained_secrets: false,
    metadata_values_retained: false,
    browser_execution_only: true,
    consumer_properties_admission_pending: true,
    executed_at: ISO_TIME,
  };
}

function buildDeploymentIdentity() {
  return {
    format: admissionContract.deployment_identity_input.format,
    status: admissionContract.deployment_identity_input.required_status,
    captured_at: ISO_TIME,
    deployment: {
      deployment_id: "synthetic-pages-consumer-properties",
      deployment_image_digest: DEPLOYMENT_DIGEST,
      source_commit: HEAD,
      inventory_complete: true,
      expected_target_count: 1,
      verified_target_count: 1,
      origin_to_repo_digest_binding: "maintainer_reviewed_external_fact",
      cryptographic_origin_to_repo_digest_binding: false,
    },
    expected_targets: [
      {
        target_id: "synthetic-target",
        metrics_url_bytes: 16,
        metrics_url_sha256: FIXTURE_HASH,
        raw_metrics_url_persisted: false,
        status: 200,
        response_bytes: 16,
        response_sha256: FIXTURE_HASH,
        raw_response_persisted: false,
        reported_source_commit: HEAD,
        source_commit_verified_equal_checkout: true,
      },
    ],
    source_files: sourceHashes(deploymentContract),
    credentials: { environment_names: [], values_persisted: false },
    privacy: {
      raw_metrics_urls_persisted: false,
      raw_metrics_responses_persisted: false,
      credential_values_persisted: false,
      tenant_page_revision_or_correlation_ids_persisted: false,
    },
    prometheus_backend_query_executed: false,
    provider_health_snapshot_evaluated: false,
    pages_provider_health_observed: false,
    pages_reference_consumer_gate_accepted: false,
    forum_wave_accepted: false,
    ffa_promoted: false,
    fba_promoted: false,
  };
}

function buildFixture() {
  return {
    rust: buildRustReceipt(),
    browser: buildBrowserEvidence(),
    deployment: buildDeploymentIdentity(),
  };
}

function writeJson(location, value) {
  writeFileSync(location, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function executeCase(name, mutate = null) {
  const fixture = buildFixture();
  if (mutate) mutate(fixture);
  mkdirSync(targetRoot, { recursive: true });
  const directory = mkdtempSync(
    path.join(targetRoot, `pages-consumer-properties-provider-admission-${name}-`),
  );
  const rustPath = path.join(directory, "rust.json");
  const browserPath = path.join(directory, "browser.json");
  const deploymentPath = path.join(directory, "deployment.json");
  const outputPath = path.join(directory, "admission.json");
  writeJson(rustPath, fixture.rust);
  writeJson(browserPath, fixture.browser);
  writeJson(deploymentPath, fixture.deployment);
  const result = spawnSync(
    process.execPath,
    [
      runnerPath,
      "--rust-receipt",
      rustPath,
      "--browser-evidence",
      browserPath,
      "--deployment-identity",
      deploymentPath,
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
  return { directory, outputPath, result };
}

function passCase() {
  const { directory, outputPath, result } = executeCase("pass");
  try {
    assert.equal(result.status, 0, result.stderr);
    const output = readJson(outputPath);
    assert.equal(output.format, admissionContract.output.format);
    assert.equal(output.status, admissionContract.output.status);
    assert.equal(output.checkout_commit, HEAD);
    assert.equal(output.lineage.rust_source_commit, HEAD);
    assert.equal(output.lineage.browser_and_deployment_source_commit, HEAD);
    assert.equal(output.deployment.deployment_image_digest, DEPLOYMENT_DIGEST);
    assert.equal(output.deployment.source_commit_verified_on_all_expected_targets, true);
    assert.equal(output.targets.consumer_contract.before, "pending");
    assert.equal(output.targets.fba_registry.before, "pending");
    assert.equal(output.boundaries.consumer_contract_mutated, false);
    assert.equal(output.boundaries.fba_registry_mutated, false);
    assert.equal(output.boundaries.executed_evidence_changed, false);
    assert.equal(output.boundaries.separate_evidence_containing_update_required, true);
    assert.equal("rustPath" in output, false);
    assert.equal("browserPath" in output, false);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

function failCase(name, mutate, expected) {
  const { directory, result } = executeCase(name, mutate);
  try {
    assert.notEqual(result.status, 0, `${name} unexpectedly passed`);
    assert.match(`${result.stderr}${result.stdout}`, expected);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

passCase();

failCase(
  "rust-source-hash",
  (fixture) => {
    const key = Object.keys(fixture.rust.source_sha256)[0];
    fixture.rust.source_sha256[key] = "0".repeat(64);
  },
  /Rust receipt source hash .* does not match checkout/u,
);

failCase(
  "rust-pr-provenance",
  (fixture) => {
    fixture.rust.provenance.event_name = "pull_request";
  },
  /Rust receipt GitHub Actions provenance drifted/u,
);

failCase(
  "browser-digest",
  (fixture) => {
    fixture.browser.deployment_digest = OTHER_DEPLOYMENT_DIGEST;
  },
  /deployment image digest differs from browser packet/u,
);

failCase(
  "browser-profile",
  (fixture) => {
    fixture.browser.observations.draft.passed = false;
  },
  /draft browser observation did not pass cleanly/u,
);

failCase(
  "deployment-incomplete",
  (fixture) => {
    fixture.deployment.deployment.verified_target_count = 0;
  },
  /deployment identity target counts are incomplete/u,
);

failCase(
  "deployment-target-source",
  (fixture) => {
    fixture.deployment.expected_targets[0].source_commit_verified_equal_checkout = false;
  },
  /deployment target synthetic-target source verification drifted/u,
);

failCase(
  "deployment-privacy",
  (fixture) => {
    fixture.deployment.privacy.raw_metrics_urls_persisted = true;
  },
  /deployment identity privacy raw_metrics_urls_persisted must remain false/u,
);

failCase(
  "promotion-boundary",
  (fixture) => {
    fixture.deployment.fba_promoted = true;
  },
  /deployment identity boundary fba_promoted must remain false/u,
);

console.log(
  "[admit-pages-consumer-properties-provider.test] PASS positive=1 fail_closed=8",
);
