#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';

import {
  CAPTURE_CONTRACT,
  DESCRIPTOR_FILE,
  END_FILE,
  MAX_SNAPSHOT_BYTES,
  START_FILE,
  absolutePath,
  analyzeWindow,
  artifactDescriptor,
  canonicalRegularFile,
  ensure,
  ensureAbsent,
  ensureInventory,
  parseSnapshot,
  parseUtc,
  readStableRegularFile,
  runnerIdentity,
  validateCommit,
  validateRepository,
  validateRunKey,
  validateWindow,
  verifySourceIdentity,
  writeJsonNew,
  writeNewFile,
} from './lib/social-graph-privacy-shadow-evidence.mjs';

const CAPTURE_OPT_IN = 'SOCIAL_GRAPH_PRIVACY_SHADOW_ALLOW_CAPTURE';
const START_INPUT_ENV = 'SOCIAL_GRAPH_PRIVACY_SHADOW_START_PROM';
const END_INPUT_ENV = 'SOCIAL_GRAPH_PRIVACY_SHADOW_END_PROM';
const OUTPUT_ROOT_ENV = 'SOCIAL_GRAPH_PRIVACY_SHADOW_OUTPUT_ROOT';
const COMMIT_ENV = 'SOCIAL_GRAPH_PRIVACY_SHADOW_COMMIT';
const RUN_KEY_ENV = 'SOCIAL_GRAPH_PRIVACY_SHADOW_RUN_KEY';
const REPOSITORY_ENV = 'SOCIAL_GRAPH_PRIVACY_SHADOW_REPOSITORY';
const WORKSPACE_ENV = 'SOCIAL_GRAPH_PRIVACY_SHADOW_WORKSPACE_ROOT';
const WINDOW_STARTED_AT_ENV = 'SOCIAL_GRAPH_PRIVACY_SHADOW_WINDOW_STARTED_AT';
const WINDOW_ENDED_AT_ENV = 'SOCIAL_GRAPH_PRIVACY_SHADOW_WINDOW_ENDED_AT';

function required(name) {
  const value = process.env[name];
  ensure(value !== undefined && value.trim() !== '', `${name} is required`);
  return value.trim();
}

function loadConfig() {
  ensure(process.env[CAPTURE_OPT_IN] === '1', `${CAPTURE_OPT_IN}=1 is required`);
  const workspaceRoot = fs.realpathSync(absolutePath(process.env[WORKSPACE_ENV] || process.cwd()));
  const workspaceMetadata = fs.lstatSync(workspaceRoot);
  ensure(workspaceMetadata.isDirectory() && !workspaceMetadata.isSymbolicLink(), 'workspace must be a regular non-symlink directory');
  ensure(fs.statSync(path.join(workspaceRoot, 'Cargo.toml')).isFile(), 'workspace must contain Cargo.toml');
  ensure(fs.existsSync(path.join(workspaceRoot, '.git')), 'workspace must be a Git checkout');

  const startInput = canonicalRegularFile(absolutePath(required(START_INPUT_ENV), workspaceRoot), 'start snapshot');
  const endInput = canonicalRegularFile(absolutePath(required(END_INPUT_ENV), workspaceRoot), 'end snapshot');
  ensure(startInput !== endInput, 'start and end snapshots must be different files');

  const commit = required(COMMIT_ENV);
  validateCommit(commit, COMMIT_ENV);
  const runKey = required(RUN_KEY_ENV);
  validateRunKey(runKey, RUN_KEY_ENV);
  const repository = process.env[REPOSITORY_ENV]?.trim() || 'RusTokRs/RusTok';
  validateRepository(repository, REPOSITORY_ENV);

  const startedAt = parseUtc(required(WINDOW_STARTED_AT_ENV), WINDOW_STARTED_AT_ENV);
  const endedAt = parseUtc(required(WINDOW_ENDED_AT_ENV), WINDOW_ENDED_AT_ENV);
  const durationSeconds = validateWindow(startedAt, endedAt);
  const outputRoot = absolutePath(
    process.env[OUTPUT_ROOT_ENV]?.trim()
      || path.join('target', 'social-graph-privacy-shadow-evidence', runKey),
    workspaceRoot,
  );
  ensure(path.basename(outputRoot) !== '' && path.basename(outputRoot) !== '.', 'output root must have a final path component');

  return {
    workspaceRoot,
    startInput,
    endInput,
    outputRoot,
    repository,
    commit,
    runKey,
    window: {
      started_at: startedAt.toISOString(),
      ended_at: endedAt.toISOString(),
      duration_seconds: durationSeconds,
    },
  };
}

function publishBundle(config, descriptor, startBytes, endBytes) {
  ensureAbsent(config.outputRoot, 'privacy-shadow evidence output');
  const parent = path.dirname(config.outputRoot);
  fs.mkdirSync(parent, { recursive: true });
  const parentMetadata = fs.lstatSync(parent);
  ensure(parentMetadata.isDirectory() && !parentMetadata.isSymbolicLink(), 'output parent must be a regular non-symlink directory');
  const finalRoot = path.join(fs.realpathSync(parent), path.basename(config.outputRoot));
  ensureAbsent(finalRoot, 'privacy-shadow evidence output');
  fs.mkdirSync(finalRoot);

  try {
    writeNewFile(path.join(finalRoot, START_FILE), startBytes, 'privacy-shadow start snapshot');
    writeNewFile(path.join(finalRoot, END_FILE), endBytes, 'privacy-shadow end snapshot');
    ensureInventory(finalRoot, [END_FILE, START_FILE]);
    writeJsonNew(path.join(finalRoot, DESCRIPTOR_FILE), descriptor, 'privacy-shadow capture descriptor');
    ensureInventory(finalRoot, [DESCRIPTOR_FILE, END_FILE, START_FILE]);
    return finalRoot;
  } catch (error) {
    fs.rmSync(finalRoot, { recursive: true, force: true });
    throw error;
  }
}

function main() {
  const config = loadConfig();
  ensureAbsent(config.outputRoot, 'privacy-shadow evidence output');
  verifySourceIdentity(config.workspaceRoot, config.commit);
  const startBytes = readStableRegularFile(config.startInput, MAX_SNAPSHOT_BYTES, 'start Prometheus snapshot');
  const endBytes = readStableRegularFile(config.endInput, MAX_SNAPSHOT_BYTES, 'end Prometheus snapshot');
  verifySourceIdentity(config.workspaceRoot, config.commit);

  const metrics = analyzeWindow(parseSnapshot(startBytes), parseSnapshot(endBytes), config.window);
  const descriptor = {
    contract: CAPTURE_CONTRACT,
    completed_at: new Date().toISOString(),
    source: {
      repository: config.repository,
      commit: config.commit,
      run_key: config.runKey,
      clean_worktree: true,
    },
    runner: runnerIdentity(
      ['SOCIAL_GRAPH_PRIVACY_SHADOW_CAPTURE_JOB', 'GITHUB_JOB'],
      'social-graph-privacy-shadow-capture',
    ),
    window: config.window,
    start: artifactDescriptor(START_FILE, startBytes),
    end: artifactDescriptor(END_FILE, endBytes),
    metrics,
    authority: {
      measurement_only: true,
      owner_result_authoritative: true,
      authoritative_cutover_authorized: false,
    },
  };

  const outputRoot = publishBundle(config, descriptor, startBytes, endBytes);
  console.log(
    `privacy shadow evidence capture complete: commit=${config.commit} run_key=${config.runKey} observations=${metrics.totals.observations} output=${outputRoot}`,
  );
}

try {
  main();
} catch (error) {
  console.error(`[capture-social-graph-privacy-shadow] ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
}
