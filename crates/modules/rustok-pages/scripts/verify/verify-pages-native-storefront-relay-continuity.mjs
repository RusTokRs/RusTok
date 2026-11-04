#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const evidence = JSON.parse(read("crates/modules/rustok-pages/contracts/evidence/pages-native-storefront-relay-continuity-source.json"));
const gateEvidence = JSON.parse(read("crates/modules/rustok-pages/contracts/evidence/pages-production-relay-generation-gate-source.json"));
const cargo = read("crates/modules/rustok-pages/storefront/Cargo.toml");
const harness = read("crates/modules/rustok-pages/storefront/tests/native_storefront_relay_continuity_sqlite.rs");
const reviewed = read("crates/modules/rustok-pages/src/services/page/reviewed_publish.rs");
const cache = read("crates/modules/rustok-pages/src/cache_invalidation.rs");
const relay = read("crates/modules/rustok-outbox/src/relay.rs");
const adapter = read("crates/modules/rustok-pages/storefront/src/transport/native_server_adapter.rs");
const serverFactory = read("apps/server/src/services/event_transport_factory.rs");
const deliveryGate = read("apps/server/src/services/tenant_generation_delivery_gate.rs");
const serverPort = read("apps/server/src/services/pages_cache_invalidation.rs");
const moduleDispatcher = read("apps/server/src/services/module_event_dispatcher.rs");
const coreDispatcher = read("crates/libs/rustok-core/src/events/handler.rs");
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

, marker, "corrected continuity packet");
for (const marker of [
  "Topology correction",
  "test-target acknowledgement",
  "production-listener-acknowledgement-gap-open",
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
