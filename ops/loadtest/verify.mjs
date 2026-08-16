#!/usr/bin/env node
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

function run(args, options = {}) {
  return execFileSync(process.execPath, args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], ...options });
}

function runMustFail(args, options = {}) {
  try {
    run(args, options);
  } catch {
    return;
  }
  throw new Error(`Expected command to fail: node ${args.join(' ')}`);
}

const root = resolve(new URL('.', import.meta.url).pathname);
const tmp = mkdtempSync(join(tmpdir(), 'rustok-loadtest-'));
try {
  for (const file of [
    'k6/comparison.js',
    'fixtures/generate.mjs',
    'fixtures/import-rustok.mjs',
    'fixtures/import-magento.mjs',
    'evidence/create-run.mjs',
    'evidence/collect-process.mjs',
    'evidence/summarize.mjs',
  ]) {
    run(['--check', resolve(root, file)]);
  }
  const rustokConfig = JSON.parse(readFileSync(resolve(root, 'config/rustok.example.json'), 'utf8'));
  const magentoConfig = JSON.parse(readFileSync(resolve(root, 'config/magento.example.json'), 'utf8'));
  JSON.parse(readFileSync(resolve(root, 'evidence/topology.example.json'), 'utf8'));
  if (rustokConfig.name !== 'rustok-rest') throw new Error(`Unexpected RusTok adapter profile '${rustokConfig.name}'`);
  if (magentoConfig.name !== 'magento-core-graphql') throw new Error(`Unexpected Magento adapter profile '${magentoConfig.name}'`);

  const outA = resolve(tmp, 'a');
  const outB = resolve(tmp, 'b');
  run([resolve(root, 'fixtures/generate.mjs'), '--count', '100', '--seed', 'verification-seed', '--out', outA]);
  run([resolve(root, 'fixtures/generate.mjs'), '--count', '100', '--seed', 'verification-seed', '--out', outB]);
  const manifestA = JSON.parse(readFileSync(resolve(outA, 'manifest.json'), 'utf8'));
  const manifestB = JSON.parse(readFileSync(resolve(outB, 'manifest.json'), 'utf8'));
  if (manifestA.manifest_core_sha256 !== manifestB.manifest_core_sha256) throw new Error('Fixture manifest digest is not deterministic');
  if (readFileSync(resolve(outA, 'products.jsonl'), 'utf8') !== readFileSync(resolve(outB, 'products.jsonl'), 'utf8')) throw new Error('products.jsonl is not byte-deterministic');
  if (readFileSync(resolve(outA, 'products.csv'), 'utf8') !== readFileSync(resolve(outB, 'products.csv'), 'utf8')) throw new Error('products.csv is not byte-deterministic');
  if (manifestA.search_cases.reduce((sum, item) => sum + item.expected_matches, 0) !== 100) throw new Error('Search cardinality does not cover all fixtures exactly once');

  const exampleTopologyPath = resolve(root, 'evidence/topology.example.json');
  const evidenceRoot = resolve(tmp, 'evidence');
  const selectedSearch = manifestA.search_cases[0];
  const requiredSearchEnv = {
    ...process.env,
    SEARCH_TERM: selectedSearch.term,
    SEARCH_EXPECTED_MATCHES: String(selectedSearch.expected_matches),
    RATE: '500',
    OPERATION: 'mixed',
  };
  runMustFail([
    resolve(root, 'evidence/create-run.mjs'),
    '--fixtures', resolve(outA, 'manifest.json'),
    '--topology', exampleTopologyPath,
    '--out-root', evidenceRoot,
    '--run-id', 'must-fail-placeholder-topology',
    '--rustok-commit', '0123456789abcdef0123456789abcdef01234567',
    '--magento-release', '2.4.x-test',
    '--workload', 'R4',
  ], { env: requiredSearchEnv });

  const topology = JSON.parse(readFileSync(exampleTopologyPath, 'utf8'));
  const resolvedTopology = JSON.parse(JSON.stringify(topology).replaceAll('REPLACE_ME', 'verification-version'));
  resolvedTopology.notes = 'verification topology';
  const resolvedTopologyPath = resolve(tmp, 'topology.json');
  writeFileSync(resolvedTopologyPath, `${JSON.stringify(resolvedTopology, null, 2)}\n`, 'utf8');

  runMustFail([
    resolve(root, 'evidence/create-run.mjs'),
    '--fixtures', resolve(outA, 'manifest.json'),
    '--topology', resolvedTopologyPath,
    '--out-root', evidenceRoot,
    '--run-id', 'must-fail-missing-search-count',
    '--rustok-commit', '0123456789abcdef0123456789abcdef01234567',
    '--magento-release', '2.4.x-test',
    '--workload', 'R4',
  ], { env: { ...process.env, SEARCH_TERM: selectedSearch.term } });

  run([
    resolve(root, 'evidence/create-run.mjs'),
    '--fixtures', resolve(outA, 'manifest.json'),
    '--topology', resolvedTopologyPath,
    '--out-root', evidenceRoot,
    '--run-id', 'verified-run',
    '--rustok-commit', '0123456789abcdef0123456789abcdef01234567',
    '--magento-release', '2.4.x-test',
    '--workload', 'R4',
  ], { env: requiredSearchEnv });
  const evidenceManifest = JSON.parse(readFileSync(resolve(evidenceRoot, 'verified-run/manifest.json'), 'utf8'));
  if (Object.keys(evidenceManifest.fixture_manifest.verified_files || {}).length !== 2) throw new Error('Evidence manifest did not verify both canonical fixture files');
  if (evidenceManifest.unresolved_fields.length !== 0) throw new Error('Resolved verification topology still contains unresolved evidence fields');
  if (evidenceManifest.run_parameters.search_expected_matches !== selectedSearch.expected_matches) throw new Error('Evidence manifest did not pin search cardinality');
  if (evidenceManifest.adapters?.rustok !== 'rustok-rest' || evidenceManifest.adapters?.magento !== 'magento-core-graphql') throw new Error('Evidence manifest did not pin adapter profiles');

  runMustFail([
    resolve(root, 'evidence/create-run.mjs'),
    '--fixtures', resolve(outA, 'manifest.json'),
    '--topology', resolvedTopologyPath,
    '--out-root', evidenceRoot,
    '--run-id', 'must-fail-search-count',
    '--rustok-commit', '0123456789abcdef0123456789abcdef01234567',
    '--magento-release', '2.4.x-test',
    '--workload', 'R4',
  ], { env: { ...requiredSearchEnv, SEARCH_EXPECTED_MATCHES: String(selectedSearch.expected_matches + 1) } });

  if (process.platform === 'linux') {
    const telemetryPath = resolve(tmp, 'telemetry.jsonl');
    run([
      resolve(root, 'evidence/collect-process.mjs'),
      '--target', `app:${process.pid}`,
      '--delay-ms', '0',
      '--interval-ms', '10',
      '--duration-ms', '30',
      '--output', telemetryPath,
    ]);
    const telemetryLines = readFileSync(telemetryPath, 'utf8').trim().split('\n').map((line) => JSON.parse(line));
    if (telemetryLines.filter((item) => item.contract === 'rustok_vs_magento_process_sample_v1').length < 2) throw new Error('Process collector produced insufficient samples');

    const summaryPath = resolve(tmp, 'summary.json');
    writeFileSync(summaryPath, `${JSON.stringify({
      metadata: { platform: 'verification', operation: 'mixed', requested_rps: 500, search_term: selectedSearch.term, search_expected_matches: selectedSearch.expected_matches },
      k6: { metrics: {
        measured_requests: { values: { count: 1500, rate: 500 } },
        'http_req_duration{phase:measure}': { values: { med: 20, 'p(95)': 50, 'p(99)': 80 } },
        'http_req_failed{phase:measure}': { values: { rate: 0 } },
        'response_validation_failures{phase:measure}': { values: { rate: 0 } },
        'dropped_iterations{scenario:measure}': { values: { count: 0 } },
      } },
    }, null, 2)}\n`, 'utf8');
    const resultPath = resolve(tmp, 'result.json');
    run([
      resolve(root, 'evidence/summarize.mjs'),
      '--summary', summaryPath,
      '--telemetry', telemetryPath,
      '--app-target', 'app',
      '--application-vcpu', '4',
      '--output', resultPath,
    ]);
    const result = JSON.parse(readFileSync(resultPath, 'utf8'));
    if (result.performance.achieved_rps !== 500 || result.performance.slo_pass !== true) throw new Error('Result summarizer did not preserve measured RPS/SLO');
    if (result.source.search_expected_matches !== selectedSearch.expected_matches) throw new Error('Result summarizer did not preserve search cardinality identity');
  }

  process.stdout.write('loadtest static verification: PASS\n');
} finally {
  rmSync(tmp, { recursive: true, force: true });
}
