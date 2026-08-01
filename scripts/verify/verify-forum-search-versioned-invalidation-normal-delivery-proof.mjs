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
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-normal-delivery-proof.json";
const parentContractPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json";
const docPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d10-normal-delivery-proof.md";
const testPath =
  "apps/server/tests/forum_versioned_invalidation_normal_delivery_iggy.rs";
const evidencePath =
  "target/forum-search-versioned-invalidation-normal-delivery-evidence.json";
const projectionInvalidationPath =
  "crates/rustok-forum/src/services/projection_invalidation.rs";
const ingressPath = "crates/rustok-search/src/forum_contract_ingress.rs";
const reconciliationPath = "crates/rustok-search/src/forum_reconciliation.rs";
const ownerCheckpointPath =
  "crates/rustok-search/src/forum_owner_checkpoint.rs";
const projectorPath = "crates/rustok-search/src/forum_projector.rs";
const storefrontExecutionPath =
  "crates/rustok-search/src/forum_storefront_execution.rs";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";

const contract = JSON.parse(read(contractPath));
assert.equal(
  contract.contract,
  "forum_search_versioned_invalidation_normal_delivery_proof_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D10");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.runtime_evidence_parent, parentContractPath);
assert.equal(contract.test, testPath);
assert.equal(contract.evidence_artifact.path, evidencePath);
assert.equal(contract.evidence_artifact.generation, "executable_test_only");
assert.equal(contract.evidence_artifact.hand_editing_forbidden, true);
assert.equal(contract.evidence_artifact.source_commit_required, true);
assert.equal(contract.required_runtime.database, "postgresql");
assert.equal(contract.required_runtime.broker, "external_iggy");
assert.equal(contract.required_runtime.delivery_profile, "outbox_iggy");
assert.equal(
  contract.required_runtime.consumer_group,
  "rustok-search-forum-projection-v1",
);
assert.equal(contract.required_runtime.topic, "domain");
assert.equal(contract.scenario.id, "normal_delivery");
assert.equal(contract.scenario.proves.length, 8);
assert.ok(
  contract.maintainer_command.includes(
    "--test forum_versioned_invalidation_normal_delivery_iggy",
  ),
);

const test = read(testPath);
requireAll(
  test,
  [
    "RUSTOK_SEARCH_TEST_DATABASE_URL",
    "RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS",
    "OutboxModule.migrations()",
    "TaxonomyModule.migrations()",
    "ForumModule.migrations()",
    "SearchModule.migrations()",
    "CategoryService::new",
    "TopicService::new",
    "d10normaldeliverytopic",
    "SELECT revision, event_id, target_type, target_id",
    'revisions[0].target_type != "forum"',
    'revisions[1].target_type != "forum_category"',
    "load_root_envelopes",
    "load_typed_envelopes",
    "ContractEventPayload::ForumSearchProjection",
    "ForumSearchProjectionEvent::InvalidationIssued",
    "envelope.causation_id() != Some(revision.event_id)",
    "IggyMode::External",
    "SerializationFormat::Json",
    "open_persistent_contract_consumer_group",
    "FORUM_SEARCH_CONTRACT_CONSUMER_GROUP",
    "FORUM_SEARCH_CONTRACT_TOPIC",
    "transport.publish_contract(envelope.clone()).await?",
    "receive_delivery()",
    "ForumSearchContractIngress::new",
    "ensure_pending_inbox(&inbox, revision, fixture)?",
    "group.acknowledge(&delivery).await?",
    "ForumSearchProjectionSourceFactory.build",
    "ForumProjectionReconciler::with_owner_revision_source",
    "report.claimed_events != 2",
    "report.completed_events != 2",
    "report.owner_revisions_checkpointed != 2",
    "report.owner_rebuilds != 0",
    "forum_normal_delivery_checkpoint_audit",
    'row.outcome != "delivery_covered"',
    "row.observed_forum_documents != 2",
    "execute_forum_storefront_search",
    "ForumSearchResultEligibilityService::new",
    "execution.result.total != 1",
    "caught_up.claimed_events != 0",
    "caught_up.owner_revisions_checkpointed != 0",
    'id: "normal_delivery"',
    evidencePath,
    '.args(["rev-parse", "HEAD"])',
  ],
  "Forum normal-delivery executable proof",
);
forbidAll(
  test,
  [
    "#[ignore]",
    "MemoryTransport",
    "InMemory",
    "FixedOwnerRevisionSource",
    "ControlledForumSource",
    "ContractEventEnvelope::new_caused_by",
    "insert_legacy_root",
    "owner_revision == ingest_sequence",
    "owner_revision != ingest_sequence",
  ],
  "Forum normal-delivery executable proof",
);
const ingressIndex = test.indexOf("ingress.ingest(&delivery.envelope).await?");
const pendingIndex = test.indexOf("ensure_pending_inbox(&inbox, revision, fixture)?");
const acknowledgeIndex = test.indexOf("group.acknowledge(&delivery).await?");
assert.ok(ingressIndex >= 0 && ingressIndex < pendingIndex);
assert.ok(pendingIndex >= 0 && pendingIndex < acknowledgeIndex);
const projectionIndex = test.indexOf("let report = reconciler.sweep_due(8, 8).await?");
const checkpointIndex = test.indexOf("let checkpoint = load_checkpoint");
const storefrontIndex = test.indexOf("let storefront_total = assert_storefront_topic");
assert.ok(projectionIndex >= 0 && projectionIndex < checkpointIndex);
assert.ok(checkpointIndex >= 0 && checkpointIndex < storefrontIndex);

const projectionInvalidation = read(projectionInvalidationPath);
requireAll(
  projectionInvalidation,
  [
    "allocate_projection_revision_in_tx",
    "publish_in_tx_with_envelope_id",
    "publish_contract_in_tx_with_causation",
    "record_projection_revision_in_tx",
    "TransactionalEventBus::publish_root_in_tx_with_envelope_id",
    "TransactionalEventBus::publish_contract_direct_in_tx_with_causation_and_envelope_id",
    "forum_projection_revision_ledger",
  ],
  "Forum atomic owner invalidation publisher",
);
forbidAll(
  projectionInvalidation,
  [
    "rustok_search",
    "search_projection_inbox",
    "search_projection_owner_checkpoints",
  ],
  "Forum atomic owner invalidation publisher",
);

const ingress = read(ingressPath);
requireAll(
  ingress,
  [
    "FORUM_SEARCH_CONTRACT_CONSUMER_GROUP",
    "FORUM_SEARCH_CONTRACT_TOPIC",
    "causation_id()",
    "search_projection_inbox",
    "DurablyAccepted",
  ],
  "production Forum Search contract ingress",
);

const reconciliation = read(reconciliationPath);
requireAll(
  reconciliation,
  [
    "ForumProjectionReconciler",
    "with_owner_revision_source",
    "claim.complete().await?",
    "owner_checkpoint",
    "owner_revisions_checkpointed",
    "owner_rebuilds",
  ],
  "production Forum Search reconciliation",
);

const ownerCheckpoint = read(ownerCheckpointPath);
requireAll(
  ownerCheckpoint,
  [
    "load_delivery_coverage",
    "DeliveryCoverage::Covered",
    "DELIVERY_COVERED_OUTCOME",
    "advance_checkpoint",
    "previous_revision = revision.owner_revision",
  ],
  "production delivery-covered owner checkpoint",
);

const projector = read(projectorPath);
requireAll(
  projector,
  [
    ".list_public_documents(",
    "delete_forum_scope(&tx, tenant_id)",
    "INSERT INTO search_documents",
    "tx.commit().await",
  ],
  "production current-state Forum projector",
);

const storefrontExecution = read(storefrontExecutionPath);
requireAll(
  storefrontExecution,
  [
    "resolve_storefront_search_result_candidates",
    "let total = visible_items.len() as u64",
    "build_forum_result_facets(&visible_items)",
    ".skip(query.offset)",
    ".take(query.limit)",
  ],
  "production Forum storefront execution",
);
const eligibilityIndex = storefrontExecution.indexOf(
  "resolve_storefront_search_result_candidates",
);
const totalIndex = storefrontExecution.indexOf(
  "let total = visible_items.len() as u64",
);
const facetsIndex = storefrontExecution.indexOf(
  "build_forum_result_facets(&visible_items)",
);
const offsetIndex = storefrontExecution.indexOf(".skip(query.offset)");
assert.ok(eligibilityIndex >= 0 && eligibilityIndex < totalIndex);
assert.ok(totalIndex >= 0 && totalIndex < facetsIndex);
assert.ok(facetsIndex >= 0 && facetsIndex < offsetIndex);

const doc = read(docPath);
requireAll(
  doc,
  [
    "`source_ready_maintainer_execution_pending`",
    "FORUM-23B2G2B3D10",
    contractPath,
    testPath,
    evidencePath,
    "revision 1: forum          / null",
    "revision 2: forum_category / category ID",
    "rustok-search-forum-projection-v1",
    "revision 1 / delivery_covered / 2 Forum documents",
    "owner repair rebuilds:        0",
    "total = 1",
    "No command above was run by the implementation agent",
  ],
  "Forum normal-delivery handoff",
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
  "FORUM-23B2G2B3D6",
  "FORUM-23B2G2B3D7",
  "FORUM-23B2G2B3D8",
  "FORUM-23B2G2B3D9",
  "FORUM-23B2G2B3D10",
]) {
  assert.ok(
    parent.source_ready_subproofs.some((subproof) => subproof.task === task),
    `${task} disappeared from the D0 source-ready subproof list`,
  );
}
const subproof = parent.source_ready_subproofs.find(
  ({ task }) => task === "FORUM-23B2G2B3D10",
);
assert.equal(subproof.contract, contractPath);
assert.equal(subproof.test, testPath);
assert.equal(subproof.evidence_artifact, evidencePath);
assert.deepEqual(subproof.covers, [
  "one_correlated_normal_owner_delivery_trace",
  "real_forum_owner_transaction_and_dual_outbox_publication",
  "external_iggy_persistent_group_delivery",
  "durable_ingress_before_source_acknowledgement",
  "production_projection_completion",
  "delivery_covered_checkpoint_order_1_2",
  "storefront_exact_topic_visibility",
  "caught_up_repeat_suppression",
]);
assert.deepEqual(subproof.does_not_cover, [
  "acknowledgement_failure_restart_or_poison_dlq",
  "multi_process_deletion_acl_or_search_disabled_profiles",
  "aggregate_d0_artifact_assembly_or_link_forum_03",
]);
assert.ok(
  parent.maintainer_commands.includes(
    "node scripts/verify/verify-forum-search-versioned-invalidation-normal-delivery-proof.mjs",
  ),
);
assert.ok(
  parent.maintainer_commands.some((command) =>
    command.includes("--test forum_versioned_invalidation_normal_delivery_iggy"),
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
    "D10 closes FORUM-23",
    "normal delivery runtime evidence passed",
    "LINK-FORUM-03 is complete",
  ],
  "FORUM-23 canonical aggregate boundary",
);

console.log("Forum Search normal-delivery proof is source-synchronized.");
