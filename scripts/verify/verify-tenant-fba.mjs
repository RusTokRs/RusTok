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
const dto = read('crates/rustok-tenant/src/dto/mod.rs');
const ports = read('crates/rustok-tenant/src/ports.rs');
const tenantService = read('crates/rustok-tenant/src/services/tenant_service.rs');
const integrationTests = read('crates/rustok-tenant/tests/integration.rs');
const localeConcurrencyPostgres = read('crates/rustok-tenant/tests/locale_policy_concurrency_postgres.rs');
const ensureConcurrencyPostgres = read('crates/rustok-tenant/tests/tenant_ensure_concurrency_postgres.rs');
const localePolicyMigration = read('crates/rustok-tenant/src/migrations/m20260726_000001_enforce_tenant_locale_policy.rs');
const serverTenantMiddleware = read('apps/server/src/middleware/tenant.rs');
const serverInstallerCli = read('apps/server/src/installer_execution.rs');
const installerPersistence = read('crates/rustok-installer-persistence/src/seaorm_ports.rs');
const authCli = read('crates/rustok-auth/cli/src/lib.rs');
const tenantAdminCargo = read('crates/rustok-tenant/admin/Cargo.toml');
const tenantAdminNative = read('crates/rustok-tenant/admin/src/transport/native_server_adapter.rs');
const storefrontEnabledModules = read('apps/storefront/src/shared/context/enabled_modules.rs');
const storefrontEnabledModulesNative = read('apps/storefront/src/shared/context/enabled_modules_native_server_adapter.rs');

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
for (const marker of [
  'let first_result = self',
  '.replace_locale_policy_owned(tenant_id, request.clone(), &idempotency_key)',
  'Err(crate::TenantError::LocalePolicyConflict { .. }) =>',
  '.replace_locale_policy_owned(tenant_id, request, &idempotency_key)',
  'durable receipt, rather than a stale',
]) {
  if (!ports.includes(marker)) fail(`tenant locale idempotency race guard missing ${marker}`);
}
if ((ports.match(/Err\(crate::TenantError::LocalePolicyConflict \{ \.\. \}\) =>/g) ?? []).length !== 1) fail('tenant locale idempotency recovery must perform exactly one bounded CAS retry');

for (const marker of [
  'pub struct TenantService {',
  'TransactionalEventBus::publish_root_in_tx(txn, tenant_id, None, event)',
  'DomainEvent::TenantCreated',
  'DomainEvent::TenantUpdated',
]) {
  if (!tenantService.includes(marker)) fail(`tenant lifecycle owner source missing ${marker}`);
}
for (const marker of [
  'let slug = input.slug.clone();',
  'match self.create_tenant(input).await',
  'self.get_tenant_by_slug(&slug).await',
  'Ok(existing) => Ok((existing, false))',
  'Err(_) => Err(error)',
]) {
  if (!tenantService.includes(marker)) fail(`tenant ensure concurrency replay missing ${marker}`);
}
for (const forbidden of [
  'event_bus: Option<TransactionalEventBus>',
  'pub fn with_event_bus',
  'if let Some(event_bus)',
  'pub async fn toggle_module(',
  'ToggleModuleInput',
  'TenantModuleActiveModel',
  'DomainEvent::TenantModuleToggled',
]) {
  if (tenantService.includes(forbidden)) fail(`tenant owner contains forbidden compatibility path ${forbidden}`);
}
if (dto.includes('struct ToggleModuleInput')) fail('tenant DTO surface must not expose ToggleModuleInput');
if (lib.includes('ToggleModuleInput')) fail('tenant crate root must not export ToggleModuleInput');
for (const forbidden of ['module_toggle_flow_legacy', '.toggle_module(', 'tenant.module.toggled']) {
  if (integrationTests.includes(forbidden)) fail(`tenant integration evidence preserves removed lifecycle bypass ${forbidden}`);
}
if (!installerPersistence.includes('TenantService::new(self.db.clone())') || !installerPersistence.includes('.ensure_tenant(CreateTenantInput')) {
  fail('installer seed path must delegate tenant creation to TenantService');
}

for (const marker of [
  'postgres_concurrent_ensure_tenant_replays_unique_winner',
  'pg_advisory_xact_lock',
  'tenant_ensure_insert_barrier',
  'service_a.ensure_tenant(input_a)',
  'service_b.ensure_tenant(input_b)',
  'wait_for_lock_waiters(',
  "wait_event_type = 'Lock'",
  'assert_eq!(first.0.id, second.0.id);',
  'assert_ne!(first.1, second.1);',
  'assert_eq!(tenant_count, 1);',
  'assert_eq!(locale_count, 1);',
  'event.event_type == "tenant.created"',
  '.or_else(|_| std::env::var("DATABASE_URL"))',
]) {
  if (!ensureConcurrencyPostgres.includes(marker)) fail(`PostgreSQL tenant ensure race evidence missing ${marker}`);
}

for (const marker of [
  'INSERT INTO tenant_locales (',
  'WHERE tl.tenant_id = t.id',
  'ADD COLUMN default_tenant_guard BINARY(16)',
  'CASE WHEN is_default THEN tenant_id ELSE NULL END',
  'ADD UNIQUE INDEX uq_tenant_locales_one_default (default_tenant_guard)',
]) {
  if (!localePolicyMigration.includes(marker)) fail(`tenant locale-policy migration guard missing ${marker}`);
}
if (localePolicyMigration.indexOf('INSERT INTO tenant_locales (') >= localePolicyMigration.indexOf('ADD CONSTRAINT ck_tenant_locales_default_enabled')) {
  fail('tenant locale-policy backfill must precede backend constraints');
}

for (const marker of [
  'let tenant_id = required_tenant_id(options)?;',
  'fn required_tenant_id(',
  '--tenant-id is required for oauth create-app',
  'oauth_create_app_requires_explicit_tenant_id',
  'oauth_create_app_rejects_invalid_tenant_id',
]) {
  if (!authCli.includes(marker)) fail(`auth CLI explicit tenant selection guard missing ${marker}`);
}
for (const forbidden of [
  'read_default_active_tenant(',
  'TenantService::new(db.clone())',
  'PortActor::system()',
  'oauth-create-app',
]) {
  if (authCli.includes(forbidden)) fail(`OAuth credential creation must not infer a tenant through ${forbidden}`);
}

if (!tenantAdminCargo.includes('"dep:rustok-modules"') || !tenantAdminCargo.includes('rustok-modules = { workspace = true, optional = true }')) {
  fail('tenant admin SSR boundary must depend on rustok-modules control-plane owner');
}
for (const marker of [
  'rustok_modules::ModuleControlPlane::new(db)',
  '.composition()',
  '.active_snapshot()',
  '.effective_policy(&registry, manifest.settings.default_enabled)',
  '.resolve_enabled(tenant.id)',
  'let enabled = effective_modules.contains(module.slug());',
  '"manifest-default"',
  '"policy-dependency"',
]) {
  if (!tenantAdminNative.includes(marker)) fail(`tenant admin effective module-policy guard missing ${marker}`);
}
for (const forbidden of [
  'enabled: if is_core',
  'explicit.unwrap_or(false)',
]) {
  if (tenantAdminNative.includes(forbidden)) fail(`tenant admin must not treat raw tenant_modules as effective policy through ${forbidden}`);
}

for (const marker of [
  'pub(crate) async fn list_enabled_modules()',
  'leptos_axum::extract::<rustok_api::TenantContext>()',
  '.list_tenant_modules(tenant.id)',
]) {
  if (!storefrontEnabledModulesNative.includes(marker)) fail(`storefront native module-state tenant guard missing ${marker}`);
}
for (const forbidden of ['tenant_slug: String', 'get_tenant_by_slug(', 'list_enabled_modules(tenant_slug)']) {
  if (storefrontEnabledModulesNative.includes(forbidden)) fail(`storefront native module-state read trusts caller tenant through ${forbidden}`);
}
for (const marker of [
  'UiTransportPath::NativeServer => fetch_enabled_modules_server().await',
  'UiTransportPath::Graphql =>',
  'let Some(tenant_slug) = configured_tenant_slug()',
  'fetch_enabled_modules_graphql(tenant_slug).await',
  'pub async fn fetch_enabled_modules_server()',
  'list_enabled_modules()',
]) {
  if (!storefrontEnabledModules.includes(marker)) fail(`storefront enabled-modules transport split missing ${marker}`);
}
if (storefrontEnabledModules.includes('fetch_enabled_modules_server(tenant_slug')) fail('native enabled-modules transport must not receive configured tenant slug');

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
if (evidence.installer_integration?.status !== 'runtime_verified' || evidence.installer_integration?.source !== 'apps/server/src/installer_execution.rs') fail('installer integration evidence drift');
for (const marker of ['TenantReadPort', 'TenantService::new(ctx.db_clone())', 'tenant_read_request(&identifier)', 'tenant_read_context(&identifier)', '.read_tenant(tenant_port_context, tenant_request)', 'TenantReadSelector::Id', 'TenantReadSelector::Slug', 'TenantReadSelector::Domain', 'include_inactive: true', 'tenant_context_from_projection', 'CachedTenantMiss::Disabled', 'set_negative(negative_key_clone.clone(), CachedTenantMiss::Disabled)', 'set_negative(negative_key_clone.clone(), CachedTenantMiss::NotFound)', 'get_or_load_with_coalescing']) {
  if (!serverTenantMiddleware.includes(marker)) fail(`server tenant middleware missing ${marker}`);
}
for (const marker of ['TenantReadPort', 'TenantService::new(db.clone())', 'read_installer_tenant_by_slug(db, &plan.tenant.slug)', 'TenantReadSelector::Slug(slug.to_string())', 'include_inactive: true', '.with_deadline(INSTALLER_TENANT_READ_DEADLINE)', 'PortActor::service("rustok-installer.execution")', 'PortErrorKind::NotFound', 'treat missing tenant as create candidate']) {
  if (!serverInstallerCli.includes(marker)) fail(`server installer CLI missing ${marker}`);
}
for (const marker of ['tenant_read_port_requires_deadline_and_valid_slug', 'tenant_read_port_preserves_projection_and_inactive_degraded_mode', 'tenant_read_port_resolves_domain_and_validates_blank_domain', 'tenant_locale_policy_port_replaces_with_cas_and_replays_idempotently', 'tenant_mutations_always_publish_outbox_events', 'PortErrorKind::Timeout', 'PortErrorKind::Validation', 'PortErrorKind::NotFound', 'include_inactive: true', 'TenantReadSelector::Domain', 'tenant.domain_empty']) {
  if (!integrationTests.includes(marker)) fail(`integration tests missing ${marker}`);
}
if (integrationTests.includes('TenantService::with_event_bus')) fail('integration evidence must exercise ordinary TenantService::new publication');
for (const marker of [
  'RUSTOK_TENANT_TEST_DATABASE_URL',
  'postgres_concurrent_locale_policy_requests_replay_one_durable_receipt',
  'wait_for_lock_waiters(',
  'FROM pg_stat_activity',
  "wait_event_type = 'Lock'",
  '.replace_locale_policy(context_a, request_a)',
  '.replace_locale_policy(context_b, request_b)',
  'blocker_transaction.commit().await?',
  'assert_eq!(first, second);',
  'assert_eq!(receipt_count, 1);',
  'assert_eq!(events.len(), 3);',
  'tenant.locale_policy_idempotency_conflict',
  '.or_else(|_| std::env::var("DATABASE_URL"))',
]) {
  if (!localeConcurrencyPostgres.includes(marker)) fail(`PostgreSQL tenant locale-policy race evidence missing ${marker}`);
}

console.log('[verify-tenant-fba] Tenant FBA metadata, concurrent ensure replay, removed lifecycle bypass, effective tenant-admin module policy, trusted storefront tenant scope, explicit OAuth tenant selection, cross-backend locale migration guards, PostgreSQL locale-policy race evidence, mandatory lifecycle outbox and static evidence are consistent');
