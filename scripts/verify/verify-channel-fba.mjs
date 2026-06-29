import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const json = (path) => JSON.parse(read(path));
const fail = (message) => { console.error(`[verify-channel-fba] ${message}`); process.exit(1); };
const sameSet = (actual, expected) => Array.isArray(actual) && Array.isArray(expected) && actual.length === expected.length && expected.every((item) => actual.includes(item));

const registryPath = 'crates/rustok-channel/contracts/channel-fba-registry.json';
const evidencePath = 'crates/rustok-channel/contracts/evidence/channel-contract-test-static-matrix.json';
const runtimeSmokePath = 'crates/rustok-channel/contracts/evidence/channel-runtime-fallback-smoke.json';
const registry = json(registryPath);
const evidence = json(evidencePath);
const runtimeSmoke = json(runtimeSmokePath);
const manifest = read('crates/rustok-channel/rustok-module.toml');
const plan = read('crates/rustok-channel/docs/implementation-plan.md');
const central = read('docs/modules/registry.md');
const lib = read('crates/rustok-channel/src/lib.rs');
const ports = read('crates/rustok-channel/src/ports.rs');

if (registry.schema_version !== 1) fail('registry schema_version must be 1');
if (registry.module !== 'channel' || registry.role !== 'provider' || registry.status !== 'boundary_ready') fail('registry identity/status drift');
if (registry.contract_version !== 'channel.read_projection.v1') fail('contract version drift');
const [port] = registry.ports ?? [];
if (!port || port.name !== 'ChannelReadPort') fail('ChannelReadPort missing');
for (const op of ['read_channel', 'list_channels_for_tenant']) {
  if (!port.operations.includes(op)) fail(`port lacks ${op}`);
}
if (port.context !== 'crates/rustok-channel/src/ports.rs::PortContext' || port.error !== 'crates/rustok-channel/src/ports.rs::PortError') fail('context/error drift');
if (port.deadline_required !== true || port.idempotency_required !== false || port.semantics !== 'read_only') fail('channel read projection must be read-only with deadline semantics');
if (!manifest.includes('[fba.provider]') || !manifest.includes('registry = "contracts/channel-fba-registry.json"') || !manifest.includes('contract_version = "channel.read_projection.v1"')) fail('manifest metadata drift');
if (!lib.includes('pub mod ports;') || !lib.includes('pub use ports::*;')) fail('lib.rs must export ports');
for (const marker of ['trait ChannelReadPort', 'impl ChannelReadPort for crate::ChannelService', 'context.require_policy(PortCallPolicy::read())?', 'ChannelReadRequest', 'ChannelListRequest', 'ChannelReadProjection', 'channel.slug_empty', 'channel.host_target_empty', 'channel.tenant_id_invalid', 'ensure_tenant_scope', 'PortErrorKind::Validation', 'PortContext', 'PortError']) {
  if (!ports.includes(marker)) fail(`ports source missing ${marker}`);
}
if (ports.includes('require_write_semantics()?')) fail('channel read port must not require write idempotency');
if (!ports.includes('Serialize, Deserialize')) fail('channel FBA DTOs must be serializable');
if (!plan.includes('- FBA status: `boundary_ready`') || !plan.includes(registryPath) || !plan.includes('ChannelReadPort') || !plan.includes('channel-contract-test-static-matrix.json') || !plan.includes('channel-runtime-fallback-smoke.json')) fail('local plan FBA evidence drift');
if (!central.includes('| `channel` |') || !central.includes(registryPath) || !central.includes('channel-runtime-fallback-smoke.json') || !central.includes('| `channel` | admin | `in_progress` | `boundary_ready`')) fail('central readiness board drift');
if (evidence.schema_version !== 1 || evidence.module !== 'channel' || evidence.status !== 'static_matrix_locked') fail('evidence identity drift');
if (evidence.generated_from !== registryPath || evidence.runner !== 'scripts/verify/verify-channel-fba.mjs' || evidence.contract_version !== registry.contract_version) fail('evidence source/runner/version drift');
if (!sameSet(evidence.profiles, registry.contract_tests.profiles)) fail('evidence profile drift');
for (const op of ['read_channel', 'list_channels_for_tenant']) {
  const registryCase = registry.contract_tests.cases.find((entry) => entry.operation === op);
  const evidenceCase = evidence.cases.find((entry) => entry.operation === op);
  if (!registryCase || !evidenceCase || evidenceCase.execution_status !== 'static_locked_runtime_pending' || !sameSet(evidenceCase.assertions, registryCase.assertions)) fail(`${op} evidence case drift`);
}
if (!sameSet(evidence.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles)) fail('fallback profile drift');
if (runtimeSmoke.schema_version !== 1 || runtimeSmoke.module !== 'channel' || runtimeSmoke.status !== 'no_compile_executable_runtime_fallback_smoke') fail('runtime smoke identity drift');
if (runtimeSmoke.generated_from !== registryPath || runtimeSmoke.runner !== 'scripts/verify/verify-channel-runtime-fallback-smoke.mjs' || runtimeSmoke.contract_version !== registry.contract_version) fail('runtime smoke source/runner/version drift');
if (!sameSet(runtimeSmoke.profiles, registry.contract_tests.fallback_smoke.profiles)) fail('runtime smoke profile drift');
for (const profile of registry.contract_tests.fallback_smoke.profiles) {
  if (!runtimeSmoke.smoke_cases.some((entry) => entry.profile === profile && entry.execution_status === 'no_compile_executable_locked')) fail(`runtime smoke missing executable no-compile profile ${profile}`);
}
for (const marker of ['impl ChannelReadPort for crate::ChannelService', 'context.require_policy(PortCallPolicy::read())?', 'ensure_tenant_scope', 'request.include_inactive || detail.channel.is_active', 'channel.tenant_id_invalid', 'channel.slug_empty', 'channel.host_target_empty']) {
  if (!ports.includes(marker)) fail(`runtime smoke source missing ${marker}`);
}
const transportFacade = read('crates/rustok-channel/admin/src/transport/mod.rs');
const nativeAdapter = read('crates/rustok-channel/admin/src/transport/native_server_adapter.rs');
const restAdapter = read('crates/rustok-channel/admin/src/transport/rest_adapter.rs');
if (!transportFacade.includes('native_server_adapter') || !transportFacade.includes('rest_adapter')) fail('runtime smoke transport facade drift');
if (!nativeAdapter.includes('#[server') || !restAdapter.includes('reqwest')) fail('runtime smoke native/rest adapter markers drift');
console.log('[verify-channel-fba] Channel FBA provider metadata, port semantics, static evidence and source-locked runtime fallback smoke are consistent');
