import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const repoRoot = process.cwd();
const script = path.join(repoRoot, 'scripts/verify/verify-product-runtime-fallback-smoke.mjs');
const fixtureFiles = [
  'modules.toml',
  'crates/rustok-product/rustok-module.toml',
  'crates/rustok-product/contracts/product-fba-registry.json',
  'crates/rustok-product/contracts/evidence/product-runtime-contract-smoke.json',
  'crates/rustok-product/contracts/evidence/product-runtime-fallback-smoke.json',
  'crates/rustok-product/src/ports.rs',
  'crates/rustok-product/README.md',
  'crates/rustok-product/docs/README.md',
  'crates/rustok-product/docs/implementation-plan.md',
  'docs/modules/registry.md',
  'package.json',
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

const missingModulesMarker = copyFixture();
const modulesPath = path.join(missingModulesMarker, 'modules.toml');
fs.writeFileSync(
  modulesPath,
  fs.readFileSync(modulesPath, 'utf8').replace('depends_on = ["taxonomy"]', 'depends_on = []'),
);
const modulesResult = run(missingModulesMarker);
assert(modulesResult.status !== 0, 'expected product modules.toml metadata drift to fail');
assert(
  modulesResult.stderr.includes('modules.toml product module metadata drift'),
  `expected modules metadata failure, got ${modulesResult.stderr}`,
);

const missingManifestMarker = copyFixture();
const manifestPath = path.join(missingManifestMarker, 'crates/rustok-product/rustok-module.toml');
fs.writeFileSync(
  manifestPath,
  fs.readFileSync(manifestPath, 'utf8').replace('contract_version = "product.catalog_read.v1"', 'contract_version = "product.catalog_read.drift"'),
);
const manifestResult = run(missingManifestMarker);
assert(manifestResult.status !== 0, 'expected product module manifest FBA drift to fail');
assert(
  manifestResult.stderr.includes('product module manifest FBA marker drift'),
  `expected module manifest failure, got ${manifestResult.stderr}`,
);

const missingPackageAggregate = copyFixture();
const packagePath = path.join(missingPackageAggregate, 'package.json');
const packageJson = JSON.parse(fs.readFileSync(packagePath, 'utf8'));
packageJson.scripts['test:verify:ecommerce:fba'] = packageJson.scripts['test:verify:ecommerce:fba'].replace(
  ' && npm run test:verify:product:runtime-fallback-smoke',
  '',
);
fs.writeFileSync(packagePath, `${JSON.stringify(packageJson, null, 2)}\n`);
const packageResult = run(missingPackageAggregate);
assert(packageResult.status !== 0, 'expected missing package aggregate marker to fail');
assert(
  packageResult.stderr.includes('package.json ecommerce FBA fixture aggregate lacks product runtime fallback smoke test'),
  `expected package aggregate failure, got ${packageResult.stderr}`,
);

const prematureRegistryTransportVerified = copyFixture();
const registryPath = path.join(
  prematureRegistryTransportVerified,
  'crates/rustok-product/contracts/product-fba-registry.json',
);
const registryJson = JSON.parse(fs.readFileSync(registryPath, 'utf8'));
registryJson.status = 'transport_verified';
fs.writeFileSync(registryPath, `${JSON.stringify(registryJson, null, 2)}\n`);
const registryResult = run(prematureRegistryTransportVerified);
assert(registryResult.status !== 0, 'expected premature registry transport_verified to fail');
assert(
  registryResult.stderr.includes('product registry must be boundary_ready for fallback smoke evidence'),
  `expected registry status failure, got ${registryResult.stderr}`,
);

const prematurePlanTransportVerified = copyFixture();
const planPath = path.join(prematurePlanTransportVerified, 'crates/rustok-product/docs/implementation-plan.md');
fs.writeFileSync(
  planPath,
  fs.readFileSync(planPath, 'utf8').replace('- FBA status: `boundary_ready`', '- FBA status: `transport_verified`'),
);
const planResult = run(prematurePlanTransportVerified);
assert(planResult.status !== 0, 'expected premature local plan transport_verified to fail');
assert(planResult.stderr.includes('local plan FBA status drift'), `expected local plan status failure, got ${planResult.stderr}`);

const missingSyncMarker = copyFixture();
const syncPlanPath = path.join(missingSyncMarker, 'crates/rustok-product/docs/implementation-plan.md');
fs.writeFileSync(
  syncPlanPath,
  fs
    .readFileSync(syncPlanPath, 'utf8')
    .replace(
      '[x] maintain sync between product runtime contract, commerce transport and module metadata.',
      '[ ] maintain sync between product runtime contract, commerce transport and module metadata.',
    ),
);
const syncMarkerResult = run(missingSyncMarker);
assert(syncMarkerResult.status !== 0, 'expected missing product runtime/transport/metadata sync marker to fail');
assert(
  syncMarkerResult.stderr.includes('local plan product runtime/transport/metadata sync marker drift'),
  `expected sync marker failure, got ${syncMarkerResult.stderr}`,
);

const staleCentralBatchSummary = copyFixture();
const centralPath = path.join(staleCentralBatchSummary, 'docs/modules/registry.md');
fs.writeFileSync(
  centralPath,
  fs
    .readFileSync(centralPath, 'utf8')
    .replace(
      '| `product` | admin + storefront | `in_progress` | `boundary_ready`',
      '| `product` | admin + storefront | `in_progress` | `in_progress`',
    ),
);
const centralResult = run(staleCentralBatchSummary);
assert(centralResult.status !== 0, 'expected stale central product status to fail');
assert(
  centralResult.stderr.includes('central readiness board product status drift'),
  `expected central product status failure, got ${centralResult.stderr}`,
);

console.log('[verify-product-runtime-fallback-smoke.test] fixture coverage passed');
