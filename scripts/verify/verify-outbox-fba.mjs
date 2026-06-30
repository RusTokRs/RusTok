import { readFileSync } from 'node:fs';
const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const json = (path) => JSON.parse(read(path));
const fail = (message) => { console.error(`[verify-outbox-fba] ${message}`); process.exit(1); };
const sameSet = (actual, expected) => Array.isArray(actual) && Array.isArray(expected) && actual.length === expected.length && expected.every((item) => actual.includes(item));
const registryPath = 'crates/rustok-outbox/contracts/outbox-fba-registry.json';
const evidencePath = 'crates/rustok-outbox/contracts/evidence/outbox-contract-test-static-matrix.json';
const registry = json(registryPath);
const evidence = json(evidencePath);
const runtimeOrderSmoke = json(registry.evidence.runtime_order_smoke);
const manifest = read('crates/rustok-outbox/rustok-module.toml');
const plan = read('crates/rustok-outbox/docs/implementation-plan.md');
const central = read('docs/modules/registry.md');
const pkg = json('package.json');
const lib = read('crates/rustok-outbox/src/lib.rs');
const ports = read('crates/rustok-outbox/src/ports.rs');
if (pkg.scripts?.['verify:outbox:fba'] !== 'node scripts/verify/verify-outbox-fba.mjs && npm run verify:owner:fba-runtime-order') fail('package script verify:outbox:fba drift');
if (registry.schema_version !== 1 || registry.module !== 'outbox' || registry.role !== 'provider' || registry.status !== 'in_progress') fail('registry identity/status drift');
if (registry.contract_version !== 'outbox.relay_control.v1') fail('contract version drift');
const [port] = registry.ports ?? [];
if (!port || port.name !== 'OutboxRelayPort' || !port.operations.includes('process_pending_once')) fail('OutboxRelayPort operation missing');
if (port.context !== 'rustok_core::ports::PortContext' || port.error !== 'rustok_core::ports::PortError') fail('context/error drift');
if (port.deadline_required !== true || port.idempotency_required !== true) fail('relay control must keep deadline + write idempotency semantics');
if (!manifest.includes('[fba.provider]') || !manifest.includes('registry = "contracts/outbox-fba-registry.json"') || !manifest.includes('contract_version = "outbox.relay_control.v1"') || !manifest.includes('context = "rustok_core::ports::PortContext"') || !manifest.includes('error = "rustok_core::ports::PortError"')) fail('manifest FBA provider drift');
if (!lib.includes('pub mod ports;') || !lib.includes('pub use ports::*;')) fail('lib must export ports');
for (const marker of ['trait OutboxRelayPort', 'impl OutboxRelayPort for crate::OutboxRelay', 'require_outbox_relay_policy(&context)?', 'PortCallPolicy::write()', 'OutboxRelayRunOnceProjection', 'PortErrorKind::Validation', 'outbox.idempotency_required']) {
  if (!ports.includes(marker)) fail(`ports marker missing ${marker}`);
}
if (!JSON.stringify(registry).includes('relay_metrics_projection_preserved')) fail('registry missing relay metrics assertion');
if (!ports.includes('Serialize, Deserialize')) fail('FBA DTOs must be serializable');
if (!plan.includes('- FBA status: `in_progress`') || !plan.includes(registryPath) || !plan.includes('OutboxRelayPort') || !plan.includes('outbox-contract-test-static-matrix.json') || !plan.includes(registry.evidence.runtime_order_smoke)) fail('local plan FBA evidence drift');
if (!central.includes('| `outbox` |') || !central.includes(registryPath) || !central.includes(registry.evidence.runtime_order_smoke) || !central.includes('`in_progress` | `in_progress`')) fail('central readiness board drift');
if (evidence.schema_version !== 1 || evidence.module !== 'outbox' || evidence.status !== 'static_matrix_locked') fail('evidence identity drift');
if (evidence.generated_from !== registryPath || evidence.runner !== 'scripts/verify/verify-outbox-fba.mjs' || evidence.contract_version !== registry.contract_version) fail('evidence source/runner/version drift');
if (!sameSet(evidence.profiles, registry.contract_tests.profiles)) fail('evidence profile drift');
const rc = registry.contract_tests.cases.find((entry) => entry.operation === 'process_pending_once');
const ec = evidence.cases.find((entry) => entry.operation === 'process_pending_once');
if (!rc || !ec || ec.execution_status !== 'runtime_cases_planned_uncompiled' || !sameSet(ec.assertions, rc.assertions)) fail('process_pending_once evidence case drift');
if (!sameSet(evidence.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles)) fail('fallback profile drift');
if (runtimeOrderSmoke.generated_from !== registryPath || runtimeOrderSmoke.runner !== registry.evidence.runtime_order_smoke_runner || runtimeOrderSmoke.status !== 'executable_no_compile' || runtimeOrderSmoke.contract_version !== registry.contract_version) fail('runtime order smoke header drift');
if (!sameSet(runtimeOrderSmoke.fallback_profiles, registry.contract_tests.fallback_smoke.profiles)) fail('runtime order fallback profile drift');
if (!sameSet(runtimeOrderSmoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes)) fail('runtime order degraded mode drift');
for (const smokeCase of runtimeOrderSmoke.cases) {
  for (const marker of smokeCase.source_order) {
    if (!ports.includes(marker)) fail(`${smokeCase.operation} runtime order source marker missing ${marker}`);
  }
}
console.log('[verify-outbox-fba] Outbox FBA provider metadata, port semantics and static evidence are consistent');
