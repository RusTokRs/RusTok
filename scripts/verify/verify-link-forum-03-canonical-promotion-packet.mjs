#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const contractPath =
  "crates/rustok-forum/contracts/forum-search-link-forum-03-canonical-promotion-packet.json";
const reviewContractPath =
  "crates/rustok-forum/contracts/forum-search-link-forum-03-complete-evidence-promotion.json";
const docsPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d19-link-forum-03-canonical-promotion-packet.md";
const builderPath =
  "scripts/evidence/prepare-link-forum-03-canonical-promotion-packet.mjs";
const verifierPath =
  "scripts/verify/verify-link-forum-03-canonical-promotion-packet.mjs";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const candidatePath =
  "target/link-forum-03-forum-index-search-complete-promotion-candidate.json";
const completePath =
  "target/link-forum-03-forum-index-search-complete-evidence.json";
const outputPath = "target/link-forum-03-canonical-promotion-packet.json";
const plannedRow =
  "| `LINK-FORUM-03` | `planned` | Forum/index/search ordering and visibility proof. |";
const doneRow =
  "| `LINK-FORUM-03` | `done` | D13-D18 provide reviewed and retained Forum/index/search ordering, recovery, multilingual, moderation, private/trusted exclusion and topic-move evidence. |";
const scenarios = [
  "normal_delivery",
  "deletion_acl_ordering",
  "search_disabled_profile",
  "translation_and_moderation_approval",
  "private_and_trusted_channel_exclusion",
  "topic_move_category_scope",
];
const sourcePaths = [
  "target/link-forum-03-forum-index-search-ordering-visibility-evidence.json",
  "target/forum-search-link-forum-03-translation-moderation-evidence.json",
  "target/forum-search-link-forum-03-private-trusted-exclusion-evidence.json",
  "target/forum-search-link-forum-03-topic-move-evidence.json",
];

function read(path) {
  return readFileSync(path, "utf8");
}

function count(text, marker) {
  return text.split(marker).length - 1;
}

function includesAll(text, markers, label) {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing marker: ${marker}`);
  }
}

const contract = JSON.parse(read(contractPath));
const reviewContract = JSON.parse(read(reviewContractPath));
const docs = read(docsPath);
const builder = read(builderPath);
const verifier = read(verifierPath);
const plan = read(planPath);

assert.equal(
  contract.contract,
  "forum_search_link_forum_03_canonical_promotion_packet_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D19");
assert.equal(contract.target_link, "LINK-FORUM-03");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan, planPath);
assert.equal(contract.review_contract, reviewContractPath);
assert.equal(contract.promotion_candidate, candidatePath);
assert.equal(contract.complete_artifact, completePath);
assert.equal(contract.packet_builder, builderPath);
assert.equal(contract.verifier, verifierPath);
assert.deepEqual(contract.output_packet, {
  path: outputPath,
  status: "ready_for_separate_canonical_source_pull_request",
  hand_editing_forbidden: true,
  source_commit_required: true,
  atomic_replace: true,
  automatic_canonical_source_mutation: false,
});
assert.deepEqual(contract.required_plan_state, {
  forum_21: "planned",
  forum_23: "in_progress",
  link_forum_03: "planned",
});
assert.deepEqual(contract.required_ledger_transition, {
  before: plannedRow,
  after: doneRow,
  exact_before_occurrences: 1,
  exact_after_occurrences_before_promotion: 0,
});
assert.deepEqual(contract.proposed_transition, {
  task: "LINK-FORUM-03",
  from: "planned",
  to: "done",
  requires_separate_canonical_source_pull_request: true,
  canonical_source_mutated_by_builder: false,
  promotes_forum_21: false,
  promotes_forum_23: false,
});
assert.equal(contract.fail_closed_requirements.length, 11);
assert.equal(contract.required_completion_record.length, 5);
assert.deepEqual(contract.maintainer_commands, [
  `node ${verifierPath}`,
  `node ${builderPath}`,
]);

assert.equal(
  reviewContract.contract,
  "forum_search_link_forum_03_complete_evidence_promotion_v1",
);
assert.equal(reviewContract.task, "FORUM-23B2G2B3D18");
assert.equal(reviewContract.target_link, "LINK-FORUM-03");
assert.equal(reviewContract.status, "source_ready_maintainer_execution_pending");
assert.equal(reviewContract.canonical_plan, planPath);
assert.equal(reviewContract.complete_artifact, completePath);
assert.equal(reviewContract.promotion_candidate.path, candidatePath);
assert.deepEqual(reviewContract.required_scenarios, scenarios);
assert.deepEqual(reviewContract.required_source_artifacts, sourcePaths);
assert.deepEqual(reviewContract.proposed_transition, {
  task: "LINK-FORUM-03",
  from: "planned",
  to: "done",
  requires_separate_canonical_source_pull_request: true,
  canonical_source_mutated_by_reviewer: false,
  promotes_forum_21: false,
  promotes_forum_23: false,
});

assert.equal(count(plan, plannedRow), 1);
assert.equal(count(plan, doneRow), 0);
assert.ok(
  plan.includes(
    "| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |",
  ),
);
assert.ok(plan.includes("| `FORUM-23` | `in_progress` |"));
assert.ok(!plan.includes("| `LINK-FORUM-03` | `done` |"));

includesAll(
  builder,
  [
    `const contractPath =\n  "${contractPath}";`,
    `const reviewContractPath =\n  "${reviewContractPath}";`,
    `const planPath = "${planPath}";`,
    `const candidatePath =\n  "${candidatePath}";`,
    `const completePath =\n  "${completePath}";`,
    `const outputPath = "${outputPath}";`,
    "git\", [\"rev-parse\", \"HEAD\"]",
    "this packet builder accepts no command-line arguments",
    "candidate.retention.attested_sha256 !== complete.sha256",
    "candidate.canonical_plan?.sha256 !== planDigest",
    "candidate.review_contract?.sha256 !== sha256(reviewContractBytes)",
    "candidate.proposed_transition?.separate_canonical_source_pull_request_required !== true",
    "contract.proposed_transition?.promotes_forum_21 !== false",
    "contract.proposed_transition?.promotes_forum_23 !== false",
    "exact_before_occurrences: 1",
    "exact_after_occurrences_before_promotion: 0",
    "automatic_application_performed: false",
    "canonical_source_mutated_by_builder: false",
    "writeFileSync(temporaryOutput",
    "renameSync(temporaryOutput, absoluteOutput)",
    "flag: \"wx\"",
  ],
  "packet builder",
);
for (const scenario of scenarios) {
  assert.ok(builder.includes(`"${scenario}"`), `builder is missing ${scenario}`);
}
for (const path of sourcePaths) {
  assert.ok(builder.includes(path), `builder is missing source path ${path}`);
}
for (const marker of [
  "approved_for_canonical_status_promotion",
  "complete_runtime_evidence_assembled_review_pending",
  "canonical_link_forum_03_runtime_scope",
  "all_six_canonical_scenarios_revalidated",
  "all_four_source_artifacts_revalidated",
  "all_source_digests_match_complete_artifact",
  "all_scenario_facts_match_retained_sources",
  "retained_digest_attested_by_maintainer",
  "canonical_plan_remained_unmodified",
]) {
  assert.ok(builder.includes(marker), `builder is missing candidate marker ${marker}`);
}

for (const forbidden of [
  "writeFileSync(planPath",
  "writeFileSync(resolve(root, planPath)",
  "renameSync(temporaryOutput, resolve(root, planPath))",
  "process.argv[2]",
  "ALLOW_MISSING",
  "SKIP_VALIDATION",
  "sourceCommitOverride",
  "bestEffort",
]) {
  assert.ok(!builder.includes(forbidden), `builder contains forbidden marker: ${forbidden}`);
}

includesAll(
  docs,
  [
    "# FORUM-23B2G2B3D19 LINK-FORUM-03 canonical promotion packet",
    "`source_ready_maintainer_execution_pending`",
    contractPath,
    builderPath,
    outputPath,
    candidatePath,
    completePath,
    plannedRow,
    doneRow,
    "LINK-FORUM-03: planned -> done",
    "canonical_source_mutated_by_builder = false",
    "promotes_forum_21 = false",
    "promotes_forum_23 = false",
    "No command above was run by the implementation agent",
  ],
  "D19 handoff",
);
for (const scenario of scenarios) {
  assert.ok(
    contract.required_completion_record.some((value) =>
      value.includes("six canonical LINK-FORUM-03 scenarios"),
    ),
    `contract completion record does not cover ${scenario}`,
  );
}

assert.ok(verifier.includes("automatic_canonical_source_mutation: false"));
assert.ok(verifier.includes("canonical_source_mutated_by_builder: false"));
assert.ok(verifier.includes("promotes_forum_21: false"));
assert.ok(verifier.includes("promotes_forum_23: false"));

console.log(
  "LINK-FORUM-03 canonical promotion packet source is ready and canonical status remains unchanged.",
);
