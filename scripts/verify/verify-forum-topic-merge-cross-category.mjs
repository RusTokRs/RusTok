#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-topic-merge-cross-category.json",
  cumulativeContract: "crates/rustok-forum/contracts/forum-topic-merge-owner.json",
  docs: "crates/rustok-forum/docs/forum-21m-topic-merge-cross-category.md",
  cumulativeDocs: "crates/rustok-forum/docs/forum-21b-topic-merge-owner.md",
  owner: "crates/rustok-forum/src/services/topic_merge.rs",
  test: "crates/rustok-forum/tests/topic_merge_cross_category_sqlite.rs",
  subscriptionReconciliation:
    "crates/rustok-forum/src/services/topic_merge_subscription_reconciliation.rs",
  readStateReconciliation:
    "crates/rustok-forum/src/services/topic_merge_read_state_reconciliation.rs",
  tagReconciliation: "crates/rustok-forum/src/services/topic_merge_tag_reconciliation.rs",
  voteReconciliation: "crates/rustok-forum/src/services/topic_merge_vote_reconciliation.rs",
  audienceReconciliation:
    "crates/rustok-forum/src/services/topic_merge_audience_reconciliation.rs",
  plan: "crates/rustok-forum/docs/implementation-plan.md",
};

const read = (path) => readFileSync(path, "utf8");
const includesAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing marker: ${marker}`);
  }
};
const functionSource = (source, name, nextNames) => {
  const marker = `${name}(`;
  const start = source.indexOf(marker);
  assert.ok(start >= 0, `missing function ${name}`);
  const ends = nextNames
    .map((next) => source.indexOf(`\n${next}(`, start + marker.length))
    .filter((value) => value >= 0);
  const end = ends.length > 0 ? Math.min(...ends) : source.length;
  return source.slice(start, end);
};

const contract = JSON.parse(read(paths.contract));
const cumulativeContract = JSON.parse(read(paths.cumulativeContract));
const docs = read(paths.docs);
const cumulativeDocs = read(paths.cumulativeDocs);
const owner = read(paths.owner);
const test = read(paths.test);
const plan = read(paths.plan);
const reconciliations = [
  paths.subscriptionReconciliation,
  paths.readStateReconciliation,
  paths.tagReconciliation,
  paths.voteReconciliation,
  paths.audienceReconciliation,
].map((path) => [path, read(path)]);

assert.equal(contract.contract, "forum_topic_merge_cross_category_v1");
assert.equal(contract.task, "FORUM-21M");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.extends, "FORUM-21B");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.owner_service, "ForumTopicMergeService");
assert.equal(contract.ordinary_method, "merge_topic");
assert.equal(contract.explicit_solution_method, "merge_topic_resolving_solution");
assert.equal(contract.shared_private_transaction_owner, "merge_topic_internal");
assert.equal(contract.authorization.required_permission, "forum_topics:manage");
assert.equal(contract.category_policy.source_and_target_may_differ, true);
assert.equal(contract.category_policy.source_and_target_categories_must_be_active, true);
assert.equal(
  contract.category_policy.source_topic_retains_source_category_id_as_archived_tombstone,
  true,
);
assert.equal(contract.category_policy.target_topic_retains_target_category_id, true);
assert.equal(contract.category_policy.category_topic_counts_change, false);
assert.equal(contract.category_policy.same_category_reply_count_change, 0);
assert.equal(
  contract.category_policy.cross_category_source_reply_count_delta,
  "-moved_published_reply_count",
);
assert.equal(
  contract.category_policy.cross_category_target_reply_count_delta,
  "+moved_published_reply_count",
);
assert.equal(contract.category_policy.counter_arithmetic_is_checked_and_fail_closed, true);
assert.equal(contract.serialization.category_counter_scopes_are_sorted_and_deduplicated, true);
assert.equal(contract.semantic_event_compatibility.event_type, "forum.topic.merged");
assert.equal(contract.semantic_event_compatibility.schema_version, 1);
assert.equal(contract.semantic_event_compatibility.payload_changed, false);
assert.equal(contract.semantic_event_compatibility.category_id_is_retained_target_category, true);
assert.equal(contract.semantic_event_compatibility.source_category_id_added_to_payload, false);
assert.equal(contract.semantic_event_compatibility.receipt_schema_changed, false);
assert.equal(contract.semantic_event_compatibility.post_merge_reconciliation_owners_changed, false);
assert.deepEqual(contract.projection_invalidation.cross_category_targets, [
  "source_topic",
  "target_topic",
  "source_category",
  "target_category",
]);
assert.equal(contract.projection_invalidation.category_targets_are_deduplicated, true);
assert.equal(contract.idempotency.cross_category_counters_are_not_reapplied, true);
assert.equal(contract.atomicity.partial_cross_category_merge_is_possible, false);
assert.equal(contract.migration_added, false);

assert.equal(cumulativeContract.latest_policy_slice, "FORUM-21M");
assert.equal(cumulativeContract.bounds.same_category_only, false);
assert.equal(cumulativeContract.cross_category_merge.task, "FORUM-21M");
assert.equal(cumulativeContract.cross_category_merge.receipt_schema_changed, false);
assert.equal(cumulativeContract.cross_category_merge.event_contract_changed, false);

includesAll(
  owner,
  [
    "forum_category, forum_category_lifecycle",
    "&[preliminary_source.category_id, preliminary_target.category_id]",
    "let source_category_id = source.category_id;",
    "let target_category_id = target.category_id;",
    "ensure_categories_active_in_tx(",
    "transfer_cross_category_reply_counters_in_tx(",
    "categories.sort();",
    "categories.dedup();",
    "source.topic_count <= 0 || target.topic_count <= 0",
    "source.reply_count < moved_published_reply_count",
    ".checked_sub(moved_published_reply_count)",
    ".checked_add(moved_published_reply_count)",
    "source_active.reply_count = Set(source_reply_count);",
    "target_active.reply_count = Set(target_reply_count);",
    "if target_category_id != source_category_id",
    "source_active.status = Set(TopicStatus::Archived);",
    "source_active.is_locked = Set(true);",
    "schema_version: Set(FORUM_TOPIC_MERGED_SCHEMA_VERSION)",
  ],
  "cross-category merge owner",
);
assert.ok(
  !owner.includes("Forum topic merge requires source and target topics in the same category"),
  "same-category rejection must be removed",
);
assert.equal((owner.match(/self\.db\.begin\(\)\.await\?/g) ?? []).length, 1);
assert.equal((owner.match(/publish_forum_topic_projection_in_tx\(/g) ?? []).length, 2);
assert.equal((owner.match(/publish_forum_category_projection_in_tx\(/g) ?? []).length, 2);

const receiptLookup = owner.indexOf("forum_topic_merge_operation::Entity::find_by_id");
const preliminaryRead = owner.indexOf("let preliminary_source =");
const categoryLocks = owner.indexOf("lock_merge_counter_scopes_in_tx(", preliminaryRead);
const topicLocks = owner.indexOf("lock_topics_in_tx(&txn", categoryLocks);
const categoryValidation = owner.indexOf("ensure_categories_active_in_tx(", topicLocks);
const solutionPlan = owner.indexOf("let solution_plan = plan_solution_merge", categoryValidation);
const counterTransfer = owner.indexOf("transfer_cross_category_reply_counters_in_tx(", solutionPlan);
const replyMove = owner.indexOf("move_replies_in_tx(", counterTransfer);
const eventInsert = owner.indexOf("forum_domain_event::ActiveModel", replyMove);
const receiptInsert = owner.indexOf("forum_topic_merge_operation::ActiveModel", eventInsert);
const invalidation = owner.indexOf("publish_forum_topic_projection_in_tx(", receiptInsert);
assert.ok(receiptLookup < preliminaryRead);
assert.ok(preliminaryRead < categoryLocks && categoryLocks < topicLocks);
assert.ok(topicLocks < categoryValidation && categoryValidation < solutionPlan);
assert.ok(solutionPlan < counterTransfer && counterTransfer < replyMove);
assert.ok(replyMove < eventInsert && eventInsert < receiptInsert && receiptInsert < invalidation);

const payload = functionSource(owner, "fn topic_merged_payload", [
  "async fn validate_existing_semantic_event_in_tx",
]);
includesAll(
  payload,
  [
    '"operation_id": operation_id',
    '"source_topic_id": source_topic_id',
    '"target_topic_id": target_topic_id',
    '"category_id": category_id',
    '"moved_reply_count": moved_reply_count',
    '"moved_published_reply_count": moved_published_reply_count',
    '"resulting_published_reply_count": resulting_published_reply_count',
    '"position_offset": position_offset',
    '"reason": reason',
  ],
  "schema-one merge payload",
);
assert.ok(!payload.includes("source_category_id"));
assert.ok(!owner.includes("FORUM_TOPIC_MERGED_CROSS_CATEGORY_SCHEMA_VERSION"));
assert.ok(!owner.includes('"cross_category"'));

includesAll(
  test,
  [
    "cross_category_topic_merge_transfers_published_reply_counters_once",
    "cross_category_topic_merge_rolls_back_on_source_counter_drift",
    "assert_category_counters(&db, tenant_id, source_category_id, 1, 0)",
    "assert_category_counters(&db, tenant_id, target_category_id, 1, 3)",
    "assert_eq!(payload_object.len(), 9);",
    '!payload_object.contains_key("source_category_id")',
    "assert_eq!(new_projection_ids.len(), 4);",
    'error.stable_code(), "FORUM_VALIDATION_FAILED"',
    "assert_eq!(merge_operation_count(&db, tenant_id).await?, 0);",
  ],
  "cross-category SQLite regression",
);

for (const [path, reconciliation] of reconciliations) {
  includesAll(
    reconciliation,
    [
      'const FORUM_TOPIC_MERGED_EVENT_TYPE: &str = "forum.topic.merged";',
      "event.schema_version != 1",
      "event.payload != expected_payload",
    ],
    `unchanged reconciliation ${path}`,
  );
}

includesAll(
  docs,
  [
    "# FORUM-21M checked cross-category topic merge",
    "`source_ready_maintainer_execution_pending`",
    paths.contract,
    "category `topic_count` values do not change",
    "forum.topic.merged / schema version 1",
    "No command above was run by the implementation agent",
  ],
  "FORUM-21M handoff",
);
includesAll(
  cumulativeDocs,
  [
    "FORUM-21M",
    "Cross-category category counters",
    paths.contract,
  ],
  "cumulative merge handoff",
);
assert.ok(plan.includes("### Delivered through `FORUM-21M`"));
assert.ok(plan.includes("checked cross-category merge"));
assert.ok(plan.includes("| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |"));
assert.ok(!plan.includes("| `FORUM-21` | `done` |"));
assert.ok(!plan.includes("| `FORUM-21` | `in_progress` |"));

console.log(
  "FORUM-21M checked cross-category topic merge source is ready; FORUM-21 remains planned.",
);
