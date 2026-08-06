#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");

const evidence = JSON.parse(read(
  "crates/rustok-pages/contracts/evidence/pages-event-delivery-profile-parity-source.json",
));
const harness = read("apps/server/tests/pages_event_delivery_profiles_sqlite.rs");
const factory = read("apps/server/src/services/event_transport_factory.rs");
const gate = read("apps/server/src/services/tenant_generation_delivery_gate.rs");
const port = read("apps/server/src/services/pages_cache_invalidation.rs");
const relay = read("crates/rustok-outbox/src/relay.rs");
const packet = read("docs/modules/pages-page-builder-event-delivery-profile-parity-packet-2026-08-05.md");
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
const between = (text, start, end, label) => {
  const from = text.indexOf(start);
  if (from < 0) {
    failures.push(`${label}: missing ${start}`);
    return "";
  }
  const to = text.indexOf(end, from + start.length);
  if (to < 0) {
    failures.push(`${label}: missing ${end}`);
    return "";
  }
  return text.slice(from, to);
};

if (evidence.format !== "pages_event_delivery_profile_parity_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_event_delivery_profile_parity_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const key of [
  "server_level_factory_harness_added",
  "full_platform_migrations_used",
  "production_build_event_runtime_used",
  "shared_cache_service_initialized_before_runtime",
  "outbox_local_profile_selected_through_settings",
  "outbox_local_application_transport_reliability_is_outbox",
  "outbox_local_profile_has_relay",
  "outbox_local_publish_persists_pending_before_rotation",
  "outbox_local_listener_is_silent_before_relay",
  "outbox_local_real_relay_used",
  "outbox_local_production_gate_rotates_before_listener_delivery",
  "outbox_local_rotation_precedes_durable_acknowledgement",
  "outbox_local_success_marks_row_dispatched",
  "outbox_local_listener_same_event_is_rotation_noop",
  "outbox_iggy_factory_branch_preserved_but_not_executed"
]) {
  if (evidence.source_contract?.[key] !== true) failures.push(`source_contract.${key} must be true`);
}
for (const key of [
  "production_pages_behavior_changed",
  "production_page_builder_behavior_changed",
  "production_event_factory_behavior_changed",
  "production_outbox_behavior_changed",
  "production_cache_policy_changed",
  "database_schema_changed",
  "public_route_changed",
  "dependencies_changed",
  "ffa_promoted",
  "fba_promoted"
]) {
  if (evidence.source_contract?.[key] !== false) failures.push(`source_contract.${key} must be false`);
}
if (
  evidence.harness?.path !== "apps/server/tests/pages_event_delivery_profiles_sqlite.rs" ||
  evidence.harness?.database !== "isolated SQLite with rustok_migrations::Migrator" ||
  JSON.stringify(evidence.harness?.profiles) !== JSON.stringify(["outbox_local"]) ||
  !Array.isArray(evidence.harness?.tests) ||
  !evidence.harness.tests.includes("outbox_local_profile_defers_rotation_and_listener_delivery_until_relay")
) {
  failures.push("event delivery profile harness registration is invalid");
}

for (const marker of [
  '#![cfg(feature = "mod-pages")]',
  "setup_test_db_with_migrations::<Migrator>()",
  "settings.events.delivery_profile = profile",
  "ctx.shared_insert(cache.clone())",
  "build_event_runtime(&ctx).await?",
  "ServerPagesCachePort::new(&self.cache)",
  "outbox_local_profile_defers_rotation_and_listener_delivery_until_relay",
  "ReliabilityLevel::Outbox",
  "TryRecvError::Empty",
  "PageCacheGenerationSnapshot::new(1, 1, 1)",
  "PageCacheInvalidationEventHandler::new",
  ".handle(envelope)"
]) need(harness, marker, "profile harness");

const outboxTest = between(
  harness,
  "async fn outbox_local_profile_defers_rotation_and_listener_delivery_until_relay()",
  "\n}",
  "outbox local profile test",
);
for (const marker of [
  "ProfileFixture::build(EventDeliveryProfile::OutboxLocal)",
  "ReliabilityLevel::Outbox",
  "SysEventStatus::Pending",
  "PageCacheGenerationSnapshot::default()",
  "listener.try_recv()",
  "TryRecvError::Empty",
  "relay.process_pending_once(Some(1)).await?",
  "SysEventStatus::Dispatched",
  "PageCacheGenerationSnapshot::new(1, 1, 1)",
  "invoke_ordinary_pages_listener(&delivered)"
]) need(outboxTest, marker, "outbox local profile test");
ordered(outboxTest, [
  "fixture.runtime.transport.publish(envelope.clone()).await?",
  "SysEvents::find_by_id(envelope.id)",
  "SysEventStatus::Pending",
  "PageCacheGenerationSnapshot::default()",
  "listener.try_recv()",
  "relay.process_pending_once(Some(1)).await?",
  "listener.recv()",
  "PageCacheGenerationSnapshot::new(1, 1, 1)",
  "SysEventStatus::Dispatched"
], "outbox local profile ordering");

const outboxFactory = between(
  factory,
  "EventDeliveryProfile::OutboxLocal | EventDeliveryProfile::OutboxIggy => {",
  "// Module listeners are started immediately after this function returns",
  "outbox factory branch",
);
for (const marker of [
  "OutboxTransport::new(ctx.db_clone())",
  "EventDeliveryProfile::OutboxLocal =>",
  "EventDeliveryProfile::OutboxIggy =>",
  "ArtifactEventProjectionTransport::new",
  "tenant_generation_transport(ctx, &cache, relay_target)",
  "OutboxRelay::new(ctx.db_clone(), relay_target)",
  "transport: outbox_transport",
  "relay_config: Some(relay_config)"
]) need(outboxFactory, marker, "outbox factory topology");
ordered(outboxFactory, [
  "ArtifactEventProjectionTransport::new",
  "tenant_generation_transport(ctx, &cache, relay_target)",
  "OutboxRelay::new(ctx.db_clone(), relay_target)"
], "outbox relay target ordering");

ordered(gate, [
  "self.ensure_local_listener_ready().await?",
  "self.pages_handler.handles(&envelope.event)",
  "self.pages_handler.handle(&envelope).await?",
  "self.inner.publish(envelope).await"
], "production Pages gate ordering");
ordered(port, [
  "serialize_event(request.event_id)",
  "is_duplicate(request.event_id)",
  "self.generations.bump(&namespace).await",
  "receipt.validate_for(&request)?",
  "self.successful_invalidations.observe(request.event_id)"
], "production Pages dedupe ordering");
const relayProcess = between(
  relay,
  "async fn process_claimed_event(&self, model: &entity::Model)",
  "fn decode_envelope(model: &entity::Model)",
  "outbox relay processing",
);
ordered(relayProcess, [
  "self.target.publish(envelope).await",
  "Ok(()) =>",
  "self.mark_dispatched(model).await?"
], "relay acknowledgement ordering");

for (const marker of [
  "source-ready / execution-pending",
  "build_event_runtime",
  "OutboxLocal profile",
  "OutboxIggy boundary",
  "Execution evidence remains pending"
]) need(packet, marker, "profile parity packet");
for (const marker of [
  "event-delivery-profile-parity-source-ready",
  "OutboxLocal/OutboxIggy parity source-ready",
  "Optional external event and delivery infrastructure remain outside the active Pages cursor"
]) need(plan, marker, "canonical Pages/Page Builder plan");

forbid(packet, "OutboxIggy execution is complete", "profile parity packet");

if (failures.length) {
  console.error("[verify-pages-event-delivery-profile-parity] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-event-delivery-profile-parity] PASS source_ready=true execution=pending");
