import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const repoRoot = process.cwd();
const script = path.join(repoRoot, 'scripts/verify/verify-product-runtime-fallback-smoke.mjs');
const fixtureFiles = [
  'crates/rustok-product/contracts/product-fba-registry.json',
  'crates/rustok-product/contracts/evidence/product-runtime-contract-smoke.json',
  'crates/rustok-product/contracts/evidence/product-runtime-fallback-smoke.json',
  'crates/rustok-product/src/ports.rs',
  'crates/rustok-product/README.md',
  'crates/rustok-product/docs/README.md',
  'crates/rustok-product/docs/implementation-plan.md',
  'docs/modules/registry.md',
];

function run(root = repoRoot) {
  return spawnSync(process.execPath, [script], {
    cwd: repoRoot,
    env: { ...process.env, PRODUCT_FBA_ROOT: root },
    encoding: 'utf8',
  });
}

function copyFixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'product-runtime-fallback-smoke-'));
  for (const file of fixtureFiles) {
    const target = path.join(root, file);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(path.join(repoRoot, file), target);
  }
  return root;
}

function assert(condition, message) {
  if (!condition) {
    console.error(`[verify-product-runtime-fallback-smoke.test] ${message}`);
    process.exit(1);
  }
}

const success = run();
assert(
  success.status === 0,
  `expected repository fixture to pass\nSTDOUT:\n${success.stdout}\nSTDERR:\n${success.stderr}`,
);

const missingReadmeMarker = copyFixture();
const readmePath = path.join(missingReadmeMarker, 'crates/rustok-product/README.md');
fs.writeFileSync(
  readmePath,
  fs.readFileSync(readmePath, 'utf8').replace('ProductCatalogReadPort` / `product.catalog_read.v1`', 'ProductCatalogReadPort drift'),
);
const readmeResult = run(missingReadmeMarker);
assert(readmeResult.status !== 0, 'expected missing product README marker to fail');
assert(
  readmeResult.stderr.includes('product README lacks FBA marker'),
  `expected README marker failure, got ${readmeResult.stderr}`,
);

const missingDocsMarker = copyFixture();
const docsReadmePath = path.join(missingDocsMarker, 'crates/rustok-product/docs/README.md');
fs.writeFileSync(
  docsReadmePath,
  fs.readFileSync(docsReadmePath, 'utf8').replaceAll('`transport_verified`', '`transport_drifted`'),
);
const docsResult = run(missingDocsMarker);
assert(docsResult.status !== 0, 'expected missing product docs README marker to fail');
assert(
  docsResult.stderr.includes('product docs README lacks FBA marker'),
  `expected docs README marker failure, got ${docsResult.stderr}`,
);

const missingHarnessMarker = copyFixture();
const portsPath = path.join(missingHarnessMarker, 'crates/rustok-product/src/ports.rs');
fs.writeFileSync(
  portsPath,
  fs.readFileSync(portsPath, 'utf8').replace('fn product_read_ports_require_deadline_policy()', 'fn product_read_ports_policy_drift()'),
);
const portsResult = run(missingHarnessMarker);
assert(portsResult.status !== 0, 'expected missing ports test harness marker to fail');
assert(
  portsResult.stderr.includes('runtime test harness source missing'),
  `expected test harness marker failure, got ${portsResult.stderr}`,
);

console.log('[verify-product-runtime-fallback-smoke.test] fixture coverage passed');
