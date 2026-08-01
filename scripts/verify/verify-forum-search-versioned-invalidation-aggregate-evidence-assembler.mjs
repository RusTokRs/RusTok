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
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-aggregate-evidence-assembler.json";
const parentPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json";
const assemblerPath =
  "scripts/evidence/assemble-forum-search-versioned-invalidation-runtime-evidence.mjs";
const verifierPath =
  "scripts/verify/verify-forum-search-versioned-invalidation-aggregate-evidence-assembler.mjs";
const docPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d11-aggregate-evidence-assembler.md";
const outputPath =
  "target/forum-search-versioned-invalidation-runtime-evidence.json";

const expectedInputs = [
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

const contract = JSON.parse(read(contractPath));
assert.equal(
  contract.contract,
  "forum_search_versioned_invalidation_aggregate_evidence_assembler_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D11");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.runtime_evidence_parent, parentPath);
assert.equal(contract.assembler, assemblerPath);
assert.equal(contract.verifier, verifierPath);
assert.deepEqual(contract.input_artifacts, expectedInputs);
assert.equal(contract.output_artifact.path, outputPath);
assert.equal(
  contract.output_artifact.generation,
  "assembler_only_after_all_runtime_subproofs_pass",
);
assert.equal(contract.output_artifact.hand_editing_forbidden, true);
assert.equal(contract.output_artifact.source_commit_required, true);
assert.equal(contract.output_artifact.atomic_replace, true);
assert.ok(contract.fail_closed_requirements.length >= 9);
assert.deepEqual(Object.keys(contract.frozen_scenario_sources), frozenScenarios);
assert.equal(
  contract.maintainer_command,
  "node scripts/evidence/assemble-forum-search-versioned-invalidation-runtime-evidence.mjs",
);

const parent = JSON.parse(read(parentPath));
assert.equal(parent.contract, "forum_search_versioned_invalidation_runtime_evidence_v1");
assert.equal(parent.task, "FORUM-23B2G2B3D0");
assert.equal(parent.status, "source_ready_maintainer_execution_pending");
assert.deepEqual(
  parent.required_scenarios.map(({ id }) => id),
  frozenScenarios,
);
assert.equal(parent.evidence_artifact.path, outputPath);
const d11 = parent.source_ready_subproofs.find(
  ({ task }) => task === "FORUM-23B2G2B3D11",
);
assert.ok(d11, "D0 must register FORUM-23B2G2B3D11");
assert.equal(d11.contract, contractPath);
assert.equal(d11.assembler, assemblerPath);
assert.equal(d11.evidence_artifact, outputPath);
assert.ok(d11.covers.includes("same_source_commit_aggregate_assembly"));
assert.ok(d11.does_not_cover.includes("runtime_execution_or_d0_status_promotion"));
for (const command of [
  `node ${verifierPath}`,
  `node ${assemblerPath}`,
]) {
  assert.ok(parent.maintainer_commands.includes(command));
}

const assembler = read(assemblerPath);
requireAll(
  assembler,
  [
    "execFileSync(\"git\", [\"rev-parse\", \"HEAD\"]",
    "createHash(\"sha256\")",
    "source_ready_maintainer_execution_pending",
    "runtime_evidence_assembled",
    "database_backend: \"postgresql\"",
    "delivery_profile: \"outbox_iggy\"",
    "rustok-search-forum-projection-v1",
    "const canonicalTopic = \"domain\"",
    "artifact.source_commit !== head",
    "artifact.database_backend !== \"postgresql\"",
    "artifact.delivery_profile !== \"outbox_iggy\"",
    "artifact.consumer_group !== canonicalConsumerGroup",
    "artifact.topic !== canonicalTopic",
    "scenario.result !== \"passed\"",
    "Object.keys(value).length === 0",
    "JSON.stringify(parentScenarios) !== JSON.stringify(frozenScenarioIds)",
    "parent.evidence_artifact.required_fields",
    "registration.evidence_artifact !== entry.path",
    "source_artifacts: sourceArtifacts",
    "parent_contract_sha256: sha256(parentBytes)",
    "all_inputs_same_source_commit: true",
    "source_commit_matches_current_head: true",
    "output_written_after_complete_validation: true",
    "writeFileSync(temporaryOutput",
    "renameSync(temporaryOutput, absoluteOutput)",
    "rmSync(temporaryOutput, { force: true })",
    "if (process.argv.length !== 2)",
    "supporting_scenario_results",
    "owner_revision_rows",
    "typed_and_root_event_ids",
    "search_inbox_rows",
    "ingest_sequences",
    "owner_checkpoints",
    "poison_receipts",
    "dlq_receipts",
    "storefront_visibility_assertions",
  ],
  "aggregate evidence assembler",
);
for (const path of expectedInputs) {
  assert.ok(assembler.includes(path), `assembler manifest is missing ${path}`);
}
for (const scenario of frozenScenarios) {
  assert.ok(assembler.includes(`\"${scenario}\"`), `assembler is missing ${scenario}`);
}
forbidAll(
  assembler,
  [
    "process.env.FORUM_SEARCH_SOURCE_COMMIT",
    "process.env.SOURCE_COMMIT",
    "--source-commit",
    "allowMissing",
    "bestEffort",
    "continueOnError",
    "writeFileSync(absoluteOutput",
    "generation: \"static_fixture\"",
    "result: \"skipped\"",
  ],
  "aggregate evidence assembler",
);

const inputSources = [
  [
    "crates/rustok-search/tests/forum_versioned_invalidation_postgres.rs",
    "forum_search_versioned_invalidation_postgres_ingress_evidence_v1",
  ],
  [
    "apps/server/tests/forum_versioned_invalidation_ack_restart_iggy.rs",
    "forum_search_versioned_invalidation_ack_restart_evidence_v1",
  ],
  [
    "apps/server/tests/forum_versioned_invalidation_raw_poison_iggy.rs",
    "forum_search_versioned_invalidation_raw_poison_evidence_v1",
  ],
  [
    "apps/server/tests/forum_versioned_invalidation_semantic_poison_iggy.rs",
    "forum_search_versioned_invalidation_semantic_poison_evidence_v1",
  ],
  [
    "crates/rustok-search/tests/forum_versioned_invalidation_missing_delivery_repair.rs",
    "forum_search_versioned_invalidation_missing_delivery_repair_evidence_v1",
  ],
  [
    "apps/server/tests/forum_versioned_invalidation_multi_process_serialization.rs",
    "forum_search_versioned_invalidation_multi_process_evidence_v1",
  ],
  [
    "apps/server/tests/forum_versioned_invalidation_deletion_acl_ordering.rs",
    "forum_search_versioned_invalidation_deletion_acl_ordering_evidence_v1",
  ],
  [
    "apps/server/tests/forum_versioned_invalidation_search_disabled_recovery.rs",
    "forum_search_versioned_invalidation_search_disabled_recovery_evidence_v1",
  ],
  [
    "apps/server/tests/forum_versioned_invalidation_normal_delivery_iggy.rs",
    "forum_search_versioned_invalidation_normal_delivery_evidence_v1",
  ],
];
for (const [path, evidenceContract] of inputSources) {
  const source = read(path);
  requireAll(
    source,
    [
      evidenceContract,
      "source_commit: String",
      "generated_at: String",
      "database_backend: &'static str",
      "scenario_results: Vec<ScenarioEvidence>",
      "source_commit()?",
      'database_backend: "postgresql"',
    ],
    path,
  );
}

const doc = read(docPath);
requireAll(
  doc,
  [
    "`source_ready_maintainer_execution_pending`",
    "FORUM-23B2G2B3D11",
    contractPath,
    assemblerPath,
    verifierPath,
    outputPath,
    "all nine `source_commit` values",
    "`git rev-parse HEAD`",
    "SHA-256 digest",
    "atomic rename",
    "There is no",
    "No command above was run by the implementation agent",
  ],
  "aggregate evidence assembler handoff",
);

console.log(
  "Forum Search aggregate runtime evidence assembler is fail-closed and source-ready.",
);
