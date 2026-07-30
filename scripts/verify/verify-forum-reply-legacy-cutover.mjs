import fs from 'node:fs';
import path from 'node:path';

const root = process.cwd();
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const exists = (relative) => fs.existsSync(path.join(root, relative));
const requireText = (source, needle, label) => {
  if (!source.includes(needle)) {
    throw new Error(`missing ${label}: ${needle}`);
  }
};

const snapshotPath = 'crates/rustok-forum/src/graphql/query.rs';
const cleanupPath =
  'crates/rustok-forum/contracts/forum-graphql-query-snapshot-cleanup.json';
const query = read('crates/rustok-forum/src/graphql/query_runtime.rs');
const graphqlModule = read('crates/rustok-forum/src/graphql/mod.rs');
const graphqlAdapter = read(
  'crates/rustok-forum/storefront/src/transport/graphql_adapter.rs',
);
const nativeAdapter = read(
  'crates/rustok-forum/storefront/src/transport/native_server_adapter.rs',
);
const selector = read('crates/rustok-forum/storefront/src/transport/mod.rs');
const contract = JSON.parse(
  read('crates/rustok-forum/contracts/forum-reply-legacy-cutover.json'),
);
const categoryContract = JSON.parse(
  read('crates/rustok-forum/contracts/forum-category-audience-read.json'),
);
const cleanup = JSON.parse(read(cleanupPath));

requireText(query, 'async fn forum_replies(', 'legacy forumReplies field');
requireText(
  query,
  'list_response_authenticated_owner_visible_with_audience_context',
  'legacy authenticated exact owner call',
);
requireText(
  query,
  'async fn forum_storefront_replies(',
  'legacy forumStorefrontReplies field',
);
requireText(
  query,
  'list_authenticated_storefront_visible_with_audience_context',
  'legacy authenticated storefront exact owner call',
);
requireText(
  query,
  'list_public_storefront_visible_with_locale_fallback',
  'legacy public storefront exact owner call',
);
requireText(
  query,
  'reply_read_audience_port_context(',
  'trusted GraphQL reply context',
);
requireText(
  query,
  'Some(&PUBLIC_REPLY_STATUSES)',
  'approved-only storefront replies',
);
requireText(
  graphqlModule,
  '#[path = "query_runtime.rs"]',
  'canonical GraphQL runtime selector',
);
requireText(
  graphqlAdapter,
  'forumStorefrontReplies',
  'canonical GraphQL legacy field request',
);
requireText(
  nativeAdapter,
  'ForumReplyAudienceReadService',
  'canonical native exact reply owner',
);
requireText(
  nativeAdapter,
  'reply_read_audience_port_context(',
  'canonical native trusted reply context',
);
requireText(
  nativeAdapter,
  'list_authenticated_storefront_visible_with_audience_context',
  'canonical native authenticated exact read',
);
requireText(
  nativeAdapter,
  'list_public_storefront_visible_with_locale_fallback',
  'canonical native public exact read',
);
requireText(
  selector,
  'native_server_adapter::fetch_storefront_forum_server',
  'single native transport request',
);
requireText(
  selector,
  'graphql_adapter::fetch_storefront_forum_graphql',
  'single GraphQL transport request',
);
if (selector.includes('reply_audience_adapter')) {
  throw new Error('transport selector must not retain reply result replacement');
}
if (
  exists(
    'crates/rustok-forum/storefront/src/transport/graphql_reply_audience_adapter.rs',
  )
) {
  throw new Error('temporary GraphQL reply adapter must be removed');
}
if (
  exists(
    'crates/rustok-forum/storefront/src/transport/native_reply_audience_adapter.rs',
  )
) {
  throw new Error('temporary native reply adapter must be removed');
}
if (exists(snapshotPath)) {
  throw new Error('legacy GraphQL query snapshot must be removed');
}

if (contract.task !== 'FORUM-20BG') throw new Error('unexpected task');
for (const key of [
  'legacy_forum_replies_field_preserved',
  'legacy_forum_replies_uses_exact_owner',
  'legacy_forum_replies_uses_trusted_graphql_context',
  'legacy_storefront_replies_field_preserved',
  'legacy_storefront_replies_uses_exact_owner',
  'legacy_storefront_replies_public_path_is_exact',
  'legacy_storefront_replies_authenticated_path_is_exact',
  'graphql_base_adapter_uses_single_reply_fetch',
  'native_base_adapter_uses_single_reply_fetch',
  'native_base_adapter_reuses_host_audience_facts',
  'transport_selector_does_not_replace_reply_results',
  'temporary_graphql_reply_adapter_removed',
  'temporary_native_reply_adapter_removed',
  'category_owner_read_changed',
]) {
  if (!contract.cutover_boundary[key]) {
    throw new Error(`contract must lock ${key}`);
  }
}
if (!contract.compatibility.canonical_graphql_runtime_moved) {
  throw new Error('reply handoff must record the canonical GraphQL runtime move');
}
if (!contract.compatibility.legacy_graphql_snapshot_removed) {
  throw new Error('reply handoff must record legacy snapshot removal');
}
if (contract.graphql_snapshot_cleanup_contract !== cleanupPath) {
  throw new Error('reply handoff must point to snapshot cleanup completion');
}
if (
  contract.downstream_completion !==
  'crates/rustok-forum/contracts/forum-category-audience-read.json'
) {
  throw new Error('reply handoff must point to category-read completion');
}
if (categoryContract.task !== 'FORUM-20BH') {
  throw new Error('unexpected category-read completion task');
}
if (cleanup.task !== 'FORUM-20BN') {
  throw new Error('unexpected snapshot cleanup task');
}
if (contract.downstream_task !== 'FORUM-20BH') {
  throw new Error('unexpected downstream task');
}

console.log('forum reply legacy cutover verified');
