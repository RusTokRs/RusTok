#!/usr/bin/env node
import { existsSync, readdirSync, readFileSync } from 'node:fs';

const checks = [];
const read = (path) => readFileSync(path, 'utf8');
const service = read('crates/modules/rustok-content/src/services/content_orchestration_service.rs');
const routeResolver = read('crates/modules/rustok-content-orchestration/src/route_resolver.rs');
const routePort = read('crates/modules/rustok-content/src/services/canonical_route_resolver.rs');
const blogRoutesMigration = read('crates/modules/rustok-blog/src/migrations/m20261008_000032_create_blog_post_routes.rs');
const forumRoutesMigration = read('crates/modules/rustok-forum/src/migrations/m20261008_000037_create_forum_topic_routes.rs');
const serverWiring = [
  'apps/server/src/services/app_runtime.rs',
  'apps/server/src/services/module_event_dispatcher.rs',
  'apps/server/src/services/graphql_schema.rs',
  'apps/server/src/graphql/schema.rs',
  'apps/storefront/src/shared/context/canonical_route_native_server_adapter.rs',
].map((file) => read(file)).join('\n');
// Production code under crates/ and apps/ must not touch the retired registry.
const retiredRegistryPatterns = [
  'content_canonical_urls',
  'content_url_aliases',
  'canonical_url::',
  'url_alias::',
  'CanonicalUrlService',
  'CanonicalUrlWriter',
  'apply_canonical_url_mutations',
  'url_updates',
];
const bridgeDir = 'crates/modules/rustok-content-orchestration/src/bridge';
const productionBridge = existsSync(bridgeDir)
  ? readdirSync(bridgeDir)
      .filter((file) => file.endsWith('.rs'))
      .map((file) => read(`crates/modules/rustok-content-orchestration/src/bridge/${file}`))
      .join('\n')
  : read('crates/modules/rustok-content-orchestration/src/lib.rs').split('#[cfg(all(\n    test,')[0];
const plan = read('crates/modules/rustok-content/docs/implementation-plan.md');
const docs = read('crates/modules/rustok-content/docs/README.md');
const runbook = read('crates/modules/rustok-content/docs/runbook.md');
const integrationTests = read('crates/modules/rustok-content/tests/integration.rs');
const registry = read('docs/modules/registry.md');
const pkg = read('package.json');

function check(name, ok, hint) {
  checks.push({ name, ok, hint });
}

function includesAll(text, markers) {
  return markers.every((marker) => text.includes(marker));
}

function between(source, startMarker, endMarker) {
  const start = source.indexOf(startMarker);
  if (start < 0) return '';
  const end = source.indexOf(endMarker, start + startMarker.length);
  return source.slice(start, end < 0 ? source.length : end);
}

const operations = [
  {
    name: 'promote_topic_to_post',
    event: 'DomainEvent::TopicPromotedToPost',
    scopes: ['Resource::ForumTopics, Action::Moderate', 'Resource::BlogPosts, Action::Create'],
  },
  {
    name: 'demote_post_to_topic',
    event: 'DomainEvent::PostDemotedToTopic',
    scopes: ['Resource::BlogPosts, Action::Moderate', 'Resource::ForumTopics, Action::Create'],
  },
  {
    name: 'split_topic',
    event: 'DomainEvent::TopicSplit',
    scopes: ['Resource::ForumTopics, Action::Moderate'],
  },
  {
    name: 'merge_topics',
    event: 'DomainEvent::TopicsMerged',
    scopes: ['Resource::ForumTopics, Action::Moderate'],
  },
];

for (const op of operations) {
  const start = service.indexOf(`pub async fn ${op.name}`);
  const end = operations
    .map((candidate) => service.indexOf(`pub async fn ${candidate.name}`, start + 1))
    .filter((idx) => idx > start)
    .sort((a, b) => a - b)[0] ?? service.indexOf('    fn ensure_scope', start);
  const body = start >= 0 && end > start ? service.slice(start, end) : '';

  check(`${op.name}: public service method exists`, start >= 0, `missing pub async fn ${op.name}`);
  check(
    `${op.name}: bridge trait method exists`,
    service.includes(`async fn ${op.name}(`),
    `ContentOrchestrationBridge must expose ${op.name}`,
  );
  check(
    `${op.name}: idempotency is checked before bridge execution`,
    body.includes('ensure_idempotency_key') && body.indexOf('fetch_idempotent_result') < body.indexOf(`.${op.name}(`),
    `${op.name} must check/replay idempotency before invoking bridge`,
  );
  check(
    `${op.name}: required RBAC scopes are enforced`,
    op.scopes.every((scope) => body.includes(scope)),
    `${op.name} is missing one or more RBAC scope checks`,
  );
  check(
    `${op.name}: route writes go through owner services, not a URL registry`,
    !body.includes('url_updates') && !body.includes('apply_canonical_url_mutations'),
    `${op.name} must not reference the retired URL registry`,
  );
  check(
    `${op.name}: outbox event is emitted`,
    body.includes('publish_in_tx') && body.includes(op.event),
    `${op.name} must emit ${op.event}`,
  );
  check(
    `${op.name}: audit/idempotency record is persisted`,
    body.includes('persist_orchestration_record') && body.includes(`operation: "${op.name}"`),
    `${op.name} must persist orchestration_operation + audit_log`,
  );
}

check(
  'service contract carries no URL registry payload',
  !service.includes('url_updates') && !service.includes('CanonicalUrlWriter') && !service.includes('CanonicalUrlMutation'),
  'ContentOrchestrationService must not carry URL registry mutations; routes are owned by Blog and Forum',
);
check(
  'conversion bridge writes routes through the owner services inside the transaction',
  includesAll(productionBridge, [
    'BlogPostRouteOwner',
    'ForumTopicRouteOwner',
    'remove_post_routes_in_tx(',
    'remove_redirects_to_target_in_tx(',
    'purge_topic_canonical_in_tx(',
    'record_redirect_in_tx(',
    'redirect_source_route_in_tx(',
    'release_slug_route_in_tx(',
  ]),
  'conversion bridges must move routes through BlogPostRouteOwner / ForumTopicRouteOwner',
);
check(
  'production code does not reference the retired URL registry',
  retiredRegistryHits().length === 0,
  `retired registry references remain: ${retiredRegistryHits().join(', ')}`,
);
check(
  'owner route tables are created by the amended blog and forum migrations',
  // Columns are declared through sea-query `Iden` variants, so match variant names.
  blogRoutesMigration.includes('"blog_post_routes"') &&
    blogRoutesMigration.includes('SourceRoute') &&
    forumRoutesMigration.includes('"forum_topic_routes"') &&
    forumRoutesMigration.includes('Locale') &&
    forumRoutesMigration.includes('SourceRoute'),
  'blog_post_routes and forum_topic_routes must be created by their owner migrations',
);
check(
  'route resolver reads owner redirects before deriving canonical routes',
  includesAll(routeResolver, [
    'impl CanonicalRouteResolver for OwnerCanonicalRouteResolver',
    'BlogPostRouteOwner::find_redirect',
    'ForumTopicRouteOwner::find_redirects',
    'redirect_required: true',
    'canonical_for_target(',
  ]) && includesAll(routePort, ['trait CanonicalRouteResolver', 'resolve_route']),
  'OwnerCanonicalRouteResolver must implement the rustok-content port over owner redirect tables',
);
check(
  'route resolver uses shared locale normalization and fallback',
  includesAll(routeResolver, ['normalize_locale_code', 'resolve_by_locale']),
  'route resolver must use rustok-content locale helpers, not local copies',
);
check(
  'route resolver is wired into server, GraphQL and storefront consumers',
  includesAll(serverWiring, [
    'OwnerCanonicalRouteResolver',
    'SharedCanonicalRouteResolver',
  ]),
  'the canonical route port must be registered in server runtime extensions and consumed by GraphQL and storefront',
);
check(
  'integration tests do not depend on the retired URL registry',
  !integrationTests.includes('content_canonical_urls') &&
    !integrationTests.includes('content_url_aliases') &&
    !integrationTests.includes('url_alias::') &&
    !integrationTests.includes('canonical_url::'),
  'rustok-content integration tests must not seed or assert the retired registry',
);
check(
  'server conversion bridge reads taxonomy through the transaction owner boundary',
  includesAll(productionBridge, [
    'TaxonomyOwnerReader',
    'load_terms_by_ids_in_tx(',
    '.filter(blog_post_tag::Column::TenantId.eq(tenant_id))',
    '.filter(forum_topic_tag::Column::TenantId.eq(tenant_id))',
  ]) &&
    !productionBridge.includes('taxonomy_term::Entity') &&
    !productionBridge.includes('taxonomy_term_translation::Entity'),
  'production conversion code must preserve the caller transaction and must not import/read Taxonomy persistence entities directly',
);

const blogTagSync = between(
  productionBridge,
  'async fn sync_blog_tags_for_post_in_tx(',
  'async fn sync_forum_tags_for_topic_in_tx(',
);
const forumTagSync = between(
  productionBridge,
  'async fn sync_forum_tags_for_topic_in_tx(',
  'fn unique_source_ids(',
);
check(
  'blog conversion tag sync is tenant-safe on delete and insert',
  includesAll(blogTagSync, [
    'blog_post_tag::Entity::delete_many()',
    '.filter(blog_post_tag::Column::TenantId.eq(tenant_id))',
    '.filter(blog_post_tag::Column::PostId.eq(post_id))',
    'tenant_id: Set(tenant_id)',
  ]),
  'Blog conversion tag sync must constrain relation deletion to the tenant and persist tenant_id on new relations',
);
check(
  'forum conversion tag sync is tenant-safe on delete and insert',
  includesAll(forumTagSync, [
    'forum_topic_tag::Entity::delete_many()',
    '.filter(forum_topic_tag::Column::TenantId.eq(tenant_id))',
    '.filter(forum_topic_tag::Column::TopicId.eq(topic_id))',
    'tenant_id: Set(tenant_id)',
  ]),
  'Forum conversion tag sync must constrain relation deletion to the tenant and persist tenant_id on new relations',
);
check(
  'local docs mention the compile-free content verifier',
  plan.includes('npm run verify:content:orchestration') && docs.includes('npm run verify:content:orchestration') && runbook.includes('npm run verify:content:orchestration'),
  'implementation plan, docs README and runbook must mention npm run verify:content:orchestration',
);
check(
  'central registry points content to orchestration guardrail',
  registry.includes('npm run verify:content:orchestration'),
  'docs/modules/registry.md content row must mention the guardrail',
);
check(
  'package.json exposes the content verifier',
  pkg.includes('"verify:content:orchestration": "node scripts/verify/verify-content-orchestration-contract.mjs"'),
  'package.json must define verify:content:orchestration',
);

function retiredRegistryHits() {
  const roots = ['crates', 'apps'];
  const hits = [];
  for (const root of roots) {
    if (!existsSync(root)) continue;
    for (const entry of readdirSync(root, { recursive: true })) {
      const file = String(entry);
      if (!file.endsWith('.rs') || !file.includes('/src/')) continue;
      if (file.includes('node_modules') || file.includes('/target/')) continue;
      const text = read(`${root}/${file}`);
      for (const pattern of retiredRegistryPatterns) {
        if (text.includes(pattern)) hits.push(`${root}/${file}: ${pattern}`);
      }
    }
  }
  return hits;
}

const failed = checks.filter((item) => !item.ok);
if (failed.length > 0) {
  console.error('content orchestration contract verification failed:');
  for (const item of failed) {
    console.error(`- ${item.name}: ${item.hint}`);
  }
  process.exit(1);
}

console.log(`content orchestration contract verification passed (${checks.length} checks)`);
