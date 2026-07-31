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
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-poison-ambiguity-source-proof.json";
const documentPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d5-versioned-invalidation-poison-ambiguity.md";
const testPath =
  "crates/rustok-search/tests/forum_versioned_invalidation_poison_ambiguity_iggy.rs";
const manifestPath = "crates/rustok-search/Cargo.toml";
const parentPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json";

const contract = JSON.parse(read(contractPath));
assert.equal(
  contract.contract,
  "forum_search_versioned_invalidation_poison_ambiguity_source_proof_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D5");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.runtime_evidence_parent, parentPath);
assert.equal(contract.predecessor.task, "FORUM-23B2G2B3D4");
assert.equal(contract.predecessor.pull_request, 2775);
assert.equal(contract.predecessor.state_at_authorship, "merged");
assert.equal(
  contract.predecessor.merge_commit,
  "b612786020859dca377f2e32971b491fbd14644a",
);
assert.equal(
  contract.predecessor.parent_registration,
  "D4_registered_D5_deferred_until_D5_merge",
);
assert.equal(contract.test, testPath);
assert.equal(
  contract.evidence_artifact.path,
  "target/forum-search-versioned-invalidation-poison-ambiguity-evidence.json",
);
assert.equal(contract.evidence_artifact.generation, "executable_test_only");
assert.equal(contract.evidence_artifact.hand_editing_forbidden, true);
assert.equal(contract.evidence_artifact.source_commit_required, true);
assert.equal(contract.evidence_artifact.written_only_after_all_modes_pass, true);
assert.equal(contract.required_runtime.database, "postgresql");
assert.equal(contract.required_runtime.delivery_profile, "outbox_iggy");
assert.equal(
  contract.required_runtime.consumer_group,
  "rustok-search-forum-projection-v1",
);
assert.equal(contract.required_runtime.source_topic, "domain");
assert.equal(contract.required_runtime.dlq_topic, "dlq");
assert.equal(contract.required_runtime.distinct_iggy_addresses_required, true);
assert.deepEqual(
  contract.required_runtime.iggy_instances.map(({ mode }) => mode),
  ["dedup_enabled", "dedup_disabled"],
);
assert.deepEqual(
  contract.required_runtime.iggy_instances.map(
    ({ expected_physical_dlq_messages_after_retry }) =>
      expected_physical_dlq_messages_after_retry,
  ),
  [1, 2],
);
assert.deepEqual(
  contract.scenarios.map(({ id }) => id),
  [
    "raw_poison_publish_mark_ambiguity_dedup_enabled",
    "raw_poison_publish_mark_ambiguity_dedup_disabled",
  ],
);
for (const scenario of contract.scenarios) {
  assert.ok(Array.isArray(scenario.proves) && scenario.proves.length >= 5);
}
assert.ok(contract.identity_invariants.length >= 5);
assert.ok(contract.non_claims.length >= 7);

const test = read(testPath);
requireAll(
  test,
  [
    "raw_poison_publish_mark_ambiguity_obeys_configured_dedup_modes",
    "RUSTOK_FORUM_SEARCH_POISON_DEDUP_ENABLED_IGGY_ADDRESS",
    "RUSTOK_FORUM_SEARCH_POISON_DEDUP_DISABLED_IGGY_ADDRESS",
    "ensure_distinct_mode_addresses",
    "FORUM_SEARCH_CONTRACT_CONSUMER_GROUP",
    "FORUM_SEARCH_CONTRACT_TOPIC",
    "rustok_iggy_connector::migrations::migrations()",
    "PersistentContractDelivery::DecodeFailure",
    "ConsumerPoisonIdentity::new",
    "ConsumerPoisonPublishClaim::Claimed",
    "ConsumerPoisonPublishClaim::Busy",
    "ConsumerPoisonReceiptError::ClaimLost",
    "ConsumerPoisonReceiptState::Publishing",
    "ConsumerPoisonReceiptState::Published",
    "ConsumerPoisonReceiptState::Acknowledged",
    "broker_message_id()",
    "move_to_dlq",
    "mark_published",
    "acknowledge_decode_failure",
    "mark_acknowledged",
    "tokio::time::sleep(LEASE_RECLAIM_WAIT)",
    "expected_physical_dlq_messages",
    "observed_physical_dlq_messages",
    "message_count",
    "target/forum-search-versioned-invalidation-poison-ambiguity-evidence.json",
    "FORUM-23B2G2B3D5",
    "source_commit()",
  ],
  "D5 external-Iggy poison ambiguity test",
);
forbidAll(
  test,
  [
    "ForumSearchContractIngress",
    "SearchModule.migrations()",
    "forum.search_projection.contract_inbox_identity_conflict",
    "search_projection_inbox",
  ],
  "D5 external-Iggy poison ambiguity test",
);

const manifest = read(manifestPath);
requireAll(
  manifest,
  [
    "[dev-dependencies]",
    "iggy.workspace = true",
    "rustok-iggy.workspace = true",
    'rustok-iggy-connector = { workspace = true, features = ["migrations"] }',
  ],
  "rustok-search test dependencies",
);

const document = read(documentPath);
requireAll(
  document,
  [
    "FORUM-23B2G2B3D5",
    "source_ready_maintainer_execution_pending",
    contractPath,
    testPath,
    "publish deterministic DLQ entry",
    "simulate process loss before mark_published",
    "reject the stale publisher with ClaimLost",
    "dedup-enabled instance must contain one physical DLQ message",
    "dedup-disabled instance must contain two physical messages",
    "merged through PR #2775",
    "D5 remains a separate publish/mark ambiguity proof",
    "No command above was run by the implementation agent",
  ],
  "D5 handoff",
);
forbidAll(document, ["pending PR #2772"], "D5 handoff");

const parent = JSON.parse(read(parentPath));
assert.equal(parent.task, "FORUM-23B2G2B3D0");
assert.equal(parent.status, "source_ready_maintainer_execution_pending");
assert.ok(
  parent.source_ready_subproofs.some(
    ({ task }) => task === "FORUM-23B2G2B3D4",
  ),
  "D0 parent must retain the merged D4 raw-poison subproof",
);
assert.ok(
  !parent.source_ready_subproofs.some(
    ({ task }) => task === "FORUM-23B2G2B3D5",
  ),
  "D0 parent must not register the unmerged D5 subproof",
);
assert.ok(
  parent.required_scenarios.some(
    ({ id }) => id === "raw_poison_dlq_redelivery",
  ),
  "D0 parent must retain the raw poison/DLQ runtime scenario",
);
assert.equal(parent.evidence_artifact.generation, "executable_runtime_only");
assert.equal(parent.evidence_artifact.hand_editing_forbidden, true);

console.log(
  "Forum Search D5 external-Iggy poison publish/mark ambiguity source proof is internally consistent.",
);
