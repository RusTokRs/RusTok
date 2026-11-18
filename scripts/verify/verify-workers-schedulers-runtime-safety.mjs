import fs from "node:fs";

const failures = [];

function requirePattern(source, pattern, message) {
  if (!pattern.test(source)) {
    failures.push(message);
  }
}

// ---------------------------------------------------------------------------
// CC-09.01 — Non-Blocking Async Invariant
// ---------------------------------------------------------------------------

// 1. apps/server host and manifest operations must use tokio::fs
const serverHost = fs.readFileSync("apps/server/src/host.rs", "utf8");
requirePattern(
  serverHost,
  /tokio::fs::read_to_string/,
  "CC-09.01: apps/server host must use tokio::fs::read_to_string for non-blocking file reads",
);

// 2. build worker runner must use tokio::fs
const buildRunner = fs.readFileSync(
  "crates/workers/rustok-module-build-worker/src/runner.rs",
  "utf8",
);
requirePattern(
  buildRunner,
  /tokio::fs::create_dir_all[\s\S]*?tokio::fs::read/,
  "CC-09.01: rustok-module-build-worker must use tokio::fs for asynchronous filesystem I/O",
);

// 3. modules source object must use tokio::fs
const sourceObject = fs.readFileSync(
  "crates/modules/rustok-modules/src/source_object.rs",
  "utf8",
);
requirePattern(
  sourceObject,
  /tokio::fs::create_dir_all/,
  "CC-09.01: rustok-modules source object store must use tokio::fs::create_dir_all",
);
requirePattern(
  sourceObject,
  /tokio::fs::File::open/,
  "CC-09.01: rustok-modules source object store must use tokio::fs::File::open",
);
requirePattern(
  sourceObject,
  /tokio::fs::remove_file/,
  "CC-09.01: rustok-modules source object store must use tokio::fs::remove_file",
);

// ---------------------------------------------------------------------------
// CC-09.02 — Graceful Worker Drain
// ---------------------------------------------------------------------------

// 1. app_lifecycle shutdown_runtime_workers coordinates graceful drain
const appLifecycle = fs.readFileSync(
  "apps/server/src/services/app_lifecycle.rs",
  "utf8",
);
requirePattern(
  appLifecycle,
  /pub async fn shutdown_runtime_workers\([\s\S]*?StopHandle::ensure\(runtime_ctx\);[\s\S]*?stop\.stop\(\)\.await;/,
  "CC-09.02: shutdown_runtime_workers must signal StopHandle before draining workers",
);
requirePattern(
  appLifecycle,
  /OutboxRelayWorkerHandle[\s\S]*?OutboxRetentionWorkerHandle[\s\S]*?AuthRetentionWorkerHandle[\s\S]*?ModuleWorkSchedulerHandle[\s\S]*?ModuleEventDispatcherHandle/,
  "CC-09.02: shutdown_runtime_workers must drain outbox relay, retention, auth retention, scheduler, and dispatcher workers",
);

// 2. worker transport shutdown signal
const workerTransport = fs.readFileSync(
  "crates/workers/rustok-worker-transport/src/lib.rs",
  "utf8",
);
requirePattern(
  workerTransport,
  /pub async fn shutdown_signal\(\)/,
  "CC-09.02: rustok-worker-transport must expose shutdown_signal for gRPC server drain",
);

// 3. gRPC workers serve_with_shutdown
const moduleBuildWorkerMain = fs.readFileSync(
  "crates/workers/rustok-module-build-worker/src/main.rs",
  "utf8",
);
requirePattern(
  moduleBuildWorkerMain,
  /\.serve_with_shutdown\(listener\.address,\s*shutdown_signal\(\)\)/,
  "CC-09.02: rustok-module-build-worker must use serve_with_shutdown(..., shutdown_signal())",
);

// ---------------------------------------------------------------------------
// CC-09.03 — Concurrency Limiting
// ---------------------------------------------------------------------------

// 1. event dispatcher concurrency and queue bounding
const eventHandler = fs.readFileSync(
  "crates/libs/rustok-core/src/events/handler.rs",
  "utf8",
);
requirePattern(
  eventHandler,
  /let semaphore = Arc::new\(Semaphore::new\(max_concurrent\)\);[\s\S]*?let queue = Arc::new\(Semaphore::new\(queue_capacity\)\);/,
  "CC-09.03: rustok-core event handler must bound listener concurrency and queue capacity using semaphores",
);

// 2. resilience bulkhead concurrency bounding
const bulkhead = fs.readFileSync(
  "crates/libs/rustok-core/src/resilience/bulkhead.rs",
  "utf8",
);
requirePattern(
  bulkhead,
  /let semaphore = Arc::new\(Semaphore::new\(config\.max_concurrent_calls\)\);/,
  "CC-09.03: rustok-core bulkhead must bound concurrent calls with Semaphore",
);

// 3. worker admission concurrency limit
requirePattern(
  workerTransport,
  /permits:\s*Arc::new\(Semaphore::new\(concurrency_limit\)\)/,
  "CC-09.03: rustok-worker-transport must bound worker admission concurrency with Semaphore",
);

// ---------------------------------------------------------------------------
// CC-09.04 — Sandbox Isolation
// ---------------------------------------------------------------------------

// 1. sandbox policy limits
const sandboxPolicy = fs.readFileSync(
  "crates/workers/rustok-sandbox/src/policy.rs",
  "utf8",
);
requirePattern(
  sandboxPolicy,
  /pub struct SandboxLimits\s*\{[\s\S]*?pub wall_clock_ms:\s*u64,[\s\S]*?pub instruction_budget:\s*u64,[\s\S]*?pub max_memory_bytes:\s*u64,[\s\S]*?pub max_output_bytes:\s*u64,[\s\S]*?pub max_concurrency:\s*u32,/,
  "CC-09.04: rustok-sandbox SandboxLimits must declare wall_clock_ms, instruction_budget, max_memory_bytes, max_output_bytes, max_concurrency",
);

// 2. rhai engine instruction and execution limits
const sandboxRhai = fs.readFileSync(
  "crates/workers/rustok-sandbox/src/rhai.rs",
  "utf8",
);
requirePattern(
  sandboxRhai,
  /engine\.set_max_operations\(limits\.instruction_budget\);[\s\S]*?engine\.set_max_call_levels\(16\);[\s\S]*?engine\.set_max_string_size[\s\S]*?engine\.set_max_array_size[\s\S]*?engine\.set_max_map_size/,
  "CC-09.04: rustok-sandbox Rhai engine must configure strict operation, call level, string, array, and map bounds",
);

// ---------------------------------------------------------------------------
// Verification outcome
// ---------------------------------------------------------------------------
if (failures.length > 0) {
  console.error("Background workers and async runtime safety verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("Background workers, schedulers, and async runtime safety verification passed successfully!");
