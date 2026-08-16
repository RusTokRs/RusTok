#!/usr/bin/env node
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import { basename, resolve } from 'node:path';

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 1) {
    const item = argv[i];
    if (!item.startsWith('--')) throw new Error(`Unexpected argument '${item}'`);
    const eq = item.indexOf('=');
    if (eq >= 0) out[item.slice(2, eq)] = item.slice(eq + 1);
    else {
      const key = item.slice(2);
      const next = argv[i + 1];
      if (!next || next.startsWith('--')) out[key] = true;
      else { out[key] = next; i += 1; }
    }
  }
  return out;
}

function sha256File(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

function commandVersion(command, args = ['--version']) {
  try {
    return execFileSync(command, args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim().split('\n')[0];
  } catch {
    return null;
  }
}

function gitSha() {
  try {
    return execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
  } catch {
    return null;
  }
}

function cleanObject(value) {
  if (Array.isArray(value)) return value.map(cleanObject);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).filter(([, item]) => item !== undefined).map(([key, item]) => [key, cleanObject(item)]));
  }
  return value;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const fixturePath = resolve(String(args.fixtures || 'target/loadtest-fixtures/s/manifest.json'));
  const topologyPath = resolve(String(args.topology || 'ops/loadtest/evidence/topology.example.json'));
  if (!existsSync(fixturePath)) throw new Error(`Fixture manifest not found: ${fixturePath}`);
  if (!existsSync(topologyPath)) throw new Error(`Topology file not found: ${topologyPath}`);

  const fixtures = JSON.parse(readFileSync(fixturePath, 'utf8'));
  if (fixtures.contract !== 'rustok_vs_magento_fixture_v1') throw new Error(`Unsupported fixture contract '${fixtures.contract}'`);
  const topology = JSON.parse(readFileSync(topologyPath, 'utf8'));
  if (topology.contract !== 'rustok_vs_magento_topology_v1') throw new Error(`Unsupported topology contract '${topology.contract}'`);

  const now = new Date();
  const defaultRunId = `${now.toISOString().replace(/[-:]/g, '').replace(/\.\d{3}Z$/, 'Z')}-${fixtures.requested_tier}`;
  const runId = String(args['run-id'] || defaultRunId);
  if (!/^[A-Za-z0-9._-]+$/.test(runId)) throw new Error('run-id may contain only A-Z, a-z, 0-9, dot, underscore, and dash');
  const root = resolve(String(args['out-root'] || 'evidence/rustok-vs-magento'), runId);
  if (existsSync(root)) throw new Error(`Refusing to overwrite existing evidence directory: ${root}`);
  mkdirSync(resolve(root, 'rustok'), { recursive: true });
  mkdirSync(resolve(root, 'magento'), { recursive: true });

  const manifest = cleanObject({
    contract: 'rustok_vs_magento_run_v1',
    run_id: runId,
    created_at: now.toISOString(),
    benchmark_contract: 'docs/benchmarks/rustok-vs-magento.md',
    rustok_commit: String(args['rustok-commit'] || process.env.RUSTOK_COMMIT || gitSha() || 'unknown'),
    magento_release: String(args['magento-release'] || process.env.MAGENTO_RELEASE || 'unresolved'),
    profile: String(args.profile || process.env.BENCHMARK_PROFILE || 'P1'),
    workload: String(args.workload || process.env.BENCHMARK_WORKLOAD || 'R4'),
    fixture_manifest: {
      path: fixturePath,
      file: basename(fixturePath),
      sha256: sha256File(fixturePath),
      contract: fixtures.contract,
      generator_version: fixtures.generator_version,
      seed: fixtures.seed,
      tier: fixtures.requested_tier,
      product_count: fixtures.product_count,
      variants_per_product: fixtures.variants_per_product,
      manifest_core_sha256: fixtures.manifest_core_sha256,
    },
    topology: {
      path: topologyPath,
      file: basename(topologyPath),
      sha256: sha256File(topologyPath),
      definition: topology,
    },
    load_generator: {
      hostname: os.hostname(),
      platform: os.platform(),
      release: os.release(),
      arch: os.arch(),
      cpu_count: os.cpus().length,
      cpu_model: os.cpus()[0]?.model || null,
      total_memory_bytes: os.totalmem(),
      node: process.version,
      k6: commandVersion('k6'),
      git: commandVersion('git'),
    },
    run_parameters: {
      rate: process.env.RATE || null,
      warmup_rate: process.env.WARMUP_RATE || null,
      warmup: process.env.WARMUP || null,
      duration: process.env.DURATION || null,
      pre_allocated_vus: process.env.PRE_ALLOCATED_VUS || null,
      max_vus: process.env.MAX_VUS || null,
      operation: process.env.OPERATION || null,
      search_term: process.env.SEARCH_TERM || fixtures.search_cases?.[0]?.term || null,
      product_sku: process.env.PRODUCT_SKU || fixtures.selection?.[1]?.sku || null,
    },
  });

  writeFileSync(resolve(root, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`, { encoding: 'utf8', flag: 'wx' });
  process.stdout.write(`${JSON.stringify({ evidence_dir: root, run_id: runId, fixture_sha256: manifest.fixture_manifest.sha256, topology_sha256: manifest.topology.sha256 })}\n`);
}

try { main(); } catch (error) { console.error(error.stack || String(error)); process.exitCode = 1; }
