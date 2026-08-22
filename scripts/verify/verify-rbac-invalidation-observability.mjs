#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? path.resolve(configuredRoot)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const read = (relativePath) => readFileSync(path.join(root, relativePath), "utf8");
const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const normalizeWhitespace = (value) => value.replace(/\s+/g, " ").trim();
const requireNormalizedText = (source, value, label) => {
  if (!normalizeWhitespace(source).includes(normalizeWhitespace(value))) {
    failures.push(`${label}: missing ${value}`);
  }
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const files = {
  metrics: "crates/rustok-telemetry/src/rbac_invalidation_metrics.rs",
  telemetry: "crates/rustok-telemetry/src/lib.rs",
  watchdog: "apps/server/src/services/rbac_invalidation_generation.rs",
  docs: "crates/rustok-rbac/docs/README.md",
  plan: "crates/rustok-rbac/docs/implementation-plan.md",
  serverPlan: "apps/server/docs/implementation-plan.md",
  telemetryPlan: "crates/rustok-telemetry/docs/implementation-plan.md",
  master: "docs/verification/PLATFORM_VERIFICATION_PLAN.md",
};

const metrics = read(files.metrics);
const telemetry = read(files.telemetry);
const watchdog = read(files.watchdog);
const docs = read(files.docs);
const plan = read(files.plan);
const serverPlan = read(files.serverPlan);
const telemetryPlan = read(files.telemetryPlan);
const master = read(files.master);

for (const metricName of [
  "rustok_rbac_invalidation_durable_generation",
  "rustok_rbac_invalidation_applied_generation",
  "rustok_rbac_invalidation_generation_lag",
  "rustok_rbac_invalidation_watchdog_running",
  "rustok_rbac_invalidation_database_read_errors_total",
  "rustok_rbac_invalidation_watchdog_restarts_total",
  "rustok_rbac_invalidation_recoveries_total",
  "rustok_rbac_invalidation_full_clears_total",
]) requireText(metrics, metricName, `${files.metrics}: metric contract`);

for (const marker of [
  "pub fn register(registry: &Registry)",
  "pub fn signed_generation_lag",
  "lag.clamp(i128::from(i64::MIN), i128::from(i64::MAX))",
  "pub fn update_generations",
  "pub fn set_watchdog_running",
  "pub fn record_database_read_error",
  "pub fn record_watchdog_restart",
  "pub fn record_recovery",
  "pub fn record_full_clear",
  "signed_lag_distinguishes_catch_up_and_regression",
  "registration_exposes_the_bounded_metric_families",
  '&["reason"]',
]) requireText(metrics, marker, `${files.metrics}: bounded implementation`);

for (const forbidden of [
  '"tenant_id"',
  '"user_id"',
  '"role_id"',
  '"permission"',
  '"session_id"',
  '"client_id"',
  '"cache_key"',
]) forbidText(metrics, forbidden, `${files.metrics}: high-cardinality or sensitive label`);

requireText(
  telemetry,
  "pub mod rbac_invalidation_metrics;",
  `${files.telemetry}: module export`,
);
requireText(
  telemetry,
  "rbac_invalidation_metrics::register(registry)?;",
  `${files.telemetry}: canonical registry`,
);

for (const marker of [
  "rbac_invalidation_metrics::set_watchdog_running(true)",
  "rbac_invalidation_metrics::set_watchdog_running(false)",
  "rbac_invalidation_metrics::update_generations(generation, applied_before)",
  "rbac_invalidation_metrics::update_generations(generation, state.current())",
  'rbac_invalidation_metrics::record_recovery("generation_regressed")',
  'rbac_invalidation_metrics::record_full_clear("generation_regressed")',
  '"generation_advanced"',
  '"initial_sync"',
  "rbac_invalidation_metrics::record_database_read_error()",
  'rbac_invalidation_metrics::record_watchdog_restart("runtime_replaced")',
  '"panic"',
  '"unexpected_exit"',
]) requireText(watchdog, marker, `${files.watchdog}: runtime instrumentation`);

for (const marker of [
  "## Durable invalidation alert policy",
  "## Durable invalidation incident runbook",
  "generation_lag < 0",
  "Redis outage or restart",
  "Missed PubSub event",
  "Generation regression",
  "Canonical role repair",
  "still an open P1",
]) requireText(docs, marker, `${files.docs}: operator contract`);

for (const marker of [
  "### Durable invalidation and recovery",
  "[x] Export bounded lag, generation, worker, and recovery telemetry.",
  "[x] Retain source packets for #2849, #2853, #2856, and #2862.",
  "[ ] Execute and retain the incident packet from #2846.",
  "Status: `in_progress`",
  "verify-rbac-invalidation-observability.mjs",
]) requireNormalizedText(plan, marker, `${files.plan}: implementation handoff`);

for (const marker of [
  "## RBAC durable invalidation observability composition",
  "signed durable-minus-applied lag",
  "Recovery still clears permission snapshots through the existing owner/runtime path.",
  "Status: `pending`",
  "one complete authorization incident trace remain required evidence.",
]) requireNormalizedText(serverPlan, marker, `${files.serverPlan}: cross-owner server handoff`);

for (const marker of [
  "## Delivered result: bounded RBAC invalidation metrics",
  "same canonical process registry",
  "performs no database reads, cache operations or worker supervision",
  "RBAC still needs one retained incident chain connecting evaluator decision",
]) requireNormalizedText(
  telemetryPlan,
  marker,
  `${files.telemetryPlan}: cross-owner telemetry handoff`,
);

for (const marker of [
  "Current item: `core/rbac`",
  "Next item: `core/rbac`",
  "`core/rbac` remains `in_progress`",
  "incident/live negative transport evidence",
]) requireNormalizedText(master, marker, `${files.master}: active cursor`);

if (failures.length > 0) {
  console.error("RBAC invalidation observability verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ RBAC durable invalidation metrics, bounded labels, watchdog transitions, owner/cross-owner plans, operator policy, and cycle cursor are synchronized",
);
