#!/usr/bin/env node

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const read = (path) => readFileSync(path, 'utf8');
const contract = JSON.parse(
  read('crates/rustok-forum/contracts/forum-topic-fork-admin-ui.json')
);
const rustModel = read('crates/rustok-forum/admin/src/topic_fork_model.rs');
const rustTransport = read('crates/rustok-forum/admin/src/transport.rs');
const rustAdapter = read(
  'crates/rustok-forum/admin/src/transport/topic_fork_graphql_adapter.rs'
);
const rustUi = read('crates/rustok-forum/admin/src/ui/topic_fork.rs');
const rustRoot = read('crates/rustok-forum/admin/src/ui/root.rs');
const nextModel = read('apps/next-admin/packages/forum/src/core/topic-fork.ts');
const nextApi = read('apps/next-admin/packages/forum/src/api/forum.ts');
const nextComponent = read(
  'apps/next-admin/packages/forum/src/components/forum-topic-fork.tsx'
);
const nextPage = read('apps/next-admin/src/app/dashboard/forum/fork/page.tsx');
const nextNav = read('apps/next-admin/packages/forum/src/nav.ts');
const docs = read('crates/rustok-forum/docs/forum-21w-topic-fork-admin-ui.md');

assert.equal(contract.contract, 'forum_topic_fork_admin_ui_v1');
assert.equal(contract.task, 'FORUM-21W');
assert.deepEqual(contract.extends, ['FORUM-21Q', 'FORUM-21U']);
assert.equal(contract.command.graphql_field, 'forkForumTopicReplyBranch');
assert.equal(contract.command.root_reply_selection_limit, 1);
assert.equal(contract.command.owner_branch_limit, 500);
assert.equal(contract.command.operation_id_is_retained_for_exact_retry, true);
assert.equal(contract.command.target_topic_id_is_retained_for_exact_retry, true);
assert.equal(contract.command.both_ids_rotate_when_command_shape_changes, true);
assert.equal(contract.preflight.owner_discovers_descendants, true);
assert.equal(contract.composition.owner_method_changed, false);
assert.equal(contract.composition.graphql_schema_changed, false);
assert.equal(contract.composition.transport_local_branch_discovery, false);
assert.equal(contract.composition.transport_local_reply_copy, false);
assert.equal(contract.composition.transport_local_counter_reconciliation, false);
assert.equal(contract.composition.transport_fallback, false);

for (const marker of [
  'MAX_FORUM_TOPIC_FORK_REPLIES: usize = 500',
  'build_forum_topic_fork_command',
  'new_forum_topic_fork_identity',
  'Choose the root reply to fork',
  'The selected root reply is not loaded for this topic',
  'exact_command_keeps_retry_and_target_identities',
  'changed_shape_rotates_both_identities'
]) {
  assert.ok(rustModel.includes(marker), `missing Rust model marker: ${marker}`);
}

for (const marker of [
  'mod topic_fork_graphql_adapter;',
  'pub async fn fetch_topic_fork_candidates',
  'pub async fn fetch_topic_fork_replies',
  'pub async fn fork_topic',
  'topic_fork_graphql_adapter::fork_topic',
  'topic_fork_uses_the_manager_graphql_transport_without_fallback'
]) {
  assert.ok(rustTransport.includes(marker), `missing Rust transport marker: ${marker}`);
}

for (const marker of [
  'FORK_CANDIDATES_QUERY',
  'FORK_REPLIES_QUERY',
  'FORK_TOPIC_MUTATION',
  'forkForumTopicReplyBranch',
  'ForkForumTopicReplyBranchGraphqlInput',
  'limit: 500',
  'copied_reply_count: copiedReplyCount',
  'copied_quote_count: copiedQuoteCount'
]) {
  assert.ok(rustAdapter.includes(marker), `missing Rust adapter marker: ${marker}`);
}
for (const forbidden of [
  'forum_topic_fork_operations',
  'forum_topic_fork_reply_items',
  'UPDATE forum_replies',
  'INSERT INTO forum_replies'
]) {
  assert.ok(!rustAdapter.includes(forbidden), `Rust adapter owns copy policy: ${forbidden}`);
  assert.ok(!rustUi.includes(forbidden), `Rust UI owns copy policy: ${forbidden}`);
}

for (const marker of [
  'pub fn ForumTopicForkAdmin',
  '"FORUM-21W"',
  'fetch_topic_fork_candidates',
  'fetch_topic_fork_replies',
  'build_forum_topic_fork_command',
  'transport::fork_topic',
  'new_forum_topic_fork_identity',
  'receipt.target_topic_id',
  'receipt.copied_reply_count'
]) {
  assert.ok(rustUi.includes(marker), `missing Leptos marker: ${marker}`);
}
for (const marker of [
  'use rustok_api::normalize_locale_tag;',
  'fn forum_topic_fork_content_lang(locale: &str) -> String',
  'lang=move || forum_topic_fork_content_lang(target_locale.get().as_str())',
  'dir="auto"',
  'dir="ltr"',
  'spellcheck="false"',
  'fork_target_content_lang_uses_shared_locale_normalization',
  'fork_target_content_lang_fails_closed_for_invalid_locale'
]) {
  assert.ok(rustUi.includes(marker), `missing fork bidi marker: ${marker}`);
}
assert.ok(rustRoot.includes('subpath_matches("fork")'));
assert.ok(rustRoot.includes('<ForumTopicForkAdmin />'));

for (const marker of [
  'MAX_FORUM_TOPIC_FORK_REPLIES = 500',
  'newForumTopicForkIdentity',
  'buildForumTopicForkCommand',
  'Choose the root reply to fork',
  'The selected root reply is not loaded for this topic'
]) {
  assert.ok(nextModel.includes(marker), `missing Next model marker: ${marker}`);
}

for (const marker of [
  'export async function forkForumTopicReplyBranch',
  'forkForumTopicReplyBranch(',
  'ForkForumTopicReplyBranchGraphqlInput',
  'rootReplyId: command.rootReplyId',
  'copiedReplyCount',
  'copiedQuoteCount'
]) {
  assert.ok(nextApi.includes(marker), `missing Next API marker: ${marker}`);
}

for (const marker of [
  'export function ForumTopicFork',
  'listForumTopicReplies',
  'forkForumTopicReplyBranch',
  'buildForumTopicForkCommand',
  'newForumTopicForkIdentity',
  'setIdentity(newForumTopicForkIdentity())',
  'receipt.targetTopicId',
  'receipt.copiedReplyCount'
]) {
  assert.ok(nextComponent.includes(marker), `missing Next component marker: ${marker}`);
}
assert.ok(nextPage.includes('ForumTopicFork, listForumTopics'));
assert.ok(nextPage.includes('<ForumTopicFork'));
assert.ok(nextNav.includes("url: '/dashboard/forum/fork'"));
assert.ok(nextNav.includes("access: { permission: 'forum_topics:manage' }"));

for (const marker of [
  '# FORUM-21W topic fork admin composition',
  'An unchanged retry retains both UUIDs',
  'The UI does not claim that the visible page is the complete branch',
  '`FORUM-21` remains `planned`',
  'No command above was run by the implementation agent'
]) {
  assert.ok(docs.includes(marker), `missing docs marker: ${marker}`);
}

console.log('forum topic fork admin UI source contract verified');
