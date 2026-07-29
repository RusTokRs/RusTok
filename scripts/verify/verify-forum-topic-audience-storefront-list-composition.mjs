import fs from 'node:fs';
import path from 'node:path';

const root = process.cwd();
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const requireText = (source, needle, label) => {
  if (!source.includes(needle)) {
    throw new Error(`missing ${label}: ${needle}`);
  }
};

const graphqlQuery = read(
  'crates/rustok-forum/src/graphql/storefront_audience_topics.rs',
);
const graphqlAdapter = read(
  'crates/rustok-forum/storefront/src/transport/graphql_adapter.rs',
);
const nativeAdapter = read(
  'crates/rustok-forum/storefront/src/transport/native_server_adapter.rs',
);
const selector = read('crates/rustok-forum/storefront/src/transport/mod.rs');
const contract = JSON.parse(
  read(
    'crates/rustok-forum/contracts/forum-topic-audience-storefront-list-composition.json',
  ),
);
const replyContract = JSON.parse(
  read('crates/rustok-forum/contracts/forum-reply-audience-read.json'),
);

requireText(
  graphqlQuery,
  'ForumTopicAudienceListService::new',
  'public GraphQL exact pagination owner',
);
requireText(
  graphqlQuery,
  'list_public_storefront_visible_with_locale_fallback',
  'public GraphQL exact list call',
);
requireText(
  graphqlQuery,
  'Permission denied: tenant scope mismatch',
  'public GraphQL tenant validation',
);
requireText(
  graphqlAdapter,
  'forumStorefrontAudienceTopics',
  'GraphQL exact public fallback field',
);
requireText(
  graphqlAdapter,
  'forumStorefrontUnreadTopics',
  'GraphQL authenticated unread path',
);
requireText(
  graphqlAdapter,
  'markForumStorefrontTopicRead',
  'GraphQL exact mark-read preservation',
);
requireText(
  nativeAdapter,
  'ForumTopicReadOperation::TopicList',
  'native exact topic-list context',
);
requireText(
  nativeAdapter,
  'list_topics_with_unread_audience_visible',
  'native authenticated exact unread owner',
);
requireText(
  nativeAdapter,
  'list_public_storefront_visible_with_locale_fallback',
  'native public exact list owner',
);
requireText(
  nativeAdapter,
  'mark_topic_read_current_audience_visible',
  'native exact mark-read preservation',
);
requireText(
  selector,
  'native_server_adapter::fetch_storefront_forum_server',
  'canonical native storefront selector',
);
requireText(
  selector,
  'graphql_adapter::fetch_storefront_forum_graphql',
  'canonical GraphQL storefront selector',
);

if (contract.task !== 'FORUM-20BE') throw new Error('unexpected task');
for (const key of [
  'native_authenticated_list_uses_exact_unread_owner',
  'native_public_list_uses_exact_public_owner',
  'graphql_public_exact_field_added',
  'graphql_adapter_public_fallback_uses_exact_field',
  'selected_topic_exact_owner_preserved',
  'mark_read_exact_owner_preserved',
]) {
  if (!contract.composition_boundary[key]) {
    throw new Error(`contract must lock ${key}`);
  }
}
if (!contract.compatibility.canonical_transport_adapters_updated) {
  throw new Error('contract must lock canonical adapter updates');
}
if (contract.compatibility.parallel_transport_adapters_added) {
  throw new Error('FORUM-20BE must not retain parallel topic-list adapters');
}
if (!contract.composition_boundary.reply_owner_read_changed) {
  throw new Error('FORUM-20BE handoff must record delivered reply-owner migration');
}
if (contract.downstream_completion !== 'crates/rustok-forum/contracts/forum-reply-audience-read.json') {
  throw new Error('FORUM-20BE handoff must point to the reply-read contract');
}
if (replyContract.task !== 'FORUM-20BF') {
  throw new Error('unexpected reply-read completion task');
}
if (contract.downstream_task !== 'FORUM-20BF') {
  throw new Error('unexpected downstream task');
}

console.log('forum topic audience storefront list composition verified');
