#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const evidence = JSON.parse(read("crates/rustok-pages/contracts/evidence/pages-native-storefront-relay-continuity-source.json"));
const cargo = read("crates/rustok-pages/storefront/Cargo.toml");
const harness = read("crates/rustok-pages/storefront/tests/native_storefront_relay_continuity_sqlite.rs");
const reviewed = read("crates/rustok-pages/src/services/page/reviewed_publish.rs");
const cache = read("crates/rustok-pages/src/cache_invalidation.rs");
const relay = read("crates/rustok-outbox/src/relay.rs");
const adapter = read("crates/rustok-pages/storefront/src/transport/native_server_adapter.rs");
const packet = read("docs/modules/pages-page-builder-native-storefront-relay-continuity-packet-2026-08-05.md");
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const failures = [];

const need = (text, marker, label) => {
  if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const ordered = (text, markers, label) => {
  let at = -1;
  for (const marker of markers) {
    at = text.indexOf(marker, at + 1);
    if (at < 0) {
      failures.push(`${label}: missing or out of order ${marker}`);
      return;
    }
  }
};

if (evidence.status !== "pages_native_storefront_relay_continuity_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const key of [
  "real_pages_reviewed_publish_used",
  "real_outbox_relay_used",
  "real_pages_cache_event_handler_used",
  "real_leptos_server_function_registry_used",
  "durable_node_created_dispatched_first",
  "node_created_does_not_rotate_pages_generations",
  "durable_node_updated_dispatched_second",
  "node_updated_rotates_route_and_page_only",
  "pre_node_published_native_route_misses_and_refills",
  "durable_node_published_dispatched_third",
  "node_published_request_is_event_and_correlation_bound",
  "node_published_rotates_route_page_and_artifact",
  "old_composite_key_remains_physically_retained",
  "old_composite_key_is_unreachable_after_rotation",
  "post_node_published_native_route_misses_and_refills",
  "post_refill_native_route_hits_without_put"
]) {
  if (evidence.source_contract?.[key] !== true) failures.push(`source_contract.${key} must be true`);
}
for (const key of [
  "production_pages_behavior_changed",
  "production_page_builder_behavior_changed",
  "production_outbox_behavior_changed",
  "production_cache_policy_changed",
  "production_database_schema_changed",
  "public_route_changed",
  "postgres_executed",
  "browser_executed",
  "ffa_promoted",
  "fba_promoted"
]) {
  if (evidence.source_contract?.[key] !== false) failures.push(`source_contract.${key} must be false`);
}

need(cargo, "rustok-events.workspace = true", "Cargo");
need(cargo, "rustok-page-builder.workspace = true", "Cargo");
for (const marker of [
  "PageBuilderReviewedPublishRuntime",
  "OutboxModule",
  "OutboxRelay",
  "PageCacheInvalidationEventHandler",
  "PagesCacheReadRuntime",
  "handle_server_fns_with_context",
  "TenantContextExtension(tenant.clone())",
  "ChannelContextExtension(channel.clone())",
  "page_publish_operation::Entity::find_by_id",
  "DomainEvent::NodeCreated",
  "DomainEvent::NodeUpdated",
  "DomainEvent::NodePublished",
  "PAGES_STOREFRONT_CACHE_TTL_SECS"
]) need(harness, marker, "harness");

ordered(harness, [
  "create_reviewed_published_page",
  "publication_events",
  "PageCacheGenerationSnapshot::new(3, 5, 7)",
  "vec![events.created_id]",
  "PageCacheGenerationSnapshot::new(3, 5, 7)",
  "vec![events.created_id, events.updated_id]",
  "PageCacheGenerationSnapshot::new(4, 6, 7)",
  "PageCacheInvalidationCause::Updated",
  "let before_published_delivery = call_storefront",
  "let old_key = old_cache.put_keys[0].clone()",
  "events.published_id",
  "PageCacheGenerationSnapshot::new(5, 7, 8)",
  "PageCacheInvalidationCause::Published",
  "let after_rotation = call_storefront",
  "let new_key = refilled.put_keys[1].clone()",
  "assert_ne!(new_key, old_key)",
  "let hit = call_storefront",
  "assert_eq!(final_cache.put_keys.len(), 2)"
], "continuity ordering");

ordered(reviewed, [
  "compile_builder_sources_with_reviewed_runtime",
  "PageBuilderArtifactService::stage_compiled_in_tx",
  "PageBuilderArtifactService::bind_existing_body_in_tx",
  "DomainEvent::NodeUpdated",
  "DomainEvent::NodePublished",
  "insert_publish_operation_in_tx",
  "txn.commit().await?"
], "reviewed publish ordering");
need(cache, "Self::Updated => &PAGE_CACHE_MUTABLE_SCOPES", "cache scope policy");
need(cache, "Self::Published | Self::Unpublished | Self::Deleted => &PAGE_CACHE_SCOPES", "cache scope policy");
ordered(relay, [
  "self.target.publish(envelope).await",
  "Ok(()) =>",
  "self.mark_dispatched(model).await?",
  "self.record_processed(elapsed_ms, true)"
], "relay acknowledgement ordering");
ordered(adapter, [
  "is_module_enabled(channel_id, MODULE_SLUG)",
  "generation_snapshot(tenant_id).await",
  "storefront_pages_cache_key(",
  "get_json::<StorefrontPagesData>(cache_key)",
  "load_public_bound_artifact_with_fallback(",
  "put_json(cache_key, &data).await"
], "native route ordering");

for (const marker of [
  "Durable event sequence",
  "Fill between NodeUpdated and NodePublished",
  "NodePublished rotation",
  "Old key retention and new-key refill",
  "execution list is empty"
]) need(packet, marker, "packet");
for (const marker of [
  "native-storefront-relay-continuity-source-ready",
  "Reviewed publish relay to native refill: source-ready",
  "The new continuity harness retains one real owner and dispatcher sequence:",
  "old composite key remains physically retained"
]) need(plan, marker, "plan");

if (failures.length) {
  console.error("[verify-pages-native-storefront-relay-continuity] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-native-storefront-relay-continuity] PASS source_ready=true execution=pending");
