#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");

const evidence = JSON.parse(read(
  "crates/modules/rustok-pages/contracts/evidence/pages-event-delivery-profile-parity-source.json",
));
const harness = read("apps/server/tests/pages_event_delivery_profiles_sqlite.rs");
const factory = read("apps/server/src/services/event_transport_factory.rs");
const gate = read("apps/server/src/services/tenant_generation_delivery_gate.rs");
const port = read("apps/server/src/services/pages_cache_invalidation.rs");
const relay = read("crates/modules/rustok-outbox/src/relay.rs");
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
, marker, "profile parity packet");
for (const marker of [
  "event-delivery-profile-parity-source-ready",
  "Outbox/OutboxIggy parity source-ready",
  "Optional external event and delivery infrastructure remain outside the active Pages cursor"
]) need(plan, marker, "canonical Pages/Page Builder plan");

forbid(packet, "OutboxIggy execution is complete", "profile parity packet");

if (failures.length) {
  console.error("[verify-pages-event-delivery-profile-parity] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-event-delivery-profile-parity] PASS source_ready=true execution=pending");
