import fs from 'node:fs';

function read(path) { return fs.readFileSync(path, 'utf8'); }
function json(path) { return JSON.parse(read(path)); }
function fail(message) { console.error(`[verify-blog-fba] ${message}`); process.exit(1); }
function hasAll(text, snippets, label) { for (const s of snippets) if (!text.includes(s)) fail(`${label} missing ${s}`); }
function sameSet(actual, expected, label) {
  const a = [...actual].sort().join('|');
  const e = [...expected].sort().join('|');
  if (a !== e) fail(`${label} drift: expected ${e}, got ${a}`);
}

const registryPath = 'crates/rustok-blog/contracts/blog-fba-registry.json';
const evidencePath = 'crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json';
const runtimeSmokePath = 'crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json';
const consumerRuntimeOrderSmokePath = 'crates/rustok-blog/contracts/evidence/blog-comments-consumer-runtime-order-smoke.json';
const providerPath = 'crates/rustok-comments/contracts/comments-fba-registry.json';
const registry = json(registryPath);
const evidence = json(evidencePath);
const runtimeSmoke = json(runtimeSmokePath);
const consumerRuntimeOrderSmoke = json(consumerRuntimeOrderSmokePath);
const provider = json(providerPath);

if (registry.schema_version !== 1) fail('registry schema_version drift');
if (registry.module !== 'blog' || registry.role !== 'consumer' || !['in_progress', 'boundary_ready'].includes(registry.status)) fail('registry identity/status drift');
if (registry.consumer_profile !== 'blog_post_comments') fail('consumer profile drift');
const dependency = registry.provider_dependencies?.[0];
if (!dependency) fail('missing comments provider dependency');
if (dependency.module !== 'comments' || dependency.registry !== providerPath) fail('provider dependency identity drift');
if (dependency.contract_version !== provider.contract_version || dependency.port !== 'CommentsThreadPort') fail('provider contract/port drift');
if (provider.module !== 'comments' || provider.role !== 'provider' || !['in_progress', 'boundary_ready'].includes(provider.status)) fail('comments provider status drift');
sameSet(dependency.operations, provider.ports?.[0]?.operations ?? [], 'consumer/provider operations');
sameSet(dependency.fallback_profiles, provider.consumers?.find(c => c.module === 'blog')?.fallback_profiles ?? [], 'consumer/provider fallback profiles');
sameSet(dependency.degraded_modes, provider.consumers?.find(c => c.module === 'blog')?.degraded_modes ?? [], 'consumer/provider degraded modes');
if (dependency.context !== 'rustok_api::ports::PortContext' || dependency.error !== 'rustok_api::ports::PortError') fail('consumer context/error drift');

const manifest = read('crates/rustok-blog/rustok-module.toml');
hasAll(manifest, ['[fba.consumer]', 'registry = "contracts/blog-fba-registry.json"', 'profile = "blog_post_comments"', 'comments.thread.v1'], 'manifest');

if (evidence.generated_from !== registryPath || evidence.status !== registry.contract_tests.status) fail('evidence header drift');
sameSet(evidence.cases.map(c => c.operation), registry.contract_tests.cases.map(c => c.operation), 'evidence/registry cases');
sameSet(evidence.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles, 'fallback profiles');
sameSet(evidence.fallback_smoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes, 'degraded modes');

if (registry.evidence.runtime_fallback_smoke !== runtimeSmokePath) fail('runtime smoke evidence path drift');
if (registry.evidence.consumer_runtime_order_smoke !== consumerRuntimeOrderSmokePath) fail('consumer runtime-order smoke evidence path drift');
if (registry.evidence.consumer_runtime_order_smoke_runner !== consumerRuntimeOrderSmoke.runner) fail('consumer runtime-order smoke runner drift');
if (registry.contract_tests.fallback_smoke.status !== 'source_verified_no_compile') fail('fallback smoke status drift');
if (runtimeSmoke.generated_from !== registryPath || runtimeSmoke.status !== registry.contract_tests.fallback_smoke.status) {
  fail('runtime smoke header/status drift');
}
if (runtimeSmoke.runner !== 'scripts/verify/verify-blog-fba.mjs') fail('runtime smoke runner drift');
if (runtimeSmoke.compile_policy !== 'not_run_by_request') fail('runtime smoke compile policy drift');
sameSet(runtimeSmoke.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles, 'runtime smoke profiles');
sameSet(runtimeSmoke.fallback_smoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes, 'runtime smoke degraded modes');
const service = read(runtimeSmoke.source_contract.consumer_service);
const errorMapping = read(runtimeSmoke.source_contract.consumer_error_mapping);
const providerRegistryPath = runtimeSmoke.source_contract.provider_port_registry;
if (providerRegistryPath !== providerPath) fail('runtime smoke provider registry drift');
if (consumerRuntimeOrderSmoke.generated_from !== registryPath || consumerRuntimeOrderSmoke.status !== 'executable_no_compile') {
  fail('consumer runtime-order smoke header/status drift');
}
if (consumerRuntimeOrderSmoke.provider !== 'comments' || consumerRuntimeOrderSmoke.role !== 'consumer') fail('consumer runtime-order smoke identity drift');
if (consumerRuntimeOrderSmoke.source_contract.consumer_service !== runtimeSmoke.source_contract.consumer_service) fail('consumer runtime-order service source drift');
if (consumerRuntimeOrderSmoke.source_contract.consumer_error_mapping !== runtimeSmoke.source_contract.consumer_error_mapping) fail('consumer runtime-order error source drift');
if (consumerRuntimeOrderSmoke.source_contract.provider_registry !== providerPath) fail('consumer runtime-order provider registry drift');
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
hasAll(plan, ['- FBA status: `boundary_ready`', 'blog-fba-registry.json', 'CommentsThreadPort', 'blog-comments-consumer-static-matrix.json', 'blog-comments-runtime-fallback-smoke.json', consumerRuntimeOrderSmokePath], 'local plan');
const central = read('docs/modules/registry.md');
hasAll(central, ['| `blog` |', 'crates/rustok-blog/contracts/blog-fba-registry.json', 'blog-comments-runtime-fallback-smoke.json', consumerRuntimeOrderSmokePath, '`in_progress` | `boundary_ready`'], 'central registry');
const unified = read('docs/research/fluid-backend-architecture-unified-plan.md');
hasAll(unified, ['`blog`', 'CommentsThreadPort', 'blog-fba-registry.json'], 'unified plan');

console.log('[verify-blog-fba] blog FBA comments consumer metadata, static evidence, and no-compile fallback smoke are consistent');
