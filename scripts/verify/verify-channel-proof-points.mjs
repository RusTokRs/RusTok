import { existsSync, readdirSync, readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
// Dir-aware read: `foo.rs` may have been refactored into a `foo/` module dir.
const read = (path) => {
  const fileUrl = new URL(path, root);
  if (existsSync(fileUrl)) return readFileSync(fileUrl, 'utf8');
  const dirPath = path.endsWith('.rs') ? path.slice(0, -3) : path;
  const dirUrl = new URL(dirPath, root);
  if (existsSync(dirUrl)) {
    return readdirSync(dirUrl, { recursive: true })
      .filter((file) => String(file).endsWith('.rs'))
      .sort()
      .map((file) => readFileSync(new URL(`${dirPath}/${file}`, root), 'utf8'))
      .join('\n');
  }
  return readFileSync(fileUrl, 'utf8');
};
const fail = (message) => {
  console.error(`[verify-channel-proof-points] ${message}`);
  process.exit(1);
};
const assertContains = (source, marker, message) => {
  if (!source.includes(marker)) fail(message);
};
const assertAll = (path, markers) => {
  const source = read(path);
  for (const marker of markers) assertContains(source, marker, `${path} missing marker: ${marker}`);
  return source;
};

const pagesStorefront = assertAll('crates/modules/rustok-pages/storefront/src/transport/native_server_adapter.rs', [
  'ChannelService::new',
  '.is_module_enabled_for_tenant(tenant_id, channel_id, MODULE_SLUG)',
  'normalize_channel_slug',
  'is_visible_for_public_channel',
  'ctx.channel_slug.as_deref()',
]);
assertContains(pagesStorefront, "Module '{MODULE_SLUG}' is not enabled for channel", 'pages storefront must return channel-binding denial context');

assertAll('crates/modules/rustok-pages/src/graphql/query.rs', [
  'ChannelService::new',
  '.is_module_enabled_for_tenant(request_context.tenant_id, channel_id, MODULE_SLUG)',
  'public_channel_slug(ctx)',
  'is_page_visible_for_channel',
  'public_request_rejects_disabled_pages_channel_binding',
]);
assertAll('crates/modules/rustok-pages/src/services/page.rs', [
  'apply_public_page_channel_filter',
  'matching_page_channel_visibility_subquery',
  'normalize_public_channel_slug',
  'is_page_visible_for_channel',
]);
assertAll('crates/modules/rustok-pages/README.md', [
  'channel module gating',
  'page_channel_visibility',
  'rustok-channel',
]);

const blogStorefront = assertAll('crates/modules/rustok-blog/storefront/src/transport/native_server_adapter.rs', [
  'BlogChannelAvailability::from_runtime',
  '.for_request(tenant_id, request_context)',
  'normalize_channel_slug',
  'read_published_post',
  'ctx.channel_slug.as_deref()',
]);
assertContains(blogStorefront, "Blog is not available for the current channel", 'blog storefront must return a channel-binding denial');
assertAll('crates/modules/rustok-blog/src/services/post/queries/single.rs', [
  'get_post_by_slug_on_channel',
  'is_post_visible_for_channel(&channel_slugs, channel_slug)',
  '.channel_is_public_visible(tenant_id, channel_slug)',
]);
const blogGraphql = assertAll('crates/modules/rustok-blog/src/graphql/query.rs', [
  'BlogGraphqlRuntimeData::attached(ctx).channel_availability()',
  '.for_request(tenant_id, request_context)',
  'public_channel_slug(ctx)',
  'is_post_visible_for_channel',
]);
if (blogGraphql.includes('ChannelService::new') || blogStorefront.includes('ChannelService::new')) {
  fail('Blog GraphQL and storefront must use the host-composed Channel availability port');
}
assertAll('crates/modules/rustok-blog/src/integrations/channel_availability.rs', [
  'ChannelPublicModuleSelector::Id(id)',
  'ChannelPublicModuleSelector::Slug(slug)',
  'CHANNEL_PUBLIC_MODULE_READ_CLAIM',
  '.is_public_module_available(',
]);
// The channel-binding denial proof test lives in the query module's tests file
// (query.rs declares `mod tests;`).
assertAll('crates/modules/rustok-blog/src/graphql/query/tests.rs', [
  'public_request_rejects_disabled_blog_channel_binding',
]);
assertAll('crates/modules/rustok-blog/src/integrations/seo_targets.rs', [
  // Channel-aware SEO resolution: public loads and route matches expose only
  // published posts visible for the requesting channel.
  'SeoTargetLoadScope::PublicRoute',
  'is_post_visible_for_channel',
  'request.channel_slug',
]);
assertAll('crates/modules/rustok-blog/README.md', [
  'channel module bindings',
  'rustok-channel',
]);
assertAll('crates/modules/rustok-blog/CRATE_API.md', [
  'channel_slugs',
  'channel visibility',
]);

assertAll('crates/modules/rustok-commerce/src/controllers/store/mod.rs', [
  'is_module_enabled_for_request_channel',
  "The commerce module is not available for the current channel",
  'request_context',
]);
assertAll('crates/modules/rustok-commerce/src/graphql/mod.rs', [
  'is_module_enabled_for_request_channel',
  "Commerce is not enabled for the current channel",
]);
assertAll('crates/modules/rustok-commerce/storefront/src/transport/native_server_adapter.rs', [
  'request_context.channel_slug',
  'channel_resolution_source',
]);
assertAll('crates/modules/rustok-commerce/storefront/src/core/presentation.rs', [
  'channel_resolution_source',
  'channel_slug',
]);
assertAll('crates/modules/rustok-commerce/tests/support.rs', [
  'rustok_channel::entities',
  'channel_module_binding::Entity',
]);
assertAll('crates/modules/rustok-commerce/tests/pricing_service_test/resolve.rs', [
  'test_resolve_variant_price_matches_channel_slug_without_channel_id',
  'test_resolve_variant_price_prefers_channel_scoped_base_price',
  'test_resolve_variant_price_does_not_leak_channel_scoped_price',
]);
assertAll('crates/modules/rustok-commerce/docs/README.md', [
  'ChannelContext',
  'channel_module_bindings',
  'channel_slug',
]);
assertAll('crates/modules/rustok-commerce/README.md', [
  'ChannelContext',
  'rustok-channel',
  'without introducing a second sales-channel domain',
]);

// Channel gating is centralized in graphql/mod.rs and delegated to from the
// runtime queries.
assertAll('crates/modules/rustok-forum/src/graphql/mod.rs', [
  'async fn require_public_forum_channel_enabled(',
  'ChannelService::new',
  '.is_module_enabled_for_tenant(tenant.id, channel_id, "forum")',
]);
assertAll('crates/modules/rustok-forum/src/graphql/query_runtime.rs', [
  'require_public_forum_channel_enabled(ctx)',
  'public_channel_slug(ctx)',
  'async fn forum_storefront_replies(',
  'list_public_storefront_visible_with_locale_fallback',
  'Some(&PUBLIC_REPLY_STATUSES)',
]);
// Topic audiences use the tenant-scoped channel filter in the base topic query.
assertAll('crates/modules/rustok-forum/src/services/topic_visibility.rs', [
  'matching_topic_channel_access_subquery',
  'forum_topic_channel_access::Entity',
]);
// The list-side filter lives in `services/topic.rs` (`topic_visibility_list.rs`
// was folded into it); the pin follows the code instead of a path that no longer
// exists, otherwise the script aborted here and never checked the tail.
assertAll('crates/modules/rustok-forum/src/services/topic.rs', [
  'apply_tenant_scoped_storefront_channel_filter',
  'matching_tenant_topic_channel_access_subquery',
  'normalize_public_channel_slug',
]);
assertAll('crates/modules/rustok-forum/src/seo_targets.rs', [
  'channel_visible',
  'normalize_channel_slug',
  'request.channel_slug',
]);
assertAll('crates/modules/rustok-forum/README.md', [
  'rustok-channel',
  'channel-restricted topics',
  'channel access',
]);
assertAll('crates/modules/rustok-forum/docs/README.md', [
  'rustok-channel',
  'visibility and SEO gating',
]);

for (const path of [
  'crates/modules/rustok-channel/docs/implementation-plan.md',
  'crates/modules/rustok-channel/docs/README.md',
  'crates/modules/rustok-channel/README.md',
  'docs/modules/registry.md',
]) {
  assertAll(path, [
    'rustok-pages',
    'rustok-blog',
    'rustok-commerce',
    'rustok-forum',
    'verify:channel:proof-points',
  ]);
}

console.log('[verify-channel-proof-points] Channel-aware pages/blog/commerce/forum proof points and docs are source-locked');
