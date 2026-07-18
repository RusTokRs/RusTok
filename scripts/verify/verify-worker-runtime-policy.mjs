#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const file = path.join(repoRoot, relativePath);
  if (!fs.existsSync(file)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  const stats = fs.lstatSync(file);
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${relativePath}: must be a regular non-symlink file`);
    return "";
  }
  return fs.readFileSync(file, "utf8");
}

function requireMarkers(relativePath, markers) {
  const source = read(relativePath);
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${relativePath}: missing marker ${marker}`);
  }
}

function forbidMarkers(relativePath, markers) {
  const source = read(relativePath);
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${relativePath}: forbidden marker ${marker}`);
  }
}

function requireCount(relativePath, marker, expected) {
  const actual = read(relativePath).split(marker).length - 1;
  if (actual !== expected) {
    failures.push(`${relativePath}: expected ${expected} occurrence(s) of ${marker}, found ${actual}`);
  }
}

requireMarkers("crates/rustok-worker-transport/Cargo.toml", ["tokio.workspace = true"]);
requireMarkers("crates/rustok-worker-transport/src/lib.rs", [
  "pub admission_timeout: Duration",
  'parse_duration_ms(prefix, "ADMISSION_TIMEOUT_MS", 250)',
  "must not exceed REQUEST_TIMEOUT_MS",
  "pub struct WorkerAdmission",
  "Arc<Semaphore>",
  "pub async fn acquire(&self) -> Result<WorkerPermit, Status>",
  "Status::resource_exhausted",
  "Status::unavailable",
  "pub async fn shutdown_signal()",
  "SignalKind::terminate()",
  "tokio::signal::ctrl_c()",
  "failed to install SIGTERM handler; stopping worker",
  "admission_sheds_after_bounded_wait",
]);
forbidMarkers("crates/rustok-worker-transport/src/lib.rs", [
  "Semaphore::new(usize::MAX)",
  "Duration::ZERO",
  "unwrap()",
]);

requireMarkers("crates/rustok-verification-transport/src/server.rs", [
  "admission: WorkerAdmission",
  "pub fn new(verifier: Arc<dyn ArtifactVerifier>, admission: WorkerAdmission)",
  "let _permit = self.admission.acquire().await?;",
]);
requireCount(
  "crates/rustok-verification-transport/src/server.rs",
  "self.admission.acquire().await?",
  1,
);
requireMarkers("crates/rustok-verification-worker/src/main.rs", [
  "WorkerAdmission::from_listener(&listener)",
  "VerificationGrpcService::new(worker, admission)",
  ".serve_with_shutdown(listener.bind_addr, shutdown_signal())",
]);
forbidMarkers("crates/rustok-verification-worker/src/main.rs", [
  ".serve(listener.bind_addr)",
]);

requireMarkers("crates/rustok-module-build-worker/src/admission.rs", [
  "pub struct AdmissionRunnerGrpcService",
  "admission: WorkerAdmission",
  "let _permit = self.admission.acquire().await?;",
  "RunnerService::start_build",
  "RunnerService::get_readiness",
]);
requireCount(
  "crates/rustok-module-build-worker/src/admission.rs",
  "self.admission.acquire().await?",
  1,
);
requireMarkers("crates/rustok-module-build-worker/src/main.rs", [
  "WorkerAdmission::from_listener(&listener)",
  "AdmissionRunnerGrpcService::new(",
  ".serve_with_shutdown(listener.bind_addr, shutdown_signal())",
]);
forbidMarkers("crates/rustok-module-build-worker/src/main.rs", [
  ".serve(listener.bind_addr)",
]);
requireMarkers("crates/rustok-module-build-worker/src/lib.rs", [
  "mod admission;",
  "pub use admission::AdmissionRunnerGrpcService;",
]);

requireCount(
  "crates/rustok-verification-worker/src/cosign.rs",
  "kill_on_drop(true)",
  2,
);
requireMarkers("crates/rustok-module-build-worker/src/runner.rs", [
  "command.kill_on_drop(true);",
  "timeout(timeout_window, command.output())",
]);
requireCount(
  "crates/rustok-module-build-worker/src/runner.rs",
  "kill_on_drop(true)",
  1,
);

requireMarkers(".github/workflows/hardening-gates.yml", [
  "Verify worker runtime backpressure policy",
  "verify-worker-runtime-policy.mjs",
]);
requireMarkers("scripts/verify/verify-all.sh", [
  "worker-runtime-policy  Verify bounded admission, graceful shutdown and subprocess cancellation",
  "verify-worker-runtime-policy.mjs:Worker Runtime Backpressure Policy",
]);

for (const temporaryWorkflow of [
  ".github/workflows/one-off-kill-cancelled-build-processes.yml",
  ".github/workflows/one-off-pin-release-actions.yml",
]) {
  if (fs.existsSync(path.join(repoRoot, temporaryWorkflow))) {
    failures.push(`${temporaryWorkflow}: temporary privileged workflow must not remain`);
  }
}

if (failures.length > 0) {
  console.error("Worker runtime policy verification failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ worker admission is bounded, readiness remains available, hosts stop gracefully, and cancelled subprocesses are killed",
);
