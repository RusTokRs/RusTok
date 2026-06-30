import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { ownerFbaModules, OwnerFbaRuntimeOrderError, verifyOwnerFbaRuntimeOrder } from './verify-owner-fba-runtime-order.mjs';

const repoRoot = process.cwd();
const files = ownerFbaModules.flatMap((module) => [
  `crates/rustok-${module}/contracts/${module}-fba-registry.json`,
  `crates/rustok-${module}/contracts/evidence/${module}-provider-runtime-order-smoke.json`,
  `crates/rustok-${module}/src/ports.rs`,
]);

function fixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'owner-fba-runtime-order-'));
  for (const file of files) {
    const target = path.join(root, file);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(path.join(repoRoot, file), target);
  }
  return root;
}

function expectFailure(root, pattern) {
  try {
    verifyOwnerFbaRuntimeOrder({ root });
  } catch (error) {
    if (error instanceof OwnerFbaRuntimeOrderError && pattern.test(error.message)) return;
    throw error;
  }
  throw new Error(`expected failure matching ${pattern}`);
}

verifyOwnerFbaRuntimeOrder();

const missingIdempotency = fixture();
const comments = path.join(missingIdempotency, 'crates/rustok-comments/src/ports.rs');
fs.writeFileSync(comments, fs.readFileSync(comments, 'utf8').replace('context.require_policy(PortCallPolicy::write())?;', '/* removed */'));
expectFailure(missingIdempotency, /comments\.create_comment source marker missing/);

const fallbackDrift = fixture();
const regionSmokePath = path.join(fallbackDrift, 'crates/rustok-region/contracts/evidence/region-provider-runtime-order-smoke.json');
const regionSmoke = JSON.parse(fs.readFileSync(regionSmokePath, 'utf8'));
regionSmoke.degraded_modes = regionSmoke.degraded_modes.slice(1);
fs.writeFileSync(regionSmokePath, `${JSON.stringify(regionSmoke, null, 2)}\n`);
expectFailure(fallbackDrift, /region degraded mode drift/);

console.log('owner FBA runtime-order fixture regressions passed');
