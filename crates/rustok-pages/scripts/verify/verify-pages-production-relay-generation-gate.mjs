#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const evidence = JSON.parse(read("crates/rustok-pages/contracts/evidence/pages-production-relay-generation-gate-source.json"));
const port = read("apps/server/src/services/pages_cache_invalidation.rs");
const gate = read("apps/server/src/services/tenant_generation_delivery_gate.rs");
const factory = read("apps/server/src/services/event_transport_factory.rs");
const dispatcher = read("apps/server/src/services/module_event_dispatcher.rs");
const pagesModule = read("crates/rustok-pages/src/lib.rs");
const relay = read("crates/rustok-outbox/src/relay.rs");
const packet = read("docs/modules/pages-page-builder-production-relay-generation-gate-packet-2026-08-05.md");
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

if (evidence.status !== "pages_production_relay_generation_gate_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution must remain empty");
}
for (const value of Object.values(evidence.validation ?? {})) {
  if (value !== false) failures.push("all validation fields must remain false");
}
for (const key of [
  "production_tenant_delivery_gate_extended",
  "gate_is_in_memory_and_outbox_transport_chain",
  "real_pages_handler_predicate_used",
  "real_pages_invalidation_runtime_used",
  "pages_invalidation_precedes_downstream_publish",
  "local_listener_readiness_precedes_pages_invalidation",
  "successful_invalidation_dedupe_is_process_bounded",
  "dedupe_key_is_stable_event_uuid",
  "same_event_work_is_serialized",
  "duplicate_event_does_not_bump_generations",
  "invalidation_failure_does_not_commit_dedupe",
  "invalidation_success_commits_before_downstream_publish",
  "downstream_failure_allows_delivery_retry_without_second_rotation",
  "asynchronous_module_listener_remains_registered",
  "asynchronous_module_listener_duplicate_is_rotation_noop",
  "memory_profile_listener_delivery_preserved",
  "outbox_delivery_preserved",
  "outbox_iggy_delivery_preserved",
  "process_restart_may_conservatively_rotate_again",
  "production_pages_behavior_changed",
  "tests_added"
]) {
  if (evidence.source_contract?.[key] !== true) failures.push(`source_contract.${key} must be true`);
}
for (const key of [
  "production_page_builder_behavior_changed",
  "production_outbox_schema_changed",
  "production_database_schema_changed",
  "production_cache_namespace_changed",
  "public_route_changed",
  "tests_run",
  "cargo_run",
  "format_run",
  "verifier_run",
  "ci_run",
  "ffa_promoted",
  "fba_promoted"
]) {
  if (evidence.source_contract?.[key] !== false) failures.push(`source_contract.${key} must be false`);
}

for (const marker of [
  "OnceLock<Arc<BoundedCacheEventDedupe>>",
  "serialize_event(request.event_id)",
  "is_duplicate(request.event_id)",
  "return self.current_receipt(&request).await",
  "self.generations.bump(&namespace).await",
  "self.successful_invalidations.observe(request.event_id)",
  "duplicate_event_returns_current_receipt_without_second_rotation"
]) need(port, marker, "server Pages cache port");
ordered(port, [
  "serialize_event(request.event_id)",
  "is_duplicate(request.event_id)",
  "let mut receipt = PageCacheInvalidationReceipt::new(&request)",
  "self.generations.bump(&namespace).await",
  "receipt.validate_for(&request)?",
  "self.successful_invalidations.observe(request.event_id)"
], "invalidation commit ordering");

for (const marker of [
  "PageCacheInvalidationEventHandler::new",
  "PagesCacheInvalidationRuntime::new(provider)",
  "pages_rotation_precedes_downstream_retry_and_async_listener_is_duplicate_safe",
  "ServerPagesCachePort::new(&cache)"
]) need(gate, marker, "delivery gate");
ordered(gate, [
  "self.ensure_local_listener_ready().await?",
  "self.pages_handler.handles(&envelope.event)",
  "self.pages_handler.handle(&envelope).await?",
  "self.inner.publish(envelope).await"
], "delivery gate ordering");

const transportUses = factory.match(/tenant_generation_transport\(/g)?.length ?? 0;
if (transportUses < 2) failures.push(`event factory: expected one call plus definition, found ${transportUses}`);
for (const marker of [
  "EventDeliveryProfile::Outbox | EventDeliveryProfile::OutboxIggy",
  "TenantGenerationDeliveryGate::new"
]) need(factory, marker, "event factory");

for (const marker of [
  "ServerPagesCachePort::new(&cache)",
  "PagesCacheInvalidationRuntime::new",
  "PagesCacheReadRuntime::new(provider)"
]) need(dispatcher, marker, "module dispatcher composition");
need(pagesModule, "registry.register(PageCacheInvalidationEventHandler::new(runtime))", "Pages listener registration");
ordered(relay, [
  "self.target.publish(envelope).await",
  "Ok(()) =>",
  "self.mark_dispatched(model).await?"
], "outbox acknowledgement ordering");

for (const marker of [
  "Production transport placement",
  "Shared idempotency",
  "Asynchronous listener compatibility",
  "process restart intentionally loses this bounded optimization",
  "execution list remains empty"
]) need(packet, marker, "packet");
for (const marker of [
  "production-relay-generation-gate-source-ready",
  "relay continuity, production gate, native route, PostgreSQL retry and profile parity",
  "Historical dated packets remain evidence for the source slices that produced the current state",
  "The following #2955–#3063 list is a retained historical snapshot"
]) need(plan, marker, "shared plan");

if (failures.length) {
  console.error("[verify-pages-production-relay-generation-gate] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-production-relay-generation-gate] PASS source_ready=true execution=pending");
