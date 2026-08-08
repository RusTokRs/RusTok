#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-mode-contract] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const modePath = 'crates/rustok-index/src/application/replay_mode.rs';
const mode = requireMarkers(modePath, [
  'pub enum IndexReplayMode {',
  'Full,',
  'Targeted,',
  'Shadow,',
  'pub enum IndexReplayExecutionSurface {',
  'DurableScan,',
  'TargetedLoad,',
  'SideEffectFreeScan,',
  'pub enum IndexReplayModeSelection {',
  'Targeted(IndexSourceLoadRequest)',
  'IndexSourceLoadRequest::new(keys)?',
  'IndexReplayExecutionSurface::DurableScan',
  'IndexReplayExecutionSurface::TargetedLoad',
  'IndexReplayExecutionSurface::SideEffectFreeScan',
  'pub fn is_admitted_to_durable_scan_runner(&self) -> bool',
  'matches!(self, Self::Full)',
  'targeted_reuses_the_canonical_bounded_load_request',
  'targeted_rejects_empty_duplicate_and_mixed_scope_keys',
  'shadow_routes_only_to_the_side_effect_free_scan_surface',
]);
for (const forbidden of [
  'PostgresIndexReplayJobStore',
  'PostgresIndexReplayCheckpointStore',
  'PostgresMutationStore',
  'DatabaseConnection',
  'partition_key',
  'request_cancel',
  'Retrying',
  'auto_requeue',
]) {
  if (mode.includes(forbidden)) fail(`${modePath} must remain an application mode/routing contract: ${forbidden}`);
}

requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'mod replay_mode;',
  'IndexReplayExecutionSurface, IndexReplayMode, IndexReplayModeSelection',
]);

const runnerPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_runner.rs';
const runner = read(runnerPath);
for (const forbidden of ['IndexReplayMode::Targeted', 'IndexReplayMode::Shadow', 'TargetedLoad', 'SideEffectFreeScan']) {
  if (runner.includes(forbidden)) {
    fail(`${runnerPath} must remain the durable full-scan runner: ${forbidden}`);
  }
}

const dryRunPath = 'crates/rustok-index/src/replay_dry_run.rs';
requireMarkers(dryRunPath, [
  'No mutation, inbox delivery, job, checkpoint, or reconciliation progress is persisted',
  'pub struct SharedIndexReplayDryRunRuntime',
  '.scan(scan_request)',
]);

requireMarkers('apps/server/src/services/index_replay_runtime_composition.rs', [
  'pub async fn run_shadow(',
  'shadow: rustok_index::SharedIndexReplayDryRunRuntime',
  'context.authorize_for(request.tenant_id())?;',
  'self.shadow.run(request).await.map_err(Into::into)',
]);

requireMarkers('crates/rustok-index/docs/m6-replay-mode-contract.md', [
  'Status: `source_complete_shadow_host_dispatch_transport_pending`.',
  '`Full` — cursor-based durable source scan',
  '`Targeted` — bounded exact-key source load',
  '`Shadow` — side-effect-free cursor scan',
  'Mode is not locale scope and is not future partition scope.',
  'returns true only for `Full`',
  'must not alias the full durable job/checkpoint identity',
  '`Shadow` host dispatch is now source-complete',
  '`IndexReplayOperatorRuntime::run_shadow`',
  'GraphQL transport remains separate',
  'The next source-only boundary is authorization-first GraphQL transport',
  'Execution/admission remains maintainer-owned.',
]);

requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-08.md', [
  'Define explicit Full/Targeted/Shadow replay mode identity and fail-closed execution surfaces.',
  'Guard the existing side-effect-free Shadow replay runtime behind the request-bound `modules:manage` operator boundary.',
  'Add authorization-first GraphQL transport for the guarded Shadow replay command.',
  'Targeted execution remains separate until a bounded mutation-application contract over `IndexSource::load` exists.',
  'Add partition replay scope only after a real partition-capable source contract exists.',
]);

console.log('[verify-index-replay-mode-contract] Full stays durable, Targeted stays bounded-load-only, and Shadow now has a guarded no-write host route without public transport');
