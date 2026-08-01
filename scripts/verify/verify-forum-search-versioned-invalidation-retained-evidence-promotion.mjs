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
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-retained-evidence-promotion.json";
const parentPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json";
const reviewerPath =
  "scripts/evidence/review-forum-search-versioned-invalidation-runtime-evidence.mjs";
const verifierPath =
  "scripts/verify/verify-forum-search-versioned-invalidation-retained-evidence-promotion.mjs";
const docPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d12-retained-evidence-promotion.md";
const d11DocPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d11-aggregate-evidence-assembler.md";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const aggregatePath =
  "target/forum-search-versioned-invalidation-runtime-evidence.json";
const candidatePath =
  "target/forum-search-versioned-invalidation-runtime-promotion-candidate.json";

const frozenScenarios = [
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
];
const sourceTasks = [
  "FORUM-23B2G2B3D2",
  "FORUM-23B2G2B3D3",
  "FORUM-23B2G2B3D4",
  "FORUM-23B2G2B3D5",
  "FORUM-23B2G2B3D6",
  "FORUM-23B2G2B3D7",
  "FORUM-23B2G2B3D8",
  "FORUM-23B2G2B3D9",
  "FORUM-23B2G2B3D10",
];
const sourceArtifacts = [
  "target/forum-search-versioned-invalidation-postgres-ingress-evidence.json",
  "target/forum-search-versioned-invalidation-ack-restart-evidence.json",
  "target/forum-search-versioned-invalidation-raw-poison-evidence.json",
  "target/forum-search-versioned-invalidation-semantic-poison-evidence.json",
  "target/forum-search-versioned-invalidation-missing-delivery-repair-evidence.json",
  "target/forum-search-versioned-invalidation-multi-process-evidence.json",
  "target/forum-search-versioned-invalidation-deletion-acl-ordering-evidence.json",
  "target/forum-search-versioned-invalidation-search-disabled-recovery-evidence.json",
  "target/forum-search-versioned-invalidation-normal-delivery-evidence.json",
];

const contract = JSON.parse(read(contractPath));
assert.equal(
  contract.contract,
  "forum_search_versioned_invalidation_retained_evidence_promotion_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D12");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.runtime_evidence_parent, parentPath);
assert.equal(contract.aggregate_artifact, aggregatePath);
assert.equal(contract.reviewer, reviewerPath);
assert.equal(contract.verifier, verifierPath);
assert.equal(contract.promotion_candidate.path, candidatePath);
assert.equal(
  contract.promotion_candidate.generation,
  "reviewer_only_after_retained_aggregate_validation",
);
assert.equal(contract.promotion_candidate.hand_editing_forbidden, true);
assert.equal(contract.promotion_candidate.source_commit_required, true);
assert.equal(contract.promotion_candidate.atomic_replace, true);
assert.equal(contract.promotion_candidate.automatic_canonical_source_mutation, false);
assert.deepEqual(contract.required_attestations, [
  "RUSTOK_FORUM_EVIDENCE_REVIEWER",
  "RUSTOK_FORUM_EVIDENCE_RETENTION_REF",
  "RUSTOK_FORUM_EVIDENCE_RETAINED_SHA256",
]);
assert.ok(contract.fail_closed_requirements.length >= 11);
assert.deepEqual(contract.proposed_transition, {
  from: "source_ready_maintainer_execution_pending",
  to: "runtime_evidence_reviewed",
  requires_separate_canonical_source_pull_request: true,
  closes_forum_23: false,
  closes_link_forum_03: false,
});
assert.ok(contract.maintainer_command.includes(reviewerPath));
assert.ok(contract.non_claims.some((claim) => claim.includes("does not automatically edit")));

const parent = JSON.parse(read(parentPath));
assert.equal(parent.contract, "forum_search_versioned_invalidation_runtime_evidence_v1");
assert.equal(parent.task, "FORUM-23B2G2B3D0");
assert.equal(parent.status, "source_ready_maintainer_execution_pending");
assert.deepEqual(
  parent.required_scenarios.map(({ id }) => id),
  frozenScenarios,
);
const d12 = parent.source_ready_subproofs.find(
  ({ task }) => task === "FORUM-23B2G2B3D12",
);
assert.ok(d12, "D0 must register FORUM-23B2G2B3D12");
assert.equal(d12.contract, contractPath);
assert.equal(d12.reviewer, reviewerPath);
assert.equal(d12.promotion_candidate, candidatePath);
assert.ok(d12.covers.includes("retained_aggregate_digest_attestation"));
assert.ok(d12.covers.includes("promotion_candidate_without_source_mutation"));
assert.ok(d12.does_not_cover.includes("canonical_d0_status_promotion"));
assert.ok(d12.does_not_cover.includes("link_forum_03"));
for (const command of [`node ${verifierPath}`, contract.maintainer_command]) {
  assert.ok(parent.maintainer_commands.includes(command));
}

const reviewer = read(reviewerPath);
requireAll(
  reviewer,
  [
    "execFileSync(\"git\", [\"rev-parse\", \"HEAD\"]",
    "createHash(\"sha256\")",
    "RUSTOK_FORUM_EVIDENCE_REVIEWER",
    "RUSTOK_FORUM_EVIDENCE_RETENTION_REF",
    "RUSTOK_FORUM_EVIDENCE_RETAINED_SHA256",
    "source_ready_maintainer_execution_pending",
    "runtime_evidence_assembled",
    "approved_for_canonical_status_promotion",
    "runtime_evidence_reviewed",
    "aggregate.assembly.parent_contract_sha256 !== sha256(parentBytes)",
    "aggregate.assembly.input_artifact_count !== manifest.length",
    "aggregate.assembly.frozen_scenario_count !== frozenScenarios.length",
    "aggregate.assembly.all_inputs_same_source_commit !== true",
    "aggregate.assembly.source_commit_matches_current_head !== true",
    "aggregate.assembly.output_written_after_complete_validation !== true",
    "retained.sha256 !== source.digest",
    "retained.byte_length !== source.bytes.length",
    "JSON.stringify(scenario.facts) !== JSON.stringify(retainedScenario.facts)",
    "JSON.stringify(value.facts) !== JSON.stringify(scenario.facts)",
    "retainedSha !== aggregateDigest",
    "all_ten_frozen_scenarios_passed: true",
    "all_nine_source_artifacts_revalidated: true",
    "all_source_digests_match_aggregate: true",
    "aggregate_parent_digest_matches_current_d0: true",
    "retained_digest_attested_by_maintainer: true",
    "separate_canonical_source_pull_request_required: true",
    "canonical_source_mutated_by_reviewer: false",
    "external_service_authentication_performed_by_script: false",
    "writeFileSync(temporaryCandidate",
    "renameSync(temporaryCandidate, absoluteCandidate)",
    "rmSync(temporaryCandidate, { force: true })",
    "if (process.argv.length !== 2)",
    candidatePath,
  ],
  "retained evidence reviewer",
);
for (const scenario of frozenScenarios) {
  assert.ok(reviewer.includes(`\"${scenario}\"`), `reviewer is missing ${scenario}`);
}
for (const task of sourceTasks) {
  assert.ok(reviewer.includes(`\"${task}\"`), `reviewer is missing ${task}`);
}
for (const path of sourceArtifacts) {
  assert.ok(reviewer.includes(path), `reviewer is missing ${path}`);
}
for (const field of [
  "owner_revision_rows",
  "typed_and_root_event_ids",
  "search_inbox_rows",
  "ingest_sequences",
  "owner_checkpoints",
  "poison_receipts",
  "dlq_receipts",
  "storefront_visibility_assertions",
]) {
  assert.ok(reviewer.includes(`\"${field}\"`), `reviewer is missing grouped field ${field}`);
}
forbidAll(
  reviewer,
  [
    "fetch(",
    "axios",
    "node:http",
    "node:https",
    "process.argv[2]",
    "--source-commit",
    "allowMissing",
    "bestEffort",
    "continueOnError",
    "result: \"skipped\"",
    "writeFileSync(parentPath",
    "writeFileSync(planPath",
    "renameSync(temporaryCandidate, resolve(root, parentPath))",
    "canonical_source_mutated_by_reviewer: true",
    "closes_forum_23: true",
    "closes_link_forum_03: true",
  ],
  "retained evidence reviewer",
);

const doc = read(docPath);
requireAll(
  doc,
  [
    "`source_ready_maintainer_execution_pending`",
    "FORUM-23B2G2B3D12",
    contractPath,
    reviewerPath,
    verifierPath,
    aggregatePath,
    candidatePath,
    "all nine source artifacts",
    "all ten frozen scenarios",
    "immutable retention reference",
    "independently supplied SHA-256",
    "does not contact or authenticate the external retention service",
    "separate source PR",
    "No command above was run by the implementation agent",
  ],
  "retained evidence promotion handoff",
);

const d11Doc = read(d11DocPath);
requireAll(
  d11Doc,
  [
    "A later retention/review step may inspect the generated aggregate artifact",
    "only then decide whether the canonical D0 status can be promoted",
  ],
  "D11 handoff to retained review",
);

const plan = read(planPath);
const forum23Start = plan.indexOf("## `FORUM-23` — search/index integration");
const forum24Start = plan.indexOf("## `FORUM-24` — localized routes", forum23Start);
assert.ok(forum23Start >= 0 && forum24Start > forum23Start);
const forum23 = plan.slice(forum23Start, forum24Start);
requireAll(
  forum23,
  [
    "**Status:** `in_progress`",
    "FORUM-23B2G2B3D0",
    "source_ready_maintainer_execution_pending",
    "`LINK-FORUM-03` cross-module runtime proof",
  ],
  "FORUM-23 canonical pending boundary",
);
forbidAll(
  forum23,
  [
    "runtime_evidence_reviewed",
    "FORUM-23 is complete",
    "LINK-FORUM-03 is complete",
  ],
  "FORUM-23 canonical pending boundary",
);

console.log(
  "Forum Search retained aggregate review and promotion-candidate gate is fail-closed and source-ready.",
);
