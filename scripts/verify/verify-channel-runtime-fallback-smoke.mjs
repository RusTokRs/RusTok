import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const json = (path) => JSON.parse(read(path));
const fail = (message) => { console.error(`[verify-channel-runtime-fallback-smoke] ${message}`); process.exit(1); };

const smokePath = 'crates/rustok-channel/contracts/evidence/channel-runtime-fallback-smoke.json';
const registryPath = 'crates/rustok-channel/contracts/channel-fba-registry.json';
const smoke = json(smokePath);
const registry = json(registryPath);
const ports = read('crates/rustok-channel/src/ports.rs');
const transportFacade = read('crates/rustok-channel/admin/src/transport/mod.rs');
const nativeAdapter = read('crates/rustok-channel/admin/src/transport/native_server_adapter.rs');
const restAdapter = read('crates/rustok-channel/admin/src/transport/rest_adapter.rs');
const adminBoundaryVerifier = read('scripts/verify/verify-channel-admin-boundary.mjs');

const requireCase = (profile, operation) => {
  const found = smoke.smoke_cases.find((entry) => entry.profile === profile && entry.operation === operation);
  if (!found) fail(`missing smoke case ${profile}/${operation}`);
  if (found.execution_status !== 'no_compile_executable_locked') fail(`${profile}/${operation} must be no_compile_executable_locked`);
  return found;
};
const requireAssertion = (entry, assertion) => {
  if (!entry.assertions.includes(assertion)) fail(`${entry.profile}/${entry.operation} lacks assertion ${assertion}`);
};
const requireSource = (source, marker, label) => {
  if (!source.includes(marker)) fail(`${label} source missing ${marker}`);
};

if (smoke.schema_version !== 1 || smoke.module !== 'channel') fail('runtime smoke identity drift');
if (smoke.status !== 'no_compile_executable_runtime_fallback_smoke') fail('runtime smoke status drift');
if (smoke.generated_from !== registryPath || smoke.contract_version !== registry.contract_version) fail('runtime smoke registry/version drift');
if (smoke.runner !== 'scripts/verify/verify-channel-runtime-fallback-smoke.mjs') fail('runtime smoke runner drift');
for (const profile of registry.contract_tests.fallback_smoke.profiles) {
  if (!smoke.profiles.includes(profile)) fail(`runtime smoke missing profile ${profile}`);
}

const readCase = requireCase('embedded_native', 'read_channel');
for (const assertion of ['in_process_channel_service_impl_exported', 'deadline_required_before_lookup', 'tenant_scope_preserved_for_id_selector', 'inactive_channel_filtered_when_include_inactive_false', 'selector_validation_before_lookup']) requireAssertion(readCase, assertion);
requireSource(ports, 'impl ChannelReadPort for crate::ChannelService', 'ports');
requireSource(ports, 'context.require_policy(PortCallPolicy::read())?;', 'ports');
requireSource(ports, 'ensure_tenant_scope(tenant_id, &detail)?;', 'ports');
requireSource(ports, 'if !request.include_inactive && !detail.channel.is_active', 'ports');
requireSource(ports, 'validate_channel_read_request(&request)?;', 'ports');

const listCase = requireCase('embedded_native', 'list_channels_for_tenant');
for (const assertion of ['in_process_channel_service_impl_exported', 'deadline_required_before_lookup', 'tenant_scope_preserved_by_list_service', 'inactive_channels_filtered_when_include_inactive_false']) requireAssertion(listCase, assertion);
requireSource(ports, 'self.list_channel_details(tenant_id)', 'ports');
requireSource(ports, '.filter(|detail| request.include_inactive || detail.channel.is_active)', 'ports');

const restCase = requireCase('rest_compatibility', 'admin_transport_fallback');
for (const assertion of ['module_owned_transport_facade_present', 'native_server_adapter_present', 'rest_adapter_present', 'ui_uses_facade_not_raw_rest']) requireAssertion(restCase, assertion);
requireSource(transportFacade, 'mod native_server_adapter;', 'transport facade');
requireSource(transportFacade, 'mod rest_adapter;', 'transport facade');
requireSource(nativeAdapter, '#[server', 'native adapter');
requireSource(restAdapter, 'reqwest', 'rest adapter');
requireSource(adminBoundaryVerifier, 'UI adapter must not call raw/pre-FFA transport', 'admin boundary verifier');

const unresolvedCase = requireCase('unresolved_context', 'read_channel');
for (const assertion of ['invalid_tenant_context_maps_to_validation', 'missing_deadline_maps_to_timeout', 'empty_slug_selector_maps_to_validation', 'empty_host_target_selector_maps_to_validation']) requireAssertion(unresolvedCase, assertion);
requireSource(ports, 'channel.tenant_id_invalid', 'ports');
requireSource(ports, 'PortErrorKind::Validation', 'ports');
requireSource(ports, 'channel.slug_empty', 'ports');
requireSource(ports, 'channel.host_target_empty', 'ports');
requireSource(ports, 'PortCallPolicy::read()', 'ports');

console.log('[verify-channel-runtime-fallback-smoke] Channel no-compile runtime fallback smoke is executable and source-locked');
