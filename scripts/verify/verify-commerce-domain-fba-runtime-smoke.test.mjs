import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { commerceDomainModules, CommerceDomainFbaRuntimeSmokeError, verifyCommerceDomainFbaRuntimeSmoke } from './verify-commerce-domain-fba-runtime-smoke.mjs';

const repoRoot = process.cwd();
const files = commerceDomainModules.flatMap((module) => [
  `crates/rustok-${module}/contracts/${module}-fba-registry.json`,
  `crates/rustok-${module}/contracts/evidence/${module}-runtime-contract-smoke.json`,
  `crates/rustok-${module}/src/ports.rs`,
]);

function fixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'commerce-domain-fba-'));
  for (const file of files) {
    const target = path.join(root, file);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(path.join(repoRoot, file), target);
  }
  return root;
}

function expectFailure(root, pattern) {
  try {
    verifyCommerceDomainFbaRuntimeSmoke({ root });
  } catch (error) {
    if (error instanceof CommerceDomainFbaRuntimeSmokeError && pattern.test(error.message)) return;
    throw error;
  }
  throw new Error(`expected verification failure matching ${pattern}`);
}

verifyCommerceDomainFbaRuntimeSmoke();

const missingPolicy = fixture();
const productPorts = path.join(missingPolicy, 'crates/rustok-product/src/ports.rs');
fs.writeFileSync(productPorts, fs.readFileSync(productPorts, 'utf8').replace('context.require_policy(PortCallPolicy::read())?;', '/* policy removed */'));
expectFailure(missingPolicy, /product\.read_product_projection source marker missing/);

const missingMode = fixture();
const taxSmokePath = path.join(missingMode, 'crates/rustok-tax/contracts/evidence/tax-runtime-contract-smoke.json');
const taxSmoke = JSON.parse(fs.readFileSync(taxSmokePath, 'utf8'));
taxSmoke.degraded_modes = [];
fs.writeFileSync(taxSmokePath, `${JSON.stringify(taxSmoke, null, 2)}\n`);
expectFailure(missingMode, /tax degraded mode drift/);

console.log('commerce-domain FBA runtime smoke fixture regressions passed');
