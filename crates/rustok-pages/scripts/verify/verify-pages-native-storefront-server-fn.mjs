#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..");
const read = (relativePath) =>
  fs.readFileSync(path.join(repoRoot, relativePath), "utf8");

const evidence = JSON.parse(
  read(
    "crates/rustok-pages/contracts/evidence/pages-native-storefront-server-fn-source.json",
  ),
);
const cargo = read("crates/rustok-pages/storefront/Cargo.toml");
const harness = read(
  "crates/rustok-pages/storefront/tests/native_storefront_server_fn_sqlite.rs",
);
const nativeAdapter = read(
  "crates/rustok-pages/storefront/src/transport/native_server_adapter.rs",
);
const appRouter = read("apps/server/src/services/app_router.rs");
const cacheContract = read("crates/rustok-pages/src/cache_invalidation.rs");
const overlay = read(
  "docs/modules/pages-page-builder-native-storefront-server-fn-packet-2026-08-05.md",
);
const continuation = read(
  "docs/modules/pages-page-builder-parity-continuation-plan.md",
);
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireOrder = (content, values, label) => {
  let previous = -1;
  for (const value of values) {
    const index = content.indexOf(value, previous + 1);
    if (index < 0) {
      failures.push(`${label}: missing or out of order ${value}`);
      return;
    }
    previous = index;
  }
};
const sliceBetween = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  if (startIndex < 0) {
    failures.push(`${label}: missing ${start}`);
    return "";
  }
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (endIndex < 0) {
    failures.push(`${label}: missing ${end}`);
    return "";
  }
  return content.slice(startIndex, endIndex);
};

if (
  evidence.status !==
  "pages_native_storefront_server_fn_source_unvalidated"
) {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("executed evidence must remain empty");
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "verifiers_run",
  "sqlite_run",
  "axum_run",
  "leptos_server_fn_run",
  "owner_create_observed",
  "owner_publish_observed",
  "cache_miss_observed",
  "cache_refill_observed",
  "cache_hit_observed",
  "generation_rotation_observed",
  "cache_failure_fallback_observed",
  "workflow_checks_run",
  "ci_run",
  "runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`validation.${key} must remain false`);
  }
}

for (const [key, expected] of Object.entries({
  real_leptos_server_function_registry_used: true,
  production_server_function_endpoint_used: true,
  production_server_function_handler_shape_used: true,
  host_runtime_context_used: true,
  trusted_tenant_extension_used: true,
  request_context_extracted_by_production_adapter: true,
  real_outbox_migration_used: true,
  real_pages_module_migrations_used: true,
  real_pages_owner_create_used: true,
  real_non_builder_publish_owner_used: true,
  typed_pages_cache_runtime_used: true,
  initial_route_page_artifact_generations_positive: true,
  first_request_misses_and_refills: true,
  same_generation_request_hits_before_owner_refresh: true,
  owner_body_change_hidden_by_same_generation_hit: true,
  all_generations_advance: true,
  rotated_request_reads_updated_owner_body: true,
  old_generation_value_remains_present: true,
  cache_read_failure_fails_open_to_owner: true,
  cache_read_failure_allows_best_effort_refill: true,
  generation_read_failure_bypasses_lookup_and_fill: true,
  generation_read_failure_fails_open_to_owner: true,
  production_storefront_behavior_changed: false,
  production_cache_policy_changed: false,
  database_schema_changed: false,
  public_route_changed: false,
  channel_admission_executed: false,
  page_builder_artifact_branch_executed: false,
  postgres_executed: false,
  browser_executed: false,
  ffa_promoted: false,
  fba_promoted: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`source_contract.${key} must be ${expected}`);
  }
}

if (
  evidence.harness?.path !==
    "crates/rustok-pages/storefront/tests/native_storefront_server_fn_sqlite.rs" ||
  evidence.harness?.test !==
    "native_storefront_server_fn_misses_hits_rotates_and_fails_open" ||
  evidence.harness?.backend !== "sqlite_in_memory" ||
  evidence.harness?.route !== "/api/fn/pages/storefront-data" ||
  evidence.harness?.codec !== "application/x-www-form-urlencoded"
) {
  failures.push("native storefront server function harness registration is invalid");
}

const devDependencies = cargo.slice(cargo.indexOf("[dev-dependencies]"));
for (const dependency of [
  "async-trait.workspace = true",
  "axum.workspace = true",
  "chrono.workspace = true",
  "sea-orm.workspace = true",
  "sea-orm-migration.workspace = true",
  "tokio.workspace = true",
  "tower.workspace = true",
  "uuid.workspace = true",
]) {
  requireText(devDependencies, dependency, "storefront test dependencies");
}

for (const marker of [
  '#![cfg(feature = "ssr")]',
  "use rustok_pages_storefront as _;",
  "handle_server_fns_with_context",
  "provide_context(host.clone())",
  'const SERVER_FN_PATH: &str = "/api/fn/pages/storefront-data"',
  'const SERVER_FN_FORM: &str = "page_slug=home&locale=en"',
  'header::CONTENT_TYPE, "application/x-www-form-urlencoded"',
  "TenantContextExtension(tenant.clone())",
  "HostRuntimeContext::new(db.clone())",
  ".with_shared_value(event_bus)",
  ".with_shared_value(PagesCacheReadRuntime::new(cache_port))",
  "SysEventsMigration.up(&manager).await?",
  "for migration in PagesModule.migrations()",
  "PageService::new(db.clone(), event_bus)",
  ".publish_non_builder_if_current(",
  "PageCacheGenerationSnapshot::new(3, 5, 7)",
  "PageCacheGenerationSnapshot::new(4, 6, 8)",
  "PageCacheGenerationSnapshot::new(5, 7, 9)",
  "Duration::from_secs(PAGES_STOREFRONT_CACHE_TTL_SECS)",
]) {
  requireText(harness, marker, "registered server function harness");
}

const testBody = sliceBetween(
  harness,
  "async fn native_storefront_server_fn_misses_hits_rotates_and_fails_open()",
  "fn native_server_fn_router(",
  "registered server function test",
);
requireOrder(
  testBody,
  [
    "let first = call_storefront",
    'assert!(first.contains("source-v1"))',
    "let old_generation_key = first_cache.put_keys[0].clone()",
    'update_body(&db, page_id, "<main>source-v2</main>")',
    "let second = call_storefront",
    "assert_eq!(second, first)",
    "cache.set_generations(PageCacheGenerationSnapshot::new(4, 6, 8))",
    "let third = call_storefront",
    'assert!(third.contains("source-v2"))',
    "assert_ne!(new_generation_key, old_generation_key)",
    "assert!(rotated_cache.keys.contains(&old_generation_key))",
    'update_body(&db, page_id, "<main>source-v3</main>")',
    "cache.set_get_error(true)",
    "let fourth = call_storefront",
    'assert!(fourth.contains("source-v3"))',
    'update_body(&db, page_id, "<main>source-v4</main>")',
    "cache.set_generation_error(true)",
    "let fifth = call_storefront",
    'assert!(fifth.contains("source-v4"))',
    "after_generation_failure.get_keys",
    "before_generation_failure.get_keys",
    "after_generation_failure.put_keys",
    "before_generation_failure.put_keys",
  ],
  "miss hit rotation fail-open ordering",
);

const nativeBody = sliceBetween(
  nativeAdapter,
  "async fn storefront_pages_native(",
  '#[cfg(not(feature = "ssr"))]',
  "production native storefront adapter",
);
for (const marker of [
  'endpoint = "pages/storefront-data"',
  "leptos_axum::extract::<rustok_api::RequestContext>()",
  "leptos_axum::extract::<rustok_api::TenantContext>()",
  "ChannelService::new(runtime_ctx.db_clone())",
]) {
  requireText(nativeAdapter, marker, "production native storefront route");
}
requireOrder(
  nativeBody,
  [
    "if let Some(channel_id) = request_context.channel_id",
    "is_module_enabled(channel_id, MODULE_SLUG)",
    "let cache_variant = storefront_cache_variant(",
    "generation_snapshot(tenant_id).await",
    "storefront_pages_cache_key(",
    "get_json::<StorefrontPagesData>(cache_key)",
    "let service = PageService::new",
    "get_by_slug_with_locale_fallback(",
    "list_public_visible(",
    "put_json(cache_key, &data).await",
  ],
  "production admission cache owner ordering",
);

for (const marker of [
  '"/api/fn/{*fn_name}"',
  "handle_server_fns_with_context(",
  "provide_context(runtime_ctx.clone())",
  "let server_fn_runtime_ctx =",
  "attach_commerce_provider_registries",
]) {
  requireText(appRouter, marker, "server host server-function composition");
}

for (const marker of [
  "pub trait PagesCacheReadPort",
  "pub struct PagesCacheReadRuntime",
  "pub fn storefront_pages_cache_key(",
  "PAGES_STOREFRONT_CACHE_TTL_SECS",
]) {
  requireText(cacheContract, marker, "Pages cache contract");
}

for (const marker of [
  "Registered route harness: ready, unvalidated",
  "/api/fn/pages/storefront-data",
  "same wildcard Axum shape used by the server host",
  "same-generation hit before owner refresh",
  "Generation-read failure",
  "execution list is empty",
]) {
  requireText(overlay, marker, "native server function packet");
}
for (const marker of [
  "Native storefront registered server function: source-ready",
  "real registered Leptos endpoint",
  "routed-channel module admission remains open",
  "durable `NodePublished` relay delivery",
]) {
  requireText(continuation, marker, "current parity continuation plan");
}

for (const forbidden of [
  "redis::",
  'cmd("SCAN")',
  'cmd("KEYS")',
  "PageCacheInvalidationPort",
]) {
  forbidText(harness, forbidden, "server function harness ownership boundary");
}

if (failures.length > 0) {
  console.error("[verify-pages-native-storefront-server-fn] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "[verify-pages-native-storefront-server-fn] PASS source_ready=true execution=pending",
);
