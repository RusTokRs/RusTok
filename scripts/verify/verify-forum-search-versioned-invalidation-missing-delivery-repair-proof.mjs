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
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-missing-delivery-repair-proof.json";
const parentContractPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json";
const docPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d6-missing-delivery-repair-proof.md";
const testPath =
  "crates/rustok-search/tests/forum_versioned_invalidation_missing_delivery_repair.rs";
const cargoPath = "crates/rustok-search/Cargo.toml";
const reconcilerPath = "crates/rustok-search/src/forum_owner_checkpoint.rs";
const ownerSourcePath = "crates/rustok-search/src/forum_reconciliation.rs";
const checkpointMigrationPath =
  "crates/rustok-search/src/migrations/m20260731_000012_create_forum_owner_revision_checkpoints.rs";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const evidencePath =
  "target/forum-search-versioned-invalidation-missing-delivery-repair-evidence.json";

const contract = JSON.parse(read(contractPath));
assert.equal(
  contract.contract,
  "forum_search_versioned_invalidation_missing_delivery_repair_proof_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D6");
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
assert.equal(
  contract.required_runtime.broker,
  "not_required_for_bounded_owner_repair",
);
assert.equal(contract.scenario.id, "missing_delivery_owner_repair");
assert.ok(Array.isArray(contract.scenario.proves));
assert.equal(contract.scenario.proves.length, 6);
assert.ok(
  contract.maintainer_command.includes(
    "--test forum_versioned_invalidation_missing_delivery_repair",
  ),
);

const test = read(testPath);
requireAll(
  test,
  [
    "RUSTOK_SEARCH_TEST_DATABASE_URL",
    "SearchModule.migrations()",
    "database_url_in_schema",
    "max_connections(max_connections)",
    "ForumProjectionReconciler::with_owner_revision_source",
    "FixedOwnerRevisionSource",
    "ForumProjectionOwnerTenantHead",
    "ForumProjectionOwnerRevisionRecord",
    "owner_revision(1, revision_one_event_id)",
    "owner_revision(2, missing_revision_event_id)",
    "owner_revision(3, revision_three_event_id)",
    "insert_completed_delivery(db, tenant_id, revision_one_event_id)",
    "insert_completed_delivery(db, tenant_id, revision_three_event_id)",
    "injected Forum projection rebuild failure",
    "failed.owner_revisions_checkpointed != 0",
    "load_checkpoint(db, tenant_id).await?.is_some()",
    "forum_missing_delivery_checkpoint_audit",
    "AFTER INSERT OR UPDATE ON search_projection_owner_checkpoints",
    "observed_forum_documents",
    "audited_revisions != [1, 2, 3]",
    "repaired.owner_revisions_checkpointed != 3",
    "repaired.owner_rebuilds != 1",
    "checkpoint.outcome != \"rebuild_repaired\"",
    "count_forum_document(db, tenant_id, stale_document_id).await? != 0",
    "count_forum_document(db, tenant_id, repaired_document_id).await? != 1",
    "caught_up.owner_rebuilds != 0",
    "projection_source.list_calls() != 2",
    evidencePath,
    ".args([\"rev-parse\", \"HEAD\"])",
  ],
  "Forum Search missing-delivery executable proof",
);
forbidAll(
  test,
  [
    "#[ignore]",
    "rustok_iggy",
    "RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS",
    "PersistentContractConsumerGroup",
    "SearchProjector::new",
    "ForumSearchContractIngress",
    "owner_revision == ingest_sequence",
    "multi_process",
  ],
  "Forum Search missing-delivery executable proof",
);

const cargo = read(cargoPath);
const devDependenciesStart = cargo.indexOf("[dev-dependencies]");
const featuresStart = cargo.indexOf("[features]", devDependenciesStart);
assert.ok(devDependenciesStart >= 0 && featuresStart > devDependenciesStart);
const devDependencies = cargo.slice(devDependenciesStart, featuresStart);
requireAll(
  devDependencies,
  ["tokio.workspace = true"],
  "rustok-search dev dependencies",
);
forbidAll(
  devDependencies,
  ["rustok-iggy", "rustok-iggy-connector", "rustok-server"],
  "rustok-search dev dependencies",
);

const reconciler = read(reconcilerPath);
requireAll(
  reconciler,
  [
    "recover_abandoned_processing",
    "list_tenant_heads",
    "try_acquire_tenant_lock",
    "load_checkpoint",
    "has_non_terminal_inbox_work",
    "resolve_forum_projection_owner_revisions",
    "load_delivery_coverage",
    "DeliveryCoverage::Missing => rebuild_required = true",
    "self.forum_projector.rebuild_tenant(head.tenant_id).await?",
    "let mut previous_revision = checkpoint",
    "for revision in &revisions",
    "advance_checkpoint(",
    "previous_revision = revision.owner_revision",
    "REBUILD_REPAIRED_OUTCOME",
  ],
  "production Forum owner checkpoint reconciler",
);

const ownerSource = read(ownerSourcePath);
requireAll(
  ownerSource,
  [
    "owner revisions must be contiguous and strictly ordered after the requested cursor",
    "expected_revision",
    "ForumProjectionOwnerRevisionImpact::FullRebuild",
    "owner_revision",
    "ingest_sequence",
  ],
  "owner revision source contract",
);

const checkpointMigration = read(checkpointMigrationPath);
requireAll(
  checkpointMigration,
  [
    "search projection owner checkpoint must start at revision 1",
    "NEW.owner_revision <> OLD.owner_revision + 1",
    "search projection owner checkpoint must advance by exactly 1",
    "BEFORE INSERT ON search_projection_owner_checkpoints",
    "BEFORE UPDATE ON search_projection_owner_checkpoints",
    "outcome IN ('delivery_covered', 'rebuild_repaired')",
  ],
  "production owner checkpoint storage guard",
);

const doc = read(docPath);
requireAll(
  doc,
  [
    "`source_ready_maintainer_execution_pending`",
    "FORUM-23B2G2B3D6",
    contractPath,
    testPath,
    evidencePath,
    "revision 1 / rebuild_repaired / current document visible",
    "revision 2 / rebuild_repaired / current document visible",
    "revision 3 / rebuild_repaired / current document visible",
    "D3-D5 external-Iggy proofs live in the `rustok-server` host package after #2781",
    "does not prove contention between separate server processes",
    "No command above was run by the implementation agent",
  ],
  "Forum Search missing-delivery proof handoff",
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
  "FORUM-23B2G2B3D5",
  "FORUM-23B2G2B3D6",
]) {
  assert.ok(
    parent.source_ready_subproofs.some((subproof) => subproof.task === task),
    `${task} disappeared from the D0 source-ready subproof list`,
  );
}
const subproof = parent.source_ready_subproofs.find(
  ({ task }) => task === "FORUM-23B2G2B3D6",
);
assert.equal(subproof.contract, contractPath);
assert.equal(subproof.test, testPath);
assert.equal(subproof.evidence_artifact, evidencePath);
assert.deepEqual(subproof.covers, [
  "bounded_owner_tenant_and_revision_scan",
  "missing_delivery_detected_by_owner_event_identity",
  "failed_rebuild_leaves_projection_and_checkpoint_unchanged",
  "one_successful_current_state_rebuild_repairs_projection",
  "checkpoint_audit_advances_exactly_1_2_3_after_rebuild",
  "caught_up_repeat_suppresses_duplicate_rebuild",
]);
assert.deepEqual(subproof.does_not_cover, [
  "host_worker_polling_loop",
  "iggy_delivery_acknowledgement_or_poison",
  "multi_process_lock_or_scan_cursor_contention",
  "deletion_acl_or_storefront_visibility",
  "search_disabled_profile_or_link_forum_03",
]);
assert.ok(
  parent.maintainer_commands.includes(
    "node scripts/verify/verify-forum-search-versioned-invalidation-missing-delivery-repair-proof.mjs",
  ),
);
assert.ok(
  parent.maintainer_commands.some((command) =>
    command.includes("--test forum_versioned_invalidation_missing_delivery_repair"),
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
    "D6 closes FORUM-23",
    "missing-delivery runtime evidence passed",
    "LINK-FORUM-03 is complete",
  ],
  "FORUM-23 canonical aggregate boundary",
);

console.log(
  "Forum Search missing-delivery owner repair proof is source-synchronized.",
);
