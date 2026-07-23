import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const fail = (message) => {
  console.error(`[validate-index-storage-evidence] ${message}`);
  process.exit(1);
};

const contracts = {
  smoke: {
    serializedScale: 'smoke',
    debugScale: 'Smoke',
    productRows: 400,
    entityRows: 1_216,
    linkRows: 2_400,
    mutationBatch: 100,
    deletedLinks: 200,
  },
  '100k': {
    serializedScale: 'rows100k',
    debugScale: 'Rows100k',
    productRows: 100_000,
    entityRows: 300_080,
    linkRows: 600_000,
    mutationBatch: 1_000,
    deletedLinks: 2_000,
  },
  '1m': {
    serializedScale: 'rows1m',
    debugScale: 'Rows1m',
    productRows: 1_000_000,
    entityRows: 3_000_160,
    linkRows: 6_000_000,
    mutationBatch: 1_000,
    deletedLinks: 2_000,
  },
};

const scale = process.env.INDEX_BENCH_SCALE;
const contract = contracts[scale];
if (!contract) fail(`INDEX_BENCH_SCALE must be smoke, 100k, or 1m; got ${scale}`);

const root = process.env.INDEX_BENCH_EVIDENCE_ROOT
  ?? path.join('evidence/index-storage', scale);
const files = ['read-report.json', 'mutation-report.json', 'maintenance-report.json'];

const readJson = (filename) => {
  const fullPath = path.join(root, filename);
  if (!existsSync(fullPath)) fail(`missing evidence file: ${fullPath}`);
  try {
    return JSON.parse(readFileSync(fullPath, 'utf8'));
  } catch (error) {
    fail(`invalid JSON in ${fullPath}: ${error.message}`);
  }
};

const requireThreePrototypes = (report, label) => {
  if (!Array.isArray(report.prototypes) || report.prototypes.length !== 3) {
    fail(`${label} must contain exactly three prototypes`);
  }
};

const read = readJson('read-report.json');
if (read.dataset?.scale !== contract.serializedScale) {
  fail(`read scale mismatch: expected ${contract.serializedScale}, got ${read.dataset?.scale}`);
}
const productRows = read.dataset?.tenants
  * read.dataset?.products_per_tenant
  * read.dataset?.locales?.length;
if (productRows !== contract.productRows) {
  fail(`read product-row mismatch: expected ${contract.productRows}, got ${productRows}`);
}
if (read.source_entity_rows !== contract.entityRows || read.source_link_rows !== contract.linkRows) {
  fail('read source cardinality mismatch');
}
requireThreePrototypes(read, 'read report');

const baselineReadWorkloads = new Map(
  read.prototypes[0].workloads.map((workload) => [
    workload.name,
    { resultRows: workload.result_rows, resultDigest: workload.result_digest },
  ]),
);
for (const prototype of read.prototypes) {
  if (prototype.entity_rows !== contract.entityRows || prototype.link_rows !== contract.linkRows) {
    fail(`${prototype.prototype} read cardinality mismatch`);
  }
  if (prototype.workloads.length !== baselineReadWorkloads.size) {
    fail(`${prototype.prototype} read workload count mismatch`);
  }
  for (const workload of prototype.workloads) {
    const baseline = baselineReadWorkloads.get(workload.name);
    if (!baseline) fail(`${prototype.prototype} introduced unknown read workload ${workload.name}`);
    if (baseline.resultRows !== workload.result_rows || baseline.resultDigest !== workload.result_digest) {
      fail(`${prototype.prototype}/${workload.name} read parity mismatch`);
    }
    if (!Array.isArray(workload.repetitions) || workload.repetitions.length !== 3) {
      fail(`${prototype.prototype}/${workload.name} read repetitions mismatch`);
    }
  }
}

const mutation = readJson('mutation-report.json');
if (mutation.dataset_scale !== contract.debugScale || mutation.repetitions !== 3) {
  fail('mutation scale/repetition mismatch');
}
requireThreePrototypes(mutation, 'mutation report');
const expectedMutations = new Map([
  ['update_product_batch', { entities: contract.mutationBatch, links: null }],
  ['delete_product_batch', { entities: contract.mutationBatch, links: contract.deletedLinks }],
]);
for (const prototype of mutation.prototypes) {
  if (prototype.workloads.length !== expectedMutations.size) {
    fail(`${prototype.prototype} mutation workload count mismatch`);
  }
  for (const workload of prototype.workloads) {
    const expected = expectedMutations.get(workload.name);
    if (!expected) fail(`${prototype.prototype} introduced unknown mutation workload ${workload.name}`);
    if (workload.affected_entities !== expected.entities || workload.affected_links !== expected.links) {
      fail(`${prototype.prototype}/${workload.name} mutation effect mismatch`);
    }
    if (!Array.isArray(workload.repetitions) || workload.repetitions.length !== 3) {
      fail(`${prototype.prototype}/${workload.name} mutation repetitions mismatch`);
    }
  }
}

const maintenance = readJson('maintenance-report.json');
if (maintenance.dataset_scale !== contract.serializedScale || maintenance.cycles !== 5) {
  fail('maintenance scale/cycle mismatch');
}
requireThreePrototypes(maintenance, 'maintenance report');
for (const prototype of maintenance.prototypes) {
  for (const phase of ['baseline', 'after_churn', 'after_vacuum']) {
    const snapshot = prototype[phase];
    if (snapshot?.entity_rows !== contract.entityRows || snapshot?.link_rows !== contract.linkRows) {
      fail(`${prototype.prototype}/${phase} maintenance cardinality mismatch`);
    }
  }
  if (!Number.isFinite(prototype.vacuum_duration_ms) || prototype.vacuum_duration_ms < 0) {
    fail(`${prototype.prototype} has invalid VACUUM duration`);
  }
}

const resourceFiles = [
  'runner-resources-before.txt',
  'runner-resources-after.txt',
].filter((filename) => existsSync(path.join(root, filename)));
if (process.env.INDEX_BENCH_REQUIRE_RUNNER_RESOURCES === '1' && resourceFiles.length !== 2) {
  fail('scale evidence must include before/after runner resource snapshots');
}

writeFileSync(
  path.join(root, 'provenance.json'),
  JSON.stringify({
    generated_at: new Date().toISOString(),
    repository: process.env.GITHUB_REPOSITORY,
    commit: process.env.GITHUB_SHA,
    ref: process.env.GITHUB_REF,
    run_id: process.env.GITHUB_RUN_ID,
    run_attempt: process.env.GITHUB_RUN_ATTEMPT,
    job: process.env.GITHUB_JOB,
    runner_os: process.env.RUNNER_OS,
    runner_arch: process.env.RUNNER_ARCH,
    postgres_image: 'postgres:16',
    scale,
    repetitions: 3,
    churn_cycles: 5,
    expected_product_rows: contract.productRows,
    expected_entity_rows: contract.entityRows,
    expected_link_rows: contract.linkRows,
    reports: files,
    runner_resource_files: resourceFiles,
  }, null, 2) + '\n',
);

console.log(
  `[validate-index-storage-evidence] ${scale} packet is consistent: `
  + `${contract.entityRows} entities, ${contract.linkRows} links`,
);
