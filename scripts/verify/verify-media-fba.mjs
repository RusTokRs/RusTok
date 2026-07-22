import fs from 'node:fs';

function read(path) { return fs.readFileSync(path, 'utf8'); }
function json(path) { return JSON.parse(read(path)); }
function fail(message) { console.error(`[verify-media-fba] ${message}`); process.exit(1); }
function hasAll(text, snippets, label) { for (const snippet of snippets) if (!text.includes(snippet)) fail(`${label} missing ${snippet}`); }
function sameSet(actual, expected, label) {
  const a = [...actual].sort().join('|');
  const e = [...expected].sort().join('|');
  if (a !== e) fail(`${label} drift: expected ${e}, got ${a}`);
}

const registryPath = 'crates/rustok-media/contracts/media-fba-registry.json';
const evidencePath = 'crates/rustok-media/contracts/evidence/media-contract-test-static-matrix.json';
const fallbackSmokePath = 'crates/rustok-media/contracts/evidence/media-runtime-fallback-smoke.json';
const portErrorMatrixPath = 'crates/rustok-media/contracts/evidence/media-port-error-matrix.json';
const registry = json(registryPath);
const evidence = json(evidencePath);
const fallbackSmoke = json(fallbackSmokePath);
const portErrorMatrix = json(portErrorMatrixPath);
const runtimeOrderSmoke = json(registry.evidence.runtime_order_smoke);

if (registry.schema_version !== 1) fail('registry schema_version drift');
if (registry.module !== 'media' || registry.role !== 'provider' || !['in_progress', 'boundary_ready'].includes(registry.status)) fail('registry identity/status drift');
if (registry.contract_version !== 'media.asset_read.v1') fail('contract_version drift');
if (registry.deployment_topology?.current_class !== 'modular_monolith' || registry.deployment_topology?.extraction_class !== 'whole_module_service' || registry.deployment_topology?.remote_transport !== 'grpc' || registry.deployment_topology?.remote_status !== 'loopback_verified') fail('media extraction topology drift');
sameSet(registry.deployment_topology.split_blockers, ['isolated_database_storage_evidence'], 'media split blockers');
if (registry.evidence?.runtime_fallback_smoke !== fallbackSmokePath) fail('runtime fallback smoke evidence drift');
if (registry.evidence?.port_error_matrix !== portErrorMatrixPath) fail('port error matrix evidence drift');
const port = registry.ports?.find((candidate) => candidate.name === 'MediaAssetReadPort');
if (!port) fail('read port name drift');
sameSet(port.operations, ['get_asset', 'list_assets', 'get_image_descriptor', 'get_translations'], 'port operations');
sameSet(port.read_operations, port.operations, 'read operations');
if ((port.write_operations ?? []).length !== 0 || port.idempotency_required !== false) fail('media read port unexpectedly declares write semantics');
if (port.context !== 'rustok_api::ports::PortContext' || port.error !== 'rustok_api::ports::PortError') fail('port context/error drift');
const writePort = registry.ports?.find((candidate) => candidate.name === 'MediaAssetWritePort');
if (!writePort || writePort.contract_version !== 'media.asset_write.v1') fail('write port identity/version drift');
sameSet(writePort.operations, ['prepare_upload', 'complete_upload', 'delete_asset', 'upsert_translation', 'reconcile_storage'], 'write port operations');
sameSet(writePort.write_operations, writePort.operations, 'write operations');
if ((writePort.read_operations ?? []).length !== 0 || writePort.idempotency_required !== true || writePort.deadline_required !== true) fail('media write port policy drift');
sameSet(writePort.upload_body_transport, ['media_owned_streaming_rest', 'presigned_object_store'], 'upload body transport');

const manifest = read('crates/rustok-media/rustok-module.toml');
hasAll(manifest, ['[fba.provider]', 'registry = "contracts/media-fba-registry.json"', 'contract_version = "media.asset_read.v1"'], 'manifest');

const lib = read('crates/rustok-media/src/lib.rs');
hasAll(lib, ['pub mod ports;', 'pub use ports::*;'], 'lib.rs');
const ports = read('crates/rustok-media/src/ports.rs');
const dto = read('crates/rustok-media/src/dto.rs');
hasAll(ports, ['pub trait MediaAssetReadPort', 'impl MediaAssetReadPort for MediaService', 'pub trait MediaAssetWritePort', 'impl MediaAssetWritePort for MediaService', 'MediaImageDescriptor', 'MediaUploadRequest', 'MEDIA_OWNER_STREAMING_UPLOAD_PATH', 'PortContext', 'PortError'], 'ports.rs');
const implStart = ports.indexOf('impl MediaAssetReadPort for MediaService');
if (implStart === -1) fail('ports.rs missing MediaService impl');
const implPorts = ports.slice(implStart);
for (const op of port.read_operations) {
  const idx = implPorts.indexOf(`async fn ${op}`);
  if (idx === -1) fail(`ports.rs missing read operation ${op}`);
  const next = implPorts.indexOf('\n    async fn ', idx + 1);
  const body = implPorts.slice(idx, next === -1 ? implPorts.length : next);
  if (!body.includes('context.require_policy(PortCallPolicy::read())?') && !body.includes('require_media_read_policy(&context)?')) fail(`${op} does not require shared read policy`);
  if (body.includes('context.require_write_semantics()?')) fail(`${op} unexpectedly requires write semantics`);
}

const writeImplStart = ports.indexOf('impl MediaAssetWritePort for MediaService');
if (writeImplStart === -1) fail('ports.rs missing MediaService write impl');
const writeImpl = ports.slice(writeImplStart);
for (const op of writePort.write_operations) {
  const idx = writeImpl.indexOf(`async fn ${op}`);
  if (idx === -1) fail(`ports.rs missing write operation ${op}`);
  const next = writeImpl.indexOf('\n    async fn ', idx + 1);
  const body = writeImpl.slice(idx, next === -1 ? writeImpl.length : next);
  if (!body.includes('require_media_write_policy(&context)?')) fail(`${op} does not require shared write policy`);
}
if (!ports.includes('context.require_policy(PortCallPolicy::write())')) fail('ports.rs missing explicit media write policy guard helper');

for (const mode of fallbackSmoke.degraded_modes) {
  if (!mode.source_marker || !mode.consumer_contract) fail(`fallback mode ${mode.name} is missing source marker/consumer contract`);
  if (!ports.includes(mode.source_marker) && !dto.includes(mode.source_marker)) fail(`fallback source marker not found for ${mode.name}`);
}
hasAll(dto, ['pub enum MediaAssetKind', 'pub enum MediaAssetUsageProfile', 'pub struct MediaAssetSummary', 'pub enum MediaImageDeliveryProfile', 'pub enum MediaImagePublicUrlPolicy', 'pub struct MediaImageDescriptor', 'pub fn from_parts', 'pub fn from_media_item', 'pub fn delivery_profile', 'pub fn is_publicly_addressable', 'pub fn public_url_policy', 'pub fn requires_public_proxy', 'pub fn should_emit_to_public_metadata', 'pub fn normalized_public_url', 'pub fn proxy_path', 'pub fn from_mime_type', 'pub fn is_streamable', 'pub fn is_public_metadata_ready', 'fn infer_mime_type', 'fn normalize_dimension'], 'dto.rs');

if (evidence.generated_from !== registryPath || evidence.status !== registry.contract_tests.status) fail('evidence header drift');
sameSet(evidence.cases.map(c => c.operation), registry.contract_tests.cases.map(c => c.operation), 'evidence/registry cases');
if (registry.contract_tests.status !== 'runtime_verified' || registry.contract_tests.runner !== 'cargo test -p rustok-media-transport --test port_conformance') fail('runtime conformance evidence drift');
sameSet(registry.contract_tests.profiles, ['in_process', 'loopback_grpc'], 'runtime conformance profiles');
for (const testCase of evidence.cases) if (testCase.runtime_evidence !== 'crates/rustok-media-transport/tests/port_conformance.rs') fail(`${testCase.operation} runtime evidence drift`);
sameSet(evidence.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles, 'fallback profiles');
sameSet(evidence.fallback_smoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes, 'degraded modes');
if (registry.contract_tests.fallback_smoke.status !== 'source_locked' || evidence.fallback_smoke.status !== 'source_locked') fail('fallback smoke status is not source_locked');
if (registry.contract_tests.fallback_smoke.source !== fallbackSmokePath || evidence.fallback_smoke.source !== fallbackSmokePath) fail('fallback smoke source drift');
if (fallbackSmoke.generated_from !== registryPath || fallbackSmoke.profile !== 'embedded_native' || fallbackSmoke.operation !== 'get_image_descriptor') fail('runtime fallback smoke header drift');
sameSet(fallbackSmoke.degraded_modes.map(mode => mode.name), registry.contract_tests.fallback_smoke.degraded_modes, 'runtime fallback degraded modes');

if (registry.contract_tests.port_error_matrix?.status !== 'source_locked' || registry.contract_tests.port_error_matrix?.source !== portErrorMatrixPath) fail('port error matrix registry drift');
if (portErrorMatrix.generated_from !== registryPath || portErrorMatrix.port !== 'MediaAssetReadPort' || portErrorMatrix.source !== 'crates/rustok-media/src/ports.rs') fail('port error matrix header drift');
sameSet(portErrorMatrix.error_mappings.map(mapping => mapping.code), [
  'media.not_found',
  'media.forbidden',
  'media.unsupported_mime_type',
  'media.file_too_large',
  'media.invalid_locale',
  'media.storage',
  'media.database',
], 'port error mapping codes');
sameSet(portErrorMatrix.context_guards.map(guard => guard.code), ['media.invalid_tenant_id', 'port.deadline_required'], 'port context guard codes');
for (const mapping of portErrorMatrix.error_mappings) {
  if (!ports.includes(mapping.code)) fail(`ports.rs missing port error code ${mapping.code}`);
}
if (!ports.includes('media.invalid_tenant_id')) fail('ports.rs missing invalid tenant context guard');
if (!ports.includes('fn require_media_read_policy') || !ports.includes('context.require_policy(PortCallPolicy::read())')) fail('ports.rs missing explicit media read policy guard helper');

const plan = read('crates/rustok-media/docs/implementation-plan.md');
hasAll(plan, ['- FBA status: `boundary_ready`', 'media-fba-registry.json', 'MediaAssetReadPort', 'MediaAssetWritePort', 'media-contract-test-static-matrix.json', 'media-runtime-fallback-smoke.json', 'media-port-error-matrix.json', 'public URL policy', 'MediaAssetSummary', 'whole-module extraction pilot', '2026-07-16-media-search-extraction-boundaries.md'], 'local plan');
const central = read('docs/modules/registry.md');
hasAll(central, ['| `media` |', 'MediaAssetWritePort', 'streaming REST', 'crates/rustok-media/contracts/media-fba-registry.json', registry.evidence.runtime_order_smoke, '`in_progress` | `boundary_ready`'], 'central registry');
const unified = read('docs/research/fluid-backend-architecture-unified-plan.md');
hasAll(unified, ['`media`', 'MediaAssetReadPort', 'media-fba-registry.json'], 'unified plan');

if (runtimeOrderSmoke.generated_from !== registryPath || runtimeOrderSmoke.runner !== registry.evidence.runtime_order_smoke_runner || runtimeOrderSmoke.status !== 'executable_no_compile' || runtimeOrderSmoke.contract_version !== registry.contract_version) fail('runtime order smoke header drift');
sameSet(runtimeOrderSmoke.fallback_profiles, registry.contract_tests.fallback_smoke.profiles, 'runtime order fallback profiles');
sameSet(runtimeOrderSmoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes, 'runtime order degraded modes');
for (const smokeCase of runtimeOrderSmoke.cases) {
  for (const marker of smokeCase.source_order) {
    if (!implPorts.includes(marker) && !ports.includes(marker)) fail(`${smokeCase.operation} runtime order source marker missing ${marker}`);
  }
}

console.log('[verify-media-fba] media FBA provider metadata, port semantics and static evidence are consistent');
