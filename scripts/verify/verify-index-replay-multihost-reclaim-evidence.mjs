#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-multihost-reclaim-evidence] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const packetPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_multihost_restart_tests.rs';
const packet = requireMarkers(packetPath, [
  'struct BlockingFirstScanSource',
  'first_host_scan_started: Arc<Notify>',
  'release_first_host_scan: Arc<Notify>',
  'let host_a = PostgresIndexReplayRunner::new(',
  'let host_b = PostgresIndexReplayRunner::new(',
  'async fn expired_host_is_reclaimed_by_second_runner_and_stale_host_cannot_publish()',
  'let host_a_task = tokio::spawn(async move { host_a.run(host_a_request).await });',
  'fixture.first_host_scan_started.notified().await;',
  "lease_expires_at = datetime('now', '-1 second')",
  '.run(fixture.request("host-b"))',
  'assert_eq!(second.status(), IndexReplayRunStatus::Complete);',
  'assert_eq!(second.attempt_count(), Some(2));',
  'fixture.release_first_host_scan.notify_one();',
  'IndexReplayRunError::LeaseLost {',
  'assert_eq!(attempt_count, 1);',
  "state = 'succeeded' AND attempt_count = 2",
  "CAST(cursor AS TEXT) = 'null'",
  "source_name = 'multihost-owner-primary'",
  "module_name = 'multihost-owner'",
]);

for (const forbidden of [
  'tokio::time::sleep',
  'std::thread::sleep',
  'interval(',
  'sleep_until(',
  'while !',
  'PostgresIndexReplayJobStore',
  'IndexReplayJobLeaseRequest',
  'INSERT INTO index_jobs',
  'INSERT INTO index_checkpoints',
  '.request_cancel(',
  '.run_interruptible(',
]) {
  if (packet.includes(forbidden)) {
    fail(`${packetPath} must retain runner-owned deterministic reclaim evidence without direct job/checkpoint lifecycle manipulation: ${forbidden}`);
  }
}

const hostAStart = packet.indexOf('let host_a_task = tokio::spawn(async move { host_a.run(host_a_request).await });');
const scanStarted = packet.indexOf('fixture.first_host_scan_started.notified().await;', hostAStart);
const expiry = packet.indexOf("lease_expires_at = datetime('now', '-1 second')", scanStarted);
const hostB = packet.indexOf('.run(fixture.request("host-b"))', expiry);
const hostBComplete = packet.indexOf('assert_eq!(second.status(), IndexReplayRunStatus::Complete);', hostB);
const releaseA = packet.indexOf('fixture.release_first_host_scan.notify_one();', hostBComplete);
const staleFence = packet.indexOf('IndexReplayRunError::LeaseLost {', releaseA);
if (
  hostAStart < 0 ||
  scanStarted <= hostAStart ||
  expiry <= scanStarted ||
  hostB <= expiry ||
  hostBComplete <= hostB ||
  releaseA <= hostBComplete ||
  staleFence <= releaseA
) {
  fail('packet order must remain host-a in-flight -> deterministic expiry -> host-b attempt-2 completion -> release host-a -> stale LeaseLost fence');
}

requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  '#[cfg(test)]\nmod source_replay_multihost_restart_tests;'.replace('\\n', '\n'),
]);

const jobPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_job.rs';
const job = requireMarkers(jobPath, [
  "state = 'running' AND lease_expires_at <= CURRENT_TIMESTAMP",
  'attempt_count = {prefix}4',
  'lease_owner = {prefix}3',
  'lease_expires_at > CURRENT_TIMESTAMP',
]);
if (job.includes('distributed_consensus')) {
  fail(`${jobPath} must not introduce a second ownership mechanism for this evidence slice`);
}

requireMarkers('crates/rustok-index/src/infrastructure/postgres/source_replay_runner.rs', [
  'IndexReplayRunError::LeaseLost',
  'checkpoint_lease_lost',
  'terminal_write_outcome',
  'await_page_with_lease_heartbeats(',
]);

requireMarkers('crates/rustok-index/docs/m6-replay-multihost-reclaim-evidence.md', [
  'Status: `source_complete_execution_pending`.',
  'two distinct `PostgresIndexReplayRunner` instances',
  'host B invokes the ordinary runner for the same replay scope',
  'host A observes the same stable delivery',
  '`IndexReplayRunError::LeaseLost`',
  'does not insert a replay job',
  'source-only multi-host/restart boundary',
  'Production execution/admission remains maintainer-owned.',
]);

requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-08.md', [
  'Retain deterministic two-host lease-expiry/reclaim/stale-owner fencing evidence through distinct replay runners.',
  'Execute/admit retained multi-host reclaim evidence.',
  'Define explicit Full/Targeted/Shadow replay mode identity and fail-closed execution surfaces.',
  'Guard the existing side-effect-free Shadow replay runtime behind the request-bound `modules:manage` operator boundary.',
  'Add authorization-first schema-wide GraphQL transport for guarded Shadow replay with sealed caller-carried continuation.',
  'Make Shadow continuation identity locale-safe before exposing exact-locale Shadow GraphQL transport.',
  'Add exact-locale Shadow dry-run/runtime/GraphQL execution using the canonical locale-safe continuation scope.',
  'Define a bounded Targeted mutation-application contract over `IndexSource::load` without aliasing durable scan ownership.',
  'Partition replay remains blocked',
]);

console.log('[verify-index-replay-multihost-reclaim-evidence] deterministic two-host reclaim keeps attempt-2 durable state authoritative and fences the late attempt-1 runner');
