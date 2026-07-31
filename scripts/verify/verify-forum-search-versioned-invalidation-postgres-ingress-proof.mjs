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
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-postgres-ingress-proof.json";
const parentContractPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json";
const docPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d2-postgres-ingress-proof.md";
const testPath =
  "crates/rustok-search/tests/forum_versioned_invalidation_postgres.rs";
const evidencePath =
  "target/forum-search-versioned-invalidation-postgres-ingress-evidence.json";

const contract = JSON.parse(read(contractPath));
assert.equal(
  contract.contract,
  "forum_search_versioned_invalidation_postgres_ingress_proof_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D2");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.runtime_evidence_parent, parentContractPath);
assert.equal(contract.test, testPath);
assert.equal(contract.evidence_artifact.path, evidencePath);
assert.equal(contract.evidence_artifact.generation, "executable_test_only");
assert.equal(contract.evidence_artifact.hand_editing_forbidden, true);
assert.equal(contract.evidence_artifact.source_commit_required, true);
assert.equal(contract.required_runtime.database, "postgresql");
assert.equal(
  contract.required_runtime.database_env,
  "RUSTOK_SEARCH_TEST_DATABASE_URL",
);
assert.equal(contract.required_runtime.migrations, "rustok_search::SearchModule");
assert.equal(contract.required_runtime.broker_required, false);
assert.deepEqual(
  contract.scenarios.map(({ id }) => id),
  [
    "typed_ingress_admission",
    "legacy_first_duplicate",
    "typed_first_duplicate",
    "semantic_identity_conflict",
  ],
);
for (const scenario of contract.scenarios) {
  assert.ok(Array.isArray(scenario.proves) && scenario.proves.length >= 3);
  for (const fact of scenario.proves) {
    assert.equal(typeof fact, "string");
    assert.ok(fact.length > 20);
  }
}
assert.ok(
  contract.maintainer_command.includes(
    "cargo test -p rustok-search --test forum_versioned_invalidation_postgres",
  ),
);

const test = read(testPath);
requireAll(
  test,
  [
    "RUSTOK_SEARCH_TEST_DATABASE_URL",
    "SearchModule.migrations()",
    "ForumSearchContractIngress::new",
    "ContractEventEnvelope::new_caused_by",
    "ForumSearchProjectionEvent::InvalidationIssued",
    "ON CONFLICT (event_id) DO NOTHING",
    "typed_ingress_admission",
    "legacy_first_duplicate",
    "typed_first_duplicate",
    "semantic_identity_conflict",
    "forum.search_projection.contract_inbox_identity_conflict",
    "typed transport envelope ID must differ from the root projection identity",
    "typed invalidation must retain the exact legacy root causation ID",
    "validate_registered_schema",
    "ingest_sequence <= 0",
    evidencePath,
    ".args([\"rev-parse\", \"HEAD\"])",
    "broker_used: false",
  ],
  "PostgreSQL ingress executable proof",
);
forbidAll(
  test,
  [
    "#[ignore]",
    "RUSTOK_FORUM_SEARCH_CONTRACT_CONSUMER_ENABLED",
    "IggyTransport",
    "PersistentContractConsumerGroup",
    "ConsumerPoisonReceiptStore",
  ],
  "PostgreSQL ingress executable proof",
);

const doc = read(docPath);
requireAll(
  doc,
  [
    "`source_ready_maintainer_execution_pending`",
    "FORUM-23B2G2B3D2",
    contractPath,
    testPath,
    evidencePath,
    "forum.search_projection.contract_inbox_identity_conflict",
    "No command above was run by the implementation agent",
  ],
  "PostgreSQL ingress proof handoff",
);

const parent = JSON.parse(read(parentContractPath));
assert.equal(parent.task, "FORUM-23B2G2B3D0");
assert.equal(parent.status, "source_ready_maintainer_execution_pending");
assert.deepEqual(
  parent.required_scenarios.map(({ id }) => id),
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
assert.ok(Array.isArray(parent.source_ready_subproofs));
const subproof = parent.source_ready_subproofs.find(
  ({ task }) => task === "FORUM-23B2G2B3D2",
);
assert.ok(subproof, "D0 runtime contract does not register the D2 subproof");
assert.equal(subproof.contract, contractPath);
assert.equal(subproof.test, testPath);
assert.equal(subproof.evidence_artifact, evidencePath);
assert.deepEqual(subproof.covers, [
  "typed_ingress_admission",
  "legacy_first_duplicate",
  "typed_first_duplicate",
  "semantic_identity_conflict_classification",
]);
assert.deepEqual(subproof.does_not_cover, [
  "broker_acknowledgement_or_restart",
  "poison_receipt_or_dlq_publication",
  "projector_or_owner_checkpoint",
  "multi_process_or_visibility_end_to_end",
]);
assert.ok(parent.maintainer_commands.includes(
  "node scripts/verify/verify-forum-search-versioned-invalidation-postgres-ingress-proof.mjs",
));
assert.ok(parent.maintainer_commands.some((command) =>
  command.includes("--test forum_versioned_invalidation_postgres"),
));

console.log(
  "Forum Search versioned invalidation PostgreSQL shared-inbox proof is source-synchronized.",
);
