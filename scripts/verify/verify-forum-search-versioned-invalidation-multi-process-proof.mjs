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
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-multi-process-proof.json";
const parentContractPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json";
const docPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d7-multi-process-serialization-proof.md";
const testPath =
  "apps/server/tests/forum_versioned_invalidation_multi_process_serialization.rs";
const serverCargoPath = "apps/server/Cargo.toml";
const reconcilerPath = "crates/rustok-search/src/forum_owner_checkpoint.rs";
const ownerSourcePath = "crates/rustok-search/src/forum_reconciliation.rs";
const checkpointMigrationPath =
  "crates/rustok-search/src/migrations/m20260731_000012_create_forum_owner_revision_checkpoints.rs";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const evidencePath =
  "target/forum-search-versioned-invalidation-multi-process-evidence.json";

const contract = JSON.parse(read(contractPath));
assert.equal(
  contract.contract,
  "forum_search_versioned_invalidation_multi_process_proof_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D7");
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
assert.equal(contract.required_runtime.minimum_concurrent_os_processes, 2);
assert.equal(contract.required_runtime.owner_tenant_page_limit, 1);
assert.equal(contract.scenario.id, "multi_process_serialization");
assert.ok(Array.isArray(contract.scenario.proves));
assert.equal(contract.scenario.proves.length, 7);
assert.ok(
  contract.maintainer_command.includes(
    "--test forum_versioned_invalidation_multi_process_serialization",
  ),
);

const test = read(testPath);
requireAll(
  test,
  [
    "RUSTOK_SEARCH_TEST_DATABASE_URL",
    "RUSTOK_FORUM_D7_DATABASE_URL",
    "RUSTOK_FORUM_D7_SCHEMA",
    "RUSTOK_FORUM_D7_ROLE",
    "SearchModule.migrations()",
    "database_url_in_schema",
    "ForumProjectionReconciler::with_owner_revision_source",
    "DatabaseOwnerSource",
    "DatabaseProjectionSource",
    "env::current_exe()",
    "Command::new(executable)",
    ".arg(\"--exact\")",
    "forum_multi_process_child",
    "HOLDER_ROLE",
    "CONTENDER_ROLE",
    "NEXT_ROLE",
    "wait_for_holder_entry",
    "release_holder",
    "owner_tenants_blocked == 1",
    "contender reached the projector despite advisory-lock exclusion",
    "cursor_audit_during_holder.len() != 1",
    "cursor_audit_after_holder.len() != 1",
    "first_revisions != [1, 2]",
    "second_checkpoint_audit[0].owner_revision != 1",
    "rebuild_calls(&evidence.db, first_tenant_id()).await? != 1",
    "rebuild_calls(&evidence.db, second_tenant_id()).await? != 1",
    "forum_d7_checkpoint_audit",
    "AFTER INSERT OR UPDATE ON search_projection_owner_checkpoints",
    "forum_d7_scan_cursor_audit",
    "AFTER INSERT OR UPDATE ON search_projection_owner_scan_cursors",
    "stale_holder_cursor_cas_suppressed",
    "tenant_skip_observed\": false",
    "cursor_regression_observed\": false",
    "checkpoint_regression_or_skip_observed\": false",
    evidencePath,
    ".args([\"rev-parse\", \"HEAD\"])",
  ],
  "Forum Search multi-process executable proof",
);
forbidAll(
  test,
  [
    "#[ignore]",
    "rustok_iggy",
    "RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS",
    "PersistentContractConsumerGroup",
    "forum_search_inbox_worker_loop",
    "RUSTOK_FORUM_SEARCH_CONTRACT_CONSUMER_ENABLED",
    "owner_revision == ingest_sequence",
  ],
  "Forum Search multi-process executable proof",
);

const serverCargo = read(serverCargoPath);
requireAll(
  serverCargo,
  [
    "rustok-search = { workspace = true, features = [\"graphql\"] }",
    "rustok-core = { workspace = true, features = [\"redis-cache\", \"server\"] }",
    "async-trait.workspace = true",
    "sea-orm.workspace = true",
    "sea-orm-migration.workspace = true",
    "tokio.workspace = true",
    "serde.workspace = true",
    "serde_json.workspace = true",
    "uuid.workspace = true",
    "chrono.workspace = true",
  ],
  "rustok-server existing host dependencies",
);

const reconciler = read(reconcilerPath);
requireAll(
  reconciler,
  [
    "let lock_key = format!(\"search:{FORUM_SOURCE_MODULE}:{tenant_id}:{FULL_SCOPE_KEY}\")",
    "pg_try_advisory_xact_lock(hashtextextended($1, 0))",
    "return Ok(TenantCheckpointOutcome::Blocked)",
    "self.forum_projector.rebuild_tenant(head.tenant_id).await?",
    "let mut previous_revision = checkpoint",
    "advance_checkpoint(",
    "previous_revision = revision.owner_revision",
    "WHERE search_projection_owner_checkpoints.owner_revision = $5",
    "Forum owner checkpoint did not advance from the expected revision",
    "WHERE search_projection_owner_scan_cursors.after_tenant_id",
    "IS NOT DISTINCT FROM $2",
    "let _ = self.store_scan_cursor(active_cursor, next_cursor).await?",
  ],
  "production Forum owner checkpoint serialization",
);

const ownerSource = read(ownerSourcePath);
requireAll(
  ownerSource,
  [
    "owner tenant heads must be strictly ordered after the requested cursor",
    "owner revisions must be contiguous and strictly ordered after the requested cursor",
    "ForumProjectionOwnerRevisionImpact::FullRebuild",
    "after_tenant_id",
    "owner_revision",
    "ingest_sequence",
  ],
  "owner scan and revision ordering contract",
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
  "production owner checkpoint exact-order trigger",
);

const doc = read(docPath);
requireAll(
  doc,
  [
    "`source_ready_maintainer_execution_pending`",
    "FORUM-23B2G2B3D7",
    contractPath,
    testPath,
    evidencePath,
    "`holder`",
    "`contender`",
    "`next`",
    "null -> first tenant",
    "first tenant -> second tenant",
    "first tenant:  revision 1, revision 2",
    "second tenant: revision 1",
    "separate child processes",
    "does not execute the long-running host polling loop",
    "No command above was run by the implementation agent",
  ],
  "Forum Search multi-process proof handoff",
);

const parent = JSON.parse(read(parentContractPath));
assert.equal(parent.task, "FORUM-23B2G2B3D0");
assert.equal(parent.status, "source_ready_maintainer_execution_pending");
assert.equal(parent.required_runtime.minimum_server_processes_for_serialization_case, 2);
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
  "FORUM-23B2G2B3D7",
]) {
  assert.ok(
    parent.source_ready_subproofs.some((subproof) => subproof.task === task),
    `${task} disappeared from the D0 source-ready subproof list`,
  );
}
const subproof = parent.source_ready_subproofs.find(
  ({ task }) => task === "FORUM-23B2G2B3D7",
);
assert.equal(subproof.contract, contractPath);
assert.equal(subproof.test, testPath);
assert.equal(subproof.evidence_artifact, evidencePath);
assert.deepEqual(subproof.covers, [
  "independent_host_os_process_contention",
  "postgresql_tenant_advisory_lock_exclusion",
  "blocked_contender_never_enters_projection",
  "stale_scan_cursor_compare_and_set_suppressed",
  "first_tenant_exact_checkpoint_order_1_2",
  "next_tenant_discovered_without_cursor_skip_or_regression",
  "second_tenant_checkpoint_revision_1",
]);
assert.deepEqual(subproof.does_not_cover, [
  "long_running_host_polling_or_restart_timing",
  "iggy_delivery_acknowledgement_or_poison",
  "deletion_acl_or_storefront_visibility",
  "search_disabled_profile_or_link_forum_03",
]);
assert.ok(
  parent.maintainer_commands.includes(
    "node scripts/verify/verify-forum-search-versioned-invalidation-multi-process-proof.mjs",
  ),
);
assert.ok(
  parent.maintainer_commands.some((command) =>
    command.includes("--test forum_versioned_invalidation_multi_process_serialization"),
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
    "D7 closes FORUM-23",
    "multi-process runtime evidence passed",
    "LINK-FORUM-03 is complete",
  ],
  "FORUM-23 canonical aggregate boundary",
);

console.log(
  "Forum Search multi-process serialization proof is source-synchronized.",
);
