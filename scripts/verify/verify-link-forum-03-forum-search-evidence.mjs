#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = process.cwd();
const read = (path) => readFileSync(resolve(root, path), "utf8");
const requireAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing marker: ${marker}`);
  }
};
const forbidAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(!text.includes(marker), `${label} contains forbidden marker: ${marker}`);
  }
};

const contractPath =
  "crates/rustok-forum/contracts/forum-search-link-forum-03-evidence-assembler.json";
const docPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d13-link-forum-03-evidence-assembler.md";
const assemblerPath =
  "scripts/evidence/assemble-link-forum-03-forum-search-evidence.mjs";
const verifierPath =
  "scripts/verify/verify-link-forum-03-forum-search-evidence.mjs";
const forumPlanPath = "crates/rustok-forum/docs/implementation-plan.md";
const searchPlanPath = "crates/rustok-search/docs/implementation-plan.md";
const d0Path =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json";
const d12Path =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-retained-evidence-promotion.json";
const d12ReviewerPath =
  "scripts/evidence/review-forum-search-versioned-invalidation-runtime-evidence.mjs";
const d8TestPath =
  "apps/server/tests/forum_versioned_invalidation_deletion_acl_ordering.rs";
const d9TestPath =
  "apps/server/tests/forum_versioned_invalidation_search_disabled_recovery.rs";
const d10TestPath =
  "apps/server/tests/forum_versioned_invalidation_normal_delivery_iggy.rs";
const candidatePath =
  "target/forum-search-versioned-invalidation-runtime-promotion-candidate.json";
const aggregatePath =
  "target/forum-search-versioned-invalidation-runtime-evidence.json";
const d8Path =
  "target/forum-search-versioned-invalidation-deletion-acl-ordering-evidence.json";
const d9Path =
  "target/forum-search-versioned-invalidation-search-disabled-recovery-evidence.json";
const d10Path =
  "target/forum-search-versioned-invalidation-normal-delivery-evidence.json";
const outputPath =
  "target/link-forum-03-forum-index-search-ordering-visibility-evidence.json";
const remainingScope = [
  "translation projection and retrieval",
  "real moderation approval transition into Search visibility",
  "topic move and category-scope projection update",
  "exact private and trusted-channel exclusion runtime profile",
  "separate review of the generated partial LINK artifact before any canonical plan change",
];

const contract = JSON.parse(read(contractPath));
assert.equal(contract.contract, "forum_search_link_forum_03_evidence_assembler_v1");
assert.equal(contract.task, "FORUM-23B2G2B3D13");
assert.equal(contract.target_link, "LINK-FORUM-03");
assert.equal(contract.coverage, "ordering_visibility_and_search_disabled_core_only");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan, forumPlanPath);
assert.equal(contract.d0_parent, d0Path);
assert.equal(contract.d12_contract, d12Path);
assert.equal(contract.assembler, assemblerPath);
assert.equal(contract.verifier, verifierPath);
assert.deepEqual(contract.required_inputs, [
  candidatePath,
  aggregatePath,
  d8Path,
  d9Path,
  d10Path,
]);
assert.equal(contract.output_artifact.path, outputPath);
assert.equal(
  contract.output_artifact.generation,
  "assembler_only_after_d12_review_candidate",
);
assert.equal(contract.output_artifact.hand_editing_forbidden, true);
assert.equal(contract.output_artifact.source_commit_required, true);
assert.equal(contract.output_artifact.atomic_replace, true);
assert.equal(contract.output_artifact.automatic_canonical_source_mutation, false);
assert.equal(
  contract.required_runtime_lineage.ordering_and_visibility,
  "FORUM-23B2G2B3D8:deletion_acl_ordering",
);
assert.equal(
  contract.required_runtime_lineage.search_disabled_recovery,
  "FORUM-23B2G2B3D9:search_disabled_profile",
);
assert.equal(
  contract.required_runtime_lineage.normal_delivery,
  "FORUM-23B2G2B3D10:normal_delivery",
);
assert.equal(
  contract.required_runtime_lineage.retained_review,
  "FORUM-23B2G2B3D12:approved_for_canonical_status_promotion",
);
assert.deepEqual(contract.remaining_link_forum_03_runtime_scope, remainingScope);
assert.deepEqual(contract.maintainer_commands, [
  `node ${verifierPath}`,
  `node ${assemblerPath}`,
]);
assert.ok(contract.fail_closed_requirements.length >= 14);
assert.ok(contract.non_claims.includes("this partial core artifact is not sufficient to mark LINK-FORUM-03 done"));

const d0 = JSON.parse(read(d0Path));
assert.equal(d0.contract, "forum_search_versioned_invalidation_runtime_evidence_v1");
assert.equal(d0.task, "FORUM-23B2G2B3D0");
assert.equal(d0.status, "source_ready_maintainer_execution_pending");
assert.equal(d0.evidence_artifact.path, aggregatePath);
assert.deepEqual(
  d0.required_scenarios.map(({ id }) => id),
  [
    "normal_delivery",
    "legacy_first_duplicate",
    "typed_first_duplicate",
    "acknowledgement_failure_restart",
    "raw_poison_dlq_redelivery",
    "semantic_poison_identity_conflict",
    "missing_delivery_owner_repair",
    "multi_process_serialization",
    "deletion_acl_ordering",
    "search_disabled_profile",
  ],
);
for (const task of [
  "FORUM-23B2G2B3D8",
  "FORUM-23B2G2B3D9",
  "FORUM-23B2G2B3D10",
  "FORUM-23B2G2B3D11",
  "FORUM-23B2G2B3D12",
]) {
  assert.ok(
    d0.source_ready_subproofs.some((subproof) => subproof.task === task),
    `${task} disappeared from D0 lineage`,
  );
}

const d12 = JSON.parse(read(d12Path));
assert.equal(
  d12.contract,
  "forum_search_versioned_invalidation_retained_evidence_promotion_v1",
);
assert.equal(d12.task, "FORUM-23B2G2B3D12");
assert.equal(d12.status, "source_ready_maintainer_execution_pending");
assert.equal(d12.aggregate_artifact, aggregatePath);
assert.equal(d12.promotion_candidate.path, candidatePath);
assert.equal(d12.proposed_transition.from, "source_ready_maintainer_execution_pending");
assert.equal(d12.proposed_transition.to, "runtime_evidence_reviewed");
assert.equal(d12.proposed_transition.requires_separate_canonical_source_pull_request, true);
assert.equal(d12.proposed_transition.closes_forum_23, false);
assert.equal(d12.proposed_transition.closes_link_forum_03, false);

const d12Reviewer = read(d12ReviewerPath);
requireAll(
  d12Reviewer,
  [
    'contract: "forum_search_versioned_invalidation_runtime_promotion_candidate_v1"',
    'status: "approved_for_canonical_status_promotion"',
    "all_ten_frozen_scenarios_passed: true",
    "all_nine_source_artifacts_revalidated: true",
    "all_source_digests_match_aggregate: true",
    "aggregate_parent_digest_matches_current_d0: true",
    "retained_digest_attested_by_maintainer: true",
    "separate_canonical_source_pull_request_required: true",
    "canonical_source_mutated_by_reviewer: false",
    "closes_link_forum_03: false",
    candidatePath,
  ],
  "D12 reviewer",
);

const d8Test = read(d8TestPath);
requireAll(
  d8Test,
  [
    "forum_search_versioned_invalidation_deletion_acl_ordering_evidence_v1",
    d8Path,
    'id: "deletion_acl_ordering"',
    'result: "passed"',
    "broker_used: false",
    "ForumProjectionReconciler",
    "execute_forum_storefront_search",
    "assert_storefront_exact",
    "search_projection_inbox",
    "forum_projection_revision_ledger",
    '.args(["rev-parse", "HEAD"])',
  ],
  "D8 proof",
);

const d9Test = read(d9TestPath);
requireAll(
  d9Test,
  [
    "forum_search_versioned_invalidation_search_disabled_recovery_evidence_v1",
    d9Path,
    'id: "search_disabled_profile"',
    'result: "passed"',
    "broker_used: false",
    "assert_search_storage_absent",
    "enable_search",
    "ForumProjectionReconciler::with_owner_revision_source",
    "owner_rebuilds != 1",
    "owner_revisions_checkpointed != 3",
    '.args(["rev-parse", "HEAD"])',
  ],
  "D9 proof",
);

const d10Test = read(d10TestPath);
requireAll(
  d10Test,
  [
    "forum_search_versioned_invalidation_normal_delivery_evidence_v1",
    d10Path,
    'id: "normal_delivery"',
    'result: "passed"',
    'delivery_profile: "outbox_iggy"',
    "open_persistent_contract_consumer_group",
    "ForumSearchContractIngress::new",
    "ForumProjectionReconciler::with_owner_revision_source",
    "execute_forum_storefront_search",
    'row.outcome != "delivery_covered"',
    '.args(["rev-parse", "HEAD"])',
  ],
  "D10 proof",
);

const assembler = read(assemblerPath);
requireAll(
  assembler,
  [
    'execFileSync("git", ["rev-parse", "HEAD"]',
    'createHash("sha256")',
    contractPath,
    forumPlanPath,
    d0Path,
    d12Path,
    candidatePath,
    aggregatePath,
    d8Path,
    d9Path,
    d10Path,
    outputPath,
    "forum_search_link_forum_03_evidence_assembler_v1",
    "ordering_visibility_and_search_disabled_core_only",
    "forum_search_versioned_invalidation_runtime_promotion_candidate_v1",
    "approved_for_canonical_status_promotion",
    "runtime_evidence_assembled",
    "forum_search_versioned_invalidation_deletion_acl_ordering_evidence_v1",
    "forum_search_versioned_invalidation_search_disabled_recovery_evidence_v1",
    "forum_search_versioned_invalidation_normal_delivery_evidence_v1",
    "candidate.parent_contract.sha256 !== sha256(d0Bytes)",
    "candidate.aggregate_artifact.sha256 !== aggregateReview.digest",
    "record.sha256 !== source.digest",
    "JSON.stringify(scenario.facts) !== JSON.stringify(source.scenario.facts)",
    'status: "partial_runtime_evidence_assembled"',
    'coverage: "ordering_visibility_and_search_disabled_core_only"',
    "status_change_allowed_from_this_artifact: false",
    "separate_follow_up_runtime_evidence_required: true",
    "closes_link_forum_03_automatically: false",
    "external_retention_authentication_performed_by_assembler: false",
    "writeFileSync(temporaryOutput",
    "renameSync(temporaryOutput, absoluteOutput)",
    "rmSync(temporaryOutput, { force: true })",
    "if (process.argv.length !== 2)",
  ],
  "D13 assembler",
);
forbidAll(
  assembler,
  [
    'status: "runtime_evidence_complete"',
    "status_change_allowed_from_this_artifact: true",
    'proposed_link_status_after_separate_canonical_review: "done"',
    "result: \"skipped\"",
    "allowMissing",
    "bestEffort",
    "continueOnError",
    "process.env.SOURCE_COMMIT",
    "--source-commit",
    "writeFileSync(resolve(root, d0Path)",
    "writeFileSync(resolve(root, forumPlanPath)",
  ],
  "D13 assembler",
);

const forumPlan = read(forumPlanPath);
requireAll(
  forumPlan,
  [
    "| `FORUM-23` | `in_progress` |",
    "maintainer PostgreSQL/Iggy plus LINK-FORUM-03 runtime evidence remain",
    "| `LINK-FORUM-03` | `planned` | Forum/index/search ordering and visibility proof. |",
    "## `LINK-FORUM-03` — index and search",
    "**Status:** `planned`",
    "**Dependencies:** FORUM-20/23",
    "Prove publish, translation, moderation approval, move, hide/delete, ACL change,",
    "out-of-order events and search-disabled behavior",
  ],
  "Forum canonical plan",
);
forbidAll(
  forumPlan,
  [
    "| `LINK-FORUM-03` | `done` |",
    "LINK-FORUM-03 runtime evidence passed",
    "FORUM-23B2G2B3D13 closes LINK-FORUM-03",
  ],
  "Forum canonical plan",
);

const searchPlan = read(searchPlanPath);
requireAll(
  searchPlan,
  [
    "The source-ready Forum-only storefront Search path composes two neutral optional",
    "owner ports without importing Forum into Search",
    "Runtime evidence remains pending",
    "Durable Forum inbox ingest ordering is `source_complete_execution_pending`",
  ],
  "Search canonical plan",
);

const doc = read(docPath);
requireAll(
  doc,
  [
    "FORUM-23B2G2B3D13",
    "`source_ready_maintainer_execution_pending`",
    contractPath,
    assemblerPath,
    verifierPath,
    outputPath,
    "D8 proves deletion",
    "D9 proves Forum owner writes",
    "D10 proves one correlated real Forum owner transaction",
    "D11 assembles all D2-D10 runtime artifacts",
    "D12 re-reads the aggregate",
    "partial_runtime_evidence_assembled",
    "ordering_visibility_and_search_disabled_core_only",
    "status_change_allowed_from_this_artifact = false",
    "translation projection and retrieval",
    "real moderation approval transition",
    "topic move and category-scope projection update",
    "exact private and trusted-channel exclusion profile",
    "No command above was run by the implementation agent",
  ],
  "D13 handoff",
);

console.log(
  "LINK-FORUM-03 ordering, visibility and Search-disabled core evidence assembler is source-ready and remains partial.",
);
