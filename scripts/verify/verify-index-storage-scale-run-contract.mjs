#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const workflow = readFileSync(
  new URL('../../.github/workflows/index-storage-scale-run.yml', import.meta.url),
  'utf8',
);
const prefix = '[verify-index-storage-scale-run-contract]';
const fail = (message) => {
  console.error(`${prefix} ${message}`);
  process.exit(1);
};

const validateStart = workflow.indexOf('  validate-inputs:\n');
const evidenceStart = workflow.indexOf('  evidence:\n');
if (validateStart < 0) fail('reusable scale workflow is missing validate-inputs job');
if (evidenceStart < 0) fail('reusable scale workflow is missing evidence job');
if (validateStart > evidenceStart) {
  fail('validate-inputs job must appear before the evidence job');
}

const validateBlock = workflow.slice(validateStart, evidenceStart);
for (const marker of [
  '    runs-on: ubuntu-slim',
  '    timeout-minutes: 5',
  '          REQUESTED_SCALE: ${{ inputs.scale }}',
  '          REQUESTED_TIMEOUT_MINUTES: ${{ inputs.timeout_minutes }}',
  '          REQUESTED_RUNNER_LABEL: ${{ inputs.runner_label }}',
  '          REQUESTED_MINIMUM_FREE_BYTES: ${{ inputs.minimum_free_bytes }}',
  '          case "$REQUESTED_SCALE" in',
  '            100k|1m) ;;',
  '          if ! [[ "$REQUESTED_TIMEOUT_MINUTES" =~ ^[1-9][0-9]*$ ]]; then',
  '          if (( REQUESTED_TIMEOUT_MINUTES > 360 )); then',
  '          if [[ -z "${REQUESTED_RUNNER_LABEL//[[:space:]]/}" ]]; then',
  '          if ! [[ "$REQUESTED_MINIMUM_FREE_BYTES" =~ ^[1-9][0-9]*$ ]]; then',
]) {
  if (!validateBlock.includes(marker)) fail(`validate-inputs job is missing ${marker}`);
}
for (const forbidden of [
  'actions/checkout',
  'dtolnay/rust-toolchain',
  'services:',
  'postgres:',
  'cargo build',
]) {
  if (validateBlock.includes(forbidden)) {
    fail(`validate-inputs job must remain lightweight; found ${forbidden}`);
  }
}

const evidenceBlock = workflow.slice(evidenceStart);
const needsIndex = evidenceBlock.indexOf('    needs: validate-inputs\n');
const runnerIndex = evidenceBlock.indexOf('    runs-on: ${{ inputs.runner_label }}\n');
const servicesIndex = evidenceBlock.indexOf('    services:\n');
if (needsIndex < 0) fail('evidence job must depend on validate-inputs');
if (runnerIndex < 0 || servicesIndex < 0) {
  fail('evidence job is missing runner or service allocation markers');
}
if (needsIndex > runnerIndex || needsIndex > servicesIndex) {
  fail('evidence dependency must be declared before runner and service allocation');
}
if (!evidenceBlock.includes('          if (( available_bytes < MINIMUM_FREE_BYTES )); then')) {
  fail('evidence job must retain the runner disk-capacity gate');
}

console.log(`${prefix} reusable scale workflow input validation is fail closed`);
