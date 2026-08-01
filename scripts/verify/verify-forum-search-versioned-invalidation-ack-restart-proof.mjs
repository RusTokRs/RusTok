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
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-ack-restart-proof.json";
const parentContractPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json";
const docPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d3-ack-restart-proof.md";
const testPath =
  "apps/server/tests/forum_versioned_invalidation_ack_restart_iggy.rs";
const searchCargoPath = "crates/rustok-search/Cargo.toml";
const serverCargoPath = "apps/server/Cargo.toml";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const evidencePath =
  "target/forum-search-versioned-invalidation-ack-restart-evidence.json";
const productionGroup = "rustok-search-forum-projection-v1";

const contract = JSON.parse(read(contractPath));
assert.equal(
  contract.contract,
  "forum_search_versioned_invalidation_ack_restart_proof_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D3");
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
assert.equal(contract.required_runtime.consumer_group, productionGroup);
assert.equal(contract.required_runtime.topic, "domain");
assert.equal(contract.required_runtime.partitions, 1);
assert.equal(contract.scenario.id, "acknowledgement_failure_restart");
assert.equal(
  contract.scenario.injected_failure,
  "replace the exact delivery acknowledgement token with a validly encoded token carrying a different consumer identity",
);
assert.ok(Array.isArray(contract.scenario.proves));
assert.equal(contract.scenario.proves.length, 5);
assert.ok(
  contract.maintainer_command.includes(
    testPath.split("/").at(-1).replace(".rs", ""),
  ),
);
assert.ok(contract.maintainer_command.includes("cargo test -p rustok-server"));

const test = read(testPath);
requireAll(
  test,
  [
    "RUSTOK_SEARCH_TEST_DATABASE_URL",
    "RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS",
    "SearchModule.migrations()",
    "IggyMode::External",
    "SerializationFormat::Json",
    "domain_partitions: 1",
    "FORUM_SEARCH_CONTRACT_CONSUMER_GROUP",
    "FORUM_SEARCH_CONTRACT_TOPIC",
    "open_persistent_contract_consumer_group",
    "publish_contract(first_envelope)",
    "publish_contract(second_envelope)",
    "ForumSearchContractIngress::new",
    "first_transport.shutdown().await?",
    ".to_string();",
    "{exact_ack_token}-injected-failure",
    "expect_err(\"mismatched acknowledgement token must fail before offset commit\")",
    "ack token does not match the outstanding Iggy consumer-group delivery",
    "redelivered_offset != first_offset",
    "redelivered.raw_payload() != first_raw_payload.as_slice()",
    "after_restart != before_restart",
    "count_event_rows(db, first_root_event_id).await? != 1",
    "restarted_group.acknowledge(&redelivered).await?",
    "next_offset <= first_offset",
    "second_snapshot.ingest_sequence <= after_restart.ingest_sequence",
    evidencePath,
    ".args([\"rev-parse\", \"HEAD\"])",
    "delivery_profile: \"outbox_iggy\"",
  ],
  "Iggy acknowledgement/restart executable proof",
);
assert.equal(
  (test.match(/FORUM_SEARCH_CONTRACT_CONSUMER_GROUP/g) ?? []).length >= 4,
  true,
  "the production consumer-group constant must drive open, reopen and evidence",
);
forbidAll(
  test,
  [
    "#[ignore]",
    "unique_name(\"consumer\")",
    "RUSTOK_FORUM_SEARCH_CONTRACT_CONSUMER_ENABLED",
    "ConsumerPoisonReceiptStore",
    "move_to_dlq",
    "owner_checkpoint",
    "ForumProjectionReconciler",
    "SearchProjector",
  ],
  "Iggy acknowledgement/restart executable proof",
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
    "FORUM-23B2G2B3D3",
    contractPath,
    testPath,
    evidencePath,
    productionGroup,
    "same broker offset",
    "one durable `search_projection_inbox` row",
    "does not run the server-owned consumer loop",
    "No command above was run by the implementation agent",
  ],
  "Iggy acknowledgement/restart proof handoff",
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
assert.ok(
  parent.source_ready_subproofs.some(
    ({ task }) => task === "FORUM-23B2G2B3D2",
  ),
  "D2 PostgreSQL ingress subproof disappeared",
);
const subproof = parent.source_ready_subproofs.find(
  ({ task }) => task === "FORUM-23B2G2B3D3",
);
assert.ok(subproof, "D0 runtime contract does not register the D3 subproof");
assert.equal(subproof.contract, contractPath);
assert.equal(subproof.test, testPath);
assert.equal(subproof.evidence_artifact, evidencePath);
assert.deepEqual(subproof.covers, [
  "durable_ingress_before_acknowledgement",
  "injected_acknowledgement_token_failure",
  "same_offset_redelivery_after_consumer_restart",
  "one_inbox_row_and_stable_ingest_sequence_after_redelivery",
  "successful_restart_acknowledgement_advances_to_next_event",
]);
assert.deepEqual(subproof.does_not_cover, [
  "server_worker_retry_backoff",
  "poison_receipt_or_dlq_publication",
  "projector_or_owner_checkpoint",
  "multi_process_or_visibility_end_to_end",
]);
assert.equal(parent.required_runtime.consumer_group, productionGroup);
assert.ok(
  parent.maintainer_commands.includes(
    "node scripts/verify/verify-forum-search-versioned-invalidation-ack-restart-proof.mjs",
  ),
);
assert.ok(
  parent.maintainer_commands.some(
    (command) =>
      command.includes("cargo test -p rustok-server") &&
      command.includes("--test forum_versioned_invalidation_ack_restart_iggy"),
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
    "D3 closes FORUM-23",
    "acknowledgement failure runtime evidence passed",
    "LINK-FORUM-03 is complete",
  ],
  "FORUM-23 canonical aggregate boundary",
);

console.log(
  "Forum Search versioned invalidation Iggy acknowledgement/restart proof is source-synchronized.",
);
