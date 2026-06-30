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
const serverTenantMiddleware = read('apps/server/src/middleware/tenant.rs');
const serverInstallerCli = read('apps/server/src/installer_cli.rs');

if (registry.schema_version !== 1) fail('registry schema_version must be 1');
if (registry.module !== 'tenant' || registry.role !== 'provider' || !['boundary_ready', 'transport_verified'].includes(registry.status)) fail('registry identity/status drift');
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
if (!plan.includes(`- FBA status: \`${registry.status}\``) || !plan.includes(registryPath) || !plan.includes('TenantReadPort') || !plan.includes('tenant-contract-test-static-matrix.json')) fail('local plan FBA evidence drift');
if (!central.includes('| `tenant` |') || !central.includes(registryPath) || !central.includes(`| \`tenant\` | admin | \`in_progress\` | \`${registry.status}\``)) fail('central readiness board drift');
if (registry.status === 'transport_verified' && evidence.status !== 'runtime_verified') fail('transport_verified tenant requires runtime_verified evidence');
if (evidence.schema_version !== 1 || evidence.module !== 'tenant' || evidence.status !== 'runtime_verified') fail('evidence identity drift');
if (evidence.generated_from !== registryPath || evidence.runner !== 'scripts/verify/verify-tenant-fba.mjs' || evidence.contract_version !== registry.contract_version) fail('evidence source/runner/version drift');
if (!sameSet(evidence.profiles, registry.contract_tests.profiles)) fail('evidence profile drift');
const registryCase = registry.contract_tests.cases.find((entry) => entry.operation === 'read_tenant');
const evidenceCase = evidence.cases.find((entry) => entry.operation === 'read_tenant');
if (!registryCase || !evidenceCase || evidenceCase.execution_status !== 'runtime_verified' || !sameSet(evidenceCase.assertions, registryCase.assertions)) fail('read_tenant evidence case drift');
if (evidence.fallback_smoke.status !== 'runtime_verified') fail('fallback smoke status drift');
if (!sameSet(evidence.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles)) fail('fallback profile drift');
if (evidence.host_integration?.status !== 'runtime_verified' || evidence.host_integration?.source !== 'apps/server/src/middleware/tenant.rs') fail('host integration evidence drift');
if (!registry.consumers?.some((entry) => entry.module === 'server-installer' && entry.profile === 'installer_provisioning_read_projection_by_slug')) fail('installer provisioning consumer metadata missing');
if (evidence.installer_integration?.status !== 'runtime_verified' || evidence.installer_integration?.source !== 'apps/server/src/installer_cli.rs') fail('installer integration evidence drift');
for (const marker of ['TenantReadPort', 'TenantService::new(ctx.db.clone())', 'tenant_read_request(&identifier)', 'tenant_read_context(&identifier)', '.read_tenant(tenant_port_context, tenant_request)', 'TenantReadSelector::Id', 'TenantReadSelector::Slug', 'TenantReadSelector::Domain', 'include_inactive: true', 'tenant_context_from_projection', 'CachedTenantMiss::Disabled', 'set_negative(negative_key_clone.clone(), CachedTenantMiss::Disabled)', 'set_negative(negative_key_clone.clone(), CachedTenantMiss::NotFound)', 'get_or_load_with_coalescing']) {
  if (!serverTenantMiddleware.includes(marker)) fail(`server tenant middleware missing ${marker}`);
}
for (const marker of ['TenantReadPort', 'TenantService::new(db.clone())', 'read_installer_tenant_by_slug(db, &plan.tenant.slug)', 'TenantReadSelector::Slug(slug.to_string())', 'include_inactive: true', '.with_deadline(INSTALLER_TENANT_READ_DEADLINE)', 'PortActor::service("rustok-server.installer")', 'PortErrorKind::NotFound', 'treat missing tenant as create candidate']) {
  if (!serverInstallerCli.includes(marker)) fail(`server installer CLI missing ${marker}`);
}
for (const marker of ['tenant_read_port_requires_deadline_and_valid_slug', 'tenant_read_port_preserves_projection_and_inactive_degraded_mode', 'tenant_read_port_resolves_domain_and_validates_blank_domain', 'PortErrorKind::Timeout', 'PortErrorKind::Validation', 'PortErrorKind::NotFound', 'include_inactive: true', 'TenantReadSelector::Domain', 'tenant.domain_empty']) {
  if (!integrationTests.includes(marker)) fail(`integration tests missing ${marker}`);
}
console.log('[verify-tenant-fba] Tenant FBA provider metadata, port semantics and static evidence are consistent');
