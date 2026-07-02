import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { commerceDomainModules, CommerceDomainFbaRuntimeSmokeError, verifyCommerceDomainFbaRuntimeSmoke } from './verify-commerce-domain-fba-runtime-smoke.mjs';

const repoRoot = process.cwd();
const files = commerceDomainModules.flatMap((module) => [
  `crates/rustok-${module}/contracts/${module}-fba-registry.json`,
  `crates/rustok-${module}/contracts/evidence/${module}-runtime-contract-smoke.json`,
  `crates/rustok-${module}/src/ports.rs`,
]).concat([
  'crates/rustok-product/contracts/evidence/product-runtime-fallback-smoke.json',
  'crates/rustok-commerce/contracts/commerce-fba-registry.json',
  'crates/rustok-commerce/contracts/evidence/commerce-domain-provider-invocation-trace.json',
  'crates/rustok-commerce/src/fba.rs',
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
expectFailure(missingMode, /tax invocation trace degraded mode drift/);

const consumerDrift = fixture();
const tracePath = path.join(consumerDrift, 'crates/rustok-commerce/contracts/evidence/commerce-domain-provider-invocation-trace.json');
const trace = JSON.parse(fs.readFileSync(tracePath, 'utf8'));
trace.modules.find((entry) => entry.provider_module === 'product').consumer_degraded_modes = ['show_product_refresh_required'];
fs.writeFileSync(tracePath, `${JSON.stringify(trace, null, 2)}\n`);
expectFailure(consumerDrift, /product invocation trace consumer degraded mode drift/);

const missingRuntimeEntrypoint = fixture();
const fbaPath = path.join(missingRuntimeEntrypoint, 'crates/rustok-commerce/src/fba.rs');
fs.writeFileSync(
  fbaPath,
  fs.readFileSync(fbaPath, 'utf8').replace('pub fn commerce_domain_provider_invocation_trace', 'fn commerce_domain_provider_invocation_trace'),
);
expectFailure(missingRuntimeEntrypoint, /commerce fba\.rs must publish an invocation trace parser/);

const missingLookupHelper = fixture();
const lookupFbaPath = path.join(missingLookupHelper, 'crates/rustok-commerce/src/fba.rs');
fs.writeFileSync(
  lookupFbaPath,
  fs.readFileSync(lookupFbaPath, 'utf8').replace('pub fn provider_entry(', 'fn provider_entry('),
);
expectFailure(missingLookupHelper, /commerce fba\.rs missing typed lookup helper/);

console.log('commerce-domain FBA runtime smoke fixture regressions passed');
