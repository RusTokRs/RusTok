import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const json = (path) => JSON.parse(read(path));
const fail = (message) => { console.error(`[verify-rbac-fba] ${message}`); process.exit(1); };
const sameSet = (actual, expected) => Array.isArray(actual) && Array.isArray(expected) && actual.length === expected.length && expected.every((item) => actual.includes(item));

const registryPath = 'crates/rustok-rbac/contracts/rbac-fba-registry.json';
const evidencePath = 'crates/rustok-rbac/contracts/evidence/rbac-contract-test-static-matrix.json';
const registry = json(registryPath);
const evidence = json(evidencePath);
const manifest = read('crates/rustok-rbac/rustok-module.toml');
const plan = read('crates/rustok-rbac/docs/implementation-plan.md');
const central = read('docs/modules/registry.md');
const cargo = read('crates/rustok-rbac/Cargo.toml');
const lib = read('crates/rustok-rbac/src/lib.rs');
const ports = read('crates/rustok-rbac/src/ports.rs');

if (registry.schema_version !== 1) fail('registry schema_version must be 1');
if (registry.module !== 'rbac' || registry.role !== 'provider' || registry.status !== 'in_progress') fail('registry identity/status drift');
if (registry.contract_version !== 'rbac.permission_decision.v1') fail('contract version drift');
const [port] = registry.ports ?? [];
if (!port || port.name !== 'RbacPermissionDecisionPort') fail('RbacPermissionDecisionPort missing');
if (!port.operations.includes('check_permissions')) fail('port lacks check_permissions');
if (port.context !== 'rustok_api::ports::PortContext' || port.error !== 'rustok_api::ports::PortError') fail('context/error drift');
if (port.deadline_required !== true || port.idempotency_required !== false) fail('permission decision must be read-like with deadline semantics');
if (!manifest.includes('[fba.provider]') || !manifest.includes('registry = "contracts/rbac-fba-registry.json"') || !manifest.includes('contract_version = "rbac.permission_decision.v1"')) fail('manifest metadata drift');
if (!cargo.includes('rustok-api.workspace = true')) fail('Cargo.toml must depend on rustok-api');
if (!lib.includes('pub mod ports;') || !lib.includes('pub use ports::*;')) fail('lib.rs must export ports');
for (const marker of ['trait RbacPermissionDecisionPort', 'impl RbacPermissionDecisionPort for crate::RbacModule', 'context.require_policy(PortCallPolicy::read())?', 'RbacPermissionCheckRequest', 'RbacPermissionCheckResponse', 'rbac.permissions_empty', 'PortErrorKind::Validation']) {
  if (!ports.includes(marker)) fail(`ports source missing ${marker}`);
}
if (ports.includes('require_write_semantics()?')) fail('RBAC decision port must not require write idempotency');
if (!ports.includes('Serialize, Deserialize')) fail('RBAC FBA DTOs must be serializable');
if (!plan.includes('- FBA status: `in_progress`') || !plan.includes(registryPath) || !plan.includes('RbacPermissionDecisionPort') || !plan.includes('rbac-contract-test-static-matrix.json')) fail('local plan FBA evidence drift');
if (!central.includes('| `rbac` |') || !central.includes(registryPath) || !central.includes('`in_progress` | `in_progress`')) fail('central readiness board drift');
if (evidence.schema_version !== 1 || evidence.module !== 'rbac' || evidence.status !== 'static_matrix_locked') fail('evidence identity drift');
if (evidence.generated_from !== registryPath || evidence.runner !== 'scripts/verify/verify-rbac-fba.mjs' || evidence.contract_version !== registry.contract_version) fail('evidence source/runner/version drift');
if (!sameSet(evidence.profiles, registry.contract_tests.profiles)) fail('evidence profile drift');
const registryCase = registry.contract_tests.cases.find((entry) => entry.operation === 'check_permissions');
const evidenceCase = evidence.cases.find((entry) => entry.operation === 'check_permissions');
if (!registryCase || !evidenceCase || evidenceCase.execution_status !== 'static_locked_runtime_pending' || !sameSet(evidenceCase.assertions, registryCase.assertions)) fail('check_permissions evidence case drift');
if (!sameSet(evidence.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles)) fail('fallback profile drift');
console.log('[verify-rbac-fba] RBAC FBA provider metadata, port semantics and static evidence are consistent');
