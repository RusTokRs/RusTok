#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (filename) => readFileSync(new URL(filename, root), 'utf8');
const prefix = '[verify-index-storage-scale-run-contract]';
const fail = (message) => {
  console.error(`${prefix} ${message}`);
  process.exit(1);
};

const reusableWorkflow = read('.github/workflows/index-storage-scale-run.yml');
const orchestrationWorkflow = read('.github/workflows/index-storage-scale-evidence.yml');
const contractWorkflow = read('.github/workflows/index-storage-scale-run-contract.yml');
const readme = read('crates/rustok-index/README.md');
const runbook = read('crates/rustok-index/docs/storage-evidence-runbook.md');

const requireMarkers = (content, label, markers) => {
  for (const marker of markers) {
    if (!content.includes(marker)) fail(`${label} is missing contract marker: ${marker}`);
  }
};

const forbidMarkers = (content, label, markers) => {
  for (const marker of markers) {
    if (content.includes(marker)) fail(`${label} contains forbidden marker: ${marker}`);
  }
};

const countMarker = (content, marker) => content.split(marker).length - 1;

const validateStart = reusableWorkflow.indexOf('  validate-inputs:\n');
const evidenceStart = reusableWorkflow.indexOf('  evidence:\n');
if (validateStart < 0) fail('reusable scale workflow is missing validate-inputs job');
if (evidenceStart < 0) fail('reusable scale workflow is missing evidence job');
if (validateStart > evidenceStart) {
  fail('validate-inputs job must appear before the evidence job');
}

const validateBlock = reusableWorkflow.slice(validateStart, evidenceStart);
requireMarkers(validateBlock, 'reusable validate-inputs job', [
  '    runs-on: ubuntu-slim',
  '    timeout-minutes: 5',
  '          REQUESTED_SCALE: ${{ inputs.scale }}',
  '          REQUESTED_TIMEOUT_MINUTES: ${{ inputs.timeout_minutes }}',
  '          REQUESTED_RUNNER_LABEL: ${{ inputs.runner_label }}',
  '          REQUESTED_MINIMUM_FREE_BYTES: ${{ inputs.minimum_free_bytes }}',
  '          set -euo pipefail',
  '          export LC_ALL=C',
  '          MAXIMUM_FREE_BYTES=9007199254740991',
  '          case "$REQUESTED_SCALE" in',
  '            100k|1m) ;;',
  '          if ! [[ "$REQUESTED_TIMEOUT_MINUTES" =~ ^[1-9][0-9]*$ ]]; then',
  '          if (( REQUESTED_TIMEOUT_MINUTES > 360 )); then',
  '          if [[ -z "${REQUESTED_RUNNER_LABEL//[[:space:]]/}" ]]; then',
  '          if [[ "$REQUESTED_RUNNER_LABEL" =~ [[:cntrl:]] ]]; then',
  '          if (( ${#REQUESTED_RUNNER_LABEL} > 128 )); then',
  '          if ! [[ "$REQUESTED_MINIMUM_FREE_BYTES" =~ ^[1-9][0-9]*$ ]]; then',
  '          if (( ${#REQUESTED_MINIMUM_FREE_BYTES} > ${#MAXIMUM_FREE_BYTES} )) ||',
]);
forbidMarkers(validateBlock, 'reusable validate-inputs job', [
  'actions/checkout',
  'dtolnay/rust-toolchain',
  'services:',
  'postgres:',
  'cargo build',
]);

const reusableEvidenceBlock = reusableWorkflow.slice(evidenceStart);
const needsIndex = reusableEvidenceBlock.indexOf('    needs: validate-inputs\n');
const runnerIndex = reusableEvidenceBlock.indexOf('    runs-on: ${{ inputs.runner_label }}\n');
const servicesIndex = reusableEvidenceBlock.indexOf('    services:\n');
if (needsIndex < 0) fail('reusable evidence job must depend on validate-inputs');
if (runnerIndex < 0 || servicesIndex < 0) {
  fail('reusable evidence job is missing runner or service allocation markers');
}
if (needsIndex > runnerIndex || needsIndex > servicesIndex) {
  fail('reusable evidence dependency must be declared before runner and service allocation');
}
requireMarkers(reusableEvidenceBlock, 'reusable evidence job', [
  '          if (( available_bytes < MINIMUM_FREE_BYTES )); then',
  '          name: index-storage-${{ inputs.scale }}-${{ github.sha }}',
  '          retention-days: 90',
]);

requireMarkers(orchestrationWorkflow, 'scale evidence orchestration workflow', [
  '  workflow_dispatch:',
  '  pull_request:',
  '      - "crates/rustok-index/**"',
  '      - "ops/benches/**"',
  '      - "scripts/verify/*index-storage*.mjs"',
  '      - "scripts/verify/storage-decision*.mjs"',
  '      - "scripts/verify/*methodology-envelope*.mjs"',
  '      - "scripts/verify/verify-index-fba.mjs"',
  '      - ".github/workflows/index-storage-*.yml"',
  '  contents: read',
]);
forbidMarkers(orchestrationWorkflow, 'scale evidence orchestration workflow', [
  '\n  push:\n',
  'agent/index-m2-scale-evidence',
  'agent/index-m2-1m-standard-runner',
  'actions/github-script',
  'issue_number: 2009',
  '  issues: write',
  '  pull-requests: write',
]);

const contractStart = orchestrationWorkflow.indexOf('  contract:\n');
const evidence100kStart = orchestrationWorkflow.indexOf('  evidence-100k:\n');
const evidence1mStart = orchestrationWorkflow.indexOf('  evidence-1m:\n');
const comparisonStart = orchestrationWorkflow.indexOf('  comparison:\n');
if ([contractStart, evidence100kStart, evidence1mStart, comparisonStart].some((index) => index < 0)) {
  fail('scale evidence orchestration workflow is missing a canonical job');
}
if (!(contractStart < evidence100kStart
    && evidence100kStart < evidence1mStart
    && evidence1mStart < comparisonStart)) {
  fail('scale evidence jobs must remain ordered contract, 100k, 1m, comparison');
}

const contractBlock = orchestrationWorkflow.slice(contractStart, evidence100kStart);
requireMarkers(contractBlock, 'scale evidence contract job', [
  '    runs-on: ubuntu-slim',
  '    timeout-minutes: 10',
  '          find scripts/verify -maxdepth 1 -type f',
  "               -name '*methodology-envelope*.mjs' -o \\",
  '            | xargs -0 -n1 node --check',
  '        run: node scripts/verify/verify-index-storage-scale-run-contract.mjs',
  '        run: node scripts/verify/index-storage-tooling.mjs contract',
  '        run: node --test scripts/verify/index-storage-validator-arguments.test.mjs',
  '        run: node scripts/verify/index-storage-tooling.mjs fixtures',
]);
forbidMarkers(contractBlock, 'scale evidence contract job', [
  "if: ${{ github.event_name != 'pull_request' }}",
  'services:',
  'postgres:',
  'cargo build',
]);

const dispatchGate = "    if: ${{ github.event_name == 'workflow_dispatch' }}\n";
if (countMarker(orchestrationWorkflow, dispatchGate) !== 3) {
  fail('100k, 1m, and comparison jobs must each require explicit workflow_dispatch');
}
const evidence100kBlock = orchestrationWorkflow.slice(evidence100kStart, evidence1mStart);
const evidence1mBlock = orchestrationWorkflow.slice(evidence1mStart, comparisonStart);
const comparisonBlock = orchestrationWorkflow.slice(comparisonStart);
requireMarkers(evidence100kBlock, '100k evidence job', [
  dispatchGate.trimEnd(),
  '    needs: contract',
  '    uses: ./.github/workflows/index-storage-scale-run.yml',
  '      scale: 100k',
]);
requireMarkers(evidence1mBlock, '1m evidence job', [
  dispatchGate.trimEnd(),
  '    needs: contract',
  '    uses: ./.github/workflows/index-storage-scale-run.yml',
  '      scale: 1m',
  "      runner_label: ${{ vars.INDEX_BENCH_LARGE_RUNNER || 'ubuntu-latest' }}",
  '      minimum_free_bytes: 35000000000',
]);
requireMarkers(comparisonBlock, 'cross-scale comparison job', [
  dispatchGate.trimEnd(),
  '      - evidence-100k',
  '      - evidence-1m',
  '          name: index-storage-100k-${{ github.sha }}',
  '          name: index-storage-1m-${{ github.sha }}',
  '          node scripts/verify/index-storage-tooling.mjs compare',
  '          name: index-storage-comparison-${{ github.sha }}',
]);

requireMarkers(contractWorkflow, 'scale workflow contract workflow', [
  '      - ".github/workflows/index-storage-scale-run.yml"',
  '      - ".github/workflows/index-storage-scale-evidence.yml"',
  '      - ".github/workflows/index-storage-scale-run-contract.yml"',
  '      - "scripts/verify/verify-index-storage-scale-run-contract.mjs"',
  '      - "crates/rustok-index/README.md"',
  '      - "crates/rustok-index/docs/storage-evidence-runbook.md"',
  '        run: node scripts/verify/verify-index-storage-scale-run-contract.mjs',
]);

requireMarkers(readme, 'Index README', [
  'M2 PostgreSQL storage benchmark: complete',
  'M2 accepted storage model: JSONB',
  '[M2 replacement evidence runbook](./docs/storage-evidence-runbook.md)',
]);
forbidMarkers(readme, 'Index README', [
  'M2 replacement evidence and storage ADR: pending',
]);
requireMarkers(runbook, 'replacement evidence runbook', [
  'Pull requests run only the lightweight contract job.',
  'Heavy replacement evidence runs only through an explicit `workflow_dispatch`.',
  'One dispatch uses one selected Git ref and fans out `100k` and `1m` from the same checkout SHA.',
  'Do not combine artifacts from different run IDs or commit SHAs.',
  'Successful evidence and comparison artifacts are retained for 90 days.',
  'The canonical replacement run is `30222913450`',
  'The accepted ADR selects JSONB.',
  'M2 is complete.',
]);
forbidMarkers(runbook, 'replacement evidence runbook', [
  'M2 remains open until the replacement packets are archived, the comparison is reviewed, and the ADR is accepted.',
]);

console.log(`${prefix} reusable inputs and owner-dispatched scale orchestration are fail closed`);
