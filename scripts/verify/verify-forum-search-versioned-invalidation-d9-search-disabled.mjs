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
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-search-disabled-proof.json";
const documentPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d9-search-disabled-recovery.md";
const testPath =
  "apps/server/tests/forum_versioned_invalidation_search_disabled_recovery.rs";
const parentPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json";
const categoryOwnerPath =
  "crates/rustok-forum/src/services/category_projection_owner.rs";
const invalidationOwnerPath =
  "crates/rustok-forum/src/services/projection_invalidation.rs";
const forumOwnerSourcePath =
  "crates/rustok-forum/src/services/event.rs";
const forumProjectionPath = "crates/rustok-forum/src/search_projection.rs";
const serverOwnerAdapterPath =
  "apps/server/src/services/forum_search_owner_revision.rs";
const serverServicesPath = "apps/server/src/services/mod.rs";
const searchReconciliationPath =
  "crates/rustok-search/src/forum_reconciliation.rs";
const serverManifestPath = "apps/server/Cargo.toml";

const contract = JSON.parse(read(contractPath));
assert.equal(
  contract.contract,
  "forum_search_versioned_invalidation_search_disabled_source_proof_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D9");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.runtime_evidence_parent, parentPath);
assert.equal(contract.predecessor.task, "FORUM-23B2G2B3D8");
assert.equal(contract.predecessor.pull_request, 2789);
assert.equal(contract.predecessor.state_at_authorship, "open_draft");
assert.equal(contract.predecessor.scope_dependency, "independent_runtime_scenario");
assert.equal(contract.host_boundary.package, "rustok-server");
assert.equal(contract.host_boundary.test, testPath);
assert.deepEqual(contract.production_references, [
  categoryOwnerPath,
  invalidationOwnerPath,
  forumOwnerSourcePath,
  forumProjectionPath,
  serverOwnerAdapterPath,
  serverServicesPath,
  searchReconciliationPath,
]);
assert.equal(
  contract.evidence_artifact.path,
  "target/forum-search-versioned-invalidation-search-disabled-evidence.json",
);
assert.equal(contract.evidence_artifact.generation, "executable_test_only");
assert.equal(contract.evidence_artifact.hand_editing_forbidden, true);
assert.equal(contract.evidence_artifact.source_commit_required, true);
assert.equal(contract.evidence_artifact.written_only_after_cleanup, true);
assert.equal(contract.required_runtime.database, "postgresql");
assert.equal(contract.required_runtime.broker, "not_required");
assert.equal(
  contract.required_runtime.disabled_search_setting,
  "rustok.search.enabled=false",
);
assert.equal(
  contract.required_runtime.reenabled_search_setting,
  "rustok.search.enabled=true",
);
assert.deepEqual(contract.required_runtime.disabled_storage_tables, [
  "search_documents",
  "search_projection_inbox",
  "search_projection_owner_checkpoints",
  "search_projection_owner_scan_cursors",
]);
assert.equal(
  contract.required_runtime.owner_command,
  "rustok_forum::CategoryService::create",
);
assert.equal(
  contract.required_runtime.recovery,
  "rustok_search::ForumProjectionReconciler::sweep_due",
);
assert.equal(contract.scenario.id, "search_disabled_profile");
assert.ok(contract.scenario.proves.length >= 8);
assert.ok(contract.identity_invariants.length >= 6);
assert.ok(contract.non_claims.length >= 8);
assert.ok(
  contract.maintainer_command.includes(
    "cargo test --locked -p rustok-server --features mod-forum",
  ),
);

const test = read(testPath);
requireAll(
  test,
  [
    "forum_owner_commit_survives_search_disable_and_reconciles_after_enable",
    "rustok_migrations::Migrator::up",
    "disabled_settings.search.enabled = false",
    "enabled_settings.search.enabled = true",
    "ALTER TABLE search_documents RENAME TO",
    "ALTER TABLE search_projection_inbox RENAME TO",
    "ALTER TABLE search_projection_owner_checkpoints RENAME TO",
    "ALTER TABLE search_projection_owner_scan_cursors RENAME TO",
    "CategoryService::new(db.clone())",
    ".create(",
    "SecurityContext::new(UserRole::Admin, None)",
    "forum_projection_revision_ledger",
    "index.reindex_requested",
    "forum.search_projection.invalidation_issued",
    "ContractEventPayload::ForumSearchProjection",
    "ForumSearchProjectionEvent::InvalidationIssued",
    "typed.causation_id() != Some(ledger.event_id)",
    "ModuleRegistry::new()",
    ".register(rustok_index::IndexModule)",
    ".register(ForumModule)",
    ".register(SearchModule)",
    "build_shared_runtime_extensions_with_host_providers",
    "search_projection_source_registry_from_extensions",
    "SharedForumProjectionOwnerRevisionSourcePort",
    "ForumProjectionReconciler::with_owner_revision_source",
    "reconciler.sweep_due(1, 8)",
    "recovered.owner_rebuilds != 1",
    "recovered.owner_revisions_checkpointed != 1",
    "checkpoint.outcome != \"rebuild_repaired\"",
    "count_rows(db, \"search_projection_inbox\")",
    "caught_up.owner_rebuilds != 0",
    "target/forum-search-versioned-invalidation-search-disabled-evidence.json",
    "FORUM-23B2G2B3D9",
    "source_commit()",
  ],
  "D9 Search-disabled recovery executable proof",
);
forbidAll(
  test,
  [
    "INSERT INTO forum_categories",
    "INSERT INTO forum_category_translations",
    "INSERT INTO forum_projection_revision_ledger",
    "INSERT INTO sys_events",
    "struct FixedOwnerRevisionSource",
    "struct DatabaseOwnerSource",
    "struct ControlledForumSource",
    "impl ForumProjectionOwnerRevisionSourcePort for",
    "impl SearchProjectionSource for",
    "IggyTransport",
    "PersistentContractConsumerGroup",
    "ConsumerPoisonReceiptStore",
  ],
  "D9 executable production-boundary proof",
);

const categoryOwner = read(categoryOwnerPath);
requireAll(
  categoryOwner,
  [
    "pub(super) async fn create",
    "forum_category::ActiveModel",
    "forum_category_translation::ActiveModel",
    "publish_forum_projection_scope_direct_in_tx",
    "txn.commit().await?",
  ],
  "Forum category projection owner",
);

const invalidationOwner = read(invalidationOwnerPath);
requireAll(
  invalidationOwner,
  [
    "allocate_projection_revision_in_tx",
    "publish_root_in_tx_with_envelope_id",
    "publish_contract_direct_in_tx_with_causation_and_envelope_id",
    "record_projection_revision_in_tx",
    "ForumSearchProjectionEvent::InvalidationIssued",
    "forum_projection_revision_ledger",
  ],
  "Forum projection invalidation owner",
);

const forumOwnerSource = read(forumOwnerSourcePath);
requireAll(
  forumOwnerSource,
  [
    "list_projection_owner_revisions",
    "list_projection_owner_revision_tenants",
    "forum_projection_revision_ledger",
    "index.reindex_requested",
  ],
  "Forum bounded owner revision source",
);

const forumProjection = read(forumProjectionPath);
requireAll(
  forumProjection,
  [
    "ForumSearchProjectionSourceFactory",
    "impl SearchProjectionSourceFactory",
    "source_module",
    "ForumPublicDiscoveryService",
    "forum_category",
    "forum_category_translation",
    "SearchProjectionDocument",
  ],
  "production Forum projection source",
);

const serverOwnerAdapter = read(serverOwnerAdapterPath);
requireAll(
  serverOwnerAdapter,
  [
    "ServerForumProjectionOwnerRevisionSourcePort",
    "ForumEventService::new",
    "list_projection_owner_revisions",
    "list_projection_owner_revision_tenants",
    "SharedForumProjectionOwnerRevisionSourcePort",
  ],
  "server Forum owner revision adapter",
);

const serverServices = read(serverServicesPath);
requireAll(
  serverServices,
  [
    "build_shared_runtime_extensions_with_host_providers",
    "ServerForumProjectionOwnerRevisionSourcePort::shared",
    "extensions.insert(owner_revision)",
  ],
  "server production runtime composition",
);

const searchReconciliation = read(searchReconciliationPath);
requireAll(
  searchReconciliation,
  [
    "pub struct ForumProjectionReconciler",
    "with_owner_revision_source",
    "pub async fn sweep_due",
    "rebuild_repaired",
  ],
  "Search owner-ledger reconciler",
);

const serverManifest = read(serverManifestPath);
requireAll(
  serverManifest,
  [
    "rustok-forum     = { workspace = true, optional = true }",
    "rustok-search = { workspace = true, features = [\"graphql\"] }",
    "rustok-index.workspace = true",
    "rustok-migrations = { path = \"../../crates/rustok-migrations\" }",
    "sea-orm-migration.workspace = true",
  ],
  "server host dependency boundary",
);

const parent = JSON.parse(read(parentPath));
assert.equal(parent.task, "FORUM-23B2G2B3D0");
assert.equal(parent.status, "source_ready_maintainer_execution_pending");
assert.ok(
  parent.source_ready_subproofs.some(
    ({ task }) => task === "FORUM-23B2G2B3D7",
  ),
  "D0 parent must retain merged D7 multi-process evidence",
);
assert.ok(
  !parent.source_ready_subproofs.some(
    ({ task }) => task === "FORUM-23B2G2B3D9",
  ),
  "D0 parent must not register unmerged D9",
);
const requiredScenario = parent.required_scenarios.find(
  ({ id }) => id === "search_disabled_profile",
);
assert.ok(requiredScenario, "D0 must retain the Search-disabled profile");
assert.ok(
  requiredScenario.requires.some((requirement) =>
    requirement.includes("Forum owner commands and transactional events still commit"),
  ),
);
assert.ok(
  requiredScenario.requires.some((requirement) =>
    requirement.includes("no synchronous Search dependency"),
  ),
);
assert.ok(
  requiredScenario.requires.some((requirement) =>
    requirement.includes("reenabling Search permits bounded owner-ledger reconciliation"),
  ),
);

const document = read(documentPath);
requireAll(
  document,
  [
    "FORUM-23B2G2B3D9",
    "source_ready_maintainer_execution_pending",
    contractPath,
    testPath,
    "CategoryService::create",
    "rustok.search.enabled=false",
    "rustok.search.enabled=true",
    "search_documents",
    "search_projection_inbox",
    "search_projection_owner_checkpoints",
    "search_projection_owner_scan_cursors",
    "forum_projection_revision_ledger.event_id",
    "typed ContractEventEnvelope.causation_id",
    "ForumProjectionReconciler::sweep_due(1, 8)",
    "outcome = rebuild_repaired",
    "must not synthesize a row",
    "Deletion/ACL ordering remains",
    "No command above was run by the implementation agent",
  ],
  "D9 Search-disabled handoff",
);
forbidAll(
  document,
  [
    "successful PostgreSQL execution passed",
    "D9 is registered in D0",
    "closes LINK-FORUM-03",
  ],
  "D9 handoff claims",
);

console.log(
  "Forum Search D9 Search-disabled owner continuity and bounded recovery source proof is internally consistent.",
);
