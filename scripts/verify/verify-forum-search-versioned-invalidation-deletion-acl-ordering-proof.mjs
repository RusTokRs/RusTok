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
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-deletion-acl-ordering-proof.json";
const parentContractPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json";
const docPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d8-deletion-acl-ordering-proof.md";
const testPath =
  "apps/server/tests/forum_versioned_invalidation_deletion_acl_ordering.rs";
const serverCargoPath = "apps/server/Cargo.toml";
const moderationOwnerPath =
  "crates/rustok-forum/src/services/moderation_owner.rs";
const topicOwnerPath = "crates/rustok-forum/src/services/topic_owner.rs";
const topicAudienceOwnerPath =
  "crates/rustok-forum/src/services/topic_audience_owner.rs";
const projectionSourcePath = "crates/rustok-forum/src/search_projection.rs";
const publicDiscoveryPath =
  "crates/rustok-forum/src/services/public_discovery.rs";
const forumEligibilityPath =
  "crates/rustok-forum/src/services/search_result_eligibility.rs";
const serverEligibilityPath =
  "apps/server/src/services/forum_search_result_eligibility.rs";
const contractIngressPath =
  "crates/rustok-search/src/forum_contract_ingress.rs";
const inboxPath = "crates/rustok-search/src/forum_inbox.rs";
const reconciliationPath =
  "crates/rustok-search/src/forum_reconciliation.rs";
const projectorPath = "crates/rustok-search/src/forum_projector.rs";
const storefrontExecutionPath =
  "crates/rustok-search/src/forum_storefront_execution.rs";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const evidencePath =
  "target/forum-search-versioned-invalidation-deletion-acl-ordering-evidence.json";

const contract = JSON.parse(read(contractPath));
assert.equal(
  contract.contract,
  "forum_search_versioned_invalidation_deletion_acl_ordering_proof_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D8");
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
  contract.required_runtime.broker,
  "not_required_for_bounded_ordering_and_visibility_proof",
);
assert.equal(contract.required_runtime.storefront_viewer, "anonymous_public");
assert.deepEqual(contract.required_runtime.owner_mutations, [
  "approved_reply_hidden",
  "topic_deleted",
  "topic_richer_audience_restricted",
]);
assert.equal(contract.scenario.id, "deletion_acl_ordering");
assert.equal(contract.scenario.proves.length, 7);
assert.ok(
  contract.maintainer_command.includes(
    "--test forum_versioned_invalidation_deletion_acl_ordering",
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
    "SearchModule.migrations()",
    "database_url_in_schema",
    "ForumSearchProjectionSourceFactory.build",
    "ForumProjectionReconciler::new",
    "ForumSearchContractIngress::new",
    "execute_forum_storefront_search",
    "ForumSearchResultEligibilityService::new",
    "ModerationService::new",
    ".hide_reply(",
    "TopicService::new",
    ".delete(",
    "ForumTopicAudiencePolicyService::new",
    "roles_any: vec![UserRole::Customer]",
    "after_hide_revision != baseline_revision + 1",
    "after_delete_revision != after_hide_revision + 2",
    "after_acl_revision != after_delete_revision + 1",
    "revisions[1..].iter().rev()",
    "insert_legacy_root(db, &deleted_legacy, \"forum\")",
    "insert_legacy_root(db, &hidden_legacy, \"forum\")",
    "ingest_typed_revision(db, fixture.tenant_id, &revisions[0])",
    "duplicate root identity",
    "inbox_order.len() != 6",
    "report.claimed_events != 6",
    "report.completed_events != 6",
    "count_forum_document",
    "insert_stale_search_documents",
    "count_stale_markers",
    "storefront owner did not reauthorize the exact stale candidates",
    "d8hiddenreplymarker",
    "d8deletedtopicmarker",
    "d8acltopicmarker",
    "visible facet",
    evidencePath,
    ".args([\"rev-parse\", \"HEAD\"])",
  ],
  "Forum Search deletion/ACL executable proof",
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
  "Forum Search deletion/ACL executable proof",
);

const moderationOwner = read(moderationOwnerPath);
requireAll(
  moderationOwner,
  [
    "DomainEvent::ForumReplyStatusChanged",
    "current == ReplyStatus::Approved && target != ReplyStatus::Approved",
    "changed_category_id",
    "publish_forum_category_projection_in_tx",
  ],
  "Forum reply hide owner path",
);

const topicOwner = read(topicOwnerPath);
requireAll(
  topicOwner,
  [
    "DomainEvent::ForumTopicStatusChanged",
    "new_status: TopicStatus::Archived.to_string()",
    "publish_forum_topic_projection_in_tx",
    "publish_forum_category_projection_in_tx",
    "mark_topic_thread_deleted_in_tx",
  ],
  "Forum topic delete owner path",
);

const topicAudienceOwner = read(topicAudienceOwnerPath);
requireAll(
  topicAudienceOwner,
  [
    "ForumTopicAudiencePolicyOwnerService",
    "input.constraints.normalize()?",
    "publish_forum_topic_projection_direct_in_tx",
    "txn.commit().await?",
  ],
  "Forum richer topic audience owner path",
);

const projectionSource = read(projectionSourcePath);
requireAll(
  projectionSource,
  [
    "ForumPublicDiscoveryService::new",
    "get_public_topic_with_locale_fallback",
    "get_public_reply_with_locale_fallback",
    "Some(&[ReplyStatus::Approved])",
    "projected_reply",
  ],
  "Forum current-state projection source",
);

const publicDiscovery = read(publicDiscoveryPath);
requireAll(
  publicDiscovery,
  [
    "Canonical public discovery owner",
    "requiring authentication, trust, Groups, explicit users, roles",
    "get_public_topic_with_locale_fallback",
    "get_public_reply_with_locale_fallback",
  ],
  "Forum public discovery fail-closed source",
);

const forumEligibility = read(forumEligibilityPath);
requireAll(
  forumEligibility,
  [
    "filter_public_storefront_visible",
    "ForumTopicAudienceVisibilityService::new",
    "ReplyStatus::Approved",
    "visible_topics",
    "allowed.push(*candidate)",
  ],
  "Forum Search result owner eligibility",
);

const serverEligibility = read(serverEligibilityPath);
requireAll(
  serverEligibility,
  [
    "ServerForumSearchResultEligibilityPort",
    "ForumSearchResultEligibilityService::new",
    "filter_public_storefront_visible",
    "StorefrontSearchResultEligibilityPort",
    "ensure_forum_enabled",
  ],
  "server Forum result eligibility adapter",
);

const contractIngress = read(contractIngressPath);
requireAll(
  contractIngress,
  [
    "ForumSearchContractIngress",
    "causation_id()",
    "root_event_id",
    "self.inbox",
    "verify_durable_root",
    "InboxIdentityConflict",
  ],
  "typed Forum invalidation ingress",
);
const inbox = read(inboxPath);
requireAll(
  inbox,
  [
    "INSERT INTO search_projection_inbox",
    "ON CONFLICT (event_id) DO NOTHING",
    "ORDER BY ingest_sequence ASC",
    "ForumReplyStatusChanged",
    "ForumTopicStatusChanged",
  ],
  "durable Forum Search inbox",
);

const reconciliation = read(reconciliationPath);
requireAll(
  reconciliation,
  [
    "DomainEvent::ForumReplyStatusChanged",
    "DomainEvent::ForumTopicStatusChanged",
    "self.forum_projector",
    "rebuild_tenant(envelope.tenant_id)",
    '("forum_topic", Some(_))',
    '("forum_category", Some(category_id))',
    "refresh_entity",
  ],
  "production Forum Search reconciliation",
);

const projector = read(projectorPath);
requireAll(
  projector,
  [
    "self.source",
    ".list_public_documents(",
    "delete_forum_scope(&tx, tenant_id)",
    "INSERT INTO search_documents",
    "if entity_type == FORUM_TOPIC_ENTITY_TYPE",
    "return self.rebuild_tenant(tenant_id).await",
    "delete_forum_entity",
  ],
  "production Forum current-state projector",
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
    "Forum storefront Search requires an explicit Forum-only resolved source scope",
  ],
  "production storefront eligibility ordering",
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

const serverCargo = read(serverCargoPath);
requireAll(
  serverCargo,
  [
    "rustok-core = { workspace = true, features = [\"redis-cache\", \"server\"] }",
    "rustok-api = { workspace = true, features = [\"server\"] }",
    "rustok-events.workspace = true",
    "rustok-forum     = { workspace = true, optional = true }",
    "rustok-search = { workspace = true, features = [\"graphql\"] }",
    "rustok-outbox.workspace = true",
    "rustok-taxonomy  = { workspace = true, optional = true }",
    "sea-orm.workspace = true",
    "sea-orm-migration.workspace = true",
    "tokio.workspace = true",
    "serde.workspace = true",
    "serde_json.workspace = true",
    "uuid.workspace = true",
    "chrono.workspace = true",
    "async-trait.workspace = true",
  ],
  "rustok-server existing host dependencies",
);

const doc = read(docPath);
requireAll(
  doc,
  [
    "`source_ready_maintainer_execution_pending`",
    "FORUM-23B2G2B3D8",
    contractPath,
    testPath,
    evidencePath,
    "revision N+1: forum_category",
    "owner revision N+4",
    "legacy reply-hide status root",
    "deliberately stale `search_documents` rows",
    "total: 0",
    "No command above was run by the implementation agent",
  ],
  "Forum Search deletion/ACL handoff",
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
]) {
  assert.ok(
    parent.source_ready_subproofs.some((subproof) => subproof.task === task),
    `${task} disappeared from the D0 source-ready subproof list`,
  );
}
const subproof = parent.source_ready_subproofs.find(
  ({ task }) => task === "FORUM-23B2G2B3D8",
);
assert.equal(subproof.contract, contractPath);
assert.equal(subproof.test, testPath);
assert.equal(subproof.evidence_artifact, evidencePath);
assert.deepEqual(subproof.covers, [
  "owner_hide_delete_and_richer_acl_revisions",
  "legacy_and_typed_dual_path_out_of_order_admission",
  "durable_duplicate_root_suppression",
  "current_state_rebuild_and_stale_category_refresh_non_restoration",
  "search_projection_denied_objects_absent",
  "storefront_owner_reauthorization_rejects_stale_rows",
  "visible_totals_items_and_facets_exclude_denied_content",
]);
assert.deepEqual(subproof.does_not_cover, [
  "long_running_host_polling_or_restart_timing",
  "iggy_acknowledgement_poison_or_dlq",
  "search_disabled_profile_or_link_forum_03",
]);
assert.ok(
  parent.maintainer_commands.includes(
    "node scripts/verify/verify-forum-search-versioned-invalidation-deletion-acl-ordering-proof.mjs",
  ),
);
assert.ok(
  parent.maintainer_commands.some((command) =>
    command.includes("--test forum_versioned_invalidation_deletion_acl_ordering"),
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
    "D8 closes FORUM-23",
    "deletion/ACL runtime evidence passed",
    "LINK-FORUM-03 is complete",
  ],
  "FORUM-23 canonical aggregate boundary",
);

console.log(
  "Forum Search deletion/ACL ordering proof is source-synchronized.",
);
