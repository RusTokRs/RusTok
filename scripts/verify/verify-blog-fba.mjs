import fs from 'node:fs';
import { collectBlogFbaVerificationChainFailures } from './blog-fba-verification-chain.mjs';

function read(path) { return fs.readFileSync(path, 'utf8'); }
function json(path) { return JSON.parse(read(path)); }
function fail(message) { console.error(`[verify-blog-fba] ${message}`); process.exit(1); }
function hasAll(text, snippets, label) { for (const s of snippets) if (!text.includes(s)) fail(`${label} missing ${s}`); }
function hasNone(text, snippets, label) { for (const s of snippets) if (text.includes(s)) fail(`${label} contains forbidden ${s}`); }
function sameSet(actual, expected, label) {
  const a = [...actual].sort().join('|');
  const e = [...expected].sort().join('|');
  if (a !== e) fail(`${label} drift: expected ${e}, got ${a}`);
}
const registryPath = 'crates/rustok-blog/contracts/blog-fba-registry.json';
const evidencePath = 'crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json';
const runtimeSmokePath = 'crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json';
const consumerRuntimeOrderSmokePath = 'crates/rustok-blog/contracts/evidence/blog-comments-consumer-runtime-order-smoke.json';
const commentsEventProjectionPath = 'crates/rustok-blog/contracts/evidence/blog-comments-event-projection.json';
const richtextInventoryPath = 'crates/rustok-blog/contracts/evidence/blog-richtext-cutover-inventory.json';
const richtextInventoryDocPath = 'crates/rustok-blog/docs/richtext-cutover-inventory.md';
const categorySearchReindexPath = 'crates/rustok-blog/contracts/evidence/blog-category-search-reindex-contract.json';
const graphqlRateLimitPath = 'crates/rustok-blog/contracts/evidence/blog-graphql-rate-limit-runtime-harness.json';
const aiRichtextBoundaryPath = 'crates/rustok-blog/contracts/evidence/blog-ai-richtext-boundary.json';
const providerPath = 'crates/rustok-comments/contracts/comments-fba-registry.json';
const packageJsonPath = 'package.json';
const registry = json(registryPath);
const evidence = json(evidencePath);
const runtimeSmoke = json(runtimeSmokePath);
const consumerRuntimeOrderSmoke = json(consumerRuntimeOrderSmokePath);
const commentsEventProjection = json(commentsEventProjectionPath);
const richtextInventory = json(richtextInventoryPath);
const categorySearchReindex = json(categorySearchReindexPath);
const graphqlRateLimit = json(graphqlRateLimitPath);
const aiRichtextBoundary = json(aiRichtextBoundaryPath);
const provider = json(providerPath);
const packageJson = json(packageJsonPath);

if (registry.schema_version !== 10) fail('registry schema_version drift');
if (registry.module !== 'blog' || registry.role !== 'consumer' || !['in_progress', 'boundary_ready'].includes(registry.status)) fail('registry identity/status drift');
for (const failure of collectBlogFbaVerificationChainFailures({
  registry,
  packageJson,
  existsSync: fs.existsSync,
})) {
  fail(failure);
}
if (registry.consumer_profile !== 'blog_post_comments') fail('consumer profile drift');
if (registry.evidence.comments_event_projection !== commentsEventProjectionPath) fail('comments event projection registry path drift');
if (commentsEventProjection.module !== 'blog' || commentsEventProjection.surface !== 'comments_event_projection' || commentsEventProjection.owner !== 'rustok-blog' || commentsEventProjection.provider !== 'rustok-comments') fail('comments event projection identity drift');
if (commentsEventProjection.status !== 'source_verified_no_compile' || commentsEventProjection.compile_policy !== 'not_run_by_request') fail('comments event projection status drift');
if (registry.evidence.richtext_cutover_inventory !== richtextInventoryPath) fail('richtext cutover inventory registry path drift');
if (registry.evidence.richtext_cutover_inventory_doc !== richtextInventoryDocPath) fail('richtext cutover inventory doc registry path drift');
if (registry.evidence.category_search_reindex !== categorySearchReindexPath) fail('category Search reindex registry path drift');
if (categorySearchReindex.module !== 'blog' || categorySearchReindex.surface !== 'category_search_reindex') fail('category Search reindex identity drift');
if (categorySearchReindex.status !== 'source_verified_no_compile' || categorySearchReindex.compile_policy !== 'not_run_by_request') fail('category Search reindex status drift');
if (registry.evidence.graphql_rate_limit !== graphqlRateLimitPath) fail('GraphQL rate-limit registry path drift');
if (graphqlRateLimit.module !== 'blog' || graphqlRateLimit.surface !== 'graphql_rate_limit') fail('GraphQL rate-limit identity drift');
if (graphqlRateLimit.status !== 'executable_no_compile' || graphqlRateLimit.compile_policy !== 'not_run_by_request') fail('GraphQL rate-limit status drift');
if (registry.evidence.ai_richtext_boundary !== aiRichtextBoundaryPath) fail('AI richtext boundary registry path drift');
if (aiRichtextBoundary.module !== 'blog' || aiRichtextBoundary.surface !== 'ai_blog_draft_richtext_boundary') fail('AI richtext boundary identity drift');
if (aiRichtextBoundary.status !== 'source_verified_no_compile' || aiRichtextBoundary.compile_policy !== 'not_run_by_request') fail('AI richtext boundary status drift');
const dependency = registry.provider_dependencies?.[0];
if (!dependency) fail('missing comments provider dependency');
if (dependency.module !== 'comments' || dependency.registry !== providerPath) fail('provider dependency identity drift');
if (dependency.contract_version !== provider.contract_version || dependency.port !== 'CommentsThreadPort') fail('provider contract/port drift');
if (provider.module !== 'comments' || provider.role !== 'provider' || !['in_progress', 'boundary_ready'].includes(provider.status)) fail('comments provider status drift');
sameSet(dependency.operations, provider.ports?.[0]?.operations ?? [], 'consumer/provider operations');
sameSet(dependency.fallback_profiles, provider.consumers?.find(c => c.module === 'blog')?.fallback_profiles ?? [], 'consumer/provider fallback profiles');
sameSet(dependency.degraded_modes, provider.consumers?.find(c => c.module === 'blog')?.degraded_modes ?? [], 'consumer/provider degraded modes');
if (dependency.context !== 'rustok_api::ports::PortContext' || dependency.error !== 'rustok_api::ports::PortError') fail('consumer context/error drift');

if (richtextInventory.schema_version !== 3) fail('richtext cutover inventory schema_version drift');
if (richtextInventory.module !== 'blog' || richtextInventory.surface !== 'article_richtext_cutover') fail('richtext cutover inventory identity drift');
if (richtextInventory.status !== 'implemented_source_verified_no_compile') fail('richtext cutover inventory status drift');
if (richtextInventory.compile_policy !== 'not_run_by_request' || richtextInventory.atomicity !== 'required') fail('richtext cutover inventory execution/atomicity drift');
if (richtextInventory.owner_contract?.write !== 'rustok_api::RichTextDocument'
  || richtextInventory.owner_contract?.read !== 'rustok_api::RichTextView'
  || richtextInventory.owner_contract?.plain_text !== 'server_derived'
  || richtextInventory.owner_contract?.profile !== 'article') {
  fail('richtext cutover owner contract drift');
}
const allowedRichtextStatuses = new Set([
  'implemented_source_verified_no_compile',
  'executable_no_run',
]);
for (const check of richtextInventory.checks ?? []) {
  if (!check.name || !check.path || !allowedRichtextStatuses.has(check.status)) {
    fail(`invalid richtext inventory check ${JSON.stringify(check)}`);
  }
  const source = read(check.path);
  hasAll(source, check.required_markers ?? [], `richtext inventory ${check.name}`);
  hasNone(source, check.forbidden_markers ?? [], `richtext inventory ${check.name}`);
}
sameSet(
  richtextInventory.blocking_surfaces ?? [],
  [],
  'richtext cutover blockers',
);
const richtextCheckNames = new Set((richtextInventory.checks ?? []).map((check) => check.name));
for (const requiredCheck of [
  'owner_article_projection',
  'storage_schema',
  'storage_migration',
  'offline_backfill',
  'graphql_transport',
  'next_admin',
  'leptos_storefront_model',
  'leptos_storefront_graphql',
  'leptos_storefront_native',
  'leptos_storefront_rendering',
  'leptos_storefront_legacy_removal',
  'search_projection',
  'seo_projection',
  'ai_blog_draft_writer',
  'content_orchestration_guard',
]) {
  if (!richtextCheckNames.has(requiredCheck)) fail(`richtext cutover inventory missing check ${requiredCheck}`);
}
hasAll(
  richtextInventory.completion_conditions ?? [],
  [
    'storage_schema_uses_canonical_document_and_server_plain_text',
    'legacy_rows_have_owner_specific_dry_run_backfill',
    'search_indexes_server_derived_plain_text',
    'seo_uses_server_derived_plain_text',
    'ai_blog_drafts_write_richtext_document',
    'no_markdown_format_alias_or_raw_json_write_path_remains',
  ],
  'richtext cutover completion conditions',
);
const richtextInventoryDoc = read(richtextInventoryDocPath);
hasAll(
  richtextInventoryDoc,
  [
    richtextInventoryPath,
    'Storage schema',
    'crates/rustok-blog/src/bin/blog_article_richtext_backfill.rs',
    'crates/rustok-blog/contracts/evidence/blog-richtext-offline-backfill.json',
    'scripts/verify/verify-blog-richtext-offline-backfill.mjs',
    'Search projection',
    'SEO projection',
    'AI Blog draft writer',
    'AI Blog owner shim',
    aiRichtextBoundaryPath,
    'scripts/verify/verify-blog-ai-richtext-boundary.mjs',
    'scripts/verify/verify-blog-fba.mjs',
  ],
  'richtext cutover inventory documentation',
);

const manifest = read('crates/rustok-blog/rustok-module.toml');
hasAll(manifest, ['[fba.consumer]', 'registry = "contracts/blog-fba-registry.json"', 'profile = "blog_post_comments"', 'comments.thread.v1'], 'manifest');

if (evidence.schema_version !== 2 || evidence.surface !== 'comments_port_boundary') fail('comments port matrix schema/identity drift');
if (evidence.generated_from !== registryPath || evidence.status !== registry.contract_tests.status) fail('evidence header drift');
if (evidence.compile_policy !== 'not_run_by_request') fail('comments port matrix compile policy drift');
sameSet(evidence.cases.map(c => c.operation), registry.contract_tests.cases.map(c => c.operation), 'evidence/registry cases');
sameSet(evidence.profiles?.source_verified ?? [], registry.contract_tests.source_profiles ?? [], 'source-verified profiles');
sameSet(evidence.profiles?.pending ?? [], registry.contract_tests.pending_profiles ?? [], 'pending profiles');
sameSet(evidence.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles, 'fallback profiles');
sameSet(evidence.fallback_smoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes, 'degraded modes');
if (registry.contract_tests.runtime_status !== 'pending') fail('contract test runtime status drift');

if (registry.evidence.runtime_fallback_smoke !== runtimeSmokePath) fail('runtime smoke evidence path drift');
if (registry.evidence.consumer_runtime_order_smoke !== consumerRuntimeOrderSmokePath) fail('consumer runtime-order smoke evidence path drift');
if (registry.evidence.consumer_runtime_order_smoke_runner !== consumerRuntimeOrderSmoke.runner) fail('consumer runtime-order smoke runner drift');
if (registry.contract_tests.fallback_smoke.status !== 'planned') fail('fallback smoke status drift');
if (runtimeSmoke.schema_version !== 2 || runtimeSmoke.generated_from !== registryPath || runtimeSmoke.status !== 'source_verified_no_compile') {
  fail('runtime smoke header/status drift');
}
if (runtimeSmoke.runner !== 'scripts/verify/verify-blog-comments-port-boundary.mjs') fail('runtime smoke runner drift');
if (runtimeSmoke.compile_policy !== 'not_run_by_request' || runtimeSmoke.runtime_status !== 'not_run') fail('runtime smoke execution policy drift');
if (runtimeSmoke.fallback_smoke?.status !== 'planned' || runtimeSmoke.fallback_smoke?.runtime_evidence !== 'pending') fail('runtime smoke degraded-mode status drift');
sameSet(runtimeSmoke.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles, 'runtime smoke profiles');
sameSet(runtimeSmoke.fallback_smoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes, 'runtime smoke degraded modes');
const service = read(runtimeSmoke.source_contract.consumer_service);
const errorMapping = read(runtimeSmoke.source_contract.consumer_error_mapping);
const providerRegistryPath = runtimeSmoke.source_contract.provider_port_registry;
if (providerRegistryPath !== providerPath) fail('runtime smoke provider registry drift');
if (runtimeSmoke.source_contract.consumer_service !== runtimeSmoke.source_contract.consumer_error_mapping) fail('active comments error mapper path drift');
if (consumerRuntimeOrderSmoke.schema_version !== 2 || consumerRuntimeOrderSmoke.generated_from !== registryPath || consumerRuntimeOrderSmoke.status !== 'executable_no_compile') {
  fail('consumer runtime-order smoke header/status drift');
}
if (consumerRuntimeOrderSmoke.compile_policy !== 'not_run_by_request') fail('consumer runtime-order compile policy drift');
if (consumerRuntimeOrderSmoke.provider !== 'comments' || consumerRuntimeOrderSmoke.role !== 'consumer') fail('consumer runtime-order smoke identity drift');
if (consumerRuntimeOrderSmoke.source_contract.consumer_service !== runtimeSmoke.source_contract.consumer_service) fail('consumer runtime-order service source drift');
if (consumerRuntimeOrderSmoke.source_contract.consumer_error_mapping !== runtimeSmoke.source_contract.consumer_error_mapping) fail('consumer runtime-order error source drift');
if (consumerRuntimeOrderSmoke.source_contract.provider_registry !== providerPath) fail('consumer runtime-order provider registry drift');
if (consumerRuntimeOrderSmoke.fallback_smoke?.status !== 'planned') fail('consumer runtime-order fallback status drift');
sameSet(consumerRuntimeOrderSmoke.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles, 'consumer runtime-order smoke profiles');
sameSet(consumerRuntimeOrderSmoke.fallback_smoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes, 'consumer runtime-order smoke degraded modes');
for (const entry of consumerRuntimeOrderSmoke.runtime_order ?? []) {
  if (!registry.contract_tests.cases.some(c => c.operation === entry.operation)) {
    fail(`consumer runtime-order operation ${entry.operation} is not declared in registry cases`);
  }
}
for (const smokeCase of runtimeSmoke.fallback_smoke.cases ?? []) {
  if (!registry.contract_tests.cases.some(c => c.operation === smokeCase.operation)) {
    fail(`runtime smoke operation ${smokeCase.operation} is not declared in registry cases`);
  }
  if (!registry.contract_tests.fallback_smoke.degraded_modes.includes(smokeCase.degraded_mode)) {
    fail(`runtime smoke degraded mode drift for ${smokeCase.operation}`);
  }
  hasAll(service, smokeCase.source_markers ?? [], `runtime service smoke ${smokeCase.operation}`);
  hasAll(errorMapping, smokeCase.typed_error_markers ?? [], `runtime error smoke ${smokeCase.operation}`);
}
hasAll(service, ['in_process_comments_thread_port', 'CommentsThreadPort', 'comments_read_port_context', 'comments_write_port_context', 'comments_port_error_to_blog_error'], 'comments port consumer boundary');
if (/\.comments\s*\.get_comment\s*\(/.test(service)) {
  fail('blog comment reads must not bypass CommentsThreadPort through CommentsService');
}
if (/\.comments\s*\.list_comments_for_target\s*\(/.test(service)) {
  fail('blog comment lists must not bypass CommentsThreadPort through CommentsService');
}
if (/\.comments\s*\.update_comment\s*\(/.test(service)) {
  fail('blog comment update must not bypass CommentsThreadPort through CommentsService');
}
const directCommentsCalls = [...service.matchAll(/\.comments\s*\.\s*([a-z_]+)\s*\(/g)]
  .map((match) => match[1])
  .sort();
if (directCommentsCalls.length !== 0) {
  fail(`blog must not bypass CommentsThreadPort through CommentsService, got ${directCommentsCalls.join('|')}`);
}
hasAll(service, ['comments_thread_port', '.create_comment(', '.delete_comment('], 'comments port lifecycle migration');
const projection = registry.event_projection;
if (!projection || projection.provider !== 'comments' || projection.handler !== 'BlogCommentProjectionHandler' || projection.delivery_ledger !== 'blog_comment_projection_deliveries' || projection.status !== 'implemented_static_only') fail('event projection registry drift');
sameSet(projection.events, ['comment.created', 'comment.deleted'], 'event projection event types');
const projectionSource = read('crates/rustok-blog/src/services/comment_projection.rs');
hasAll(projectionSource, ['impl EventHandler for BlogCommentProjectionHandler', 'DomainEvent::CommentCreated', 'DomainEvent::CommentDeleted', 'blog_comment_projection_delivery::Entity::find_by_id', 'DomainEvent::BlogPostUpdated', '.publish_in_tx('], 'blog comment projection');
const migration = read('crates/rustok-blog/src/migrations/m20260716_000001_create_blog_comment_projection_deliveries.rs');
hasAll(migration, ['BlogCommentProjectionDeliveries', 'EventId', 'TenantId', 'PostId'], 'blog comment projection migration');
const moduleSource = read('crates/rustok-blog/src/lib.rs');
hasAll(moduleSource, ['fn register_event_listeners(', 'BlogCommentProjectionHandler::new(ctx.db.clone())'], 'blog event-listener registration');

const plan = read('crates/rustok-blog/docs/implementation-plan.md');
hasAll(plan, ['- FBA status: `boundary_ready`', 'blog-fba-registry.json', commentsEventProjectionPath, categorySearchReindexPath, graphqlRateLimitPath, aiRichtextBoundaryPath, 'CommentsThreadPort', 'blog-comments-consumer-static-matrix.json', 'blog-comments-runtime-fallback-smoke.json', consumerRuntimeOrderSmokePath, 'verify:blog:comments-port-boundary', 'test:verify:blog:comments-port-boundary', 'verify:blog:comments-event-projection', 'test:verify:blog:comments-event-projection', 'degraded UI modes remain planned'], 'local plan');
const central = read('docs/modules/registry.md');
hasAll(central, ['| `blog` |', 'crates/rustok-blog/contracts/blog-fba-registry.json', 'blog-comments-runtime-fallback-smoke.json', consumerRuntimeOrderSmokePath, '`in_progress` | `boundary_ready`'], 'central registry');
const unified = read('docs/research/fluid-backend-architecture-unified-plan.md');
hasAll(unified, ['`blog`', 'CommentsThreadPort', 'blog-fba-registry.json'], 'unified plan');

console.log('[verify-blog-fba] Blog FBA registry, exact admin/storefront/comments-port/comments-projection/category/rate-limit/GraphQL/AI richtext source-gate chain, comments consumer metadata, and no-compile evidence are consistent');
