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
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-search-disabled-recovery-proof.json";
const parentContractPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json";
const docPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d9-search-disabled-recovery-proof.md";
const testPath =
  "apps/server/tests/forum_versioned_invalidation_search_disabled_recovery.rs";
const evidencePath =
  "target/forum-search-versioned-invalidation-search-disabled-recovery-evidence.json";

const contract = JSON.parse(read(contractPath));
assert.equal(
  contract.contract,
  "forum_search_versioned_invalidation_search_disabled_recovery_proof_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D9");
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
assert.equal(contract.required_runtime.host_package, "rustok-server");
assert.equal(
  contract.required_runtime.disabled_profile,
  "outbox_taxonomy_and_forum_migrations_without_search_migrations_or_runtime",
);
assert.equal(
  contract.required_runtime.enable_transition,
  "apply_search_migrations_then_start_bounded_owner_ledger_reconciler",
);
assert.equal(
  contract.required_runtime.broker,
  "not_required_for_owner_ledger_recovery_proof",
);
assert.equal(contract.scenario.id, "search_disabled_profile");
assert.equal(contract.scenario.proves.length, 7);
assert.ok(
  contract.maintainer_command.includes(
    "--test forum_versioned_invalidation_search_disabled_recovery",
  ),
);

const test = read(testPath);
requireAll(
  test,
  [
    "RUSTOK_SEARCH_TEST_DATABASE_URL",
    "OutboxModule.migrations()",
    "TaxonomyModule.migrations()",
    "ForumModule.migrations()",
    "async fn enable_search(&self)",
    "SearchModule.migrations()",
    "assert_search_storage_absent",
    '"search_projection_inbox"',
    '"search_projection_owner_checkpoints"',
    '"search_documents"',
    "CategoryService::new",
    "TopicService::new",
    "d9searchdisabledtopicone",
    "d9searchdisabledtopictwo",
    "snapshot.revisions.iter().map(|row| row.revision)",
    'snapshot.revisions[0].target_type != "forum"',
    'snapshot.revisions[1].target_type != "forum_category"',
    "load_root_event_ids",
    "load_typed_causation_ids",
    "ContractEventEnvelope",
    ".causation_id()",
    "evidence.enable_search().await?",
    "count_rows(db, \"search_projection_inbox\").await? != 0",
    "ForumSearchProjectionSourceFactory.build",
    "ForumEventService::new",
    "ForumProjectionReconciler::with_owner_revision_source",
    "recovered.owner_tenants_scanned != 1",
    "recovered.owner_tenants_reconciled != 1",
    "recovered.owner_rebuilds != 1",
    "recovered.owner_revisions_checkpointed != 3",
    "forum_search_disabled_checkpoint_audit",
    "row.observed_forum_documents != 3",
    'checkpoint.outcome != "rebuild_repaired"',
    "after_recovery != before_enable",
    '"synthetic_inbox_deliveries_created": false',
    "caught_up.owner_rebuilds != 0",
    "caught_up.owner_revisions_checkpointed != 0",
    evidencePath,
    '.args(["rev-parse", "HEAD"])',
  ],
  "Forum Search-disabled executable proof",
);
forbidAll(
  test,
  [
    "#[ignore]",
    "rustok_iggy",
    "RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS",
    "PersistentContractConsumerGroup",
    "RUSTOK_FORUM_SEARCH_CONTRACT_CONSUMER_ENABLED",
    "owner_revision == ingest_sequence",
  ],
  "Forum Search-disabled executable proof",
);
const fixtureIndex = test.indexOf("let fixture = create_forum_fixture(db).await?");
const enableIndex = test.indexOf("evidence.enable_search().await?");
const reconcilerIndex = test.indexOf(
  "ForumProjectionReconciler::with_owner_revision_source",
);
assert.ok(fixtureIndex >= 0 && fixtureIndex < enableIndex);
assert.ok(enableIndex >= 0 && enableIndex < reconcilerIndex);

const forumCargo = read("crates/rustok-forum/Cargo.toml");
assert.ok(!forumCargo.includes("rustok-search"));

const invalidation = read(
  "crates/rustok-forum/src/services/projection_invalidation.rs",
);
requireAll(
  invalidation,
  [
    "allocate_projection_revision_in_tx",
    "publish_in_tx_with_envelope_id",
    "publish_contract_in_tx_with_causation",
    "record_projection_revision_in_tx",
    "forum_projection_revision_counters",
    "forum_projection_revision_ledger",
    "TransactionalEventBus::publish_root_in_tx_with_envelope_id",
    "TransactionalEventBus::publish_contract_direct_in_tx_with_causation_and_envelope_id",
  ],
  "Forum owner-local projection invalidation",
);
forbidAll(
  invalidation,
  [
    "rustok_search",
    "search_documents",
    "search_projection_inbox",
    "search_projection_owner_checkpoints",
  ],
  "Forum owner-local projection invalidation",
);

requireAll(
  read("crates/rustok-forum/src/services/category_projection_owner.rs"),
  [
    "CategoryProjectionOwnerService",
    "publish_forum_projection_scope_direct_in_tx",
    "txn.commit().await?",
  ],
  "Forum category owner transaction",
);
requireAll(
  read("crates/rustok-forum/src/services/topic_inline.rs"),
  [
    "DomainEvent::ForumTopicCreated",
    "publish_forum_category_projection_in_tx",
    "txn.commit().await?",
  ],
  "Forum topic owner transaction",
);
requireAll(
  read("crates/rustok-forum/src/services/event.rs"),
  [
    "ForumEventService",
    "list_projection_owner_revisions",
    "list_projection_owner_revision_tenants",
    "forum_projection_revision_ledger",
    "ORDER BY revision ASC",
    "MAX_FORUM_PROJECTION_OWNER_REVISION_PAGE",
    "MAX_FORUM_PROJECTION_OWNER_TENANT_PAGE",
  ],
  "Forum owner revision API",
);
requireAll(
  read("apps/server/src/services/forum_search_owner_revision.rs"),
  [
    "ServerForumProjectionOwnerRevisionSourcePort",
    "ForumEventService::new",
    "list_projection_owner_revisions",
    "list_projection_owner_revision_tenants",
    "ForumProjectionOwnerRevisionImpact::FullRebuild",
  ],
  "server Forum owner revision adapter",
);
requireAll(
  read("crates/rustok-search/src/forum_owner_checkpoint.rs"),
  [
    "list_tenant_heads(active_cursor, tenant_limit)",
    "resolve_forum_projection_owner_revisions",
    "load_delivery_coverage",
    "DeliveryCoverage::Missing => rebuild_required = true",
    "self.forum_projector.rebuild_tenant(head.tenant_id).await?",
    "REBUILD_REPAIRED_OUTCOME",
    "advance_checkpoint",
    "previous_revision = revision.owner_revision",
  ],
  "Search owner-ledger recovery",
);
requireAll(
  read("crates/rustok-search/src/forum_reconciliation.rs"),
  [
    "pub fn with_owner_revision_source(",
    "if let Some(owner_checkpoint)",
    ".sweep_due(",
    "owner_revisions_checkpointed",
    "owner_rebuilds",
  ],
  "Search bounded reconciliation composition",
);
requireAll(
  read("crates/rustok-search/src/forum_projector.rs"),
  [
    "self.source.list_public_documents",
    "delete_forum_scope(&tx, tenant_id)",
    "INSERT INTO search_documents",
    "tx.commit().await",
  ],
  "Search current-state Forum projector",
);

const doc = read(docPath);
requireAll(
  doc,
  [
    "`source_ready_maintainer_execution_pending`",
    "FORUM-23B2G2B3D9",
    contractPath,
    testPath,
    evidencePath,
    "Search migrations are deliberately not applied",
    "revision 1: forum / null",
    "revision 3: forum_category / category ID",
    "Search inbox rows:       0",
    "revision 3 / rebuild_repaired / 3 documents",
    "Search enable and reconciliation may materialize a projection and checkpoint",
    "No command above was run by the implementation agent",
  ],
  "Forum Search-disabled handoff",
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
]) {
  assert.ok(
    parent.source_ready_subproofs.some((subproof) => subproof.task === task),
    `${task} disappeared from the D0 source-ready subproof list`,
  );
}
const subproof = parent.source_ready_subproofs.find(
  ({ task }) => task === "FORUM-23B2G2B3D9",
);
assert.equal(subproof.contract, contractPath);
assert.equal(subproof.test, testPath);
assert.equal(subproof.evidence_artifact, evidencePath);
assert.deepEqual(subproof.covers, [
  "forum_owner_commands_commit_without_search_storage",
  "legacy_and_typed_owner_events_persist_without_search_runtime",
  "contiguous_owner_revisions_survive_disabled_period",
  "late_search_enable_starts_without_inbox_or_checkpoint_state",
  "bounded_real_owner_ledger_scan_repairs_projection_once",
  "checkpoint_advances_exactly_1_2_3_after_rebuild",
  "forum_owner_state_and_event_history_remain_unchanged",
  "caught_up_repeat_suppresses_duplicate_recovery",
]);
assert.deepEqual(subproof.does_not_cover, [
  "long_running_host_process_restart_or_polling_cadence",
  "iggy_acknowledgement_poison_or_dlq",
  "deployment_orchestration_or_link_forum_03",
]);
assert.ok(
  parent.maintainer_commands.includes(
    "node scripts/verify/verify-forum-search-versioned-invalidation-search-disabled-recovery-proof.mjs",
  ),
);
assert.ok(
  parent.maintainer_commands.some((command) =>
    command.includes("--test forum_versioned_invalidation_search_disabled_recovery"),
  ),
);

const plan = read("crates/rustok-forum/docs/implementation-plan.md");
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
    "D9 closes FORUM-23",
    "Search-disabled runtime evidence passed",
    "LINK-FORUM-03 is complete",
  ],
  "FORUM-23 canonical aggregate boundary",
);

console.log(
  "Forum Search-disabled recovery proof is source-synchronized.",
);
