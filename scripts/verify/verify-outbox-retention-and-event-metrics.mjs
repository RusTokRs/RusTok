#!/usr/bin/env node
// Fast source-level guardrails for outbox delivered-event retention and for the
// production call sites of the `rustok_event_bus_*` metric family.
//
// Two invariants are easy to lose in a refactor and expensive to notice in
// production:
//   1. retention must delete only rows that already reached the transport, and
//      it must be bounded per run, observable and supervised;
//   2. the event-bus metrics must be produced by the runtime, not only by the
//      telemetry crate's own tests, otherwise a dashboard on those names reads
//      zero forever.

import { readFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function readRepo(relativePath) {
  try {
    return readFileSync(path.join(repoRoot, relativePath), "utf8");
  } catch {
    failures.push(`${relativePath}: source file is missing`);
    return "";
  }
}

function requireText(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) failures.push(description);
}

// --- owner: bounded retention over delivered rows ---------------------------
const retention = readRepo("crates/modules/rustok-outbox/src/retention.rs");
requireText(retention, "pub struct OutboxRetentionConfig", "retention configuration type");
requireText(retention, "pub struct OutboxRetention {", "owner retention pruner");
requireText(retention, "pub async fn prune_once(", "owner retention prune entry point");
requireText(
  retention,
  "pub async fn prune_once(&self) -> Result<OutboxPruneReport>",
  "retention prune signature",
);
requireText(retention, "self.config.validate()?;", "retention prune must validate its config");
requireText(retention, "pub fn validate(&self) -> Result<()>", "retention config validation");
requireText(
  retention,
  "rustok_telemetry::metrics::record_outbox_pruned(deleted);",
  "retention prune must export how many rows it removed",
);
requireText(
  retention,
  "rustok_telemetry::metrics::record_outbox_retention_run();",
  "retention prune must export that a run completed",
);
const pruneBody = retention.slice(
  retention.indexOf("pub async fn prune_once"),
  retention.indexOf("#[cfg(test)]"),
);
requireText(
  pruneBody,
  "candidate_ids.len() as u64 >= self.config.batch_size",
  "retention prune must report whether it filled its batch",
);

const outboxLib = readRepo("crates/modules/rustok-outbox/src/lib.rs");
requireText(outboxLib, "pub mod retention;", "outbox retention module export");
for (const ownerExport of [
  "DEFAULT_OUTBOX_RETENTION_BATCH_SIZE",
  "OutboxPruneReport",
  "OutboxRetention",
  "OutboxRetentionConfig",
]) {
  requireText(outboxLib, ownerExport, `outbox retention re-export ${ownerExport}`);
}

// --- telemetry: the retention metric pair is registered ---------------------
const telemetryMetrics = readRepo("crates/libs/rustok-telemetry/src/metrics.rs");
requireText(
  telemetryMetrics,
  'create_int_counter(\n        "rustok_outbox_pruned_total"',
  "outbox pruned counter definition",
);
requireText(
  telemetryMetrics,
  '"rustok_outbox_retention_last_run_timestamp_seconds"',
  "outbox retention run gauge definition",
);
requireText(
  telemetryMetrics,
  "registry.register(Box::new(OUTBOX_PRUNED_TOTAL.clone()))?;",
  "outbox pruned counter registration",
);
requireText(
  telemetryMetrics,
  "registry.register(Box::new(OUTBOX_RETENTION_LAST_RUN_TIMESTAMP_SECONDS.clone()))?;",
  "outbox retention run gauge registration",
);
requireText(
  telemetryMetrics,
  "pub fn record_outbox_pruned(count: u64)",
  "outbox pruned telemetry helper",
);
requireText(
  telemetryMetrics,
  "pub fn record_outbox_retention_run()",
  "outbox retention run telemetry helper",
);

// --- server: retention is configured, supervised and observable -------------
const settings = readRepo("apps/server/src/common/settings.rs");
requireText(
  settings,
  "pub outbox_retention: OutboxRetentionSettings,",
  "events settings must expose the retention policy",
);
requireText(
  settings,
  "pub struct OutboxRetentionSettings {",
  "retention settings type",
);
requireText(settings, "fn default_outbox_retention_enabled() -> bool {", "retention default switch");
requireText(
  settings,
  "fn default_outbox_retention_batch_size() -> u64 {\n    rustok_outbox::DEFAULT_OUTBOX_RETENTION_BATCH_SIZE",
  "retention batch default must come from the owner bound",
);

const factory = readRepo("apps/server/src/services/event_transport_factory.rs");
requireText(
  factory,
  "let outbox_retention = resolve_outbox_retention(ctx, &settings.events.outbox_retention)?;",
  "event runtime must resolve the retention policy from settings",
);
requireText(
  factory,
  "pub outbox_retention: Option<OutboxRetentionRuntimeConfig>,",
  "event runtime must carry the resolved retention policy",
);
requireText(
  factory,
  "const MAX_OUTBOX_RETENTION_DAYS: u64 = 3_650;",
  "retention window must be bounded at boot",
);
requireText(
  factory,
  "rustok_outbox::OutboxRetention::new(ctx.db_clone()).with_config(retention)",
  "resolved retention must be built by the owner",
);

const worker = readRepo("apps/server/src/services/outbox_retention_worker.rs");
requireText(
  worker,
  "pub fn spawn_outbox_retention_worker(",
  "retention worker spawn entry point",
);
requireText(
  worker,
  "match config.retention.prune_once().await {",
  "retention worker must call the owner prune",
);
requireText(
  worker,
  "OUTBOX_RETENTION_FAILURE_TOTAL.fetch_add(1, Ordering::Relaxed);",
  "a failed prune run must be counted",
);
requireText(
  worker,
  "pub fn outbox_retention_supervisor_metrics_snapshot()",
  "retention supervisor metrics snapshot",
);
const workerLoop = worker.slice(
  worker.indexOf("async fn supervise_outbox_retention"),
  worker.indexOf("#[cfg(test)]"),
);
requireText(
  workerLoop,
  "if *stop_rx.borrow() {",
  "retention worker must observe the shutdown signal before a run",
);
requireText(
  workerLoop,
  "changed = stop_rx.changed() => {",
  "retention worker must wake up on shutdown between runs",
);
if (!workerLoop.includes("tokio::select!")) {
  failures.push("retention worker must not sleep through a shutdown signal");
}

const lifecycle = readRepo("apps/server/src/services/app_lifecycle.rs");
requireText(
  lifecycle,
  "pub struct OutboxRetentionWorkerHandle {",
  "retention worker lifecycle handle",
);
requireText(
  lifecycle,
  "if !runtime_ctx.shared_contains::<OutboxRetentionWorkerHandle>() {",
  "retention worker must be spawned once per runtime",
);
requireText(
  lifecycle,
  ".and_then(|runtime| runtime.outbox_retention.clone());",
  "retention worker must only start when retention is enabled",
);
requireText(
  lifecycle,
  "_handle: spawn_outbox_retention_worker(retention_config, stop_rx),",
  "retention worker must be spawned through the owner supervisor",
);

const health = readRepo("apps/server/src/controllers/health.rs");
requireText(health, '"worker:outbox_retention",', "retention worker readiness check");
requireText(
  health,
  ".and_then(|runtime| runtime.outbox_retention.clone())",
  "readiness must require the retention worker only when retention is enabled",
);

const serverMetrics = readRepo("apps/server/src/controllers/metrics.rs");
requireText(
  serverMetrics,
  "update_queue_depth(\"outbox\", i64::try_from(backlog).unwrap_or(i64::MAX));",
  "the durable bus backlog must feed the event-bus queue-depth gauge",
);
requireText(
  serverMetrics,
  '"rustok_runtime_worker_failures_total{{worker=\\"outbox_retention\\"}} {failure_total}\\n"',
  "retention failures must be exported",
);
requireText(
  serverMetrics,
  '"outbox_retention",\n        retention_required,',
  "retention worker state must be exported",
);

// --- runtime: the event-bus metric family is produced by production code ----
const eventBus = readRepo("crates/libs/rustok-core/src/events/bus.rs");
requireText(
  eventBus,
  "rustok_telemetry::metrics::record_event_published(",
  "event bus publish path must record published events",
);

const dispatcher = readRepo("crates/libs/rustok-core/src/events/handler.rs");
requireText(
  dispatcher,
  "rustok_telemetry::metrics::record_event_dispatched(event_type, handler_name);",
  "handler dispatch must record dispatched events",
);
requireText(
  dispatcher,
  "rustok_telemetry::metrics::record_event_processing_duration(",
  "handler dispatch must record processing duration",
);
requireText(
  dispatcher,
  "consumer_runtime.record_publish_lag(&event_type, envelope.timestamp);",
  "dispatch must record publish-to-dispatch lag",
);

const consumer = readRepo("crates/libs/rustok-core/src/events/consumer.rs");
requireText(
  consumer,
  "rustok_telemetry::metrics::record_event_lag(event_type, lag_secs);",
  "publish lag must be exported through the event-bus lag histogram",
);

if (failures.length > 0) {
  console.error("outbox retention and event metric verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("outbox retention and event metric verification passed");
