import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
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

const pagesStorefront = assertAll('crates/rustok-pages/storefront/src/api.rs', [
  'ChannelService::new',
  '.is_module_enabled(channel_id, MODULE_SLUG)',
  'normalize_channel_slug',
  'is_visible_for_public_channel',
  'request_context.channel_slug',
]);
assertContains(pagesStorefront, "Module '{MODULE_SLUG}' is not enabled for channel", 'pages storefront must return channel-binding denial context');

assertAll('crates/rustok-pages/src/graphql/query.rs', [
  'ChannelService::new',
  '.is_module_enabled(channel_id, MODULE_SLUG)',
  'public_channel_slug(ctx)',
  'is_page_visible_for_channel',
  'public_request_rejects_disabled_pages_channel_binding',
]);
assertAll('crates/rustok-pages/src/services/page.rs', [
  'apply_public_page_channel_filter',
  'matching_page_channel_visibility_subquery',
  'normalize_public_channel_slug',
  'is_page_visible_for_channel',
]);
assertAll('crates/rustok-pages/README.md', [
  'channel_module_bindings',
  'channelSlugs',
  'rustok-channel',
]);

const blogStorefront = assertAll('crates/rustok-blog/storefront/src/api.rs', [
  'ChannelService::new',
  '.is_module_enabled(channel_id, MODULE_SLUG)',
  'normalize_channel_slug',
  'is_visible_for_public_channel',
  'request_context.channel_slug',
]);
assertContains(blogStorefront, "Module '{MODULE_SLUG}' is not enabled for channel", 'blog storefront must return channel-binding denial context');
assertAll('crates/rustok-blog/src/graphql/query.rs', [
  'ChannelService::new',
  '.is_module_enabled(channel_id, MODULE_SLUG)',
  'public_channel_slug(ctx)',
  'is_post_visible_for_channel',
  'public_request_rejects_disabled_blog_channel_binding',
]);
assertAll('crates/rustok-blog/src/seo_targets.rs', [
  'channel_visible',
  'normalize_channel_slug',
  'request.channel_slug',
]);
assertAll('crates/rustok-blog/README.md', [
  'channel_module_bindings',
  'channelSlugs',
  'rustok-channel',
]);
assertAll('crates/rustok-blog/CRATE_API.md', [
  'channel_slugs',
  'channelSlugs',
]);

assertAll('crates/rustok-commerce/src/controllers/store/mod.rs', [
  'is_module_enabled_for_request_channel',
  "Module '{MODULE_SLUG}' is not enabled for channel",
  'request_context',
]);
assertAll('crates/rustok-commerce/src/graphql/mod.rs', [
  'is_module_enabled_for_request_channel',
  "Module '{MODULE_SLUG}' is not enabled for channel",
]);
assertAll('crates/rustok-commerce/storefront/src/api.rs', [
  'normalize_public_channel_slug',
  'request_context.channel_slug',
  '.channel_id',
  'channel_resolution_source',
  'resolved_price.channel_id',
]);
assertAll('crates/rustok-commerce/storefront/src/core/presentation.rs', [
  'channel_resolution_source',
  'channel_slug',
]);
assertAll('crates/rustok-commerce/tests/support.rs', [
  'rustok_channel::entities',
  'channel_module_binding::Entity',
]);
assertAll('crates/rustok-commerce/tests/pricing_service_test/resolve.rs', [
  'test_resolve_variant_price_matches_channel_slug_without_channel_id',
  'test_resolve_variant_price_prefers_channel_scoped_base_price',
  'test_resolve_variant_price_does_not_leak_channel_scoped_price',
]);
assertAll('crates/rustok-commerce/docs/README.md', [
  'ChannelContext',
  'channel_module_bindings',
  'channel_slug',
]);
assertAll('crates/rustok-commerce/README.md', [
  'ChannelContext',
  'rustok-channel',
  'without introducing a second sales-channel domain',
]);

assertAll('crates/rustok-forum/src/graphql/query.rs', [
  'ChannelService::new',
  '.is_module_enabled(channel_id, MODULE_SLUG)',
  'public_channel_slug(ctx)',
  'is_topic_visible_for_channel',
  'storefront_replies_return_empty_for_channel_ineligible_topic',
]);
assertAll('crates/rustok-forum/src/services/topic.rs', [
  'apply_public_topic_channel_filter',
  'matching_topic_channel_access_subquery',
  'normalize_public_channel_slug',
  'forum_topic_channel_access::Entity',
]);
assertAll('crates/rustok-forum/src/seo_targets.rs', [
  'channel_visible',
  'normalize_channel_slug',
  'request.channel_slug',
]);
assertAll('crates/rustok-forum/README.md', [
  'rustok-channel',
  'channel-restricted topics',
  'channel access',
]);
assertAll('crates/rustok-forum/docs/README.md', [
  'rustok-channel',
  'visibility/pilot gating',
]);

for (const path of [
  'crates/rustok-channel/docs/implementation-plan.md',
  'crates/rustok-channel/docs/README.md',
  'crates/rustok-channel/README.md',
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
