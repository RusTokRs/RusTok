#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-topic-fork-owner.json",
  docs: "crates/rustok-forum/docs/forum-21q-topic-fork-owner.md",
  facade: "crates/rustok-forum/src/services/topic_fork.rs",
  owner: "crates/rustok-forum/src/services/topic_fork_owner.rs",
  storage: "crates/rustok-forum/src/services/topic_fork_storage.rs",
  serviceRegistry: "crates/rustok-forum/src/services/mod.rs",
  crateApi: "crates/rustok-forum/src/lib.rs",
  error: "crates/rustok-forum/src/error.rs",
  migration:
    "crates/rustok-forum/src/migrations/m20260804_000021_add_forum_topic_fork_operations.rs",
  migrationRegistry: "crates/rustok-forum/src/migrations/mod.rs",
  sqlite: "crates/rustok-forum/tests/topic_fork_sqlite.rs",
};

const read = (path) => readFileSync(path, "utf8");
const includesAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing marker: ${marker}`);
  }
};

const contract = JSON.parse(read(paths.contract));
const docs = read(paths.docs);
const facade = read(paths.facade);
const owner = read(paths.owner);
const storage = read(paths.storage);
const serviceRegistry = read(paths.serviceRegistry);
const crateApi = read(paths.crateApi);
const error = read(paths.error);
const migration = read(paths.migration);
const migrationRegistry = read(paths.migrationRegistry);
const sqlite = read(paths.sqlite);

assert.equal(contract.contract, "forum_topic_fork_owner_v1");
assert.equal(contract.task, "FORUM-21Q");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.owner, "ForumTopicForkService");
assert.equal(contract.command, "fork_reply_branch");
assert.equal(contract.authorization.required_permission, "forum_topics:manage");
assert.equal(contract.authorization.human_actor_required, true);
assert.equal(contract.command_shape.maximum_branch_replies, 500);
assert.equal(contract.branch.selection, "root_plus_all_current_descendants");
assert.equal(contract.branch.root_may_have_external_parent, true);
assert.equal(contract.branch.copied_root_parent, null);
assert.equal(contract.branch.source_requires_parent_before_child_positions, true);
assert.equal(contract.reply_identity.source_rows_modified, false);
assert.equal(contract.reply_identity.target_ids_deterministic, true);
assert.equal(contract.content_copy.current_localized_bodies, "all");
assert.equal(
  contract.content_copy.quote_targets,
  "preserve_original_immutable_target_and_revision_ids",
);
assert.equal(contract.content_copy.mention_notifications_emitted, false);
assert.equal(contract.target_topic.target_cannot_broaden_source_access, true);
assert.equal(contract.explicit_non_copy.topic_votes, true);
assert.equal(contract.explicit_non_copy.reply_votes, true);
assert.equal(contract.explicit_non_copy.subscriptions, true);
assert.equal(contract.explicit_non_copy.read_states, true);
assert.equal(contract.explicit_non_copy.accepted_solution, true);
assert.equal(contract.explicit_non_copy.target_starts_unsolved, true);
assert.equal(contract.explicit_non_copy.source_solution_unchanged, true);
assert.equal(
  contract.idempotency.stable_conflict_code,
  "FORUM_TOPIC_FORK_OPERATION_CONFLICT",
);
assert.equal(contract.audit.receipt_append_only, true);
assert.equal(contract.audit.reply_mapping_append_only, true);
assert.equal(contract.audit.revision_mapping_append_only, true);
assert.equal(contract.audit.semantic_event, "forum.topic.forked");
assert.equal(contract.compatibility.graphql_schema_changed, false);
assert.equal(contract.compatibility.rest_routes_changed, false);
assert.equal(contract.compatibility.admin_ui_changed, false);

includesAll(
  facade,
  [
    'include!("topic_fork_owner.rs")',
    'include!("topic_fork_storage.rs")',
    "ForumTopicForkService",
    "lock_topic_solution_scopes_in_tx",
    "&[source_topic_id]",
  ],
  "topic fork facade",
);

includesAll(
  owner,
  [
    "pub struct ForkForumReplyBranchInput",
    "pub struct ForumTopicForkResult",
    "pub struct ForumTopicForkService",
    "pub async fn fork_reply_branch(",
    "Resource::ForumTopics, Action::Manage",
    "MAX_FORUM_TOPIC_FORK_REPLIES: usize = 500",
    "WITH RECURSIVE branch(id)",
    "prepared.root_reply_id",
    "requires parent-before-child reply positions",
    "derive_target_reply_id",
    "FORUM_TOPIC_FORK_REPLY_ID_DOMAIN",
    "Sha256::new()",
    "bytes[6] = (bytes[6] & 0x0f) | 0x50",
    "copy_reply_rows_in_tx",
    "copy_reply_bodies_in_tx",
    "copy_reply_revisions_in_tx",
    "copy_relation_revisions_in_tx",
    "copy_relation_children_in_tx",
    "quoted_id: Set(quote.quoted_id)",
    "quoted_revision_id: Set(quote.quoted_revision_id)",
    "adjust_copied_reply_author_stats_in_tx",
  ],
  "topic fork domain owner",
);

includesAll(
  storage,
  [
    "lock_topic_fork_tenant_in_tx",
    "lock_fork_counter_scopes_in_tx",
    "lock_fork_author_scopes_in_tx",
    "source published reply counter is inconsistent",
    "clone_topic_access_in_tx",
    "clone_topic_tags_in_tx",
    "validate_cloned_topic_shape_in_tx",
    "visibility policy clone is inconsistent",
    "reply-create policy clone is inconsistent",
    "increment_category_counters_in_tx",
    "validate_source_unchanged_in_tx",
    "changed source accepted solution",
    "validate_target_solution_absent_in_tx",
    "target must remain unsolved",
    "insert_fork_operation_in_tx",
    "insert_fork_reply_audit_in_tx",
    "insert_fork_revision_audit_in_tx",
    "validate_replay_in_tx",
    "TopicForkOperationConflict",
    '"reply_identity_policy": "new_deterministic_ids"',
    '"quote_identity_policy": "preserve_original_targets"',
    '"solution_policy": "source_only_not_copied"',
    '"votes_subscriptions_read_state_policy": "not_copied"',
  ],
  "topic fork storage/replay owner",
);

for (const source of [owner, storage]) {
  assert.ok(!source.includes("async_graphql"));
  assert.ok(!source.includes("axum"));
  assert.ok(!source.includes("GraphqlRequest"));
  assert.ok(!source.includes("reqwest"));
  assert.ok(!source.includes("rest_adapter"));
  assert.ok(!source.includes("forum_topic_votes::ActiveModel"));
  assert.ok(!source.includes("forum_reply_votes::ActiveModel"));
  assert.ok(!source.includes("forum_topic_subscription::ActiveModel"));
  assert.ok(!source.includes("forum_topic_read_state::ActiveModel"));
  assert.ok(!source.includes("forum_solution::ActiveModel"));
}

includesAll(
  migration,
  [
    "CREATE TABLE IF NOT EXISTS forum_topic_fork_locks",
    "CREATE TABLE IF NOT EXISTS forum_topic_fork_operations",
    "CREATE TABLE IF NOT EXISTS forum_topic_fork_reply_items",
    "CREATE TABLE IF NOT EXISTS forum_topic_fork_revision_items",
    "copied_reply_count BETWEEN 1 AND 500",
    "copied_body_count BETWEEN copied_reply_count AND 2000",
    "revision_kind IN ('reply', 'relation')",
    "forum topic fork audit is append-only",
    "forum_topic_fork_operation_update",
    "forum_topic_fork_operation_delete",
    "forum_topic_fork_reply_item_update",
    "forum_topic_fork_reply_item_delete",
    "forum_topic_fork_revision_item_update",
    "forum_topic_fork_revision_item_delete",
    "DatabaseBackend::Postgres",
    "DatabaseBackend::Sqlite",
  ],
  "topic fork migration",
);

includesAll(
  error,
  [
    "TopicForkOperationConflict(Uuid)",
    '"FORUM_TOPIC_FORK_OPERATION_CONFLICT"',
  ],
  "Forum error contract",
);
includesAll(
  serviceRegistry,
  [
    "mod topic_fork;",
    "ForkForumReplyBranchInput",
    "ForumTopicForkResult",
    "ForumTopicForkService",
    "MAX_FORUM_TOPIC_FORK_REPLIES",
  ],
  "service registry",
);
includesAll(
  crateApi,
  [
    "ForkForumReplyBranchInput",
    "ForumTopicForkResult",
    "ForumTopicForkService",
    "MAX_FORUM_TOPIC_FORK_REPLIES",
  ],
  "crate API",
);
includesAll(
  migrationRegistry,
  [
    "mod m20260804_000021_add_forum_topic_fork_operations;",
    "m20260804_000021_add_forum_topic_fork_operations::Migration",
  ],
  "migration registry",
);

includesAll(
  sqlite,
  [
    "reply_branch_fork_is_atomic_idempotent_and_preserves_provenance",
    "reply_branch_fork_rejects_non_topological_source_positions_atomically",
    "assert_eq!(first, replay)",
    "forum_topic_fork_reply_items",
    "forum_topic_fork_revision_items",
    "forum_reply_bodies",
    "forum_reply_revisions",
    "forum_relation_revisions",
    "forum_user_mentions",
    "forum_audience_mentions",
    "forum_quotes",
    "forum_solutions",
    "TopicForkOperationConflict",
    "UPDATE forum_topic_fork_operations",
    "DELETE FROM forum_topic_fork_reply_items",
    "UPDATE forum_topic_fork_revision_items",
  ],
  "SQLite owner contract",
);

includesAll(
  docs,
  [
    "# FORUM-21Q reply-branch fork owner",
    "`source_ready_maintainer_execution_pending`",
    paths.contract,
    "source reply rows and IDs remain unchanged",
    "retains the original `quoted_id` and `quoted_revision_id`",
    "The target starts unsolved",
    "No command above was run by the implementation agent",
  ],
  "FORUM-21Q handoff",
);

console.log(
  "FORUM-21Q reply-branch fork owner source is ready; maintainer execution evidence and public transport/UI remain pending.",
);
