#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const evidence = JSON.parse(read("crates/rustok-pages/contracts/evidence/pages-native-storefront-relay-continuity-source.json"));
const gateEvidence = JSON.parse(read("crates/rustok-pages/contracts/evidence/pages-production-relay-generation-gate-source.json"));
const cargo = read("crates/rustok-pages/storefront/Cargo.toml");
const harness = read("crates/rustok-pages/storefront/tests/native_storefront_relay_continuity_sqlite.rs");
const reviewed = read("crates/rustok-pages/src/services/page/reviewed_publish.rs");
const cache = read("crates/rustok-pages/src/cache_invalidation.rs");
const relay = read("crates/rustok-outbox/src/relay.rs");
const adapter = read("crates/rustok-pages/storefront/src/transport/native_server_adapter.rs");
const serverFactory = read("apps/server/src/services/event_transport_factory.rs");
const deliveryGate = read("apps/server/src/services/tenant_generation_delivery_gate.rs");
const serverPort = read("apps/server/src/services/pages_cache_invalidation.rs");
const moduleDispatcher = read("apps/server/src/services/module_event_dispatcher.rs");
const coreDispatcher = read("crates/rustok-core/src/events/handler.rs");
const packet = read("docs/modules/pages-page-builder-native-storefront-relay-continuity-packet-2026-08-05.md");
const correction = read("docs/modules/pages-page-builder-native-storefront-relay-topology-correction-2026-08-05.md");
const gatePacket = read("docs/modules/pages-page-builder-production-relay-generation-gate-packet-2026-08-05.md");
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const failures = [];

const need = (text, marker, label) => {
  if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (text, marker, label) => {
  if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
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
const requireEmptyPendingEvidence = (value, expectedStatus, label) => {
  if (value.status !== expectedStatus) failures.push(`${label} status mismatch: ${value.status}`);
  if (!Array.isArray(value.execution) || value.execution.length !== 0) {
    failures.push(`${label} execution must remain empty`);
  }
  for (const [key, flag] of Object.entries(value.validation ?? {})) {
    if (flag !== false) failures.push(`${label} validation.${key} must remain false`);
  }
};

if (evidence.format !== "pages_native_storefront_relay_continuity_source_v2") {
  failures.push(`continuity evidence format mismatch: ${evidence.format}`);
}
requireEmptyPendingEvidence(
  evidence,
  "pages_native_storefront_relay_continuity_source_corrected_unvalidated",
  "continuity evidence",
);
requireEmptyPendingEvidence(
  gateEvidence,
  "pages_production_relay_generation_gate_source_unvalidated",
  "production gate evidence",
);

for (const key of [
  "real_pages_reviewed_publish_used",
  "real_outbox_relay_used",
  "real_pages_cache_event_handler_used",
  "custom_synchronous_relay_target_used",
  "test_target_success_precedes_test_outbox_acknowledgement",
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
  if (evidence.source_contract?.[key] !== true) failures.push(`continuity source_contract.${key} must be true`);
}
for (const key of [
  "production_server_relay_target_used",
  "production_module_event_dispatcher_used",
  "production_listener_acknowledgement_coupled_to_outbox_relay",
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
  if (evidence.source_contract?.[key] !== false) failures.push(`continuity source_contract.${key} must be false`);
}
for (const key of [
  "production_tenant_delivery_gate_extended",
  "gate_is_in_memory_and_outbox_transport_chain",
  "real_pages_handler_predicate_used",
  "real_pages_invalidation_runtime_used",
  "pages_invalidation_precedes_downstream_publish",
  "successful_invalidation_dedupe_is_process_bounded",
  "dedupe_key_is_stable_event_uuid",
  "same_event_work_is_serialized",
  "duplicate_event_does_not_bump_generations",
  "downstream_failure_allows_delivery_retry_without_second_rotation",
  "asynchronous_module_listener_remains_registered",
  "asynchronous_module_listener_duplicate_is_rotation_noop",
  "process_restart_may_conservatively_rotate_again"
]) {
  if (gateEvidence.source_contract?.[key] !== true) failures.push(`gate source_contract.${key} must be true`);
}

need(cargo, "rustok-events.workspace = true", "Cargo");
need(cargo, "rustok-page-builder.workspace = true", "Cargo");
for (const marker of [
  "PageBuilderReviewedPublishRuntime",
  "OutboxModule",
  "OutboxRelay",
  "struct ContinuityTarget",
  "PageCacheInvalidationEventHandler",
  "self.handler.handle(&envelope).await?",
  "PagesCacheReadRuntime",
  "handle_server_fns_with_context",
  "TenantContextExtension(tenant.clone())",
  "ChannelContextExtension(channel.clone())",
  "DomainEvent::NodeCreated",
  "DomainEvent::NodeUpdated",
  "DomainEvent::NodePublished"
]) need(harness, marker, "continuity harness");
ordered(harness, [
  "vec![events.created_id]",
  "PageCacheGenerationSnapshot::new(3, 5, 7)",
  "vec![events.created_id, events.updated_id]",
  "PageCacheGenerationSnapshot::new(4, 6, 7)",
  "let before_published_delivery = call_storefront",
  "let old_key = old_cache.put_keys[0].clone()",
  "events.published_id",
  "PageCacheGenerationSnapshot::new(5, 7, 8)",
  "let after_rotation = call_storefront",
  "let new_key = refilled.put_keys[1].clone()",
  "assert_ne!(new_key, old_key)",
  "let hit = call_storefront"
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
  "self.mark_dispatched(model).await?"
], "relay target acknowledgement ordering");
ordered(adapter, [
  "is_module_enabled(channel_id, MODULE_SLUG)",
  "generation_snapshot(tenant_id).await",
  "storefront_pages_cache_key(",
  "get_json::<StorefrontPagesData>(cache_key)",
  "load_public_bound_artifact_with_fallback(",
  "put_json(cache_key, &data).await"
], "native route ordering");

const artifactBodyGuard = '#[cfg(feature = "ssr")]\nfn published_artifact_page_body(';
if (adapter.split(artifactBodyGuard).length - 1 !== 1) {
  failures.push("native adapter must retain exactly one SSR guard on published_artifact_page_body");
}
forbid(
  adapter,
  '#[cfg(feature = "ssr")]\n#[cfg(feature = "ssr")]\nfn published_artifact_page_body(',
  "native adapter cfg normalization",
);

for (const marker of [
  "EventDeliveryProfile::Outbox | EventDeliveryProfile::OutboxIggy",
  "TenantGenerationDeliveryGate::new",
  "OutboxRelay::new(ctx.db_clone(), relay_target)"
]) need(serverFactory, marker, "production server transport topology");
ordered(deliveryGate, [
  "self.ensure_local_listener_ready().await?",
  "self.pages_handler.handles(&envelope.event)",
  "self.pages_handler.handle(&envelope).await?",
  "self.inner.publish(envelope).await"
], "production Pages delivery gate");
ordered(serverPort, [
  "serialize_event(request.event_id)",
  "is_duplicate(request.event_id)",
  "self.generations.bump(&namespace).await",
  "receipt.validate_for(&request)?",
  "self.successful_invalidations.observe(request.event_id)"
], "production Pages invalidation dedupe");
need(serverPort, "OnceLock<Arc<BoundedCacheEventDedupe>>", "production Pages invalidation dedupe");
ordered(moduleDispatcher, [
  ".listener_bus",
  "build_module_event_dispatcher(registry, bus, db, extensions.as_ref())",
  "dispatcher.start()"
], "production module listener topology");
for (const marker of [
  ".filter(|handler| handler.handles(&envelope.event))",
  "tokio::spawn(",
  "Self::handle_with_retry(handler, envelope, &config).await"
]) need(coreDispatcher, marker, "production asynchronous dispatcher");

for (const marker of [
  "synchronous test relay target",
  "does not mount the production server relay target",
  "production module dispatcher remains a separate boundary",
  "execution list is empty"
]) need(packet, marker, "corrected continuity packet");
for (const marker of [
  "Topology correction",
  "test-target acknowledgement",
  "production listener acknowledgement gap",
  "no production behavior change"
]) need(correction, marker, "topology correction packet");
for (const marker of [
  "Production transport placement",
  "Shared idempotency",
  "Asynchronous listener compatibility",
  "process restart intentionally loses this bounded optimization"
]) need(gatePacket, marker, "production gate packet");
for (const marker of [
  "production-relay-generation-gate-source-ready",
  "synchronous generation gate source-ready",
  "production-relay-native-route-source-ready",
  "gate-to-native-route composition source-ready"
]) need(plan, marker, "current parity plan");

if (failures.length) {
  console.error("[verify-pages-native-storefront-relay-continuity] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-native-storefront-relay-continuity] PASS test_target=source_corrected production_gate=source_ready execution=pending");
