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
    "crates/rustok-pages/contracts/evidence/pages-native-storefront-channel-admission-source.json",
  ),
);
const harness = read(
  "crates/rustok-pages/storefront/tests/native_storefront_channel_admission_sqlite.rs",
);
const nativeAdapter = read(
  "crates/rustok-pages/storefront/src/transport/native_server_adapter.rs",
);
const channelService = read("crates/rustok-channel/src/services/channel_service.rs");
const cacheContract = read("crates/rustok-pages/src/cache_invalidation.rs");
const overlay = read(
  "docs/modules/pages-page-builder-native-storefront-channel-admission-packet-2026-08-05.md",
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
  "pages_native_storefront_channel_admission_source_unvalidated"
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
  "channel_owner_observed",
  "channel_denial_observed",
  "cache_bypass_observed",
  "enabled_refill_observed",
  "cached_denial_observed",
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
  trusted_channel_extension_used: true,
  request_context_extracted_by_production_adapter: true,
  real_outbox_migration_used: true,
  real_channel_module_migrations_used: true,
  real_pages_module_migrations_used: true,
  real_channel_owner_create_used: true,
  real_channel_owner_module_binding_used: true,
  real_pages_owner_create_used: true,
  real_non_builder_publish_owner_used: true,
  typed_pages_cache_runtime_used: true,
  disabled_binding_rejects_route: true,
  disabled_binding_rejects_before_generation_read: true,
  disabled_binding_rejects_before_cache_get: true,
  disabled_binding_rejects_before_cache_put: true,
  enabled_binding_allows_route: true,
  enabled_binding_misses_and_refills: true,
  enabled_binding_uses_production_ttl: true,
  cached_value_exists_before_second_denial: true,
  second_disabled_binding_rejects_route: true,
  second_disabled_binding_does_not_read_generation: true,
  second_disabled_binding_does_not_read_cache: true,
  second_disabled_binding_does_not_write_cache: true,
  cached_value_cannot_bypass_channel_admission: true,
  production_storefront_behavior_changed: false,
  production_cache_policy_changed: false,
  database_schema_changed: false,
  public_route_changed: false,
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
    "crates/rustok-pages/storefront/tests/native_storefront_channel_admission_sqlite.rs" ||
  evidence.harness?.test !==
    "native_storefront_channel_admission_precedes_cache_lookup" ||
  evidence.harness?.backend !== "sqlite_in_memory" ||
  evidence.harness?.route !== "/api/fn/pages/storefront-data" ||
  evidence.harness?.codec !== "application/x-www-form-urlencoded"
) {
  failures.push("channel admission harness registration is invalid");
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
  "ChannelContextExtension(channel.clone())",
  "HostRuntimeContext::new(db.clone())",
  ".with_shared_value(event_bus)",
  ".with_shared_value(PagesCacheReadRuntime::new(cache_port))",
  "SysEventsMigration.up(&manager).await?",
  "for migration in ChannelModule.migrations()",
  "for migration in PagesModule.migrations()",
  "ChannelService::new(db.clone())",
  ".create_channel(CreateChannelInput",
  ".bind_module(",
  "PageService::new(db.clone(), event_bus)",
  ".publish_non_builder_if_current(",
  "PageCacheGenerationSnapshot::new(3, 5, 7)",
  "Duration::from_secs(PAGES_STOREFRONT_CACHE_TTL_SECS)",
]) {
  requireText(harness, marker, "registered channel admission harness");
}

const testBody = sliceBetween(
  harness,
  "async fn native_storefront_channel_admission_precedes_cache_lookup()",
  "fn native_server_fn_router(",
  "registered channel admission test",
);
requireOrder(
  testBody,
  [
    'is_enabled: false',
    "let disabled = call_storefront",
    "assert_ne!(disabled.status, StatusCode::OK)",
    "assert_eq!(disabled_cache.generation_reads, 0)",
    "assert!(disabled_cache.get_keys.is_empty())",
    "assert!(disabled_cache.put_keys.is_empty())",
    'is_enabled: true',
    "let enabled = call_storefront",
    "assert_eq!(enabled.status, StatusCode::OK)",
    "assert_eq!(enabled_cache.generation_reads, 1)",
    "assert_eq!(enabled_cache.get_keys.len(), 1)",
    "assert_eq!(enabled_cache.put_keys.len(), 1)",
    "assert_eq!(enabled_cache.keys.len(), 1)",
    'is_enabled: false',
    "let before_second_denial = cache.snapshot()",
    "let disabled_with_cached_value = call_storefront",
    "assert_ne!(disabled_with_cached_value.status, StatusCode::OK)",
    "after_second_denial.generation_reads",
    "before_second_denial.generation_reads",
    "after_second_denial.get_keys",
    "before_second_denial.get_keys",
    "after_second_denial.put_keys",
    "before_second_denial.put_keys",
  ],
  "disabled enabled cached-denial ordering",
);

const nativeBody = sliceBetween(
  nativeAdapter,
  "async fn storefront_pages_native(",
  '#[cfg(not(feature = "ssr"))]',
  "production native storefront adapter",
);
requireOrder(
  nativeBody,
  [
    "if let Some(channel_id) = request_context.channel_id",
    "ChannelService::new(runtime_ctx.db_clone())",
    "is_module_enabled(channel_id, MODULE_SLUG)",
    "if !enabled",
    "return Err(ServerFnError::new(format!(",
    "let cache_variant = storefront_cache_variant(",
    "generation_snapshot(tenant_id).await",
    "storefront_pages_cache_key(",
    "get_json::<StorefrontPagesData>(cache_key)",
    "let service = PageService::new",
    "get_by_slug_with_locale_fallback(",
    "list_public_visible(",
    "put_json(cache_key, &data).await",
  ],
  "production channel admission cache owner ordering",
);

const moduleRead = sliceBetween(
  channelService,
  "pub async fn is_module_enabled(",
  "#[instrument(skip(self, input), fields(channel_id = %channel_id, target_type = %input.target_type))]",
  "channel module admission owner",
);
requireOrder(
  moduleRead,
  [
    "self.ensure_channel_exists(channel_id).await?",
    "channel_module_binding::Entity::find()",
    "channel_module_binding::Column::ChannelId.eq(channel_id)",
    "channel_module_binding::Column::ModuleSlug.eq(module_slug)",
    "Ok(binding.map(|item| item.is_enabled).unwrap_or(true))",
  ],
  "channel binding owner ordering",
);
const bindWrite = sliceBetween(
  channelService,
  "pub async fn bind_module(",
  "#[instrument(skip(self), fields(channel_id = %channel_id, binding_id = %binding_id))]",
  "channel module binding write owner",
);
for (const marker of [
  "self.ensure_channel_exists(channel_id).await?",
  "active.is_enabled = Set(input.is_enabled)",
  "ChannelModuleBindingActiveModel",
]) {
  requireText(bindWrite, marker, "channel binding write owner");
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
  "Registered route admission packet: ready, unvalidated",
  "Disabled before cache access",
  "Enabled miss and refill",
  "Disabled with a populated cache",
  "cached value cannot bypass routed-channel admission",
  "execution list is empty",
]) {
  requireText(overlay, marker, "channel admission packet");
}
for (const marker of [
  "native-storefront-channel-admission-source-ready",
  "Routed-channel admission before native lookup: source-ready",
  "populated composite cache cannot bypass channel module admission",
  "verified immutable Page Builder artifact",
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
  forbidText(harness, forbidden, "channel admission harness ownership boundary");
}

if (failures.length > 0) {
  console.error("[verify-pages-native-storefront-channel-admission] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "[verify-pages-native-storefront-channel-admission] PASS source_ready=true execution=pending",
);
