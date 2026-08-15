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
    "crates/rustok-pages/contracts/evidence/pages-native-storefront-cache-source.json",
  ),
);
const harness = read(
  "crates/rustok-pages/tests/native_storefront_cache_contract.rs",
);
const nativeAdapter = read(
  "crates/rustok-pages/storefront/src/transport/native_server_adapter.rs",
);
const cacheContract = read("crates/rustok-pages/src/cache_invalidation.rs");
const parityPlan = read(
  "docs/modules/pages-page-builder-parity-continuation-plan.md",
);
const packet = read(
  "docs/modules/pages-page-builder-native-storefront-cache-packet-2026-08-05.md",
);
const actualization = read(
  "docs/modules/page-builder-parity-actualization-2026-08-05.md",
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

if (evidence.status !== "pages_native_storefront_cache_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "verifiers_run",
  "native_server_function_run",
  "database_run",
  "cache_miss_observed",
  "owner_read_observed",
  "cache_refill_observed",
  "cache_hit_observed",
  "generation_rotation_observed",
  "cache_failure_fallback_observed",
  "browser_run",
  "workflow_checks_run",
  "ci_run",
  "runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`validation.${key} must remain false`);
  }
}

for (const [key, expected] of Object.entries({
  production_native_server_function_inspected: true,
  module_admission_precedes_cache_lookup: true,
  tenant_locale_channel_variant_bound: true,
  route_page_artifact_generations_bound: true,
  same_public_storefront_cache_key_used: true,
  same_typed_pages_cache_runtime_used: true,
  cache_hit_short_circuits_owner_reads: true,
  owner_page_and_artifact_reads_precede_fill: true,
  generation_read_failure_fails_open: true,
  cache_read_failure_fails_open: true,
  cache_fill_failure_fails_open: true,
  contract_harness_added: true,
  initial_generation_miss_refill_source_ready: true,
  same_generation_hit_source_ready: true,
  generation_rotation_miss_refill_source_ready: true,
  old_generation_value_retained: true,
  cache_read_failure_source_fallback_ready: true,
  generation_failure_source_fallback_ready: true,
  cache_ttl_contract_retained: true,
  production_storefront_behavior_changed: false,
  production_cache_behavior_changed: false,
  database_schema_changed: false,
  public_transport_changed: false,
  page_builder_contract_changed: false,
  native_server_function_executed: false,
  database_executed: false,
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
    "crates/rustok-pages/tests/native_storefront_cache_contract.rs" ||
  evidence.harness?.test !==
    "native_storefront_cache_misses_refills_hits_rotates_and_fails_open" ||
  evidence.harness?.backend !== "recording_pages_cache_port"
) {
  failures.push("native storefront harness registration is invalid");
}

requireOrder(
  nativeAdapter,
  [
    "ChannelService::new(runtime_ctx.db_clone())",
    ".is_module_enabled(channel_id, MODULE_SLUG)",
    "let cache_runtime = runtime_ctx.shared_get::<PagesCacheReadRuntime>();",
    "let cache_variant = storefront_cache_variant(",
    "generation_snapshot(tenant_id).await",
    "storefront_pages_cache_key(tenant_id, generations, cache_variant.as_str())",
    ".get_json::<StorefrontPagesData>(cache_key)",
    "let service = PageService::new(runtime_ctx.db_clone(), event_bus);",
    ".get_by_slug_with_locale_fallback(",
    "PageBuilderArtifactService::new(runtime_ctx.db_clone())",
    ".load_public_bound_artifact_with_fallback(",
    ".list_public_visible_with_locale_fallback(",
    "let data = StorefrontPagesData {",
    "cache_runtime.put_json(cache_key, &data).await",
  ],
  "native storefront admission/cache/owner/fill ordering",
);
for (const marker of [
  "Pages storefront generation read failed; bypassing cache",
  "Pages storefront cache read failed; loading source data",
  "Pages storefront cache fill failed",
  "return Ok(cached)",
  "is_visible_for_public_channel",
  "published_artifact_page_body",
]) {
  requireText(nativeAdapter, marker, "production native storefront boundary");
}

for (const marker of [
  "pub trait PagesCacheReadPort",
  "pub struct PagesCacheReadRuntime",
  "pub fn storefront_pages_cache_key(",
  "generations.route, generations.page, generations.artifact",
  "Duration::from_secs(PAGES_STOREFRONT_CACHE_TTL_SECS)",
]) {
  requireText(cacheContract, marker, "Pages cache contract");
}

for (const marker of [
  "struct RecordingCachePort",
  "impl PagesCacheReadPort for RecordingCachePort",
  "PageCacheGenerationSnapshot::new(3, 5, 7)",
  "PageCacheGenerationSnapshot::new(4, 6, 8)",
  "PageCacheGenerationSnapshot::new(5, 7, 9)",
  "storefront_pages_cache_key(tenant_id, generations, cache_variant)",
  ".get_json::<StorefrontSnapshot>(cache_key)",
  "cache_runtime.put_json(cache_key, &data).await",
  "Duration::from_secs(PAGES_STOREFRONT_CACHE_TTL_SECS)",
  "set_get_error(true)",
  "set_generation_error(true)",
]) {
  requireText(harness, marker, "native storefront contract harness");
}

const testBody = sliceBetween(
  harness,
  "async fn native_storefront_cache_misses_refills_hits_rotates_and_fails_open()",
  "\n}",
  "native storefront cache test",
);
requireOrder(
  testBody,
  [
    "let first =",
    "assert_eq!(source_calls.load(Ordering::SeqCst), 1)",
    "let old_generation_key = first_cache.put_keys[0].clone()",
    "let second =",
    "assert_eq!(second, StorefrontSnapshot::new(1))",
    "cache.set_generations(PageCacheGenerationSnapshot::new(4, 6, 8))",
    "let third =",
    "let new_generation_key = rotated_cache.put_keys[1].clone()",
    "assert_ne!(new_generation_key, old_generation_key)",
    "assert!(rotated_cache.keys.contains(&old_generation_key))",
    "cache.set_get_error(true)",
    "let fourth =",
    "assert_eq!(fourth, StorefrontSnapshot::new(3))",
    "cache.set_generation_error(true)",
    "let fifth =",
    "assert_eq!(fifth, StorefrontSnapshot::new(4))",
    "after_generation_failure.get_keys",
    "before_generation_failure.get_keys",
    "after_generation_failure.put_keys",
    "before_generation_failure.put_keys",
  ],
  "miss/hit/rotation/fail-open source ordering",
);

for (const marker of [
  "source-parity-current",
  "cache policy, public reads",
  "production-relay-generation-gate-source-ready",
  "production-relay-native-route-source-ready",
]) {
  requireText(parityPlan, marker, "current parity continuation plan");
}
for (const marker of [
  "source-ready / execution-pending",
  "same public cache primitives used by the native server function",
  "old-generation value remains physically present",
  "native server-function execution remains pending",
]) {
  requireText(packet, marker, "native storefront cache packet");
}
for (const marker of [
  "typed metadata contribution is source-complete",
  "Immutable rollback is source-complete.",
  "The native storefront cache source contract is ready.",
  "Source parity has advanced, but execution and rollout remain open.",
]) {
  requireText(actualization, marker, "Page Builder parity actualization");
}

for (const forbidden of [
  "redis::",
  'cmd("SCAN")',
  'cmd("KEYS")',
  "PageService::new",
  "PageBuilderArtifactService::new",
  "HostRuntimeContext",
]) {
  forbidText(harness, forbidden, "contract harness ownership boundary");
}

if (failures.length > 0) {
  console.error("[verify-pages-native-storefront-cache] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "[verify-pages-native-storefront-cache] PASS source_ready=true execution=pending",
);
