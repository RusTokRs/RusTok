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
  "crates/rustok-forum/contracts/forum-search-link-forum-03-complete-evidence-assembler.json";
const docPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d17-complete-link-evidence-assembler.md";
const assemblerPath =
  "scripts/evidence/assemble-link-forum-03-complete-forum-search-evidence.mjs";
const verifierPath =
  "scripts/verify/verify-link-forum-03-complete-forum-search-evidence.mjs";
const outputPath =
  "target/link-forum-03-forum-index-search-complete-evidence.json";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const d13ContractPath =
  "crates/rustok-forum/contracts/forum-search-link-forum-03-evidence-assembler.json";
const d14ContractPath =
  "crates/rustok-forum/contracts/forum-search-link-forum-03-translation-moderation-proof.json";
const d15ContractPath =
  "crates/rustok-forum/contracts/forum-search-link-forum-03-private-trusted-exclusion-proof.json";
const d16ContractPath =
  "crates/rustok-forum/contracts/forum-search-link-forum-03-topic-move-proof.json";
const d13ArtifactPath =
  "target/link-forum-03-forum-index-search-ordering-visibility-evidence.json";
const d14ArtifactPath =
  "target/forum-search-link-forum-03-translation-moderation-evidence.json";
const d15ArtifactPath =
  "target/forum-search-link-forum-03-private-trusted-exclusion-evidence.json";
const d16ArtifactPath =
  "target/forum-search-link-forum-03-topic-move-evidence.json";
const scenarios = [
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
  "forum_search_link_forum_03_complete_evidence_assembler_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D17");
assert.equal(contract.target_link, "LINK-FORUM-03");
assert.equal(contract.coverage, "complete_canonical_runtime_scope_review_pending");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan, planPath);
assert.equal(contract.assembler, assemblerPath);
assert.equal(contract.verifier, verifierPath);
assert.deepEqual(contract.required_contracts, [
  d13ContractPath,
  d14ContractPath,
  d15ContractPath,
  d16ContractPath,
]);
assert.deepEqual(contract.required_inputs, [
  d13ArtifactPath,
  d14ArtifactPath,
  d15ArtifactPath,
  d16ArtifactPath,
]);
assert.deepEqual(contract.required_scenarios, scenarios);
assert.equal(contract.output_artifact.path, outputPath);
assert.equal(
  contract.output_artifact.status,
  "complete_runtime_evidence_assembled_review_pending",
);
assert.equal(contract.output_artifact.hand_editing_forbidden, true);
assert.equal(contract.output_artifact.source_commit_required, true);
assert.equal(contract.output_artifact.same_commit_inputs_required, true);
assert.equal(contract.output_artifact.atomic_replace, true);
assert.equal(contract.output_artifact.automatic_canonical_source_mutation, false);
assert.ok(contract.fail_closed_requirements.length >= 14);
assert.ok(contract.proves_after_maintainer_execution.length >= 6);
assert.ok(contract.remaining_after_assembly.length >= 3);
assert.ok(contract.maintainer_commands.includes(`node ${verifierPath}`));
assert.ok(contract.maintainer_commands.includes(`node ${assemblerPath}`));
assert.ok(
  contract.non_claims.some((claim) => claim.includes("cannot mark LINK-FORUM-03 done")),
);
assert.ok(
  contract.non_claims.some((claim) => claim.includes("cannot promote the independent FORUM-21")),
);

const sourceContracts = [
  [
    d13ContractPath,
    "forum_search_link_forum_03_evidence_assembler_v1",
    "FORUM-23B2G2B3D13",
  ],
  [
    d14ContractPath,
    "forum_search_link_forum_03_translation_moderation_proof_v1",
    "FORUM-23B2G2B3D14",
  ],
  [
    d15ContractPath,
    "forum_search_link_forum_03_private_trusted_exclusion_proof_v1",
    "FORUM-23B2G2B3D15",
  ],
  [
    d16ContractPath,
    "forum_search_link_forum_03_topic_move_proof_v1",
    "FORUM-23B2G2B3D16",
  ],
];
for (const [path, identity, task] of sourceContracts) {
  const source = JSON.parse(read(path));
  assert.equal(source.contract, identity, `${task} contract identity drifted`);
  assert.equal(source.task, task, `${task} task identity drifted`);
  assert.equal(
    source.status,
    "source_ready_maintainer_execution_pending",
    `${task} source status drifted`,
  );
}

const assembler = read(assemblerPath);
requireAll(
  assembler,
  [
    "FORUM-23B2G2B3D17",
    "link_forum_03_forum_index_search_complete_evidence_v1",
    "complete_runtime_evidence_assembled_review_pending",
    "canonical_link_forum_03_runtime_scope",
    d13ArtifactPath,
    d14ArtifactPath,
    d15ArtifactPath,
    d16ArtifactPath,
    outputPath,
    "execFileSync(\"git\", [\"rev-parse\", \"HEAD\"]",
    "this assembler accepts no command-line arguments",
    "Object.keys(artifact.selected_scenario_evidence)",
    "coreScenarioIds",
    "completeScenarioIds",
    "source_commit !== head",
    "database_backend !== \"postgresql\"",
    "artifact.broker_used !== false",
    "scenario.result !== \"passed\"",
    "english_topic_remained_visible",
    "french_topic_became_visible",
    "approved_reply_visible_after_approval",
    "pending_reply_visible_before_approval !== false",
    "legitimate_private_topic_documents !== 0",
    "legitimate_trusted_topic_documents !== 0",
    "stale_search_rows_injected !== 2",
    "trusted_exact_member_allowed",
    "topic_identity_retained",
    "reply_identity_retained",
    "source_category_scope_empty_after_move",
    "target_category_scope_contains_topic_and_reply_after_move",
    "exact_replay_created_new_owner_revision",
    "requireDigest(artifact.retained_lineage[field]",
    "external_retention_authentication_performed_by_d17: false",
    "complete_artifact_independently_reviewed: false",
    "complete_artifact_retention_attested: false",
    "status_change_allowed_from_this_artifact: false",
    "separate_review_gate_required: true",
    "separate_canonical_source_pull_request_required: true",
    "closes_forum_21_automatically: false",
    "closes_forum_23_automatically: false",
    "closes_link_forum_03_automatically: false",
    "writeFileSync(temporaryOutput",
    "flag: \"wx\"",
    "renameSync(temporaryOutput, absoluteOutput)",
    "rmSync(temporaryOutput, { force: true })",
  ],
  "D17 assembler",
);
forbidAll(
  assembler,
  [
    "process.argv[2]",
    "ALLOW_MISSING",
    "bestEffort",
    "result === \"skipped\"",
    "status_change_allowed_from_this_artifact: true",
    "complete_artifact_independently_reviewed: true",
    "complete_artifact_retention_attested: true",
    "writeFileSync(resolve(root, planPath)",
    "writeFileSync(resolve(root, d13ArtifactPath)",
    "writeFileSync(resolve(root, d14ArtifactPath)",
    "writeFileSync(resolve(root, d15ArtifactPath)",
    "writeFileSync(resolve(root, d16ArtifactPath)",
  ],
  "D17 assembler",
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
    "| `FORUM-23` | `done` |",
    "| `LINK-FORUM-03` | `done` |",
    "FORUM-23B2G2B3D17 closes LINK-FORUM-03",
  ],
  "Forum canonical plan",
);

const doc = read(docPath);
requireAll(
  doc,
  [
    "`source_ready_maintainer_execution_pending`",
    "FORUM-23B2G2B3D17",
    "LINK-FORUM-03",
    contractPath,
    assemblerPath,
    verifierPath,
    outputPath,
    d13ArtifactPath,
    d14ArtifactPath,
    d15ArtifactPath,
    d16ArtifactPath,
    "complete_runtime_evidence_assembled_review_pending",
    "complete_artifact_independently_reviewed = false",
    "complete_artifact_retention_attested = false",
    "status_change_allowed_from_this_artifact = false",
    "A later bounded reviewer slice",
    "No command above was run by the implementation agent",
  ],
  "D17 handoff",
);

console.log(
  "LINK-FORUM-03 complete runtime evidence assembler is source-ready and remains review-pending.",
);
