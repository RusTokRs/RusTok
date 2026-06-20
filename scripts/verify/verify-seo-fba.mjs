import fs from 'node:fs';

function read(path) { return fs.readFileSync(path, 'utf8'); }
function json(path) { return JSON.parse(read(path)); }
function fail(message) { console.error(`[verify-seo-fba] ${message}`); process.exit(1); }
function hasAll(text, snippets, label) { for (const snippet of snippets) if (!text.includes(snippet)) fail(`${label} missing ${snippet}`); }
function sameSet(actual, expected, label) {
  const a = [...actual].sort().join('|');
  const e = [...expected].sort().join('|');
  if (a !== e) fail(`${label} drift: expected ${e}, got ${a}`);
}

const registryPath = 'crates/rustok-seo/contracts/seo-fba-registry.json';
const evidencePath = 'crates/rustok-seo/contracts/evidence/seo-media-consumer-static-matrix.json';
const providerPath = 'crates/rustok-media/contracts/media-fba-registry.json';
const providerFallbackSmokePath = 'crates/rustok-media/contracts/evidence/media-runtime-fallback-smoke.json';
const registry = json(registryPath);
const evidence = json(evidencePath);
const provider = json(providerPath);
const providerFallbackSmoke = json(providerFallbackSmokePath);

if (registry.schema_version !== 1) fail('registry schema_version drift');
if (registry.module !== 'seo' || registry.role !== 'consumer' || registry.status !== 'in_progress') fail('registry identity/status drift');
if (registry.consumer_profile !== 'seo_image_descriptor') fail('consumer profile drift');
const dependency = registry.provider_dependencies?.[0];
if (!dependency) fail('missing media provider dependency');
if (dependency.module !== 'media' || dependency.registry !== providerPath) fail('provider dependency identity drift');
if (dependency.contract_version !== provider.contract_version || dependency.port !== 'MediaAssetReadPort') fail('provider contract/port drift');
if (provider.module !== 'media' || provider.role !== 'provider' || provider.status !== 'in_progress') fail('media provider status drift');
const providerOperations = provider.ports?.[0]?.operations ?? [];
for (const operation of dependency.operations) if (!providerOperations.includes(operation)) fail(`consumer operation ${operation} is absent from media provider`);
const mediaConsumer = provider.consumers?.find(c => c.module === 'seo');
if (!mediaConsumer) fail('media provider registry lacks seo consumer profile');
sameSet(dependency.fallback_profiles, mediaConsumer.fallback_profiles, 'consumer/provider fallback profiles');
sameSet(dependency.degraded_modes, mediaConsumer.degraded_modes, 'consumer/provider degraded modes');
if (dependency.context !== 'rustok_api::ports::PortContext' || dependency.error !== 'rustok_api::ports::PortError') fail('consumer context/error drift');

const manifest = read('crates/rustok-seo/rustok-module.toml');
hasAll(manifest, ['[fba.consumer]', 'registry = "contracts/seo-fba-registry.json"', 'profile = "seo_image_descriptor"', 'media.asset_read.v1'], 'manifest');

if (registry.evidence?.provider_fallback_smoke !== providerFallbackSmokePath) fail('registry missing provider fallback smoke source');
if (evidence.generated_from !== registryPath || evidence.status !== registry.contract_tests.fallback_smoke.status) fail('evidence header drift');
sameSet(evidence.cases.map(c => c.operation), registry.contract_tests.cases.map(c => c.operation), 'evidence/registry cases');
sameSet(evidence.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles, 'fallback profiles');
sameSet(evidence.fallback_smoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes, 'degraded modes');
if (registry.contract_tests.fallback_smoke.provider_source !== providerFallbackSmokePath) fail('registry fallback smoke provider source drift');
if (evidence.fallback_smoke.provider_source !== providerFallbackSmokePath) fail('evidence fallback smoke provider source drift');
if (providerFallbackSmoke.port !== 'MediaAssetReadPort' || providerFallbackSmoke.operation !== 'get_image_descriptor') fail('provider fallback smoke identity drift');
sameSet(evidence.fallback_smoke.profiles, [providerFallbackSmoke.profile], 'provider/evidence fallback profiles');
const providerFallbackSmokeModes = (providerFallbackSmoke.degraded_modes ?? []).map(mode => mode.name);
for (const mode of evidence.fallback_smoke.degraded_modes) {
  if (!providerFallbackSmokeModes.includes(mode)) fail(`provider fallback smoke lacks degraded mode ${mode}`);
}
if ((evidence.static_source_assertions ?? []).length < 4) fail('missing static source assertions');
for (const row of evidence.static_source_assertions) {
  if (!row.path || !(row.must_contain ?? []).length) fail(`invalid static assertion row ${row.name ?? '<unnamed>'}`);
  hasAll(read(row.path), row.must_contain, `static assertion ${row.name}`);
}
if ((evidence.runtime_closeout_required ?? []).length < 4) fail('runtime closeout requirements are incomplete');

const plan = read('crates/rustok-seo/docs/implementation-plan.md');
hasAll(plan, ['- FBA status: `in_progress`', 'seo-fba-registry.json', 'MediaAssetReadPort', 'seo-media-consumer-static-matrix.json', 'source_locked_pending_consumer_runtime'], 'local plan');
const central = read('docs/modules/registry.md');
hasAll(central, ['| `seo` |', 'crates/rustok-seo/contracts/seo-fba-registry.json', '`in_progress` | `in_progress`'], 'central registry');
const unified = read('docs/research/fluid-backend-architecture-unified-plan.md');
hasAll(unified, ['`seo`', 'MediaAssetReadPort', 'seo-fba-registry.json', 'source_locked_pending_consumer_runtime'], 'unified plan');

console.log('[verify-seo-fba] seo FBA media consumer metadata and static evidence are consistent');
