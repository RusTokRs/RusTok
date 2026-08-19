#!/usr/bin/env node

import { spawnSync } from "node:child_process";
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
const runnerPath = path.join(repoRoot, "scripts/evidence/admit-pages-consumer-properties.mjs");
const admissionContractPath =
  "crates/rustok-pages/contracts/evidence/pages-consumer-properties-admission-source.json";
const sourceContractPath =
  "crates/rustok-pages/contracts/evidence/pages-consumer-properties-source-execution.json";
const browserContractPath =
  "crates/rustok-pages/contracts/evidence/pages-published-metadata-browser-execution-contract.json";
const tempRoot = path.join(repoRoot, "target", `pages-consumer-properties-admission-test-${process.pid}`);

function fail(message) {
  throw new Error(`Pages consumer-properties admission synthetic test failed: ${message}`);
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function repoBytes(relativePath) {
  return readFileSync(path.join(repoRoot, relativePath));
}

function repoJson(relativePath) {
  return JSON.parse(repoBytes(relativePath).toString("utf8"));
}

function sourceHashes(contract) {
  return Object.fromEntries(
    contract.required_source_files.map((relativePath) => [relativePath, sha256(repoBytes(relativePath))]),
  );
}

function currentCommit() {
  const result = spawnSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
    shell: false,
  });
  if (result.status !== 0) fail("git rev-parse HEAD failed");
  const value = result.stdout.trim();
  if (!/^[0-9a-f]{40}$/u.test(value)) fail("HEAD is not canonical");
  return value;
}

function writeJson(name, document) {
  const location = path.join(tempRoot, name);
  writeFileSync(location, `${JSON.stringify(document, null, 2)}\n`, "utf8");
  return location;
}

function fileRecord(relativePath) {
  const bytes = repoBytes(relativePath);
  return { path: relativePath, bytes: bytes.length, sha256: sha256(bytes) };
}

function runAdmission(source, browser, deployment, outputName) {
  const sourcePath = writeJson(`${outputName}-source.json`, source);
  const browserPath = writeJson(`${outputName}-browser.json`, browser);
  const deploymentPath = writeJson(`${outputName}-deployment.json`, deployment);
  const outputPath = path.join(tempRoot, `${outputName}-output.json`);
  const result = spawnSync(
    process.execPath,
    [
      runnerPath,
      "--source-receipt",
      sourcePath,
      "--browser-evidence",
      browserPath,
      "--deployment-provenance",
      deploymentPath,
      "--output",
      outputPath,
    ],
    {
      cwd: repoRoot,
      encoding: "utf8",
      shell: false,
      maxBuffer: 4 * 1024 * 1024,
    },
  );
  return { result, outputPath };
}

function expectPass(label, source, browser, deployment, expectedSourceCommit) {
  const { result, outputPath } = runAdmission(source, browser, deployment, label);
  if (result.status !== 0) {
    fail(`${label} unexpectedly failed: ${result.stderr || result.stdout}`);
  }
  const output = JSON.parse(readFileSync(outputPath, "utf8"));
  if (
    output.format !== "pages_consumer_properties_admission_v1" ||
    output.status !== "consumer_properties_execution_evidence_admitted_registry_update_pending" ||
    output.source_receipt_commit !== expectedSourceCommit ||
    output.browser_deployment_source_commit !== currentCommit() ||
    output.admission?.source_receipt_ancestor_lineage_bound !== true ||
    output.admission?.source_receipt_required_sources_equal_current_checkout !== true ||
    output.admission?.browser_and_deployment_exact_source_commit_bound !== true ||
    output.admission?.registry_update_ready_for_later_evidence_containing_pr !== true ||
    output.boundaries?.consumer_contract_mutated !== false ||
    output.boundaries?.fba_registry_mutated !== false ||
    output.boundaries?.executed_evidence_verified !== false ||
    output.boundaries?.cryptographic_origin_to_repo_digest_binding_claimed !== false
  ) {
    fail(`${label} output boundary drifted`);
  }
}

function expectReject(label, source, browser, deployment) {
  const { result } = runAdmission(source, browser, deployment, label.replaceAll(" ", "-"));
  if (result.status === 0) fail(`${label} unexpectedly passed`);
}

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function jsonSha256(document) {
  return sha256(`${JSON.stringify(document, null, 2)}\n`);
}

function refreshPacketHashes(deployment, source, browser) {
  deployment.input_packet_sha256.source_receipt = jsonSha256(source);
  deployment.input_packet_sha256.browser_evidence = jsonSha256(browser);
}

function main() {
  rmSync(tempRoot, { recursive: true, force: true });
  mkdirSync(tempRoot, { recursive: true });
  try {
    const head = currentCommit();
    const admissionContract = repoJson(admissionContractPath);
    const sourceContract = repoJson(sourceContractPath);
    const browserContract = repoJson(browserContractPath);
    const consumer = fileRecord(admissionContract.target_preconditions.consumer_contract.path);
    const registry = fileRecord(admissionContract.target_preconditions.fba_registry.path);
    const deploymentDigest = `ghcr.io/rustokrs/rustok@sha256:${"a".repeat(64)}`;
    const routeHashes = Object.fromEntries(
      ["published", "draft", "archived", "missing"].map((profile) => [
        profile,
        sha256(`https://reviewed.example.invalid/${profile}`),
      ]),
    );

    const sourceReceipt = {
      format: sourceContract.output.format,
      status: sourceContract.output.success_status,
      generated_at: new Date(0).toISOString(),
      source_commit: head,
      provenance: {
        repository: "RusTokRs/RusTok",
        workflow: "Pages Consumer Properties Source Evidence",
        run_id: "32177516104",
        run_attempt: "1",
        event_name: "push",
        head_branch: "main",
        github_actions: true,
        cryptographic_ci_attestation_claimed: false,
      },
      targets: {
        consumer_contract: {
          path: admissionContract.target_preconditions.consumer_contract.path,
          status_before: admissionContract.target_preconditions.consumer_contract.required_status,
          sha256: consumer.sha256,
          json_pointer:
            admissionContract.target_preconditions.consumer_contract.executed_evidence_json_pointer,
          before: "pending",
        },
        fba_registry: {
          path: admissionContract.target_preconditions.fba_registry.path,
          status_before: admissionContract.target_preconditions.fba_registry.required_status,
          sha256: registry.sha256,
          json_pointer:
            admissionContract.target_preconditions.fba_registry.executed_evidence_json_pointer,
          before: "pending",
        },
      },
      execution: {
        test_list_command: sourceContract.execution.test_list_command,
        required_test_name_fragments: sourceContract.execution.required_test_name_fragments,
        verifier_commands: sourceContract.execution.verifier_commands,
        test_commands: sourceContract.execution.test_commands,
        check_command: sourceContract.execution.check_command,
        all_commands_passed: true,
        packet_generated_only_after_test_and_check_steps: true,
        network_runtime_under_test: false,
        database_used: false,
        browser_used: false,
        browser_evidence_pending: true,
      },
      source_sha256: sourceHashes(sourceContract),
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

    const hiddenObservation = {
      passed: true,
      criticalFailures: 0,
      facts: {
        registered_published_surface_absent: true,
        metadata_surface_error_absent: true,
      },
    };
    const browserPacket = {
      format: browserContract.output.format,
      status: browserContract.output.status,
      source_commit: head,
      deployment_digest: deploymentDigest,
      node_version: process.version,
      playwright_version: "synthetic",
      source_files: sourceHashes(browserContract),
      input_records: {
        editor_storage_state: {
          bytes: 128,
          sha256: sha256("synthetic-editor-storage-state"),
        },
        profile_url_sha256: routeHashes,
      },
      observations: {
        published: {
          passed: true,
          criticalFailures: 0,
          facts: {
            registered_surface_visible: true,
            published_only_admission: true,
            fly_canvas_unmounted: true,
            document_authoring_unmounted: true,
            registered_runtime_present: true,
            owner_port_persistence_declared: true,
            registered_property_panel_ready: true,
            save_action_available_without_mutation: true,
          },
        },
        draft: clone(hiddenObservation),
        archived: clone(hiddenObservation),
        missing: clone(hiddenObservation),
      },
      retained_secrets: false,
      metadata_values_retained: false,
      browser_execution_only: true,
      consumer_properties_admission_pending: true,
      executed_at: new Date(0).toISOString(),
    };

    const deploymentProvenance = {
      format: admissionContract.deployment_provenance_input.format,
      status: admissionContract.deployment_provenance_input.required_status,
      source_commit: head,
      deployment_image_digest: deploymentDigest,
      reviewed_at: new Date(0).toISOString(),
      input_packet_sha256: {
        source_receipt: jsonSha256(sourceReceipt),
        browser_evidence: jsonSha256(browserPacket),
      },
      review: {
        reviewer_id: "synthetic-reviewer",
        classification: "maintainer_reviewed_external_fact",
        source_commit_reviewed: true,
        deployment_image_digest_reviewed: true,
        browser_profile_route_hashes_reviewed: true,
        source_workflow_index_reviewed: true,
        browser_workflow_index_reviewed: true,
        cryptographic_signature_present: false,
      },
      workflow_evidence: {
        source: {
          context: "pages-consumer-properties-source-evidence-index",
          run_id: "32177516104",
          source_commit: head,
          status: "success",
        },
        browser: {
          context: "pages-published-metadata-browser-evidence-index",
          run_id: "32179999999",
          source_commit: head,
          status: "success",
        },
        exact_bound_commit_statuses_reviewed: true,
      },
      profile_url_sha256: routeHashes,
      binding: {
        origin_to_repo_digest: "maintainer_reviewed_external_fact",
        cryptographic_origin_to_repo_digest_binding: false,
        raw_profile_urls_retained: false,
        credentials_retained: false,
      },
    };

    expectPass("accepts exact head as valid ancestor lineage", sourceReceipt, browserPacket, deploymentProvenance, head);

    const nonAncestor = clone(sourceReceipt);
    nonAncestor.source_commit = "b".repeat(40);
    const nonAncestorProvenance = clone(deploymentProvenance);
    nonAncestorProvenance.workflow_evidence.source.source_commit = nonAncestor.source_commit;
    refreshPacketHashes(nonAncestorProvenance, nonAncestor, browserPacket);
    expectReject("rejects non ancestor source receipt", nonAncestor, browserPacket, nonAncestorProvenance);

    const staleSourceHash = clone(sourceReceipt);
    const stalePath = Object.keys(staleSourceHash.source_sha256)[0];
    staleSourceHash.source_sha256[stalePath] = "c".repeat(64);
    const staleSourceProvenance = clone(deploymentProvenance);
    refreshPacketHashes(staleSourceProvenance, staleSourceHash, browserPacket);
    expectReject("rejects stale source receipt hash", staleSourceHash, browserPacket, staleSourceProvenance);

    const browserCommitDrift = clone(browserPacket);
    browserCommitDrift.source_commit = "d".repeat(40);
    const browserCommitProvenance = clone(deploymentProvenance);
    refreshPacketHashes(browserCommitProvenance, sourceReceipt, browserCommitDrift);
    expectReject(
      "rejects browser checkout commit drift",
      sourceReceipt,
      browserCommitDrift,
      browserCommitProvenance,
    );

    const digestDrift = clone(deploymentProvenance);
    digestDrift.deployment_image_digest = `ghcr.io/rustokrs/rustok@sha256:${"b".repeat(64)}`;
    expectReject("rejects deployment digest drift", sourceReceipt, browserPacket, digestDrift);

    const failedBrowser = clone(browserPacket);
    failedBrowser.observations.published.passed = false;
    const failedBrowserProvenance = clone(deploymentProvenance);
    refreshPacketHashes(failedBrowserProvenance, sourceReceipt, failedBrowser);
    expectReject(
      "rejects failed browser observation",
      sourceReceipt,
      failedBrowser,
      failedBrowserProvenance,
    );

    const routeDrift = clone(deploymentProvenance);
    routeDrift.profile_url_sha256.published = "e".repeat(64);
    expectReject("rejects route provenance drift", sourceReceipt, browserPacket, routeDrift);

    const sourceRunDrift = clone(deploymentProvenance);
    sourceRunDrift.workflow_evidence.source.run_id = "32170000000";
    expectReject("rejects source workflow run drift", sourceReceipt, browserPacket, sourceRunDrift);

    const sourceCommitReviewDrift = clone(deploymentProvenance);
    sourceCommitReviewDrift.workflow_evidence.source.source_commit = "f".repeat(40);
    expectReject(
      "rejects source workflow commit review drift",
      sourceReceipt,
      browserPacket,
      sourceCommitReviewDrift,
    );

    const browserPacketHashDrift = clone(deploymentProvenance);
    browserPacketHashDrift.input_packet_sha256.browser_evidence = "a".repeat(64);
    expectReject(
      "rejects browser packet hash drift",
      sourceReceipt,
      browserPacket,
      browserPacketHashDrift,
    );

    const cryptographicOverclaim = clone(deploymentProvenance);
    cryptographicOverclaim.binding.cryptographic_origin_to_repo_digest_binding = true;
    expectReject(
      "rejects cryptographic deployment overclaim",
      sourceReceipt,
      browserPacket,
      cryptographicOverclaim,
    );

    console.log(
      "[admit-pages-consumer-properties.test] PASS ancestor_or_equal_lineage=accepted fail_closed_mutations=10",
    );
  } finally {
    rmSync(tempRoot, { recursive: true, force: true });
  }
}

try {
  main();
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
