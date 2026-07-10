import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const json = (path) => JSON.parse(read(path));
const fail = (message) => { console.error(`[verify-index-fba] ${message}`); process.exit(1); };
const sameSet = (actual, expected) => Array.isArray(actual) && Array.isArray(expected) && actual.length === expected.length && expected.every((item) => actual.includes(item));

const registryPath = 'crates/rustok-index/contracts/index-fba-registry.json';
const evidencePath = 'crates/rustok-index/contracts/evidence/index-contract-test-static-matrix.json';
const runtimeSmokePath = 'crates/rustok-index/contracts/evidence/index-runtime-fallback-smoke.json';
const registry = json(registryPath);
const evidence = json(evidencePath);
const runtimeSmoke = json(runtimeSmokePath);
const manifest = read('crates/rustok-index/rustok-module.toml');
const plan = read('crates/rustok-index/docs/implementation-plan.md');
const central = read('docs/modules/registry.md');
const unified = read('docs/research/fluid-backend-architecture-unified-plan.md');
const lib = read('crates/rustok-index/src/lib.rs');
const ports = read('crates/rustok-index/src/ports.rs');

if (registry.schema_version !== 1) fail('registry schema_version must be 1');
if (registry.module !== 'index' || registry.role !== 'provider' || registry.status !== 'boundary_ready') fail('registry identity/status drift');
if (registry.contract_version !== 'index.read_model.v1') fail('contract version drift');
for (const [name, op] of [['IndexReadModelPort', 'read_index_document'], ['IndexReadModelPort', 'list_index_documents'], ['IndexRebuildPort', 'request_rebuild']]) {
  const port = registry.ports.find((entry) => entry.name === name);
  if (!port || !port.operations.includes(op)) fail(`${name} lacks ${op}`);
  if (port.context !== 'rustok_api::PortContext' || port.error !== 'rustok_api::PortError') fail(`${name} context/error drift`);
}
const readPort = registry.ports.find((entry) => entry.name === 'IndexReadModelPort');
const rebuildPort = registry.ports.find((entry) => entry.name === 'IndexRebuildPort');
if (readPort.deadline_required !== true || readPort.idempotency_required !== false || readPort.semantics !== 'read_only') fail('read model port must be read-only with deadline semantics');
if (rebuildPort.deadline_required !== true || rebuildPort.idempotency_required !== true || rebuildPort.semantics !== 'operator_write') fail('rebuild port must require deadline + idempotency semantics');
if (!manifest.includes('[fba.provider]') || !manifest.includes('registry = "contracts/index-fba-registry.json"') || !manifest.includes('contract_version = "index.read_model.v1"')) fail('manifest metadata drift');
if (!lib.includes('pub mod ports;') || !lib.includes('pub use ports::*;')) fail('lib.rs must export ports');
for (const marker of ['trait IndexReadModelPort', 'trait IndexRebuildPort', 'PortCallPolicy::read()', 'PortCallPolicy::write()', 'IndexReadRequest', 'IndexListRequest', 'IndexRebuildRequest', 'IndexRebuildOutcome', 'IndexDocument', 'PortErrorKind::Timeout', 'PortContext', 'PortError']) {
  if (!ports.includes(marker) && !registryPath.includes(marker) && !evidencePath.includes(marker)) fail(`source/metadata missing ${marker}`);
}
if (!ports.includes('Serialize, Deserialize')) fail('index FBA DTOs must be serializable');
if (!plan.includes('- FBA status: `boundary_ready`') || !plan.includes(registryPath) || !plan.includes('IndexReadModelPort') || !plan.includes('index-contract-test-static-matrix.json') || !plan.includes('index-runtime-fallback-smoke.json')) fail('local plan FBA evidence drift');
if (!central.includes('| `index` | admin | `in_progress` | `boundary_ready`') || !central.includes(registryPath) || !central.includes('index-runtime-fallback-smoke.json')) fail('central readiness board drift');
if (!unified.includes('`index` added as a provider track') || !unified.includes(registryPath)) fail('unified FBA plan drift');
if (evidence.schema_version !== 1 || evidence.module !== 'index' || evidence.status !== 'static_matrix_locked') fail('evidence identity drift');
if (evidence.generated_from !== registryPath || evidence.runner !== 'scripts/verify/verify-index-fba.mjs' || evidence.contract_version !== registry.contract_version) fail('evidence source/runner/version drift');
if (!sameSet(evidence.profiles, registry.contract_tests.profiles)) fail('evidence profile drift');
for (const registryCase of registry.contract_tests.cases) {
  const evidenceCase = evidence.cases.find((entry) => entry.operation === registryCase.operation);
  if (!evidenceCase || evidenceCase.execution_status !== 'static_locked_runtime_pending' || !sameSet(evidenceCase.assertions, registryCase.assertions)) fail(`${registryCase.operation} evidence case drift`);
}
if (!sameSet(evidence.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles)) fail('fallback profile drift');
if (runtimeSmoke.schema_version !== 1 || runtimeSmoke.module !== 'index' || !['no_compile_executable_runtime_fallback_smoke', 'no_compile_source_locked_runtime_adapter_smoke'].includes(runtimeSmoke.status)) fail('runtime smoke identity drift');
if (runtimeSmoke.generated_from !== registryPath || runtimeSmoke.runner !== 'scripts/verify/verify-index-runtime-fallback-smoke.mjs' || runtimeSmoke.contract_version !== registry.contract_version) fail('runtime smoke source/runner/version drift');
if (!sameSet(runtimeSmoke.profiles, registry.contract_tests.fallback_smoke.profiles)) fail('runtime smoke profile drift');
for (const profile of registry.contract_tests.fallback_smoke.profiles) {
  if (!runtimeSmoke.smoke_cases.some((entry) => entry.profile === profile && entry.execution_status === 'no_compile_executable_locked')) fail(`runtime smoke missing executable no-compile profile ${profile}`);
}
if (registry.in_process_provider_impl?.read_adapter !== 'InProcessIndexReadModelAdapter' || registry.in_process_provider_impl?.rebuild_adapter !== 'RebuildDisabledIndexAdapter') fail('in-process provider impl metadata drift');
for (const marker of ['validate_index_read_request', 'validate_index_list_request', 'validate_index_rebuild_request', 'ensure_index_document_tenant_scope', 'index.rebuild_disabled', 'impl IndexReadModelPort for InProcessIndexReadModelAdapter', 'impl IndexRebuildPort for RebuildDisabledIndexAdapter', 'parse_index_context_tenant_id']) {
  if (!ports.includes(marker)) fail(`runtime smoke source missing ${marker}`);
}
console.log('[verify-index-fba] Index FBA provider metadata, port semantics and static evidence and source-locked runtime fallback smoke are consistent');
