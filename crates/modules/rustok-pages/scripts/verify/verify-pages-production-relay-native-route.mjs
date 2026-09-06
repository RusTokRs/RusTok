#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const evidence = JSON.parse(read("crates/rustok-pages/contracts/evidence/pages-production-relay-native-route-source.json"));
const cargo = read("apps/server/Cargo.toml");
const harness = read("apps/server/tests/pages_production_relay_native_route_sqlite.rs");
const gate = read("apps/server/src/services/tenant_generation_delivery_gate.rs");
const port = read("apps/server/src/services/pages_cache_invalidation.rs");
const relay = read("crates/rustok-outbox/src/relay.rs");
const adapter = read("crates/rustok-pages/storefront/src/transport/native_server_adapter.rs");
const packet = read("docs/modules/pages-page-builder-production-relay-native-route-packet-2026-08-05.md");
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

if (evidence.format !== "pages_production_relay_native_route_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_production_relay_native_route_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution must remain empty");
}
for (const value of Object.values(evidence.validation ?? {})) {
  if (value !== false) failures.push("all validation fields must remain false");
}
for (const key of [
  "real_reviewed_publish_used",
  "real_outbox_relay_used",
  "production_tenant_generation_delivery_gate_used",
  "real_server_pages_cache_port_used_by_gate",
  "real_server_pages_cache_port_used_by_route",
  "single_cache_service_owns_generations_and_values",
  "canonical_local_listener_readiness_used",
  "registered_leptos_server_function_used",
  "real_channel_module_admission_used",
  "durable_node_created_delivered_without_pages_rotation",
  "durable_node_updated_rotates_route_and_page_before_ack",
  "pre_publish_native_route_misses_and_fills_old_key",
  "durable_node_published_rotates_all_scopes_before_ack",
  "asynchronous_pages_listener_same_event_is_duplicate_noop",
  "old_composite_key_remains_physically_retained",
  "post_publish_native_route_misses_and_refills_new_key",
  "post_refill_native_route_hits_without_another_put",
  "reviewed_immutable_artifact_url_is_stable_across_rotation"
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
  "cross_process_exact_once_claimed",
  "tests_executed",
  "verifiers_executed",
  "cargo_executed",
  "sqlite_executed",
  "axum_executed",
  "leptos_server_function_executed",
  "postgres_executed",
  "browser_executed",
  "ffa_promoted",
  "fba_promoted"
]) {
  if (evidence.source_contract?.[key] !== false) failures.push(`source_contract.${key} must be false`);
}

need(cargo, 'rustok-pages-storefront = { path = "../../crates/rustok-pages/storefront", default-features = false, features = ["ssr"] }', "server test dependency");
for (const marker of [
  "PageService::new",
  ".publish_reviewed(",
  "OutboxRelay::new",
  "TenantGenerationDeliveryGate::new",
  "start_tenant_cache_generation_listener",
  "ServerPagesCachePort::new(&cache)",
  "PagesCacheReadRuntime::new",
  "rustok_pages_storefront as _",
  "handle_server_fns_with_context",
  "TenantContextExtension(tenant.clone())",
  "ChannelContextExtension(channel.clone())",
  "DomainEvent::NodeCreated",
  "DomainEvent::NodeUpdated",
  "DomainEvent::NodePublished",
  "PAGES_STOREFRONT_CACHE_TTL_SECS"
]) need(harness, marker, "server integration harness");

ordered(harness, [
  "start_tenant_cache_generation_listener",
  "RecordingReadPort::new(Arc::new(ServerPagesCachePort::new(&cache)))",
  "TenantGenerationDeliveryGate::new",
  "relay.process_pending_once(Some(1))",
  "PageCacheGenerationSnapshot::default()",
  "relay.process_pending_once(Some(1))",
  "PageCacheGenerationSnapshot::new(1, 1, 0)",
  "let before_published_delivery = call_storefront",
  "let old_key = before_rotation.put_keys[0].clone()",
  "relay.process_pending_once(Some(1))",
  "PageCacheGenerationSnapshot::new(2, 2, 1)",
  "PageCacheInvalidationEventHandler::new",
  "let after_rotation = call_storefront",
  "let new_key = refilled.put_keys[1].clone()",
  "assert_ne!(new_key, old_key)",
  "let hit = call_storefront",
  "assert_eq!(final_cache.put_keys.len(), 2)"
], "production relay native route ordering");

ordered(gate, [
  "self.ensure_local_listener_ready().await?",
  "self.pages_handler.handles(&envelope.event)",
  "self.pages_handler.handle(&envelope).await?",
  "self.inner.publish(envelope).await"
], "production gate ordering");
for (const marker of [
  "OnceLock<Arc<BoundedCacheEventDedupe>>",
  "serialize_event(request.event_id)",
  "is_duplicate(request.event_id)",
  "self.successful_invalidations.observe(request.event_id)"
]) need(port, marker, "production Pages cache port");
ordered(relay, [
  "self.target.publish(envelope).await",
  "Ok(()) =>",
  "self.mark_dispatched(model).await?"
], "relay acknowledgement ordering");
ordered(adapter, [
  "is_module_enabled(channel_id, MODULE_SLUG)",
  "generation_snapshot(tenant_id).await",
  "storefront_pages_cache_key(",
  "get_json::<StorefrontPagesData>(cache_key)",
  "load_public_bound_artifact_with_fallback(",
  "put_json(cache_key, &data).await"
], "registered native route ordering");

for (const marker of [
  "Production components mounted",
  "Durable event and route sequence",
  "Old-key retention",
  "Asynchronous listener compatibility",
  "execution list remains empty"
]) need(packet, marker, "packet");
for (const marker of [
  "production-relay-native-route-source-ready",
  "gate-to-native-route composition source-ready",
  "production-relay-generation-gate-source-ready",
  "synchronous generation gate source-ready"
]) need(plan, marker, "shared plan");

if (failures.length) {
  console.error("[verify-pages-production-relay-native-route] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-production-relay-native-route] PASS source_ready=true execution=pending");
