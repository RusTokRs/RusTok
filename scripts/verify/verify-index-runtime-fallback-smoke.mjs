import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const json = (path) => JSON.parse(read(path));
const fail = (message) => { console.error(`[verify-index-runtime-fallback-smoke] ${message}`); process.exit(1); };

const smokePath = 'crates/rustok-index/contracts/evidence/index-runtime-fallback-smoke.json';
const registryPath = 'crates/rustok-index/contracts/index-fba-registry.json';
const smoke = json(smokePath);
const registry = json(registryPath);
const ports = read('crates/rustok-index/src/ports.rs');
const adminTransport = read('crates/rustok-index/admin/src/transport/mod.rs');
const nativeAdapter = read('crates/rustok-index/admin/src/transport/native_server_adapter.rs');

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

if (smoke.schema_version !== 1 || smoke.module !== 'index') fail('runtime smoke identity drift');
if (!['no_compile_executable_runtime_fallback_smoke', 'no_compile_source_locked_runtime_adapter_smoke'].includes(smoke.status)) fail('runtime smoke status drift');
if (smoke.generated_from !== registryPath || smoke.contract_version !== registry.contract_version) fail('runtime smoke registry/version drift');
if (smoke.runner !== 'scripts/verify/verify-index-runtime-fallback-smoke.mjs') fail('runtime smoke runner drift');
for (const profile of registry.contract_tests.fallback_smoke.profiles) {
  if (!smoke.profiles.includes(profile)) fail(`runtime smoke missing profile ${profile}`);
}

const readCase = requireCase('embedded_native', 'read_index_document');
for (const assertion of ['read_policy_required_before_lookup', 'document_id_selector_supported', 'slug_selector_validates_doc_type_locale_slug', 'tenant_scope_preserved', 'index_search_boundary_preserved', 'in_process_read_adapter_filters_selector']) requireAssertion(readCase, assertion);
requireSource(ports, 'validate_index_read_request(request)?;', 'ports');
requireSource(ports, 'require_index_read_policy(context)?;', 'ports');
requireSource(ports, 'IndexReadSelector::DocumentId', 'ports');
requireSource(ports, 'IndexReadSelector::Slug', 'ports');
requireSource(ports, 'index.read_selector_slug_empty', 'ports');
requireSource(ports, 'index.read_selector_locale_empty', 'ports');
requireSource(ports, 'index.read_selector_doc_type_empty', 'ports');
requireSource(ports, 'ensure_index_document_tenant_scope', 'ports');
requireSource(ports, 'impl IndexReadModelPort for InProcessIndexReadModelAdapter', 'ports');
requireSource(ports, 'Self::matches_selector(document, &request.selector)', 'ports');
requireSource(ports, 'parse_index_context_tenant_id(&context)?', 'ports');

const listCase = requireCase('embedded_native', 'list_index_documents');
for (const assertion of ['read_policy_required_before_list', 'bounded_limit_preserved', 'locale_filter_optional', 'tenant_scope_preserved', 'index_search_boundary_preserved', 'in_process_list_adapter_filters_tenant_type_locale_limit']) requireAssertion(listCase, assertion);
requireSource(ports, 'validate_index_list_request(request)?;', 'ports');
requireSource(ports, 'const MAX_INDEX_LIST_LIMIT: u32 = 100;', 'ports');
requireSource(ports, 'index.list_limit_invalid', 'ports');
requireSource(ports, 'index.list_limit_too_large', 'ports');

const adminCase = requireCase('admin_read_only', 'request_rebuild');
for (const assertion of ['write_policy_required_before_rebuild', 'dry_run_preserved', 'owner_module_validation', 'entity_type_validation', 'typed_disabled_error_available']) requireAssertion(adminCase, assertion);
requireSource(ports, 'validate_index_rebuild_request(request)?;', 'ports');
requireSource(ports, 'require_index_rebuild_policy(context)?;', 'ports');
requireSource(ports, 'request.dry_run', 'ports');
requireSource(ports, 'index.rebuild_owner_module_empty', 'ports');
requireSource(ports, 'index.rebuild_entity_type_empty', 'ports');
requireSource(ports, 'index.rebuild_disabled', 'ports');

const disabledCase = requireCase('rebuild_disabled', 'request_rebuild');
for (const assertion of ['rebuild_disabled_maps_to_unavailable', 'idempotency_required_by_write_policy', 'deadline_required_by_write_policy', 'tenant_scope_preserved']) requireAssertion(disabledCase, assertion);
requireSource(ports, 'PortErrorKind::Unavailable', 'ports');
requireSource(ports, 'PortCallPolicy::write()', 'ports');
requireSource(ports, 'PortCallPolicy::read()', 'ports');
requireSource(ports, 'impl IndexRebuildPort for RebuildDisabledIndexAdapter', 'ports');
requireSource(ports, 'Err(index_rebuild_disabled_error())', 'ports');
requireSource(adminTransport, 'fetch_bootstrap', 'admin transport facade');
requireSource(nativeAdapter, '#[server', 'native adapter');

console.log('[verify-index-runtime-fallback-smoke] Index no-compile runtime fallback smoke is executable and source-locked');
