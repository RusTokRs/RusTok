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
  "crates/rustok-forum/contracts/forum-search-link-forum-03-translation-moderation-proof.json";
const docPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d14-translation-moderation-proof.md";
const testPath =
  "apps/server/tests/forum_versioned_invalidation_translation_moderation.rs";
const verifierPath =
  "scripts/verify/verify-forum-search-link-forum-03-translation-moderation-proof.mjs";
const evidencePath =
  "target/forum-search-link-forum-03-translation-moderation-evidence.json";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const topicInlinePath = "crates/rustok-forum/src/services/topic_inline.rs";
const categoryOwnerPath =
  "crates/rustok-forum/src/services/category_projection_owner.rs";
const moderationOwnerPath =
  "crates/rustok-forum/src/services/moderation_owner.rs";
const replyOwnerPath = "crates/rustok-forum/src/services/reply_owner.rs";
const projectionSourcePath = "crates/rustok-forum/src/search_projection.rs";
const invalidationContractPath =
  "crates/rustok-forum/contracts/forum-projection-invalidation.json";

const contract = JSON.parse(read(contractPath));
assert.equal(
  contract.contract,
  "forum_search_link_forum_03_translation_moderation_proof_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D14");
assert.equal(contract.target_link, "LINK-FORUM-03");
assert.equal(contract.coverage, "translation_and_moderation_approval_only");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
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
assert.equal(contract.required_owner_trace.length, 6);
assert.ok(contract.fail_closed_requirements.length >= 12);
assert.ok(contract.proves_after_maintainer_execution.length >= 5);
assert.deepEqual(contract.maintainer_commands, [
  `node ${verifierPath}`,
  'RUSTOK_SEARCH_TEST_DATABASE_URL="$DATABASE_URL" cargo test -p rustok-server --test forum_versioned_invalidation_translation_moderation -- --nocapture --test-threads=1',
]);
assert.ok(
  contract.non_claims.some((claim) => claim.includes("FORUM-21")),
  "D14 must retain the topic-move owner dependency",
);
assert.ok(
  contract.non_claims.some((claim) => claim.includes("not sufficient")),
  "D14 must not claim LINK-FORUM-03 completion",
);

const test = read(testPath);
requireAll(
  test,
  [
    "forum_search_link_forum_03_translation_moderation_evidence_v1",
    'task: "FORUM-23B2G2B3D14"',
    evidencePath,
    "PostgresTranslationModerationEvidence::setup",
    "OutboxModule.migrations()",
    "TaxonomyModule.migrations()",
    "ForumModule.migrations()",
    "SearchModule.migrations()",
    "CategoryService::new",
    "TopicService::new",
    "ReplyService::new",
    "ModerationService::new",
    "UpdateCategoryInput",
    "UpdateTopicInput",
    'locale: "fr".to_string()',
    "FRENCH_CATEGORY_MARKER",
    "FRENCH_TOPIC_MARKER",
    "APPROVED_REPLY_MARKER",
    "reply.status != \"pending\"",
    ".approve_reply(",
    '"pending",\n        "approved"',
    "forum_projection_revision_ledger",
    "load_root_envelope",
    "load_typed_envelope",
    "ContractEventPayload::ForumSearchProjection",
    "ForumSearchProjectionEvent::InvalidationIssued",
    "typed.causation_id() != Some(revision.event_id)",
    "ForumSearchContractIngress::new",
    "ForumSearchContractIngressOutcome::DurablyAccepted",
    "insert_legacy_root(db, &approval_status_event, \"forum\")",
    "search_projection_inbox",
    "approval_inbox_order[0].event_id != approval_status_event.id",
    "approval_inbox_order[1].event_id != approval_revisions[0].event_id",
    "ForumSearchProjectionSourceFactory.build",
    "ForumProjectionReconciler::new",
    "execute_forum_storefront_search",
    "ForumSearchResultEligibilityService::new",
    "ensure_baseline_documents",
    "ensure_translated_documents",
    "ensure_approved_documents",
    "pending_reply_visible_before_approval",
    "approved_reply_visible_after_approval",
    "owner_revision_compared_to_ingest_sequence",
    "caught_up_repeat_performed_work",
    '"topic_move_executed": false',
    '"topic_move_blocked_on": "FORUM-21"',
    'id: "translation_and_moderation_approval"',
    'result: "passed"',
    '.args(["rev-parse", "HEAD"])',
    "evidence.cleanup().await",
  ],
  "D14 executable proof",
);
forbidAll(
  test,
  [
    "#[ignore]",
    "result: \"skipped\"",
    "static_fixture",
    "MockForum",
    "FakeForum",
    "InMemory",
    "INSERT INTO search_documents",
    "UPDATE forum_topics SET category_id",
    "UPDATE forum_topics\nSET category_id",
    "ForumSearchProjectionEvent::InvalidationIssued {\n            owner_revision: revision.revision",
    "owner_revision == inbox.ingest_sequence",
    "owner_revision as i64 ==",
  ],
  "D14 executable proof",
);

const topicInline = read(topicInlinePath);
requireAll(
  topicInline,
  [
    "update_with_inline_relations",
    "upsert_translation_in_tx",
    "publish_forum_topic_projection_in_tx",
    "txn.commit().await?",
  ],
  "topic translation owner",
);

const categoryOwner = read(categoryOwnerPath);
requireAll(
  categoryOwner,
  [
    "CategoryProjectionOwnerService",
    "pub(super) async fn update",
    "taxonomy_sync::sync_category_copy_in_tx",
    "publish_forum_projection_scope_direct_in_tx",
    "txn.commit().await?",
  ],
  "category translation owner",
);
forbidAll(
  categoryOwner,
  ["forum_category_translation"],
  "category translation owner",
);

const moderationOwner = read(moderationOwnerPath);
requireAll(
  moderationOwner,
  [
    "pub async fn approve_reply",
    "current.validate_transition(&target)?",
    "ReplyService::set_status_in_tx",
    "DomainEvent::ForumReplyStatusChanged",
    "DomainEvent::ForumTopicReplied",
    "publish_forum_category_projection_in_tx",
    "txn.commit().await?",
  ],
  "moderation approval owner",
);

const replyOwner = read(replyOwnerPath);
requireAll(
  replyOwner,
  [
    "let status = if category.moderated",
    "ReplyStatus::Pending",
    "if status == ReplyStatus::Approved",
    "publish_forum_category_projection_in_tx",
  ],
  "pending reply owner",
);

const projectionSource = read(projectionSourcePath);
requireAll(
  projectionSource,
  [
    "forum_category_taxonomy_binding::Entity::find()",
    "TaxonomyOwnerCategoryReader",
    "projection.available_locales",
    "forum_topic_translation::Entity::find()",
    "forum_reply_body::Entity::find()",
    "ReplyStatus::Approved",
    'document_key: format!("forum_topic:{}:{locale}"',
    'document_key: format!("forum_reply:{reply_id}:{locale}")',
    '"category_id": topic.category_id',
    '"topic_id": topic.id',
  ],
  "Forum Search projection source",
);
forbidAll(
  projectionSource,
  ["forum_category_translation::Entity::find()"],
  "Forum Search projection source",
);

const invalidationContract = JSON.parse(read(invalidationContractPath));
assert.equal(
  invalidationContract.forum_scope_invalidation.category_content_or_translation_update,
  true,
);
assert.equal(
  invalidationContract.topic_target_invalidation
    .topic_content_translation_metadata_tags_or_channel_update,
  true,
);
assert.equal(
  invalidationContract.category_target_invalidation
    .reply_moderation_public_state_transition_updates_reply_count,
  true,
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
    "FORUM-23B2G2B3D14 closes LINK-FORUM-03",
  ],
  "Forum canonical plan",
);

const doc = read(docPath);
requireAll(
  doc,
  [
    "`source_ready_maintainer_execution_pending`",
    "FORUM-23B2G2B3D14",
    "LINK-FORUM-03",
    contractPath,
    testPath,
    evidencePath,
    "CategoryService::update",
    "TopicService::update",
    "ModerationService::approve_reply",
    "pending content remains owner state",
    "Forum `owner_revision` is an independent causal clock",
    "Topic move remains a planned owner workflow under `FORUM-21`",
    "No command above was run by the implementation agent",
  ],
  "D14 handoff",
);

console.log(
  "Forum Search LINK-FORUM-03 translation and moderation proof is source-ready and fail-closed.",
);
