import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import {
  FoundationFbaRuntimeSmokeError,
  foundationFbaRuntimeSmokeModules,
  verifyFoundationFbaRuntimeSmoke
} from './verify-foundation-fba-runtime-smoke.mjs';

const repoRoot = process.cwd();
const files = [
  ...new Set(foundationFbaRuntimeSmokeModules.flatMap((module) => [
    module.registry,
    module.smoke,
    ...module.markers.map(([file]) => file)
  ]))
];

function fixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'foundation-fba-runtime-smoke-'));
  for (const file of files) {
    const target = path.join(root, file);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(path.join(repoRoot, file), target);
  }
  return root;
}

function expectFailure(root, pattern) {
  try {
    verifyFoundationFbaRuntimeSmoke({ root });
  } catch (error) {
    if (error instanceof FoundationFbaRuntimeSmokeError && pattern.test(error.message)) return;
    throw error;
  }
  throw new Error(`expected failure matching ${pattern}`);
}

verifyFoundationFbaRuntimeSmoke();

const tenantPolicyDrift = fixture();
const tenantPorts = path.join(tenantPolicyDrift, 'crates/rustok-tenant/src/ports.rs');
fs.writeFileSync(
  tenantPorts,
  fs.readFileSync(tenantPorts, 'utf8').replace('context.require_policy(PortCallPolicy::read())?;', '/* removed */')
);
expectFailure(tenantPolicyDrift, /tenant source marker missing/);

const emailFallbackDrift = fixture();
const emailSmokePath = path.join(emailFallbackDrift, 'crates/rustok-email/contracts/evidence/email-runtime-fallback-smoke.json');
const emailSmoke = JSON.parse(fs.readFileSync(emailSmokePath, 'utf8'));
emailSmoke.profiles = emailSmoke.profiles.slice(1);
fs.writeFileSync(emailSmokePath, `${JSON.stringify(emailSmoke, null, 2)}\n`);
expectFailure(emailFallbackDrift, /email fallback profiles drift/);

console.log('foundation FBA runtime smoke fixture regressions passed');
