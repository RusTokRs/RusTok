#!/usr/bin/env node

import assert from "node:assert/strict";
import test from "node:test";

import {
  evaluateReceiptBoundary,
  evaluateRunMetadata,
} from "./admit-page-builder-static-sanitization-registry-update.mjs";

const sourceCommit = "3b747b6fec1ca0042a528bfedd5545eac7cc4ceb";

function requirements() {
  return {
    repository: "RusTokRs/RusTok",
    workflow_name: "Page Builder Static Sanitization Evidence",
    workflow_path: ".github/workflows/page-builder-static-sanitization-evidence.yml",
    event: "push",
    head_branch: "main",
    required_status: "completed",
    required_conclusion: "success",
  };
}

function executionSource() {
  return {
    output: {
      format: "page_builder_static_sanitization_execution_v1",
      success_status: "static_sanitization_execution_passed_registry_update_pending",
    },
    execution: {
      test_list_command: "cargo test --locked -p rustok-page-builder --lib -- --list",
      required_test_name_fragments: [
        "publish_sanitization::tests::sanitization_assigns_stable_ids_and_hashes_policy_bound_project",
        "static_publish_policy::tests::",
        "static_publish_resource_limits::tests::",
      ],
      test_commands: [
        "cargo test --locked -p rustok-page-builder --lib publish_sanitization::tests:: -- --nocapture",
        "cargo test --locked -p rustok-page-builder --lib static_publish_policy::tests:: -- --nocapture",
        "cargo test --locked -p rustok-page-builder --lib static_publish_resource_limits::tests:: -- --nocapture",
      ],
    },
  };
}

function target() {
  return {
    fba_registry: "crates/rustok-page-builder/contracts/page-builder-fba-registry.json",
    registry_required_status: "boundary_ready",
    executed_evidence_json_pointer: "/provider/static_sanitization_contract/executed_evidence",
    required_before_value: "pending",
  };
}

function receipt() {
  const source = executionSource();
  return {
    format: source.output.format,
    status: source.output.success_status,
    source_commit: sourceCommit,
    provenance: {
      repository: "RusTokRs/RusTok",
      workflow: "Page Builder Static Sanitization Evidence",
      run_id: "31669509868",
      run_attempt: "1",
      event_name: "push",
      github_actions: true,
      cryptographic_ci_attestation_claimed: false,
    },
    target: {
      fba_registry: target().fba_registry,
      registry_status_before: "boundary_ready",
      registry_sha256: "a".repeat(64),
      executed_evidence_json_pointer: target().executed_evidence_json_pointer,
      executed_evidence_before: "pending",
    },
    execution: {
      test_list_command: source.execution.test_list_command,
      required_test_name_fragments: source.execution.required_test_name_fragments,
      test_commands: source.execution.test_commands,
      all_commands_passed: true,
      packet_generated_only_after_test_steps: true,
      network_runtime_under_test: false,
      database_used: false,
      browser_used: false,
    },
    governance: {
      registry_mutated: false,
      executed_evidence_cleared: false,
      terminal_inventory_complete_claimed: false,
      owner_approval_claimed: false,
      platform_approval_claimed: false,
      page_builder_fba_promoted: false,
      later_evidence_containing_registry_pr_required: true,
    },
  };
}

function runMetadata() {
  return {
    id: 31669509868,
    name: "Page Builder Static Sanitization Evidence",
    path: ".github/workflows/page-builder-static-sanitization-evidence.yml",
    event: "push",
    head_branch: "main",
    head_sha: sourceCommit,
    status: "completed",
    conclusion: "success",
    run_attempt: 1,
    repository: {
      full_name: "RusTokRs/RusTok",
    },
  };
}

test("accepts only the exact completed successful main push run", () => {
  const result = evaluateRunMetadata(runMetadata(), receipt(), requirements());
  assert.equal(result.valid, true);
  assert.deepEqual(result.failures, []);
});

test("rejects workflow repository drift", () => {
  const run = runMetadata();
  run.repository.full_name = "example/other";
  const result = evaluateRunMetadata(run, receipt(), requirements());
  assert.equal(result.valid, false);
  assert.ok(result.failures.includes("workflow repository mismatch"));
});

test("rejects queued workflow metadata", () => {
  const run = runMetadata();
  run.status = "queued";
  run.conclusion = null;
  const result = evaluateRunMetadata(run, receipt(), requirements());
  assert.equal(result.valid, false);
  assert.ok(result.failures.includes("workflow status is not completed"));
  assert.ok(result.failures.includes("workflow conclusion is not success"));
});

test("rejects completed failed workflow metadata", () => {
  const run = runMetadata();
  run.conclusion = "failure";
  const result = evaluateRunMetadata(run, receipt(), requirements());
  assert.equal(result.valid, false);
  assert.ok(result.failures.includes("workflow conclusion is not success"));
});

test("rejects pull request evidence when main push is required", () => {
  const run = runMetadata();
  run.event = "pull_request";
  run.head_branch = "agent/example";
  const result = evaluateRunMetadata(run, receipt(), requirements());
  assert.equal(result.valid, false);
  assert.ok(result.failures.includes("workflow event mismatch"));
  assert.ok(result.failures.includes("workflow head branch mismatch"));
});

test("rejects workflow head SHA drift", () => {
  const run = runMetadata();
  run.head_sha = "4".repeat(40);
  const result = evaluateRunMetadata(run, receipt(), requirements());
  assert.equal(result.valid, false);
  assert.ok(result.failures.includes("workflow head SHA mismatch"));
});

test("rejects workflow run identity drift", () => {
  const run = runMetadata();
  run.id += 1;
  run.run_attempt += 1;
  const result = evaluateRunMetadata(run, receipt(), requirements());
  assert.equal(result.valid, false);
  assert.ok(result.failures.includes("workflow run id mismatch"));
  assert.ok(result.failures.includes("workflow run attempt mismatch"));
});

test("rejects cryptographic attestation overclaim", () => {
  const candidate = receipt();
  candidate.provenance.cryptographic_ci_attestation_claimed = true;
  const result = evaluateRunMetadata(runMetadata(), candidate, requirements());
  assert.equal(result.valid, false);
  assert.ok(result.failures.includes("receipt overclaims cryptographic attestation"));
});

test("accepts the exact execution receipt boundary", () => {
  const result = evaluateReceiptBoundary(receipt(), executionSource(), target());
  assert.equal(result.valid, true);
  assert.deepEqual(result.failures, []);
});

test("rejects test command drift", () => {
  const candidate = receipt();
  candidate.execution.test_commands = ["cargo test --all"];
  const result = evaluateReceiptBoundary(candidate, executionSource(), target());
  assert.equal(result.valid, false);
  assert.ok(result.failures.includes("test command set mismatch"));
});

test("rejects target pointer drift", () => {
  const candidate = receipt();
  candidate.target.executed_evidence_json_pointer = "/provider/other/executed_evidence";
  const result = evaluateReceiptBoundary(candidate, executionSource(), target());
  assert.equal(result.valid, false);
  assert.ok(result.failures.includes("receipt target pointer mismatch"));
});

test("rejects receipts that already claim registry mutation", () => {
  const candidate = receipt();
  candidate.governance.registry_mutated = true;
  candidate.governance.executed_evidence_cleared = true;
  const result = evaluateReceiptBoundary(candidate, executionSource(), target());
  assert.equal(result.valid, false);
  assert.ok(result.failures.includes("receipt claims registry mutation"));
  assert.ok(result.failures.includes("receipt claims executed evidence was already cleared"));
});

test("rejects receipts that infer terminal readiness or FBA promotion", () => {
  const candidate = receipt();
  candidate.governance.terminal_inventory_complete_claimed = true;
  candidate.governance.page_builder_fba_promoted = true;
  const result = evaluateReceiptBoundary(candidate, executionSource(), target());
  assert.equal(result.valid, false);
  assert.ok(result.failures.includes("receipt claims terminal inventory completion"));
  assert.ok(result.failures.includes("receipt claims FBA promotion"));
});
