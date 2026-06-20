import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const json = (path) => JSON.parse(read(path));
const fail = (message) => { console.error(`[verify-region-fba] ${message}`); process.exit(1); };
const sameSet = (actual, expected) => Array.isArray(actual) && Array.isArray(expected) && actual.length === expected.length && expected.every((item) => actual.includes(item));

const registryPath = 'crates/rustok-region/contracts/region-fba-registry.json';
const evidencePath = 'crates/rustok-region/contracts/evidence/region-contract-test-static-matrix.json';
const registry = json(registryPath);
const evidence = json(evidencePath);
const manifest = read('crates/rustok-region/rustok-module.toml');
const plan = read('crates/rustok-region/docs/implementation-plan.md');
const central = read('docs/modules/registry.md');
const lib = read('crates/rustok-region/src/lib.rs');
const ports = read('crates/rustok-region/src/ports.rs');

if (registry.schema_version !== 1) fail('registry schema_version must be 1');
if (registry.module !== 'region' || registry.role !== 'provider' || registry.status !== 'in_progress') fail('registry identity/status drift');
if (registry.contract_version !== 'region.read_projection.v1') fail('contract version drift');
const [port] = registry.ports ?? [];
if (!port || port.name !== 'RegionReadPort') fail('RegionReadPort missing');
for (const op of ['read_region', 'list_regions_for_tenant']) {
  if (!port.operations.includes(op)) fail(`port lacks ${op}`);
}
if (port.context !== 'rustok_api::ports::PortContext' || port.error !== 'rustok_api::ports::PortError') fail('context/error drift');
if (port.deadline_required !== true || port.idempotency_required !== false || port.semantics !== 'read_only') fail('region read projection must be read-only with deadline semantics');
if (!manifest.includes('[fba.provider]') || !manifest.includes('registry = "contracts/region-fba-registry.json"') || !manifest.includes('contract_version = "region.read_projection.v1"') || !manifest.includes('context = "rustok_api::ports::PortContext"') || !manifest.includes('error = "rustok_api::ports::PortError"')) fail('manifest metadata drift');
if (!lib.includes('pub mod ports;') || !lib.includes('pub use ports::*;')) fail('lib.rs must export ports');
for (const marker of ['trait RegionReadPort', 'impl RegionReadPort for crate::RegionService', 'context.require_policy(PortCallPolicy::read())?', 'RegionReadRequest', 'RegionListRequest', 'RegionReadProjection', 'region.country_code_empty', 'region.tenant_id_invalid', 'PortContext', 'PortError']) {
  if (!ports.includes(marker)) fail(`ports source missing ${marker}`);
}
if (!ports.includes('use rustok_api::{PortCallPolicy, PortContext, PortError};')) fail('region port must import shared rustok-api primitives');
if (ports.includes('require_write_semantics()?')) fail('region read port must not require write idempotency');
if (!ports.includes('Serialize, Deserialize')) fail('region FBA DTOs must be serializable');
if (!plan.includes('- FBA status: `in_progress`') || !plan.includes(registryPath) || !plan.includes('RegionReadPort') || !plan.includes('region-contract-test-static-matrix.json')) fail('local plan FBA evidence drift');
if (!central.includes('| `region` |') || !central.includes(registryPath) || !central.includes('| `region` | admin + storefront | `in_progress` | `in_progress`')) fail('central readiness board drift');
if (evidence.schema_version !== 1 || evidence.module !== 'region' || evidence.status !== 'static_matrix_locked') fail('evidence identity drift');
if (evidence.generated_from !== registryPath || evidence.runner !== 'scripts/verify/verify-region-fba.mjs' || evidence.contract_version !== registry.contract_version) fail('evidence source/runner/version drift');
if (!sameSet(evidence.profiles, registry.contract_tests.profiles)) fail('evidence profile drift');
for (const op of ['read_region', 'list_regions_for_tenant']) {
  const registryCase = registry.contract_tests.cases.find((entry) => entry.operation === op);
  const evidenceCase = evidence.cases.find((entry) => entry.operation === op);
  if (!registryCase || !evidenceCase || evidenceCase.execution_status !== 'static_locked_runtime_pending' || !sameSet(evidenceCase.assertions, registryCase.assertions)) fail(`${op} evidence case drift`);
}
if (!sameSet(evidence.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles)) fail('fallback profile drift');
console.log('[verify-region-fba] Region FBA provider metadata, port semantics and static evidence are consistent');
