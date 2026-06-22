import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const json = (path) => JSON.parse(read(path));
const fail = (message) => { console.error(`[verify-tenant-fba] ${message}`); process.exit(1); };
const sameSet = (actual, expected) => Array.isArray(actual) && Array.isArray(expected) && actual.length === expected.length && expected.every((item) => actual.includes(item));

const registryPath = 'crates/rustok-tenant/contracts/tenant-fba-registry.json';
const evidencePath = 'crates/rustok-tenant/contracts/evidence/tenant-contract-test-static-matrix.json';
const registry = json(registryPath);
const evidence = json(evidencePath);
const manifest = read('crates/rustok-tenant/rustok-module.toml');
const plan = read('crates/rustok-tenant/docs/implementation-plan.md');
const central = read('docs/modules/registry.md');
const cargo = read('crates/rustok-tenant/Cargo.toml');
const lib = read('crates/rustok-tenant/src/lib.rs');
const ports = read('crates/rustok-tenant/src/ports.rs');
const integrationTests = read('crates/rustok-tenant/tests/integration.rs');

if (registry.schema_version !== 1) fail('registry schema_version must be 1');
if (registry.module !== 'tenant' || registry.role !== 'provider' || registry.status !== 'in_progress') fail('registry identity/status drift');
if (registry.contract_version !== 'tenant.read_projection.v1') fail('contract version drift');
const [port] = registry.ports ?? [];
if (!port || port.name !== 'TenantReadPort') fail('TenantReadPort missing');
if (!port.operations.includes('read_tenant')) fail('port lacks read_tenant');
if (port.context !== 'crates/rustok-tenant/src/ports.rs::PortContext' || port.error !== 'crates/rustok-tenant/src/ports.rs::PortError') fail('context/error drift');
if (port.deadline_required !== true || port.idempotency_required !== false) fail('tenant read projection must be read-like with deadline semantics');
if (!manifest.includes('[fba.provider]') || !manifest.includes('registry = "contracts/tenant-fba-registry.json"') || !manifest.includes('contract_version = "tenant.read_projection.v1"')) fail('manifest metadata drift');
if (!cargo.includes('rustok-api.workspace = true')) fail('tenant FBA provider must depend on shared rustok-api PortContext/PortError');
if (!lib.includes('pub mod ports;') || !lib.includes('pub use ports::*;')) fail('lib.rs must export ports');
for (const marker of ['trait TenantReadPort', 'impl TenantReadPort for crate::TenantService', 'context.require_policy(PortCallPolicy::read())?', 'TenantReadRequest', 'TenantReadProjection', 'TenantReadSelector::Domain', 'get_tenant_by_domain', 'tenant.slug_empty', 'tenant.domain_empty', 'PortErrorKind::Validation', 'PortContext', 'PortError']) {
  if (!ports.includes(marker)) fail(`ports source missing ${marker}`);
}
if (ports.includes('require_write_semantics()?')) fail('tenant read port must not require write idempotency');
if (!ports.includes('Serialize, Deserialize')) fail('tenant FBA DTOs must be serializable');
if (!plan.includes('- FBA status: `in_progress`') || !plan.includes(registryPath) || !plan.includes('TenantReadPort') || !plan.includes('tenant-contract-test-static-matrix.json')) fail('local plan FBA evidence drift');
if (!central.includes('| `tenant` |') || !central.includes(registryPath) || !central.includes('`in_progress` | `in_progress`')) fail('central readiness board drift');
if (evidence.schema_version !== 1 || evidence.module !== 'tenant' || evidence.status !== 'static_matrix_locked') fail('evidence identity drift');
if (evidence.generated_from !== registryPath || evidence.runner !== 'scripts/verify/verify-tenant-fba.mjs' || evidence.contract_version !== registry.contract_version) fail('evidence source/runner/version drift');
if (!sameSet(evidence.profiles, registry.contract_tests.profiles)) fail('evidence profile drift');
const registryCase = registry.contract_tests.cases.find((entry) => entry.operation === 'read_tenant');
const evidenceCase = evidence.cases.find((entry) => entry.operation === 'read_tenant');
if (!registryCase || !evidenceCase || evidenceCase.execution_status !== 'runtime_cases_authored_uncompiled' || !sameSet(evidenceCase.assertions, registryCase.assertions)) fail('read_tenant evidence case drift');
if (evidence.fallback_smoke.status !== 'runtime_smoke_authored_uncompiled') fail('fallback smoke status drift');
if (!sameSet(evidence.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles)) fail('fallback profile drift');
for (const marker of ['tenant_read_port_requires_deadline_and_valid_slug', 'tenant_read_port_preserves_projection_and_inactive_degraded_mode', 'tenant_read_port_resolves_domain_and_validates_blank_domain', 'PortErrorKind::Timeout', 'PortErrorKind::Validation', 'PortErrorKind::NotFound', 'include_inactive: true', 'TenantReadSelector::Domain', 'tenant.domain_empty']) {
  if (!integrationTests.includes(marker)) fail(`integration tests missing ${marker}`);
}
console.log('[verify-tenant-fba] Tenant FBA provider metadata, port semantics and static evidence are consistent');
