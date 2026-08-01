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
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-semantic-poison-proof.json";
const parentContractPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json";
const docPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d5-semantic-poison-proof.md";
const testPath =
  "apps/server/tests/forum_versioned_invalidation_semantic_poison_iggy.rs";
const workerPath =
  "apps/server/src/services/forum_search_contract_consumer.rs";
const searchCargoPath = "crates/rustok-search/Cargo.toml";
const serverCargoPath = "apps/server/Cargo.toml";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const evidencePath =
  "target/forum-search-versioned-invalidation-semantic-poison-evidence.json";
const stableError =
  "forum.search_projection.contract_inbox_identity_conflict";

const contract = JSON.parse(read(contractPath));
assert.equal(
  contract.contract,
  "forum_search_versioned_invalidation_semantic_poison_proof_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D5");
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
assert.equal(contract.required_runtime.broker, "external_iggy");
assert.equal(
  contract.required_runtime.consumer_group,
  "rustok-search-forum-projection-v1",
);
assert.equal(contract.required_runtime.topic, "domain");
assert.equal(contract.required_runtime.partitions, 1);
assert.equal(contract.scenario.id, "semantic_poison_identity_conflict");
assert.equal(contract.scenario.stable_error_code, stableError);
assert.equal(contract.scenario.proves.length, 6);
assert.ok(
  contract.maintainer_command.includes(
    "--test forum_versioned_invalidation_semantic_poison_iggy",
  ),
);
assert.ok(contract.maintainer_command.includes("cargo test -p rustok-server"));

const test = read(testPath);
requireAll(
  test,
  [
    "SearchModule.migrations()",
    "rustok_iggy_connector::migrations::migrations()",
    "IggyMode::External",
    "SerializationFormat::Json",
    "domain_partitions: 1",
    "FORUM_SEARCH_CONTRACT_CONSUMER_GROUP",
    "FORUM_SEARCH_CONTRACT_TOPIC",
    "insert_legacy_root",
    "ForumSearchContractIngress::new",
    "ForumSearchContractIngressError::InboxIdentityConflict",
    stableError,
    "ContractDecodeFailureKind::SchemaValidation",
    "ConsumedContractDecodeFailure::new",
    "ConsumerPoisonIdentity::new",
    ".with_broker_message_id(delivery_id)",
    "ConsumerPoisonPublishClaim::Claimed",
    "ConsumerPoisonReceiptState::Publishing",
    "first_transport.move_to_dlq(first_entry).await?",
    "ConsumerPoisonReceiptState::Published",
    "first_transport.shutdown().await?",
    "ConsumerPoisonPublishClaim::AlreadyPublished",
    "assert_no_duplicate_dlq_message",
    "restarted_group.acknowledge(&redelivered).await?",
    "store.mark_acknowledged(&identity).await?",
    "ConsumerPoisonReceiptState::Acknowledged",
    "ForumSearchContractIngressOutcome::DurablyAccepted",
    "source_acknowledgement_advanced_group\": true",
    evidencePath,
    ".args([\"rev-parse\", \"HEAD\"])",
  ],
  "Forum Search semantic poison executable proof",
);
forbidAll(
  test,
  [
    "#[ignore]",
    "RUSTOK_FORUM_SEARCH_CONTRACT_CONSUMER_ENABLED",
    "SearchProjector",
    "owner_checkpoint",
    "ForumProjectionReconciler",
    "advisory_lock",
  ],
  "Forum Search semantic poison executable proof",
);

const worker = read(workerPath);
requireAll(
  worker,
  [
    "Forum Search typed invalidation is semantic poison",
    "terminalize_semantic_poison",
    "semantic_poison_descriptor",
    "ContractDecodeFailureKind::SchemaValidation",
    "ConsumedContractDecodeFailure::new",
    "ConsumerPoisonIdentity::new",
    "DlqEntry::new",
    ".with_connector_metadata(consumed.connector_metadata.clone())",
    ".with_broker_message_id(delivery_id)",
    "establish_poison_result",
    "acknowledge_event_with_receipt",
  ],
  "server Forum Search semantic poison protocol",
);

const searchCargo = read(searchCargoPath);
forbidAll(
  searchCargo,
  ["rustok-iggy.workspace = true", "rustok-iggy-connector"],
  "rustok-search owner manifest",
);
const serverCargo = read(serverCargoPath);
requireAll(
  serverCargo,
  [
    "rustok-search = { workspace = true, features = [\"graphql\"] }",
    "rustok-iggy.workspace = true",
    "rustok-iggy-connector = { workspace = true, features = [\"migrations\"] }",
    "tokio.workspace = true",
  ],
  "server host dependencies",
);

const doc = read(docPath);
requireAll(
  doc,
  [
    "`source_ready_maintainer_execution_pending`",
    "FORUM-23B2G2B3D5",
    contractPath,
    testPath,
    evidencePath,
    "`rustok-search-forum-projection-v1`",
    stableError,
    "`AlreadyPublished`",
    "No second physical DLQ message may appear",
    "no new worker API",
    "No command above was run by the implementation agent",
  ],
  "Forum Search semantic poison proof handoff",
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
for (const task of [
  "FORUM-23B2G2B3D2",
  "FORUM-23B2G2B3D3",
  "FORUM-23B2G2B3D4",
  "FORUM-23B2G2B3D5",
]) {
  assert.ok(
    parent.source_ready_subproofs.some((subproof) => subproof.task === task),
    `${task} disappeared from the D0 source-ready subproof list`,
  );
}
const subproof = parent.source_ready_subproofs.find(
  ({ task }) => task === "FORUM-23B2G2B3D5",
);
assert.equal(subproof.contract, contractPath);
assert.equal(subproof.test, testPath);
assert.equal(subproof.evidence_artifact, evidencePath);
assert.deepEqual(subproof.covers, [
  "non_retryable_semantic_identity_conflict",
  "durable_conflict_row_preserved_and_unique",
  "stable_connector_and_deterministic_dlq_identity_for_typed_bytes",
  "published_receipt_before_source_acknowledgement",
  "same_offset_redelivery_and_already_published_duplicate_suppression",
  "acknowledged_receipt_and_group_advancement_to_valid_event",
]);
assert.deepEqual(subproof.does_not_cover, [
  "server_worker_retry_backoff",
  "dlq_publish_failure_or_publish_mark_ambiguity",
  "projector_or_owner_checkpoint",
  "missing_delivery_repair",
  "multi_process_or_visibility_end_to_end",
]);
assert.ok(
  parent.maintainer_commands.includes(
    "node scripts/verify/verify-forum-search-versioned-invalidation-semantic-poison-proof.mjs",
  ),
);
assert.ok(
  parent.maintainer_commands.some(
    (command) =>
      command.includes("cargo test -p rustok-server") &&
      command.includes("--test forum_versioned_invalidation_semantic_poison_iggy"),
  ),
);

const plan = read(planPath);
const forum23Start = plan.indexOf("## `FORUM-23` — search/index integration");
const forum24Start = plan.indexOf(
  "## `FORUM-24` — localized routes",
  forum23Start,
);
assert.ok(forum23Start >= 0 && forum24Start > forum23Start);
const forum23 = plan.slice(forum23Start, forum24Start);
requireAll(
  forum23,
  [
    "**Status:** `in_progress`",
    "FORUM-23B2G2B3D0",
    "source_ready_maintainer_execution_pending",
    "execute and retain every D0 PostgreSQL/Iggy scenario",
    "`LINK-FORUM-03` cross-module runtime proof",
  ],
  "FORUM-23 canonical aggregate boundary",
);
forbidAll(
  forum23,
  [
    "D5 closes FORUM-23",
    "semantic poison runtime evidence passed",
    "LINK-FORUM-03 is complete",
  ],
  "FORUM-23 canonical aggregate boundary",
);

console.log(
  "Forum Search semantic identity-conflict poison proof is source-synchronized.",
);
