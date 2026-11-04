#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const evidence = JSON.parse(read("crates/modules/rustok-pages/contracts/evidence/pages-production-relay-generation-gate-source.json"));
const port = read("apps/server/src/services/pages_cache_invalidation.rs");
const gate = read("apps/server/src/services/tenant_generation_delivery_gate.rs");
const factory = read("apps/server/src/services/event_transport_factory.rs");
const dispatcher = read("apps/server/src/services/module_event_dispatcher.rs");
const pagesModule = read("crates/modules/rustok-pages/src/lib.rs");
const relay = read("crates/modules/rustok-outbox/src/relay.rs");
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
, marker, "packet");
for (const marker of [
  "production-relay-generation-gate-source-ready",
  "synchronous generation gate source-ready",
  "production-relay-native-route-source-ready",
  "gate-to-native-route composition source-ready",
  "production-gate-postgres-restart-source-ready",
  "PostgreSQL retry source-ready"
]) need(plan, marker, "shared plan");

if (failures.length) {
  console.error("[verify-pages-production-relay-generation-gate] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-production-relay-generation-gate] PASS source_ready=true execution=pending");
