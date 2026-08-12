#!/usr/bin/env node

import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const runner = path.join(
  repoRoot,
  "scripts/evidence/admit-pages-page-builder-terminal-readiness.mjs",
);
const executionContractPath =
  "crates/rustok-forum/contracts/evidence/forum-page-builder-ffa-fba-promotion-execution-source.json";
const digest = `ghcr.io/rustok/server@sha256:${"a".repeat(64)}`;

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function head() {
  return execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
  }).trim();
}

function jsonBytes(document) {
  return Buffer.from(`${JSON.stringify(document, null, 2)}\n`, "utf8");
}

function writeJson(location, document) {
  const bytes = jsonBytes(document);
  writeFileSync(location, bytes);
  return { bytes, sha256: sha256(bytes) };
}

function fixtureDocuments() {
  const sourceCommit = head();
  const generatedAt = new Date().toISOString();
  const nextDueAt = new Date(Date.now() + 60 * 60 * 1000).toISOString();
  const review = {
    format: "forum_page_builder_ffa_fba_promotion_review_v1",
    status: "owner_approved_ffa_fba_promotion_review_execution_pending",
    reviewed_at: generatedAt,
    source_commit: sourceCommit,
    deployment_image_digest: digest,
    observed_acceptance: {
      wave_next_due_at: nextDueAt,
    },
    promotion_review: {
      decision: "approve_ffa_fba_promotion_review",
      targets: ["ffa", "fba"],
    },
    boundaries: {
      control_plane_or_rollout_mutated: false,
      ffa_promoted: false,
      fba_promoted: false,
    },
  };
  const executionSourceHash = sha256(
    readFileSync(path.join(repoRoot, executionContractPath)),
  );
  const accessibility = {
    format: "page_builder_generic_accessibility_browser_packet_verification_v1",
    status: "browser_packet_verified_owner_review_ready_screen_reader_pending",
    source_commit: sourceCommit,
    deployment_digest: digest,
    profiles: {
      full: { passed: true, critical_failures: 0, page_count: 2 },
      read_only: { passed: true, critical_failures: 0, page_count: 2 },
    },
    owner_review_required: true,
    screen_reader_execution_pending: true,
    wcag_conformance_not_claimed: true,
    tenant_rollout_not_claimed: true,
    cryptographic_origin_to_repo_digest_binding_claimed: false,
  };
  return { sourceCommit, generatedAt, nextDueAt, review, accessibility, executionSourceHash };
}

function runCase(name, mutate, expectedSuccess, expectedMessage) {
  const root = mkdtempSync(path.join(os.tmpdir(), "rustok-terminal-readiness-"));
  try {
    const output = path.join(
      repoRoot,
      "target",
      `terminal-readiness-test-${process.pid}-${Date.now()}.json`,
    );
    mkdirSync(path.dirname(output), { recursive: true });
    const fixtures = fixtureDocuments();
    const reviewPath = path.join(root, "review.json");
    const reviewRecord = writeJson(reviewPath, fixtures.review);
    const execution = {
      format: "forum_page_builder_ffa_fba_promotion_execution_v1",
      status: "control_plane_change_executed_readiness_promotion_pending",
      generated_at: fixtures.generatedAt,
      source_commit: fixtures.sourceCommit,
      source_sha256: {
        [executionContractPath]: fixtures.executionSourceHash,
      },
      promotion_review: {
        sha256: reviewRecord.sha256,
        decision: "approve_ffa_fba_promotion_review",
        observed_wave_next_due_at: fixtures.nextDueAt,
      },
      target: {
        deployment_image_digest: digest,
      },
      mutation: {
        outcome: "confirmed",
        control_plane_execution_confirmed: true,
        tenant_rollout_mutation_confirmed: true,
        applied_settings_semantic_sha256: "b".repeat(64),
      },
      postcondition: {
        passed: true,
        current_provider_health_asserted: false,
      },
      rollback: {
        attempted: false,
        outcome: "not_required",
        net_target_state_retained: true,
      },
      readiness: {
        ffa_promoted: false,
        fba_promoted: false,
        registry_or_local_plan_status_mutated: false,
        separate_evidence_backed_governance_change_required: true,
      },
      boundaries: {
        control_plane_change_executed: true,
        tenant_rollout_mutated: true,
        canonical_source_mutated: false,
        readiness_board_mutated: false,
        cryptographic_origin_to_repo_digest_binding_claimed: false,
      },
    };
    const documents = {
      review: fixtures.review,
      execution,
      accessibility: fixtures.accessibility,
    };
    mutate(documents);
    const mutatedReviewRecord = writeJson(reviewPath, documents.review);
    if (documents.execution.promotion_review.sha256 === reviewRecord.sha256) {
      documents.execution.promotion_review.sha256 = mutatedReviewRecord.sha256;
    }
    const executionPath = path.join(root, "execution.json");
    const accessibilityPath = path.join(root, "accessibility.json");
    writeJson(executionPath, documents.execution);
    writeJson(accessibilityPath, documents.accessibility);

    const result = spawnSync(
      process.execPath,
      [
        runner,
        "--execution",
        executionPath,
        "--promotion-review",
        reviewPath,
        "--accessibility",
        accessibilityPath,
        "--output",
        output,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    if (expectedSuccess) {
      assert.equal(result.status, 0, `${name}: ${result.stderr}`);
      const admitted = JSON.parse(readFileSync(output, "utf8"));
      assert.equal(
        admitted.status,
        "rollout_accessibility_prerequisites_admitted_terminal_inventory_pending",
      );
      assert.equal(
        admitted.potential_terminal_targets.pages_ffa.potential_terminal_status,
        "parity_verified",
      );
      assert.equal(
        admitted.potential_terminal_targets.page_builder_fba.potential_terminal_status,
        "transport_verified",
      );
      assert.equal(admitted.potential_terminal_targets.pages_ffa.terminal_candidate_ready, false);
      assert.equal(
        admitted.potential_terminal_targets.page_builder_fba.terminal_candidate_ready,
        false,
      );
      assert.equal(admitted.terminal_evidence_inventory.complete, false);
      assert.equal(admitted.terminal_evidence_inventory.owner_platform_review_ready, false);
      assert.ok(
        admitted.terminal_evidence_inventory.page_builder_fba.pending_executed_evidence_count > 0,
      );
      assert.equal(admitted.terminal_evidence_inventory.pages_ffa.pending_marker_present, true);
      assert.equal(admitted.governance.terminal_evidence_inventory_complete, false);
      assert.equal(admitted.governance.owner_platform_review_ready, false);
      assert.equal(admitted.governance.admission_is_not_approval, true);
      assert.equal(admitted.governance.pages_ffa_promoted, false);
      assert.equal(admitted.governance.page_builder_fba_promoted, false);
      assert.equal(admitted.privacy.raw_input_paths_retained, false);
    } else {
      assert.notEqual(result.status, 0, `${name}: expected failure`);
      assert.match(result.stderr, expectedMessage);
    }
    rmSync(output, { force: true });
    console.log(`ok - ${name}`);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

runCase(
  "admits rollout and accessibility prerequisites while retaining incomplete terminal inventory",
  () => {},
  true,
);

runCase(
  "rejects non-successful promotion execution status",
  ({ execution }) => {
    execution.status = "control_plane_change_postcondition_failed_rolled_back";
  },
  false,
  /not a successful readiness-pending receipt/u,
);

runCase(
  "rejects promotion-review decision drift",
  ({ review }) => {
    review.promotion_review.decision = "reject";
  },
  false,
  /decision or targets drifted/u,
);

runCase(
  "rejects promotion-review source commit drift",
  ({ review }) => {
    review.source_commit = "0".repeat(40);
  },
  false,
  /promotion review source_commit does not equal checkout HEAD/u,
);

runCase(
  "rejects execution source commit drift",
  ({ execution }) => {
    execution.source_commit = "0".repeat(40);
  },
  false,
  /promotion execution source_commit does not equal checkout HEAD/u,
);

runCase(
  "rejects accessibility deployment digest drift",
  ({ accessibility }) => {
    accessibility.deployment_digest = `ghcr.io/rustok/server@sha256:${"c".repeat(64)}`;
  },
  false,
  /accessibility deployment RepoDigest differs/u,
);

runCase(
  "rejects a promotion execution that required rollback",
  ({ execution }) => {
    execution.rollback = {
      attempted: true,
      outcome: "confirmed_restored",
      net_target_state_retained: false,
    };
  },
  false,
  /must retain the successful target state without rollback/u,
);

runCase(
  "rejects failed full accessibility profile",
  ({ accessibility }) => {
    accessibility.profiles.full.passed = false;
  },
  false,
  /accessibility profile full did not pass/u,
);

runCase(
  "rejects WCAG conformance overclaim",
  ({ accessibility }) => {
    accessibility.wcag_conformance_not_claimed = false;
  },
  false,
  /wcag_conformance_not_claimed must be true/u,
);

runCase(
  "rejects execution generated after observed Wave lease",
  ({ execution }) => {
    execution.generated_at = new Date(Date.now() + 2 * 60 * 60 * 1000).toISOString();
  },
  false,
  /generated after the retained observed-Wave lease expired/u,
);

runCase(
  "rejects execution readiness overclaim",
  ({ execution }) => {
    execution.readiness.ffa_promoted = true;
  },
  false,
  /promotion execution readiness\.ffa_promoted must remain false/u,
);
