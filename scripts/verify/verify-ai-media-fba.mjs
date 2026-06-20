import fs from 'node:fs';

function read(path) { return fs.readFileSync(path, 'utf8'); }
function json(path) { return JSON.parse(read(path)); }
function fail(message) { console.error(`[verify-ai-media-fba] ${message}`); process.exit(1); }
function hasAll(text, snippets, label) { for (const snippet of snippets) if (!text.includes(snippet)) fail(`${label} missing ${snippet}`); }
function sameSet(actual, expected, label) {
  const a = [...actual].sort().join('|');
  const e = [...expected].sort().join('|');
  if (a !== e) fail(`${label} drift: expected ${e}, got ${a}`);
}

const registryPath = 'crates/rustok-ai-media/contracts/ai-media-fba-registry.json';
const evidencePath = 'crates/rustok-ai-media/contracts/evidence/ai-media-consumer-static-matrix.json';
const fallbackSmokePath = 'crates/rustok-ai-media/contracts/evidence/ai-media-runtime-fallback-smoke.json';
const providerPath = 'crates/rustok-media/contracts/media-fba-registry.json';
const registry = json(registryPath);
const evidence = json(evidencePath);
const fallbackSmoke = json(fallbackSmokePath);
const provider = json(providerPath);

if (registry.schema_version !== 1) fail('registry schema_version drift');
if (registry.module !== 'ai-media' || registry.crate !== 'rustok-ai-media' || registry.role !== 'consumer_support_adapter' || registry.status !== 'in_progress') fail('registry identity/status drift');
if (registry.consumer_profile !== 'ai_asset_descriptor') fail('consumer profile drift');
const dependency = registry.provider_dependencies?.[0];
if (!dependency) fail('missing media provider dependency');
if (dependency.module !== 'media' || dependency.registry !== providerPath) fail('provider dependency identity drift');
if (dependency.contract_version !== provider.contract_version || dependency.port !== 'MediaAssetReadPort') fail('provider contract/port drift');
const mediaConsumer = provider.consumers?.find(c => c.module === 'ai-media');
if (!mediaConsumer) fail('media provider registry lacks ai-media consumer profile');
sameSet(dependency.fallback_profiles, mediaConsumer.fallback_profiles, 'consumer/provider fallback profiles');
sameSet(dependency.degraded_modes, mediaConsumer.degraded_modes, 'consumer/provider degraded modes');
for (const operation of dependency.operations) if (!(provider.ports?.[0]?.operations ?? []).includes(operation)) fail(`consumer operation ${operation} is absent from media provider`);

const source = read(registry.support_adapter.source);
hasAll(source, [
  'IMAGE_ASSET_TASK_SLUG: &str = "image_asset"',
  'IMAGE_ASSET_TOOL_NAME: &str = "direct.media.generate_image"',
  'register_media_ai_vertical_handlers',
  'normalize_image_size',
  'width > 4096 || height > 4096'
], 'support adapter source');

if (evidence.generated_from !== registryPath || evidence.status !== registry.contract_tests.status) fail('evidence header drift');
sameSet(evidence.cases.map(c => c.operation), registry.contract_tests.cases.map(c => c.operation), 'evidence/registry cases');
sameSet(evidence.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles, 'fallback profiles');
sameSet(evidence.fallback_smoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes, 'degraded modes');
if (fallbackSmoke.generated_from !== registryPath || fallbackSmoke.status !== 'source_smoke_locked') fail('fallback smoke header drift');
if (fallbackSmoke.profile !== registry.contract_tests.fallback_smoke.profiles[0]) fail('fallback smoke profile drift');
if (fallbackSmoke.degraded_mode !== registry.contract_tests.fallback_smoke.degraded_modes[0]) fail('fallback smoke degraded mode drift');
sameSet(fallbackSmoke.cases.map(c => c.operation), registry.contract_tests.cases.map(c => c.operation), 'fallback smoke cases');

const plan = read('crates/rustok-ai-media/docs/implementation-plan.md');
hasAll(plan, ['- FBA status: `in_progress`', 'ai-media-fba-registry.json', 'MediaAssetReadPort', 'ai-media-consumer-static-matrix.json', 'ai-media-runtime-fallback-smoke.json'], 'local plan');
const central = read('docs/modules/registry.md');
hasAll(central, ['| `rustok-ai-media` |', 'crates/rustok-ai-media/contracts/ai-media-fba-registry.json', 'crates/rustok-ai-media/contracts/evidence/ai-media-runtime-fallback-smoke.json'], 'central registry');
const unified = read('docs/research/fluid-backend-architecture-unified-plan.md');
hasAll(unified, ['`ai-media`', 'MediaAssetReadPort', 'ai-media-fba-registry.json', 'ai-media-runtime-fallback-smoke.json'], 'unified plan');

console.log('[verify-ai-media-fba] ai-media FBA media consumer support metadata and static evidence are consistent');
