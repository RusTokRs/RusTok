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
  "crates/rustok-forum/contracts/forum-search-link-forum-03-topic-move-proof.json";
const ownerContractPath =
  "crates/rustok-forum/contracts/forum-topic-move-owner.json";
const docPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d16-topic-move-proof.md";
const testPath =
  "apps/server/tests/forum_versioned_invalidation_topic_move.rs";
const verifierPath =
  "scripts/verify/verify-forum-search-link-forum-03-topic-move-proof.mjs";
const evidencePath =
  "target/forum-search-link-forum-03-topic-move-evidence.json";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const moveServicePath = "crates/rustok-forum/src/services/topic_move.rs";
const projectionSourcePath = "crates/rustok-forum/src/search_projection.rs";
const reconcilerPath = "crates/rustok-search/src/forum_reconciliation.rs";
const categoryOwnerPath =
  "crates/rustok-forum/src/services/category_projection_owner.rs";
const topicOwnerPath = "crates/rustok-forum/src/services/topic_inline.rs";
const replyOwnerPath = "crates/rustok-forum/src/services/reply_owner.rs";

const contract = JSON.parse(read(contractPath));
assert.equal(
  contract.contract,
  "forum_search_link_forum_03_topic_move_proof_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D16");
assert.equal(contract.target_link, "LINK-FORUM-03");
assert.equal(contract.coverage, "topic_move_category_scope_only");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.deepEqual(contract.owner_dependency, {
  task: "FORUM-21A",
  contract: ownerContractPath,
  service: "ForumTopicMoveService",
  operation_receipt: "forum_topic_move_operations",
  semantic_event: "forum.topic.moved",
});
assert.equal(contract.test, testPath);
assert.equal(contract.verifier, verifierPath);
assert.equal(contract.evidence_artifact, evidencePath);
assert.equal(contract.required_runtime.database, "postgresql");
assert.equal(contract.required_runtime.broker_used, false);
assert.equal(contract.required_runtime.host_package, "rustok-server");
assert.equal(contract.required_runtime.search_inbox, "search_projection_inbox");
assert.equal(contract.required_runtime.projector, "ForumProjectionReconciler");
assert.equal(
  contract.required_runtime.storefront_execution,
  "execute_forum_storefront_search",
);
assert.equal(contract.required_owner_trace.length, 8);
assert.ok(contract.fail_closed_requirements.length >= 15);
assert.ok(contract.proves_after_maintainer_execution.length >= 6);
assert.deepEqual(contract.maintainer_commands, [
  `node ${verifierPath}`,
  'RUSTOK_SEARCH_TEST_DATABASE_URL="$DATABASE_URL" cargo test -p rustok-server --test forum_versioned_invalidation_topic_move -- --nocapture --test-threads=1',
]);
assert.ok(
  contract.non_claims.some((claim) => claim.includes("FORUM-21A")),
  "D16 must retain the separate owner promotion gate",
);
assert.ok(
  contract.non_claims.some((claim) => claim.includes("not sufficient")),
  "D16 must not claim canonical completion",
);

const ownerContract = JSON.parse(read(ownerContractPath));
assert.equal(ownerContract.contract, "forum_topic_move_owner_v1");
assert.equal(ownerContract.task, "FORUM-21A");
assert.equal(ownerContract.parent_task, "FORUM-21");
assert.equal(ownerContract.status, "source_ready_maintainer_execution_pending");
assert.equal(ownerContract.canonical_plan_status, "planned");
assert.equal(ownerContract.owner_service, "ForumTopicMoveService");
assert.equal(ownerContract.receipt_table, "forum_topic_move_operations");
assert.deepEqual(ownerContract.semantic_event, {
  journal: "forum_domain_events",
  event_type: "forum.topic.moved",
  schema_version: 1,
  event_id_equals_operation_id: true,
  shared_rustok_events_contract_changed: false,
});

const test = read(testPath);
requireAll(
  test,
  [
    "forum_search_link_forum_03_topic_move_evidence_v1",
    'task: "FORUM-23B2G2B3D16"',
    evidencePath,
    "PostgresTopicMoveEvidence::setup",
    "OutboxModule.migrations()",
    "TaxonomyModule.migrations()",
    "ForumModule.migrations()",
    "SearchModule.migrations()",
    "CategoryService::new",
    "TopicService::new",
    "ReplyService::new",
    "ForumTopicMoveService::new",
    "MoveForumTopicInput",
    "MOVE_REASON",
    "forum_topic_move_operations",
    "forum.topic.moved",
    "event_id != operation_id",
    "forum_projection_revision_ledger",
    '("forum", None)',
    '("forum_topic", Some(fixture.topic_id))',
    '("forum_category", Some(fixture.source_category_id))',
    '("forum_category", Some(fixture.target_category_id))',
    "load_root_envelope",
    "load_typed_envelope",
    "ContractEventPayload::ForumSearchProjection",
    "ForumSearchProjectionEvent::InvalidationIssued",
    "typed.causation_id() != Some(revision.event_id)",
    "ForumSearchContractIngress::new",
    "ForumSearchContractIngressOutcome::DurablyAccepted",
    "search_projection_inbox",
    "ForumSearchProjectionSourceFactory.build",
    "ForumProjectionReconciler::new",
    "execute_forum_storefront_search",
    "ForumSearchResultEligibilityService::new",
    "ensure_document_scope",
    "document_category_id(topic)? != expected_category_id",
    "document_category_id(reply)? != expected_category_id",
    'source.payload["topic_count"]',
    'target.payload["reply_count"]',
    "roots_before_replay",
    "typed_before_replay",
    "max_ingest_before_replay",
    "load_owner_revisions_after(db, fixture.tenant_id, 7)",
    "count_move_receipts",
    "count_move_semantic_events",
    'id: "topic_move_category_scope"',
    'result: "passed"',
    '"topic_identity_retained": true',
    '"reply_identity_retained": true',
    '"source_category_scope_empty_after_move": true',
    '"target_category_scope_contains_topic_and_reply_after_move": true',
    '"exact_replay_created_new_owner_revision": false',
    '"owner_revision_compared_to_ingest_sequence": false',
    '"caught_up_repeat_performed_work": false',
    '.args(["rev-parse", "HEAD"])',
    "evidence.cleanup().await",
  ],
  "D16 executable proof",
);
forbidAll(
  test,
  [
    "#[ignore]",
    'result: "skipped"',
    "static_fixture",
    "MockForum",
    "FakeForum",
    "InMemory",
    "INSERT INTO search_documents",
    "UPDATE search_documents",
    "UPDATE forum_topics SET category_id",
    "UPDATE forum_topics\nSET category_id",
    "INSERT INTO forum_topic_move_operations",
    "INSERT INTO forum_domain_events",
    "owner_revision == inbox.ingest_sequence",
    "owner_revision as i64 ==",
  ],
  "D16 executable proof",
);

const moveService = read(moveServicePath);
requireAll(
  moveService,
  [
    "pub struct ForumTopicMoveService",
    "pub async fn move_topic",
    "lock_topic_move_tenant_in_tx(&txn, tenant_id).await?",
    "forum_topic_move_operation::Entity::find_by_id",
    "validate_existing_semantic_event_in_tx",
    "ForumError::TopicMoveOperationConflict",
    "transfer_category_counters_in_tx",
    "active.category_id = Set(input.target_category_id)",
    'const FORUM_TOPIC_MOVED_EVENT_TYPE: &str = "forum.topic.moved";',
    "event_id: Set(input.operation_id)",
    "publish_forum_topic_projection_in_tx",
    "publish_forum_category_projection_in_tx",
    "txn.commit().await?",
  ],
  "FORUM-21A topic move owner",
);
assert.ok(
  (moveService.match(/publish_forum_category_projection_in_tx\(/g) ?? []).length >= 2,
  "topic move owner must invalidate source and target categories",
);
assert.ok(
  moveService.indexOf("publish_forum_topic_projection_in_tx(") <
    moveService.indexOf("publish_forum_category_projection_in_tx("),
  "topic invalidation must precede category invalidations",
);
forbidAll(
  moveService,
  [".max(0)", "saturating_", "DomainEvent::ForumTopicMoved"],
  "FORUM-21A topic move owner",
);

const categoryOwner = read(categoryOwnerPath);
requireAll(
  categoryOwner,
  [
    "CategoryProjectionOwnerService",
    "pub(super) async fn create",
    "publish_forum_projection_scope_direct_in_tx",
    "txn.commit().await?",
  ],
  "category owner",
);
const topicOwner = read(topicOwnerPath);
requireAll(
  topicOwner,
  [
    "create_with_inline_relations",
    "publish_forum_category_projection_in_tx",
    "txn.commit().await?",
  ],
  "topic owner",
);
const replyOwner = read(replyOwnerPath);
requireAll(
  replyOwner,
  [
    "if status == ReplyStatus::Approved",
    "TopicService::adjust_reply_count_in_tx",
    "CategoryService::adjust_counters_in_tx",
    "publish_forum_category_projection_in_tx",
    "txn.commit().await?",
  ],
  "approved reply owner",
);

const projectionSource = read(projectionSourcePath);
requireAll(
  projectionSource,
  [
    'document_key: format!("forum_topic:{}:{locale}"',
    'document_key: format!("forum_reply:{reply_id}:{locale}")',
    '"category_id": topic.category_id',
    '"topic_id": topic.id',
    '"topic_count": category.topic_count',
    '"reply_count": category.reply_count',
    "ReplyStatus::Approved",
  ],
  "Forum Search projection source",
);

const reconciler = read(reconcilerPath);
requireAll(
  reconciler,
  [
    "pub struct ForumProjectionReconciler",
    "pub async fn sweep_due",
    '("forum", _) | ("forum_topic", Some(_))',
    ".rebuild_tenant(envelope.tenant_id)",
    '("forum_category", Some(category_id))',
    '.refresh_entity(envelope.tenant_id, "forum_category", *category_id)',
  ],
  "Forum production reconciler",
);

const plan = read(planPath);
requireAll(
  plan,
  [
    "| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |",
    "| `FORUM-23` | `in_progress` |",
    "| `LINK-FORUM-03` | `planned` | Forum/index/search ordering and visibility proof. |",
    "## `LINK-FORUM-03` — index and search",
    "Prove publish, translation, moderation approval, move, hide/delete, ACL change,",
  ],
  "Forum canonical plan",
);
forbidAll(
  plan,
  [
    "| `LINK-FORUM-03` | `done` |",
    "| `FORUM-21` | `done` | Move, merge, split and fork topic workflows. |",
    "FORUM-23B2G2B3D16 closes LINK-FORUM-03",
  ],
  "Forum canonical plan",
);

const doc = read(docPath);
requireAll(
  doc,
  [
    "`source_ready_maintainer_execution_pending`",
    "FORUM-23B2G2B3D16",
    "LINK-FORUM-03",
    contractPath,
    testPath,
    evidencePath,
    "ForumTopicMoveService::move_topic",
    "revisions 1 through 7",
    "source category storefront searches return zero items",
    "target category storefront searches return the exact topic and reply",
    "No command above was run by the implementation agent",
  ],
  "D16 handoff",
);

console.log(
  "Forum Search LINK-FORUM-03 topic-move proof is source-ready and fail-closed.",
);
