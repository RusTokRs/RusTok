#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = process.cwd();
const read = (path) => readFileSync(resolve(root, path), "utf8");
const requireAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing required marker: ${marker}`);
  }
};
const forbidAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(!text.includes(marker), `${label} contains forbidden marker: ${marker}`);
  }
};

const contractPath =
  "crates/rustok-forum/contracts/forum-search-link-forum-03-complete-evidence-promotion.json";
const d17ContractPath =
  "crates/rustok-forum/contracts/forum-search-link-forum-03-complete-evidence-assembler.json";
const reviewerPath =
  "scripts/evidence/review-link-forum-03-complete-forum-search-evidence.mjs";
const verifierPath =
  "scripts/verify/verify-link-forum-03-complete-forum-search-evidence-promotion.mjs";
const docPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d18-complete-link-evidence-promotion.md";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const completePath =
  "target/link-forum-03-forum-index-search-complete-evidence.json";
const candidatePath =
  "target/link-forum-03-forum-index-search-complete-promotion-candidate.json";
const sourcePaths = [
  "target/link-forum-03-forum-index-search-ordering-visibility-evidence.json",
  "target/forum-search-link-forum-03-translation-moderation-evidence.json",
  "target/forum-search-link-forum-03-private-trusted-exclusion-evidence.json",
  "target/forum-search-link-forum-03-topic-move-evidence.json",
];
const scenarioIds = [
  "normal_delivery",
  "deletion_acl_ordering",
  "search_disabled_profile",
  "translation_and_moderation_approval",
  "private_and_trusted_channel_exclusion",
  "topic_move_category_scope",
];

const contract = JSON.parse(read(contractPath));
assert.equal(
  contract.contract,
  "forum_search_link_forum_03_complete_evidence_promotion_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D18");
assert.equal(contract.target_link, "LINK-FORUM-03");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan, planPath);
assert.equal(contract.complete_assembler_contract, d17ContractPath);
assert.equal(contract.complete_artifact, completePath);
assert.equal(contract.reviewer, reviewerPath);
assert.equal(contract.verifier, verifierPath);
assert.equal(contract.promotion_candidate.path, candidatePath);
assert.equal(
  contract.promotion_candidate.generation,
  "reviewer_only_after_complete_artifact_and_source_revalidation",
);
assert.equal(contract.promotion_candidate.hand_editing_forbidden, true);
assert.equal(contract.promotion_candidate.source_commit_required, true);
assert.equal(contract.promotion_candidate.atomic_replace, true);
assert.equal(contract.promotion_candidate.automatic_canonical_source_mutation, false);
assert.deepEqual(contract.required_scenarios, scenarioIds);
assert.deepEqual(contract.required_source_artifacts, sourcePaths);
assert.deepEqual(contract.required_attestations, [
  "RUSTOK_LINK_FORUM_03_EVIDENCE_REVIEWER",
  "RUSTOK_LINK_FORUM_03_EVIDENCE_RETENTION_REF",
  "RUSTOK_LINK_FORUM_03_EVIDENCE_RETAINED_SHA256",
]);
assert.ok(contract.fail_closed_requirements.length >= 14);
assert.equal(contract.proposed_transition.task, "LINK-FORUM-03");
assert.equal(contract.proposed_transition.from, "planned");
assert.equal(contract.proposed_transition.to, "done");
assert.equal(
  contract.proposed_transition.requires_separate_canonical_source_pull_request,
  true,
);
assert.equal(contract.proposed_transition.canonical_source_mutated_by_reviewer, false);
assert.equal(contract.proposed_transition.promotes_forum_21, false);
assert.equal(contract.proposed_transition.promotes_forum_23, false);
assert.ok(contract.maintainer_command.includes(reviewerPath));
assert.ok(
  contract.non_claims.some((claim) => claim.includes("does not automatically edit")),
);
assert.ok(contract.non_claims.some((claim) => claim.includes("FORUM-21")));
assert.ok(contract.non_claims.some((claim) => claim.includes("FORUM-23")));

const d17Contract = JSON.parse(read(d17ContractPath));
assert.equal(
  d17Contract.contract,
  "forum_search_link_forum_03_complete_evidence_assembler_v1",
);
assert.equal(d17Contract.task, "FORUM-23B2G2B3D17");
assert.equal(d17Contract.status, "source_ready_maintainer_execution_pending");
assert.equal(d17Contract.coverage, "complete_canonical_runtime_scope_review_pending");
assert.deepEqual(d17Contract.required_scenarios, scenarioIds);
assert.deepEqual(d17Contract.required_inputs, sourcePaths);
assert.equal(d17Contract.output_artifact.path, completePath);
assert.equal(
  d17Contract.output_artifact.status,
  "complete_runtime_evidence_assembled_review_pending",
);
assert.equal(d17Contract.output_artifact.automatic_canonical_source_mutation, false);

const reviewer = read(reviewerPath);
requireAll(
  reviewer,
  [
    "FORUM-23B2G2B3D18",
    "forum_search_link_forum_03_complete_evidence_promotion_v1",
    "link_forum_03_forum_index_search_complete_promotion_candidate_v1",
    "approved_for_canonical_status_promotion",
    completePath,
    candidatePath,
    ...sourcePaths,
    ...scenarioIds,
    "RUSTOK_LINK_FORUM_03_EVIDENCE_REVIEWER",
    "RUSTOK_LINK_FORUM_03_EVIDENCE_RETENTION_REF",
    "RUSTOK_LINK_FORUM_03_EVIDENCE_RETAINED_SHA256",
    'execFileSync("git", ["rev-parse", "HEAD"]',
    "requireDigest(retainedSha, retainedShaEnv)",
    "retainedSha !== completeDigest",
    "complete_runtime_evidence_assembled_review_pending",
    "partial_runtime_evidence_assembled",
    "ordering_visibility_and_search_disabled_core_only",
    "selected_scenario_evidence",
    "artifact.scenario_evidence[scenarioId]",
    "core.artifact.selected_scenario_evidence[scenarioId]",
    "D17 core scenario ${scenarioId}",
    "source.artifact.scenario_results[0]",
    "D17 extension attribution drifted",
    "reviewed_core_lineage",
    "inherited_retained_lineage",
    "external_retention_authentication_performed_by_d17",
    "complete_artifact_independently_reviewed",
    "complete_artifact_retention_attested",
    "status_change_allowed_from_this_artifact",
    "separate_review_gate_required",
    "separate_canonical_source_pull_request_required",
    "promotes_forum_21: false",
    "promotes_forum_23: false",
    "canonical_source_mutated_by_reviewer: false",
    "external_service_authentication_performed_by_script: false",
    "cryptographic_signature_created_by_script: false",
    "all_six_canonical_scenarios_revalidated: true",
    "all_four_source_artifacts_revalidated: true",
    "all_source_digests_match_complete_artifact: true",
    "all_scenario_facts_match_retained_sources: true",
    "canonical_plan_remained_unmodified: true",
    "writeFileSync(temporaryCandidate",
    "renameSync(temporaryCandidate, absoluteCandidate)",
    "flag: \"wx\"",
    "this reviewer accepts no command-line arguments",
  ],
  "D18 reviewer",
);
forbidAll(
  reviewer,
  [
    "process.argv[2]",
    "--force",
    "--allow-missing",
    "static_fixture",
    "result === \"skipped\"",
    "result: \"skipped\"",
    "writeFileSync(resolve(root, planPath)",
    "updateFile(planPath",
    "status_change_allowed_from_this_artifact: true",
    "promotes_forum_21: true",
    "promotes_forum_23: true",
    "canonical_source_mutated_by_reviewer: true",
    "external_service_authentication_performed_by_script: true",
    "cryptographic_signature_created_by_script: true",
  ],
  "D18 reviewer",
);

const plan = read(planPath);
requireAll(
  plan,
  [
    "| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |",
    "| `FORUM-23` | `in_progress` |",
    "| `LINK-FORUM-03` | `planned` | Forum/index/search ordering and visibility proof. |",
  ],
  "Forum canonical plan",
);
forbidAll(
  plan,
  [
    "| `LINK-FORUM-03` | `done` | Forum/index/search ordering and visibility proof. |",
    "FORUM-23B2G2B3D18 closes LINK-FORUM-03",
    "FORUM-23B2G2B3D18 marks FORUM-23 done",
    "FORUM-23B2G2B3D18 marks FORUM-21 done",
  ],
  "Forum canonical plan",
);

const doc = read(docPath);
requireAll(
  doc,
  [
    "`source_ready_maintainer_execution_pending`",
    "FORUM-23B2G2B3D18",
    "complete_runtime_evidence_assembled_review_pending",
    "complete_artifact_independently_reviewed = false",
    "complete_artifact_retention_attested = false",
    "status_change_allowed_from_this_artifact = false",
    contractPath,
    reviewerPath,
    candidatePath,
    ...sourcePaths,
    "D13 core scenario entries preserve their original D8, D9 and D10 source",
    "LINK-FORUM-03: planned -> done",
    "promotes_forum_21 = false",
    "promotes_forum_23 = false",
    "canonical_source_mutated_by_reviewer = false",
    "does not retrieve the retention object",
    "does not create a cryptographic signature",
    "No command above was run by the implementation agent",
  ],
  "D18 handoff",
);

console.log(
  "LINK-FORUM-03 complete evidence promotion gate is source-ready and preserves canonical status until separate review and plan PRs.",
);
