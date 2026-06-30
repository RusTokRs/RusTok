import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { OrchestratorFbaRuntimeOrderError, verifyOrchestratorFbaRuntimeOrder } from './verify-orchestrator-fba-runtime-order.mjs';

const repoRoot = process.cwd();
const files = [
  'crates/rustok-ai/contracts/ai-fba-registry.json',
  'crates/rustok-ai/contracts/evidence/ai-orchestrator-runtime-order-smoke.json',
  'crates/rustok-ai/src/router.rs',
  'crates/rustok-ai/src/direct.rs',
  'crates/rustok-ai/admin/src/transport/mod.rs',
  'crates/rustok-ai/admin/src/ui/leptos.rs',
  'crates/rustok-page-builder/contracts/page-builder-fba-registry.json',
  'crates/rustok-page-builder/contracts/evidence/page-builder-orchestrator-runtime-order-smoke.json',
  'crates/rustok-page-builder/src/service.rs',
  'crates/rustok-page-builder/src/transport.rs',
  'crates/rustok-page-builder/src/adapters.rs',
];

const aiRegistry = JSON.parse(fs.readFileSync(path.join(repoRoot, files[0]), 'utf8'));
for (const adapter of aiRegistry.support_adapters) files.push(adapter.registry, adapter.runtime_binding);

function fixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'orchestrator-fba-'));
  for (const file of new Set(files)) {
    const target = path.join(root, file);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(path.join(repoRoot, file), target);
  }
  return root;
}

function expectFailure(root, pattern) {
  try {
    verifyOrchestratorFbaRuntimeOrder({ root });
  } catch (error) {
    if (error instanceof OrchestratorFbaRuntimeOrderError && pattern.test(error.message)) return;
    throw error;
  }
  throw new Error(`expected failure matching ${pattern}`);
}

verifyOrchestratorFbaRuntimeOrder();

const missingRegistration = fixture();
const mediaBinding = path.join(missingRegistration, 'crates/rustok-ai/src/direct_domain_media.rs');
fs.writeFileSync(mediaBinding, fs.readFileSync(mediaBinding, 'utf8').replaceAll('register_media_ai_vertical_handlers', 'removed_registration'));
expectFailure(missingRegistration, /ai runtime binding lacks register_media_ai_vertical_handlers/);

const publishOrderDrift = fixture();
const servicePath = path.join(publishOrderDrift, 'crates/rustok-page-builder/src/service.rs');
fs.writeFileSync(servicePath, fs.readFileSync(servicePath, 'utf8').replace('ensure_capability(&self.flags, BuilderCapabilityKind::Publish)?;', '/* publish capability guard removed */'));
expectFailure(publishOrderDrift, /page-builder guarded publish source marker missing/);

console.log('orchestrator FBA runtime-order fixture regressions passed');
