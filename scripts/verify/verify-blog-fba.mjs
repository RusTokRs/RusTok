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
const projectionHandlerPath = 'crates/rustok-blog/src/services/comment_projection.rs';
const projectionPostgresHarnessPath = 'crates/rustok-blog/tests/comment_projection_postgres_test.rs';
const projectionRestartHarnessPath = 'crates/rustok-blog/tests/comment_projection_restart_postgres_test.rs';
const projectionHarnessCommand = 'cargo test -p rustok-blog --lib services::comment_projection::tests';
const projectionPostgresHarnessCommand = 'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_postgres_test';
const projectionRestartHarnessCommand = 'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_restart_postgres_test';
const projectionPostgresHarnessEnvironment = 'RUSTOK_BLOG_TEST_DATABASE_URL';
const blogInitialMigrationPath = 'crates/rustok-blog/src/migrations/m20260328_000001_create_blog_post_tables.rs';
const removedRichtextArtifacts = [
  'crates/rustok-blog/src/migrations/m20260730_000006_cutover_blog_article_richtext.rs',
  'crates/rustok-blog/src/bin/blog_article_richtext_backfill.rs',
  'crates/rustok-blog/contracts/evidence/blog-richtext-cutover-inventory.json',
  'crates/rustok-blog/contracts/evidence/blog-richtext-offline-backfill.json',
  'crates/rustok-blog/docs/richtext-cutover-inventory.md',
  'scripts/verify/verify-blog-richtext-offline-backfill.mjs',
  'scripts/verify/verify-blog-richtext-offline-backfill.test.mjs',
];
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
const projectionPostgresHarness = read(projectionPostgresHarnessPath);
const projectionRestartHarness = read(projectionRestartHarnessPath);
const categorySearchReindex = json(categorySearchReindexPath);
const graphqlRateLimit = json(graphqlRateLimitPath);
const aiRichtextBoundary = json(aiRichtextBoundaryPath);
const provider = json(providerPath);
const packageJson = json(packageJsonPath);

if (registry.schema_version !== 13) fail('registry schema_version drift');
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
if (commentsEventProjection.schema_version !== 4) fail('comments event projection schema_version drift');
if (commentsEventProjection.module !== 'blog' || commentsEventProjection.surface !== 'comments_event_projection' || commentsEventProjection.owner !== 'rustok-blog' || commentsEventProjection.provider !== 'rustok-comments') fail('comments event projection identity drift');
if (commentsEventProjection.status !== 'source_verified_no_compile' || commentsEventProjection.compile_policy !== 'not_run_by_request' || commentsEventProjection.runtime_status !== 'pending') fail('comments event projection status drift');
if (
  commentsEventProjection.source_harness?.status !== 'executable_no_run'
  || commentsEventProjection.source_harness?.path !== projectionHandlerPath
  || commentsEventProjection.source_harness?.module !== 'services::comment_projection::tests'
  || commentsEventProjection.source_harness?.command !== projectionHarnessCommand
) fail('comments event projection source harness drift');
if (
  commentsEventProjection.postgres_harness?.status !== 'executable_no_run'
  || commentsEventProjection.postgres_harness?.runtime_status !== 'not_run'
  || commentsEventProjection.postgres_harness?.path !== projectionPostgresHarnessPath
  || commentsEventProjection.postgres_harness?.environment !== projectionPostgresHarnessEnvironment
  || commentsEventProjection.postgres_harness?.command !== projectionPostgresHarnessCommand
  || commentsEventProjection.postgres_harness?.isolation !== 'unique_schema_one_connection_pool'
) fail('comments event projection PostgreSQL harness drift');
sameSet(
  commentsEventProjection.postgres_harness?.cases ?? [],
  [
    'duplicate_delivery_updates_counter_and_outbox_once',
    'optimistic_retry_limit_rolls_back_and_replays_after_conflict_clears',
    'delete_before_create_stays_non_negative_and_replays_in_order',
    'missing_post_replay_commits_only_after_source_appears',
    'outbox_failure_rolls_back_counter_and_delivery_before_retry',
  ],
  'comments event projection PostgreSQL cases',
);
if (
  commentsEventProjection.restart_harness?.status !== 'executable_no_run'
  || commentsEventProjection.restart_harness?.runtime_status !== 'not_run'
  || commentsEventProjection.restart_harness?.path !== projectionRestartHarnessPath
  || commentsEventProjection.restart_harness?.environment !== projectionPostgresHarnessEnvironment
  || commentsEventProjection.restart_harness?.command !== projectionRestartHarnessCommand
  || commentsEventProjection.restart_harness?.isolation !== 'unique_schema_new_connection'
) fail('comments event projection restart harness drift');
sameSet(
  commentsEventProjection.restart_harness?.cases ?? [],
  ['restarted_handler_reuses_delivery_ledger_without_reapplying_counter'],
  'comments event projection restart cases',
);
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
if (provider.schema_version !== 4) fail('comments provider registry schema_version drift');
if (provider.module !== 'comments' || provider.role !== 'provider' || !['in_progress', 'boundary_ready'].includes(provider.status)) fail('comments provider status drift');
sameSet(dependency.operations, provider.ports?.[0]?.operations ?? [], 'consumer/provider operations');
sameSet(dependency.fallback_profiles, provider.consumers?.find(c => c.module === 'blog')?.fallback_profiles ?? [], 'consumer/provider fallback profiles');
sameSet(dependency.degraded_modes, provider.consumers?.find(c => c.module === 'blog')?.degraded_modes ?? [], 'consumer/provider degraded modes');
if (dependency.context !== 'rustok_api::ports::PortContext' || dependency.error !== 'rustok_api::ports::PortError') fail('consumer context/error drift');

const blogInitialMigration = read(blogInitialMigrationPath);
hasAll(
  blogInitialMigration,
  ['ColumnDef::new(BlogPostTranslations::Body).text().not_null()'],
  'canonical Blog richtext storage schema',
);
hasNone(
  blogInitialMigration,
  ['BodyFormat', 'body_format'],
  'canonical Blog richtext storage schema',
);
for (const removedPath of removedRichtextArtifacts) {
  if (fs.existsSync(removedPath)) fail(`removed Blog richtext artifact was restored: ${removedPath}`);
}

const manifest = read('crates/rustok-blog/rustok-module.toml');
hasAll(manifest, ['[fba.consumer]', 'registry = "contracts/blog-fba-registry.json"', 'profile = "blog_post_comments"', 'comments.thread.v1'], 'manifest');

if (evidence.schema_version !== 3 || evidence.surface !== 'comments_port_boundary') fail('comments port matrix schema/identity drift');
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
  hasAll(errorMapping, smokeCase.typed_error_markers ?? [], `runtime error smoke ${smokeCase.operation}:error`);
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
if (
  !projection
  || projection.provider !== 'comments'
  || projection.handler !== 'BlogCommentProjectionHandler'
  || projection.delivery_ledger !== 'blog_comment_projection_deliveries'
  || projection.status !== 'implemented_static_only'
  || projection.runtime_status !== 'pending'
) fail('event projection registry drift');
sameSet(projection.events, ['comment.created', 'comment.deleted'], 'event projection event types');
if (
  projection.source_harness?.path !== projectionHandlerPath
  || projection.source_harness?.status !== 'executable_no_run'
  || projection.source_harness?.command !== projectionHarnessCommand
) fail('event projection registry source harness drift');
if (
  projection.postgres_harness?.path !== projectionPostgresHarnessPath
  || projection.postgres_harness?.status !== 'executable_no_run'
  || projection.postgres_harness?.runtime_status !== 'not_run'
  || projection.postgres_harness?.environment !== projectionPostgresHarnessEnvironment
  || projection.postgres_harness?.command !== projectionPostgresHarnessCommand
) fail('event projection registry PostgreSQL harness drift');
if (
  projection.restart_harness?.path !== projectionRestartHarnessPath
  || projection.restart_harness?.status !== 'executable_no_run'
  || projection.restart_harness?.runtime_status !== 'not_run'
  || projection.restart_harness?.environment !== projectionPostgresHarnessEnvironment
  || projection.restart_harness?.command !== projectionRestartHarnessCommand
) fail('event projection registry restart harness drift');
if (registry.verification_chain?.source_gates?.comments_event_projection?.unit_test !== projectionHandlerPath) fail('event projection source-gate unit test drift');
if (registry.verification_chain?.source_gates?.comments_event_projection?.postgres_test !== projectionPostgresHarnessPath) fail('event projection source-gate PostgreSQL test drift');
if (registry.verification_chain?.source_gates?.comments_event_projection?.restart_test !== projectionRestartHarnessPath) fail('event projection source-gate restart test drift');
const projectionSource = read(projectionHandlerPath);
hasAll(projectionSource, [
  'impl EventHandler for BlogCommentProjectionHandler',
  'fn comment_projection_change(event: &DomainEvent) -> Option<CommentProjectionChange>',
  'let Some(change) = comment_projection_change(&envelope.event) else',
  'comment_projection_change(event).is_some()',
  'fn next_comment_projection_state(comment_count: i32, version: i32, delta: i32)',
  'fn classifies_blog_comment_lifecycle_events()',
  'fn ignores_non_blog_targets_and_unrelated_events()',
  'fn counter_transition_is_non_negative_and_saturating()',
  'blog_comment_projection_delivery::Entity::find_by_id',
  'DomainEvent::BlogPostUpdated',
  '.publish_in_tx(',
], 'blog comment projection');
hasAll(projectionPostgresHarness, [
  'const BLOG_TEST_DATABASE_ENV: &str = "RUSTOK_BLOG_TEST_DATABASE_URL";',
  'struct PostgresBlogProjectionTestDb',
  '.max_connections(1)',
  'SET search_path TO',
  'async fn duplicate_delivery_updates_counter_and_outbox_once()',
  'async fn delete_before_create_stays_non_negative_and_replays_in_order()',
  'async fn missing_post_replay_commits_only_after_source_appears()',
  'async fn outbox_failure_rolls_back_counter_and_delivery_before_retry()',
  'DROP TABLE sys_events',
  'CREATE TABLE blog_comment_projection_deliveries',
  'CREATE TABLE sys_events',
], 'blog comment projection PostgreSQL target');
hasNone(projectionPostgresHarness, ['#[ignore]', 'runtime_verified'], 'blog comment projection PostgreSQL target');
hasAll(projectionRestartHarness, [
  'const BLOG_TEST_DATABASE_ENV: &str = "RUSTOK_BLOG_TEST_DATABASE_URL";',
  'struct PostgresBlogProjectionRestartTestDb',
  'database_url: String',
  'async fn restarted_connection(&self)',
  'async fn restarted_handler_reuses_delivery_ledger_without_reapplying_counter()',
  'let first_handler = BlogCommentProjectionHandler::new(test_db.db.clone());',
  'drop(first_handler);',
  'let restarted_db = test_db.restarted_connection().await?;',
  'let restarted_handler = BlogCommentProjectionHandler::new(restarted_db.clone());',
  'restarted_handler.handle(&envelope).await?;',
  'count_delivery(&restarted_db, envelope.id).await?, 1',
  'count_outbox_events(&restarted_db).await?, 1',
], 'blog comment projection restart target');
hasNone(projectionRestartHarness, ['#[ignore]', 'runtime_verified'], 'blog comment projection restart target');
const migration = read('crates/rustok-blog/src/migrations/m20260716_000001_create_blog_comment_projection_deliveries.rs');
hasAll(migration, ['BlogCommentProjectionDeliveries', 'EventId', 'TenantId', 'PostId'], 'blog comment projection migration');
const moduleSource = read('crates/rustok-blog/src/lib.rs');
hasAll(moduleSource, ['fn register_event_listeners(', 'BlogCommentProjectionHandler::new(ctx.db.clone())'], 'blog event-listener registration');

const plan = read('crates/rustok-blog/docs/implementation-plan.md');
hasAll(plan, ['- FBA status: `boundary_ready`', 'blog-fba-registry.json', commentsEventProjectionPath, categorySearchReindexPath, graphqlRateLimitPath, aiRichtextBoundaryPath, 'CommentsThreadPort', 'blog-comments-consumer-static-matrix.json', 'blog-comments-runtime-fallback-smoke.json', consumerRuntimeOrderSmokePath, 'verify:blog:comments-port-boundary', 'test:verify:blog:comments-port-boundary', 'verify:blog:comments-event-projection', 'test:verify:blog:comments-event-projection', 'services::comment_projection::tests', 'comment_projection_postgres_test', 'comment_projection_restart_postgres_test', 'RUSTOK_BLOG_TEST_DATABASE_URL', 'registry schema v13', 'degraded UI modes remain planned'], 'local plan');
const central = read('docs/modules/registry.md');
hasAll(central, ['| `blog` |', 'crates/rustok-blog/contracts/blog-fba-registry.json', 'blog-comments-runtime-fallback-smoke.json', consumerRuntimeOrderSmokePath, '`in_progress` | `boundary_ready`'], 'central registry');
const unified = read('docs/research/fluid-backend-architecture-unified-plan.md');
hasAll(unified, ['`blog`', 'CommentsThreadPort', 'blog-fba-registry.json'], 'unified plan');

console.log('[verify-blog-fba] Blog FBA registry, exact admin/storefront/comments-port/comments-projection/category/rate-limit/GraphQL/AI richtext source-gate chain, Comments projection unit, PostgreSQL, and restart harnesses, consumer metadata, and no-compile evidence are consistent');
