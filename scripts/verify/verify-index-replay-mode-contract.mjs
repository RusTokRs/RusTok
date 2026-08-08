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
    fail(`${runnerPath} must remain the durable full-scan runner until a separate dispatcher is added: ${forbidden}`);
  }
}

const dryRunPath = 'crates/rustok-index/src/replay_dry_run.rs';
requireMarkers(dryRunPath, [
  'No mutation, inbox delivery, job, checkpoint, or reconciliation progress is persisted',
  'pub struct SharedIndexReplayDryRunRuntime',
  '.scan(scan_request)',
]);

requireMarkers('crates/rustok-index/docs/m6-replay-mode-contract.md', [
  'Status: `source_complete_execution_routing_pending`.',
  '`Full` — cursor-based durable source scan',
  '`Targeted` — bounded exact-key source load',
  '`Shadow` — side-effect-free cursor scan',
  'Mode is not locale scope and is not future partition scope.',
  'returns true only for `Full`',
  'must not alias the full durable job/checkpoint identity',
  'The current `IndexReplayRunRequest`, `PostgresIndexReplayRunner`, `IndexReplayOperatorRuntime` and GraphQL',
  'does not add caller-controlled mode input',
  'next source-only boundary is request-bound host dispatch',
  'Execution/admission remains maintainer-owned.',
]);

requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-08.md', [
  'Define explicit Full/Targeted/Shadow replay mode identity and fail-closed execution surfaces.',
  'Add request-bound host dispatch for the existing side-effect-free Shadow replay surface.',
  'Targeted execution remains separate until a bounded mutation-application contract over `IndexSource::load` exists.',
  'Add partition replay scope only after a real partition-capable source contract exists.',
]);

console.log('[verify-index-replay-mode-contract] full, targeted and shadow modes remain explicit, bounded and routed to non-aliasing execution surfaces');
