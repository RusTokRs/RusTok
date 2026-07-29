import fs from 'node:fs';
import path from 'node:path';

const root = process.cwd();
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const requireText = (source, needle, label) => {
  if (!source.includes(needle)) {
    throw new Error(`missing ${label}: ${needle}`);
  }
};

const owner = read('crates/rustok-forum/src/services/topic_audience_list.rs');
const readState = read('crates/rustok-forum/src/services/storefront_read_state.rs');
const graphql = read('crates/rustok-forum/src/graphql/storefront_read_state.rs');
const transport = read('crates/rustok-forum/src/topic_read_transport.rs');
const contract = JSON.parse(
  read('crates/rustok-forum/contracts/forum-topic-audience-pagination.json'),
);

requireText(owner, 'pub struct ForumTopicAudienceListService', 'list owner');
requireText(owner, 'FORUM_TOPIC_AUDIENCE_SCAN_PAGE_SIZE', 'bounded scan page');
requireText(owner, '.is_topic_visible(tenant_id, topic.id, channel_slug, &viewer)', 'exact audience decision');
requireText(owner, 'visible_total >= requested_start', 'post-decision pagination');
requireText(readState, 'list_topics_with_unread_audience_visible', 'exact unread owner method');
requireText(readState, 'page.items, page.total', 'shared exact page and total');
requireText(graphql, 'ForumTopicReadOperation::TopicList', 'GraphQL topic-list context');
requireText(graphql, '.list_topics_with_unread_audience_visible(', 'GraphQL exact unread composition');
requireText(transport, 'TopicList', 'topic-list operation identity');

if (contract.task !== 'FORUM-20BD') throw new Error('unexpected task');
if (!contract.pagination_boundary.items_and_total_share_allowed_sequence) {
  throw new Error('contract must lock exact items/total sequence');
}
if (!contract.pagination_boundary.graphql_unread_query_uses_exact_owner) {
  throw new Error('contract must lock GraphQL unread composition');
}
if (contract.pagination_boundary.native_storefront_composed) {
  throw new Error('FORUM-20BD must not claim native storefront composition');
}

console.log('forum topic audience pagination composition verified');
