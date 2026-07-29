import fs from 'node:fs';
import path from 'node:path';

const root = process.cwd();
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const requireText = (source, needle, label) => {
  if (!source.includes(needle)) {
    throw new Error(`missing ${label}: ${needle}`);
  }
};

const topicVisibility = read(
  'crates/rustok-forum/src/services/topic_audience_visibility.rs',
);
const replyOwner = read(
  'crates/rustok-forum/src/services/reply_audience_read.rs',
);
const transport = read('crates/rustok-forum/src/reply_read_transport.rs');
const rest = read('crates/rustok-forum/src/controllers/replies.rs');
const graphql = read('crates/rustok-forum/src/graphql/reply_audience_query.rs');
const graphqlRuntime = read('crates/rustok-forum/src/graphql/runtime_data.rs');
const selector = read('crates/rustok-forum/storefront/src/transport/mod.rs');
const graphqlAdapter = read(
  'crates/rustok-forum/storefront/src/transport/graphql_reply_audience_adapter.rs',
);
const nativeAdapter = read(
  'crates/rustok-forum/storefront/src/transport/native_reply_audience_adapter.rs',
);
const contract = JSON.parse(
  read('crates/rustok-forum/contracts/forum-reply-audience-read.json'),
);

requireText(
  topicVisibility,
  'pub async fn is_topic_owner_visible',
  'owner topic audience visibility',
);
requireText(
  topicVisibility,
  '.is_topic_category_visible_to_viewer(',
  'owner category floor',
);
requireText(
  replyOwner,
  'pub struct ForumReplyAudienceReadService',
  'reply read owner',
);
requireText(
  replyOwner,
  '.is_topic_owner_visible(',
  'reply owner parent-topic authorization',
);
requireText(
  replyOwner,
  '.is_topic_visible(',
  'reply storefront parent-topic authorization',
);
requireText(
  transport,
  'ForumReplyReadOperation',
  'reply read operation identity',
);
requireText(
  transport,
  'FORUM_REPLY_READ_FACTS_DEADLINE',
  'reply facts deadline',
);
requireText(
  rest,
  '.list_authenticated_owner_visible_with_audience_context(',
  'REST exact reply list',
);
requireText(
  rest,
  '.get_authenticated_owner_visible_with_audience_context(',
  'REST exact selected reply',
);
requireText(
  graphql,
  'async fn forum_audience_replies',
  'authenticated exact GraphQL field',
);
requireText(
  graphql,
  'async fn forum_storefront_audience_replies',
  'storefront exact GraphQL field',
);
requireText(
  graphqlRuntime,
  'reply_audience_read_service',
  'GraphQL runtime exact owner factory',
);
requireText(
  graphqlAdapter,
  'forumStorefrontAudienceReplies',
  'GraphQL storefront exact reply query',
);
requireText(
  nativeAdapter,
  'list_authenticated_storefront_visible_with_audience_context',
  'native authenticated exact reply owner',
);
requireText(
  nativeAdapter,
  'list_public_storefront_visible_with_locale_fallback',
  'native public exact reply owner',
);
requireText(
  selector,
  'data.replies = native_reply_audience_adapter::fetch_storefront_replies_server',
  'native final reply replacement',
);
requireText(
  selector,
  'data.replies = graphql_reply_audience_adapter::fetch_storefront_replies_graphql',
  'GraphQL final reply replacement',
);

if (contract.task !== 'FORUM-20BF') throw new Error('unexpected task');
for (const key of [
  'owner_topic_visibility_enforces_category_and_richer_layers',
  'selected_reply_resolves_parent_topic_before_content',
  'reply_list_resolves_parent_topic_before_pagination',
  'rest_get_reply_uses_exact_owner',
  'rest_list_replies_uses_exact_owner',
  'graphql_authenticated_exact_field_added',
  'graphql_storefront_exact_field_added',
  'native_storefront_final_replies_use_exact_owner',
  'graphql_storefront_final_replies_use_exact_owner',
]) {
  if (!contract.read_boundary[key]) {
    throw new Error(`contract must lock ${key}`);
  }
}
if (contract.read_boundary.legacy_graphql_forum_replies_replaced) {
  throw new Error('FORUM-20BF must not claim legacy forumReplies replacement');
}
if (contract.read_boundary.legacy_base_storefront_reply_fetch_removed) {
  throw new Error('FORUM-20BF must not claim duplicate fetch removal');
}
if (contract.downstream_task !== 'FORUM-20BG') {
  throw new Error('unexpected downstream task');
}

console.log('forum reply audience read composition verified');
