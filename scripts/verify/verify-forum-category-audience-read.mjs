import fs from 'node:fs';
import path from 'node:path';

const root = process.cwd();
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const requireText = (source, needle, label) => {
  if (!source.includes(needle)) {
    throw new Error(`missing ${label}: ${needle}`);
  }
};

const transport = read('crates/rustok-forum/src/category_read_transport.rs');
const visibility = read(
  'crates/rustok-forum/src/services/category_audience_visibility.rs',
);
const owner = read(
  'crates/rustok-forum/src/services/category_audience_read.rs',
);
const services = read('crates/rustok-forum/src/services/mod.rs');
const lib = read('crates/rustok-forum/src/lib.rs');
const rest = read('crates/rustok-forum/src/controllers/categories.rs');
const restTree = read('crates/rustok-forum/src/controllers/category_tree.rs');
const graphql = read('crates/rustok-forum/src/graphql/query_runtime.rs');
const graphqlTree = read(
  'crates/rustok-forum/src/graphql/category_tree_query.rs',
);
const graphqlRuntime = read('crates/rustok-forum/src/graphql/runtime_data.rs');
const graphqlModule = read('crates/rustok-forum/src/graphql/mod.rs');
const graphqlAdapter = read(
  'crates/rustok-forum/storefront/src/transport/graphql_adapter.rs',
);
const nativeAdapter = read(
  'crates/rustok-forum/storefront/src/transport/native_server_adapter.rs',
);
const contract = JSON.parse(
  read('crates/rustok-forum/contracts/forum-category-audience-read.json'),
);

requireText(
  transport,
  'ForumCategoryReadOperation',
  'category read operation identity',
);
requireText(
  transport,
  'FORUM_CATEGORY_READ_FACTS_DEADLINE',
  'category facts deadline',
);
requireText(
  visibility,
  'ForumCategoryAudienceVisibilityService',
  'category audience visibility owner',
);
requireText(
  visibility,
  '.is_category_visible_to_viewer(',
  'base category visibility floor',
);
requireText(
  visibility,
  'policy.effective_layers',
  'inherited richer category layers',
);
requireText(
  owner,
  'pub struct ForumCategoryAudienceReadService',
  'category read owner',
);
requireText(
  owner,
  'list_authenticated_owner_visible_with_audience_context',
  'authenticated exact category list',
);
requireText(
  owner,
  'list_public_storefront_visible_with_locale_fallback',
  'public exact category list',
);
requireText(
  owner,
  'tree_authenticated_owner_visible_with_audience_context',
  'exact category tree',
);
requireText(
  owner,
  'visible_total',
  'single allowed pagination sequence',
);
requireText(
  owner,
  'prune_category_nodes',
  'tree pruning before output',
);
requireText(
  owner,
  'category_tree_stats',
  'tree metadata recomputation',
);
requireText(
  services,
  'ForumCategoryAudienceReadService',
  'services export',
);
requireText(
  lib,
  'category_read_audience_port_context',
  'public category transport export',
);
requireText(
  rest,
  '.list_authenticated_owner_visible_with_audience_context(',
  'REST category list exact owner',
);
requireText(
  rest,
  '.get_authenticated_owner_visible_with_audience_context(',
  'REST selected category exact owner',
);
requireText(
  restTree,
  '.tree_authenticated_owner_visible_with_audience_context(',
  'REST category tree exact owner',
);
for (const field of [
  'async fn forum_categories(',
  'async fn forum_category(',
  'async fn forum_storefront_categories(',
]) {
  requireText(graphql, field, `canonical GraphQL field ${field}`);
}
requireText(
  graphql,
  'category_read_audience_port_context(',
  'trusted GraphQL category context',
);
requireText(
  graphql,
  'exact_category_audience_owner',
  'GraphQL exact category metrics',
);
requireText(
  graphqlTree,
  '.tree_authenticated_owner_visible_with_audience_context(',
  'GraphQL category tree exact owner',
);
requireText(
  graphqlRuntime,
  'category_audience_read_service',
  'GraphQL runtime category owner factory',
);
requireText(
  graphqlModule,
  '#[path = "query_runtime.rs"]',
  'canonical GraphQL query selector',
);
requireText(
  graphqlAdapter,
  'forumStorefrontCategories',
  'canonical GraphQL storefront category request',
);
requireText(
  nativeAdapter,
  'ForumCategoryAudienceReadService',
  'native exact category owner',
);
requireText(
  nativeAdapter,
  'category_read_audience_port_context(',
  'native trusted category context',
);
requireText(
  nativeAdapter,
  'visible_category_ids',
  'native requested-category exact gate',
);

if (contract.task !== 'FORUM-20BH') throw new Error('unexpected task');
for (const key of [
  'base_public_authenticated_floor_preserved',
  'inherited_richer_category_layers_enforced',
  'list_filters_before_output_pagination',
  'list_items_and_total_share_exact_allowed_sequence',
  'tree_prunes_denied_nodes_before_output',
  'tree_total_and_max_depth_recomputed_after_pruning',
  'rest_category_list_uses_exact_owner',
  'rest_selected_category_uses_exact_owner',
  'rest_category_tree_uses_exact_owner',
  'graphql_forum_categories_uses_exact_owner',
  'graphql_forum_category_uses_exact_owner',
  'graphql_storefront_categories_uses_exact_owner',
  'graphql_category_tree_uses_exact_owner',
  'native_storefront_categories_use_exact_owner',
  'native_requested_category_must_be_in_allowed_sequence',
]) {
  if (!contract.read_boundary[key]) {
    throw new Error(`contract must lock ${key}`);
  }
}
if (!contract.compatibility.canonical_graphql_runtime_uses_query_runtime) {
  throw new Error('contract must lock the canonical GraphQL runtime selector');
}
if (!contract.compatibility.legacy_query_snapshot_is_uncompiled) {
  throw new Error('contract must identify the uncompiled legacy query snapshot');
}
if (contract.downstream_task !== 'FORUM-20BI') {
  throw new Error('unexpected downstream task');
}

console.log('forum category audience read composition verified');
