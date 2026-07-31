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
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-raw-poison-proof.json";
const parentContractPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json";
const docPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d4-raw-poison-proof.md";
const testPath =
  "crates/rustok-search/tests/forum_versioned_invalidation_raw_poison_iggy.rs";
const cargoPath = "crates/rustok-search/Cargo.toml";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const decodeFailurePath = "crates/rustok-iggy/src/contract_decode_failure.rs";
const dlqPublisherPath = "crates/rustok-iggy/src/dlq_publisher.rs";
const evidencePath =
  "target/forum-search-versioned-invalidation-raw-poison-evidence.json";

const contract = JSON.parse(read(contractPath));
assert.equal(
  contract.contract,
  "forum_search_versioned_invalidation_raw_poison_proof_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D4");
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
  contract.required_runtime.broker_address_env,
  "RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS",
);
assert.equal(contract.required_runtime.serialization, "json");
assert.equal(
  contract.required_runtime.consumer_group,
  "rustok-search-forum-projection-v1",
);
assert.equal(contract.required_runtime.topic, "domain");
assert.equal(contract.required_runtime.partitions, 1);
assert.equal(contract.scenario.id, "raw_poison_dlq_redelivery");
assert.equal(contract.scenario.poison_kind, "contract_deserialization_failure");
assert.ok(Array.isArray(contract.scenario.proves));
assert.equal(contract.scenario.proves.length, 6);
assert.ok(
  contract.maintainer_command.includes(
    "--test forum_versioned_invalidation_raw_poison_iggy",
  ),
);

const test = read(testPath);
requireAll(
  test,
  [
    "RUSTOK_SEARCH_TEST_DATABASE_URL",
    "RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS",
    "rustok_iggy_connector::migrations::migrations()",
    "IggyMode::External",
    "SerializationFormat::Json",
    "domain_partitions: 1",
    "FORUM_SEARCH_CONTRACT_CONSUMER_GROUP",
    "FORUM_SEARCH_CONTRACT_TOPIC",
    "open_persistent_contract_consumer_group",
    "ContractDecodeFailureKind::Deserialize",
    "iggy.contract.decode_invalid",
    "ConsumerPoisonReceiptStore::new",
    "ConsumerPoisonPublishClaim::Claimed",
    "ConsumerPoisonReceiptState::Publishing",
    "first_transport.move_to_dlq(first_dlq_entry).await?",
    "ConsumerPoisonReceiptState::Published",
    "first_transport.shutdown().await?",
    "ConsumerPoisonPublishClaim::AlreadyPublished",
    "assert_no_duplicate_dlq_message",
    "acknowledge_decode_failure(&redelivered)",
    "store.mark_acknowledged(&identity).await?",
    "ConsumerPoisonReceiptState::Acknowledged",
    "next_failure.offset() <= first_offset",
    "first_dlq_entry.broker_message_id() != Some(first_delivery_id)",
    "duplicate_dlq_message_observed\": false",
    evidencePath,
    ".args([\"rev-parse\", \"HEAD\"])",
    "delivery_profile: \"outbox_iggy\"",
  ],
  "Forum Search raw poison executable proof",
);
forbidAll(
  test,
  [
    "#[ignore]",
    "RUSTOK_FORUM_SEARCH_CONTRACT_CONSUMER_ENABLED",
    "ForumSearchContractIngress",
    "SearchProjector",
    "owner_checkpoint",
    "ForumProjectionReconciler",
    "semantic_poison_descriptor",
  ],
  "Forum Search raw poison executable proof",
);

const cargo = read(cargoPath);
const devDependenciesStart = cargo.indexOf("[dev-dependencies]");
const featuresStart = cargo.indexOf("[features]", devDependenciesStart);
assert.ok(devDependenciesStart >= 0 && featuresStart > devDependenciesStart);
const devDependencies = cargo.slice(devDependenciesStart, featuresStart);
requireAll(
  devDependencies,
  [
    "rustok-iggy.workspace = true",
    "rustok-iggy-connector = { workspace = true, features = [\"migrations\"] }",
    "tokio.workspace = true",
  ],
  "rustok-search dev dependencies",
);
assert.equal(
  cargo.slice(0, devDependenciesStart).includes("rustok-iggy-connector"),
  false,
  "rustok-iggy-connector must remain a test-only Search dependency",
);

const decodeFailure = read(decodeFailurePath);
requireAll(
  decodeFailure,
  [
    "CONTRACT_DECODE_FAILURE_ID_DOMAIN",
    "pub fn delivery_id(&self) -> Uuid",
    "self.stream.as_bytes()",
    "self.topic.as_bytes()",
    "self.partition.to_be_bytes()",
    "self.source_offset.to_be_bytes()",
    "self.raw_payload",
    ".with_broker_message_id(delivery_id)",
  ],
  "connector raw poison identity",
);

const dlqPublisher = read(dlqPublisherPath);
requireAll(
  dlqPublisher,
  [
    "deterministic Iggy message ID",
    "entry.broker_message_id()",
    ".id(message_id.as_u128())",
    ".payload(entry.payload.clone().into())",
  ],
  "deterministic Iggy DLQ publisher",
);

const doc = read(docPath);
requireAll(
  doc,
  [
    "`source_ready_maintainer_execution_pending`",
    "FORUM-23B2G2B3D4",
    contractPath,
    testPath,
    evidencePath,
    "`rustok-search-forum-projection-v1`",
    "`AlreadyPublished`",
    "No second physical DLQ message may appear",
    "does not execute the server-owned Forum Search worker loop",
    "No command above was run by the implementation agent",
  ],
  "Forum Search raw poison proof handoff",
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
for (const task of [
  "FORUM-23B2G2B3D2",
  "FORUM-23B2G2B3D3",
  "FORUM-23B2G2B3D4",
]) {
  assert.ok(
    parent.source_ready_subproofs.some((subproof) => subproof.task === task),
    `${task} disappeared from the D0 source-ready subproof list`,
  );
}
const subproof = parent.source_ready_subproofs.find(
  ({ task }) => task === "FORUM-23B2G2B3D4",
);
assert.equal(subproof.contract, contractPath);
assert.equal(subproof.test, testPath);
assert.equal(subproof.evidence_artifact, evidencePath);
assert.deepEqual(subproof.covers, [
  "stable_connector_delivery_identity",
  "durable_publishing_and_published_before_source_acknowledgement",
  "deterministic_dlq_broker_message_identity",
  "same_offset_poison_redelivery_after_consumer_restart",
  "already_published_suppresses_duplicate_dlq_publication",
  "source_acknowledgement_then_acknowledged_state_and_group_advancement",
]);
assert.deepEqual(subproof.does_not_cover, [
  "server_worker_retry_backoff",
  "dlq_publish_failure_or_publish_mark_ambiguity",
  "semantic_identity_conflict_poison",
  "projector_or_owner_checkpoint",
  "multi_process_or_visibility_end_to_end",
]);
assert.ok(
  parent.maintainer_commands.includes(
    "node scripts/verify/verify-forum-search-versioned-invalidation-raw-poison-proof.mjs",
  ),
);
assert.ok(
  parent.maintainer_commands.some((command) =>
    command.includes("--test forum_versioned_invalidation_raw_poison_iggy"),
  ),
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
    "execute and retain every D0 PostgreSQL/Iggy scenario",
    "`LINK-FORUM-03` cross-module runtime proof",
  ],
  "FORUM-23 canonical aggregate boundary",
);
forbidAll(
  forum23,
  [
    "D4 closes FORUM-23",
    "raw poison runtime evidence passed",
    "LINK-FORUM-03 is complete",
  ],
  "FORUM-23 canonical aggregate boundary",
);

console.log(
  "Forum Search raw poison receipt and deterministic DLQ proof is source-synchronized.",
);
