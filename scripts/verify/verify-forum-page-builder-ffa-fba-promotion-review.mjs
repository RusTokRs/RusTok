#!/usr/bin/env node

import { readFileSync, lstatSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const failures = [];

function read(relativePath) {
  try {
    const location = path.join(repoRoot, relativePath);
    const stat = lstatSync(location);
    if (!stat.isFile() || stat.isSymbolicLink()) {
      failures.push(`${relativePath}: must be a regular non-symlink file`);
      return "";
    }
    return readFileSync(location, "utf8");
  } catch (error) {
    failures.push(`${relativePath}: ${error.message}`);
    return "";
  }
}

function json(relativePath) {
  const source = read(relativePath);
  try {
    return JSON.parse(source);
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return {};
  }
}

function normalize(source) {
  return source.replace(/\s+/gu, " ").trim();
}

function requireValue(condition, message) {
  if (!condition) failures.push(message);
}

function requireText(source, marker, label) {
  if (!source.includes(marker) && !normalize(source).includes(normalize(marker))) {
    failures.push(`${label}: missing '${marker}'`);
  }
}

const contractPath =
  "crates/rustok-forum/contracts/evidence/forum-page-builder-ffa-fba-promotion-review-source.json";
const predecessorPath =
  "crates/rustok-forum/contracts/evidence/forum-page-builder-wave-observed-acceptance-source.json";
const runnerPath = "scripts/evidence/review-forum-page-builder-ffa-fba-promotion.mjs";
const testsPath = "scripts/evidence/review-forum-page-builder-ffa-fba-promotion.test.mjs";
const actualizationPath =
  "docs/modules/forum-page-builder-ffa-fba-promotion-review-actualization-2026-08-12.md";
const planPath = "docs/modules/pages-page-builder-parity-continuation-plan.md";
const workflowPath = ".github/workflows/pages-page-builder-provider-health.yml";

const contract = json(contractPath);
const predecessor = json(predecessorPath);
const runner = read(runnerPath);
const tests = read(testsPath);
const actualization = read(actualizationPath);
const plan = read(planPath);
const workflow = read(workflowPath);

requireValue(
  contract.format === "forum_page_builder_ffa_fba_promotion_review_source_v1" &&
    contract.status === "source_ready_maintainer_execution_pending" &&
    contract.module === "forum" &&
    contract.wave === "1",
  `${contractPath}: identity drifted`,
);
requireValue(
  contract.predecessor?.format === "forum_page_builder_wave_observed_acceptance_v1" &&
    contract.predecessor?.accepted_status ===
      "owner_accepted_observed_control_plane_wave_promotion_review_pending" &&
    contract.predecessor?.source_commit_must_equal_checkout_head === true &&
    contract.predecessor?.owner_decision_must_equal === "accept_observed_wave_evidence" &&
    contract.predecessor?.freshness_verifier_passed_at_owner_review_must_be_true === true &&
    contract.predecessor?.admission_lineage_verifier_passed_at_owner_review_must_be_true === true &&
    contract.predecessor?.wave_next_due_at_must_still_be_future_at_promotion_review === true,
  `${contractPath}: predecessor boundary drifted`,
);
requireValue(
  contract.promotion_review?.runner === runnerPath &&
    JSON.stringify(contract.promotion_review?.decisions) ===
      JSON.stringify(["approve_ffa_fba_promotion_review", "reject"]) &&
    JSON.stringify(contract.promotion_review?.targets) === JSON.stringify(["ffa", "fba"]),
  `${contractPath}: promotion review decision contract drifted`,
);
for (const key of [
  "approval_is_not_control_plane_execution",
  "approval_does_not_mutate_rollout",
  "approval_does_not_promote_ffa_or_fba",
]) {
  requireValue(contract.promotion_review?.[key] === true, `${contractPath}: ${key} must remain true`);
}
requireValue(
  contract.output?.format === "forum_page_builder_ffa_fba_promotion_review_v1" &&
    contract.output?.approved_status ===
      "owner_approved_ffa_fba_promotion_review_execution_pending" &&
    contract.output?.rejected_status === "owner_rejected_ffa_fba_promotion_review",
  `${contractPath}: output contract drifted`,
);
for (const key of [
  "approved_review_is_required_before_ffa_fba_control_plane_change",
  "approved_review_does_not_execute_control_plane_change",
  "approved_review_does_not_mutate_pages_or_forum_persistence",
  "approved_review_does_not_assert_current_provider_health",
  "approved_review_does_not_claim_cryptographic_origin_to_repo_digest_binding",
  "actual_ffa_fba_promotion_remains_separate_maintainer_execution",
]) {
  requireValue(contract.execution_boundary?.[key] === true, `${contractPath}: ${key} must remain true`);
}
requireValue(
  contract.next_cursor?.ffa_fba_promotion_review ===
      "source_ready_blocked_on_accepted_observed_wave_evidence" &&
    contract.next_cursor?.ffa_fba_control_plane_promotion ===
      "blocked_on_approved_promotion_review",
  `${contractPath}: next cursor drifted`,
);

requireValue(
  predecessor.format === "forum_page_builder_wave_observed_acceptance_source_v1" &&
    predecessor.output?.format === "forum_page_builder_wave_observed_acceptance_v1" &&
    predecessor.output?.accepted_status ===
      "owner_accepted_observed_control_plane_wave_promotion_review_pending" &&
    predecessor.promotion_boundary
      ?.accepted_packet_is_eligible_input_for_explicit_ffa_fba_promotion_review === true &&
    predecessor.promotion_boundary?.accepted_packet_does_not_itself_promote_ffa_or_fba === true &&
    predecessor.promotion_boundary?.accepted_packet_does_not_mutate_control_plane_or_rollout === true,
  `${predecessorPath}: promotion predecessor drifted`,
);

for (const marker of [
  "--observed-acceptance",
  "--owner-id",
  "approve_ffa_fba_promotion_review",
  "contract.output.approved_status",
  "git\", [\"rev-parse\", \"HEAD\"]",
  "observed Wave evidence is stale at promotion review time",
  "freshness_verifier_passed_at_review",
  "admission_lineage_verifier_passed_at_review",
  "control_plane_or_rollout_mutated: false",
  "ffa_promoted: false",
  "fba_promoted: false",
  "separate_control_plane_execution_required: approved",
  "raw_input_path_persisted: false",
]) requireText(runner, marker, runnerPath);
for (const forbidden of ["fetch(", "@playwright/test", "chromium", 'spawnSync("cargo"']) {
  if (runner.includes(forbidden)) failures.push(`${runnerPath}: forbidden execution marker '${forbidden}'`);
}

for (const label of [
  "approves promotion review without promoting FFA or FBA",
  "retains explicit promotion review reject without rollout mutation",
  "rejects non-accepted observed Wave owner packet",
  "rejects observed acceptance source-commit drift",
  "rejects stale observed Wave evidence at promotion review time",
  "rejects prior owner decision drift",
  "rejects missing retained freshness verifier success",
  "rejects missing retained admission-lineage verifier success",
  "rejects prior rollout mutation overclaim",
  "rejects prior FFA promotion overclaim",
  "rejects retained privacy overclaim",
  "rejects invalid promotion-review owner identifier",
  "rejects unsupported promotion-review decision",
]) requireText(tests, label, testsPath);

for (const marker of [
  "forum-ffa-fba-promotion-review-source-ready",
  "accepted observed-Wave owner packet",
  "approve_ffa_fba_promotion_review",
  "does not mutate rollout",
  "does not promote FFA/FBA",
  "maintainer execution remains pending",
]) requireText(actualization, marker, actualizationPath);

requireText(plan, "separate explicit FFA/FBA promotion review", planPath);
requireText(plan, "accepted observed-Wave owner evidence", planPath);

for (const marker of [
  contractPath,
  runnerPath,
  testsPath,
  "Verify Forum FFA/FBA promotion review source",
  "Forum FFA/FBA promotion review synthetic tests",
]) requireText(workflow, marker, workflowPath);

if (failures.length > 0) {
  console.error("[verify-forum-page-builder-ffa-fba-promotion-review] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("[verify-forum-page-builder-ffa-fba-promotion-review] PASS");
console.log(
  "module=forum; wave=1; promotion_review=source_ready_blocked_on_accepted_observed_wave_evidence; rollout_mutation=false",
);
