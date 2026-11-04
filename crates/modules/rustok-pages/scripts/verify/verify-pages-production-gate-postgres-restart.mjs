#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const evidence = JSON.parse(read(
  "crates/modules/rustok-pages/contracts/evidence/pages-production-gate-postgres-restart-source.json",
));
const harness = read("apps/server/tests/pages_production_gate_postgres_restart.rs");
const gate = read("apps/server/src/services/tenant_generation_delivery_gate.rs");
const port = read("apps/server/src/services/pages_cache_invalidation.rs");
const relay = read("crates/modules/rustok-outbox/src/relay.rs");
const historicalOwner = read("crates/modules/rustok-pages/tests/publish_rollback_outbox_cache_postgres.rs");
const historicalRestart = read("crates/modules/rustok-pages/tests/outbox_relay_restart_postgres.rs");
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const localPlan = read("crates/modules/rustok-pages/docs/implementation-plan.md");
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

if (evidence.format !== "pages_production_gate_postgres_restart_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_production_gate_postgres_restart_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
, marker, "production gate PostgreSQL packet");
for (const marker of [
  "production-gate-postgres-restart-source-ready",
  "Production gate PostgreSQL publish/rollback restart: source-ready",
  "post-invalidation downstream failure",
  "historical owner-transaction and pre-handler restart packets remain separate"
]) need(plan, marker, "canonical Pages/Page Builder plan");
for (const marker of [
  "production gate PostgreSQL publish/rollback restart harness",
  "post-invalidation downstream failure",
  "process-bounded dedupe prevents a",
  "second rotation when a new relay instance retries"
]) need(localPlan, marker, "Pages local plan");

if (failures.length) {
  console.error("[verify-pages-production-gate-postgres-restart] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-production-gate-postgres-restart] PASS source_ready=true postgres_execution=pending");
