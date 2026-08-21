#!/usr/bin/env node

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const read = (path) => readFileSync(path, 'utf8');
const contract = JSON.parse(
  read('crates/rustok-forum/contracts/forum-topic-split-admin-ui.json')
);
const rustModel = read('crates/rustok-forum/admin/src/topic_split_model.rs');
const rustTransport = read('crates/rustok-forum/admin/src/transport.rs');
const rustAdapter = read(
  'crates/rustok-forum/admin/src/transport/topic_split_graphql_adapter.rs'
);
const rustUi = read('crates/rustok-forum/admin/src/ui/topic_split.rs');
const rustRoot = read('crates/rustok-forum/admin/src/ui/root.rs');
const nextModel = read('apps/next-admin/packages/forum/src/core/topic-split.ts');
const nextApi = read('apps/next-admin/packages/forum/src/api/forum.ts');
const nextComponent = read(
  'apps/next-admin/packages/forum/src/components/forum-topic-split.tsx'
);
const nextPage = read('apps/next-admin/src/app/dashboard/forum/split/page.tsx');
const nextNav = read('apps/next-admin/packages/forum/src/nav.ts');
const docs = read('crates/rustok-forum/docs/forum-21v-topic-split-admin-ui.md');

assert.equal(contract.contract, 'forum_topic_split_admin_ui_v1');
assert.equal(contract.task, 'FORUM-21V');
assert.deepEqual(contract.extends, ['FORUM-21P', 'FORUM-21R']);
assert.equal(contract.command.graphql_field, 'splitForumTopicReplies');
assert.equal(contract.command.reply_selection_limit, 500);
assert.equal(contract.command.operation_id_is_retained_for_exact_retry, true);
assert.equal(contract.command.target_topic_id_is_retained_for_exact_retry, true);
assert.equal(contract.command.both_ids_rotate_when_command_shape_changes, true);
assert.equal(contract.composition.owner_method_changed, false);
assert.equal(contract.composition.graphql_schema_changed, false);
assert.equal(contract.composition.transport_local_reply_copy, false);
assert.equal(contract.composition.transport_local_counter_reconciliation, false);
assert.equal(contract.composition.transport_fallback, false);

for (const marker of [
  'MAX_FORUM_TOPIC_SPLIT_REPLIES: usize = 500',
  'build_forum_topic_split_command',
  'new_forum_topic_split_identity',
  'The source topic must retain at least one reply',
  'A selected child reply requires its parent to be selected',
  'Selecting a parent requires every loaded child to be selected',
  'exact_command_keeps_retry_and_target_identities',
  'changed_shape_rotates_both_identities'
]) {
  assert.ok(rustModel.includes(marker), `missing Rust model marker: ${marker}`);
}

for (const marker of [
  'mod topic_split_graphql_adapter;',
  'pub async fn fetch_topic_split_candidates',
  'pub async fn fetch_topic_split_replies',
  'pub async fn split_topic',
  'topic_split_graphql_adapter::split_topic',
  'topic_split_uses_the_manager_graphql_transport_without_fallback'
]) {
  assert.ok(rustTransport.includes(marker), `missing Rust transport marker: ${marker}`);
}
assert.ok(!rustTransport.match(/split_topic[\s\S]{0,700}native_server_adapter/));

for (const marker of [
  'SPLIT_CANDIDATES_QUERY',
  'SPLIT_REPLIES_QUERY',
  'SPLIT_TOPIC_MUTATION',
  'splitForumTopicReplies',
  'SplitForumTopicRepliesGraphqlInput',
  'limit: 500',
  'target_resulting_published_reply_count: targetResultingPublishedReplyCount'
]) {
  assert.ok(rustAdapter.includes(marker), `missing Rust adapter marker: ${marker}`);
}
for (const forbidden of [
  'forum_topic_split_operations',
  'forum_topic_split_reply_items',
  'UPDATE forum_replies',
  'INSERT INTO forum_replies'
]) {
  assert.ok(!rustAdapter.includes(forbidden), `Rust adapter owns policy: ${forbidden}`);
  assert.ok(!rustUi.includes(forbidden), `Rust UI owns policy: ${forbidden}`);
}

for (const marker of [
  'pub fn ForumTopicSplitAdmin',
  '"FORUM-21V"',
  'fetch_topic_split_candidates',
  'fetch_topic_split_replies',
  'build_forum_topic_split_command',
  'transport::split_topic',
  'new_forum_topic_split_identity',
  'receipt.target_topic_id',
  'receipt.moved_reply_count'
]) {
  assert.ok(rustUi.includes(marker), `missing Leptos marker: ${marker}`);
}
for (const marker of [
  'use rustok_api::normalize_locale_tag;',
  'fn forum_topic_split_content_lang(locale: &str) -> String',
  'lang=move || forum_topic_split_content_lang(target_locale.get().as_str())',
  'dir="auto"',
  'dir="ltr"',
  'spellcheck="false"',
  'split_target_content_lang_uses_shared_locale_normalization',
  'split_target_content_lang_fails_closed_for_invalid_locale'
]) {
  assert.ok(rustUi.includes(marker), `missing split bidi marker: ${marker}`);
}
assert.ok(rustRoot.includes('subpath_matches("split")'));
assert.ok(rustRoot.includes('<ForumTopicSplitAdmin />'));

for (const marker of [
  'MAX_FORUM_TOPIC_SPLIT_REPLIES = 500',
  'newForumTopicSplitIdentity',
  'buildForumTopicSplitCommand',
  'The source topic must retain at least one reply',
  'A selected child reply requires its parent to be selected',
  'Selecting a parent requires every loaded child to be selected'
]) {
  assert.ok(nextModel.includes(marker), `missing Next model marker: ${marker}`);
}

for (const marker of [
  'export async function listForumTopicReplies',
  'export async function splitForumTopicReplies',
  'splitForumTopicReplies(',
  'SplitForumTopicRepliesGraphqlInput',
  'targetResultingPublishedReplyCount',
  'first: input.first ?? 500'
]) {
  assert.ok(nextApi.includes(marker), `missing Next API marker: ${marker}`);
}

for (const marker of [
  'export function ForumTopicSplit',
  'listForumTopicReplies',
  'splitForumTopicReplies',
  'buildForumTopicSplitCommand',
  'newForumTopicSplitIdentity',
  'setIdentity(newForumTopicSplitIdentity())',
  'receipt.targetTopicId',
  'receipt.movedReplyCount'
]) {
  assert.ok(nextComponent.includes(marker), `missing Next component marker: ${marker}`);
}
assert.ok(nextPage.includes("ForumTopicSplit, listForumTopics"));
assert.ok(nextPage.includes('<ForumTopicSplit'));
assert.ok(nextNav.includes("url: '/dashboard/forum/split'"));
assert.ok(nextNav.includes("access: { permission: 'forum_topics:manage' }"));

for (const marker of [
  '# FORUM-21V topic split admin composition',
  'An unchanged retry retains both UUIDs',
  'The owner remains authoritative',
  'FORUM-21 remains `planned`',
  'No command above was run by the implementation agent'
]) {
  assert.ok(docs.includes(marker), `missing docs marker: ${marker}`);
}

console.log('forum topic split admin UI source contract verified');
