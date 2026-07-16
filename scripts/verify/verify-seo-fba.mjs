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
const consumerRuntimeOrderSmokePath = 'crates/rustok-seo/contracts/evidence/seo-media-consumer-runtime-order-smoke.json';
const registry = json(registryPath);
const evidence = json(evidencePath);
const provider = json(providerPath);
const providerFallbackSmoke = json(providerFallbackSmokePath);
const consumerRuntimeOrderSmoke = json(consumerRuntimeOrderSmokePath);

if (registry.schema_version !== 1) fail('registry schema_version drift');
if (registry.module !== 'seo' || registry.role !== 'consumer' || registry.status !== 'in_progress') fail('registry identity/status drift');
if (registry.consumer_profile !== 'seo_image_descriptor') fail('consumer profile drift');
const dependency = registry.provider_dependencies?.[0];
if (!dependency) fail('missing media provider dependency');
if (dependency.module !== 'media' || dependency.registry !== providerPath) fail('provider dependency identity drift');
if (dependency.contract_version !== provider.contract_version || dependency.port !== 'MediaAssetReadPort') fail('provider contract/port drift');
const gap = registry.implementation_gap;
if (!gap || gap.status !== 'product_consumer_composed_other_target_providers_pending' || !gap.remaining_work.includes('media_asset_id') || !gap.remaining_work.includes('live provider')) fail('SEO media consumer implementation gap drift');
const seoCargo = read('crates/rustok-seo/Cargo.toml');
if (!seoCargo.includes('rustok-media')) fail('SEO media consumer must depend on rustok-media');
const seoTargets = read('crates/rustok-seo/src/services/targets.rs');
const seoService = read('crates/rustok-seo/src/services/mod.rs');
const serverComposition = read('apps/server/src/services/module_event_dispatcher.rs');
hasAll(seoTargets, ['MediaAssetReadPort', '.get_image_descriptor(', '.with_deadline(Duration::from_secs(2))', 'unwrap_or(image.url)'], 'SEO media consumer');
hasAll(seoService, ['SeoMediaAssetReadProvider', 'with_media_asset_read_port', 'get::<SeoMediaAssetReadProvider>()'], 'SEO media provider injection');
hasAll(serverComposition, ['rustok_media::MediaService::new', 'rustok_seo::SeoMediaAssetReadProvider::new'], 'server media provider composition');
const targets = read('crates/rustok-seo-targets/src/lib.rs');
hasAll(targets, ['pub media_asset_id: Option<Uuid>', 'pub fn with_media_asset_id'], 'SEO target media reference contract');
const productSeoTargets = read('crates/rustok-product/src/seo_targets.rs');
hasAll(productSeoTargets, ['ProductImageResponse', '.with_media_asset_id(image.media_id)'], 'product SEO media reference handoff');
if (provider.module !== 'media' || provider.role !== 'provider' || !['in_progress', 'boundary_ready'].includes(provider.status)) fail('media provider status drift');
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
if (registry.evidence?.consumer_runtime_order_smoke !== consumerRuntimeOrderSmokePath) fail('registry consumer runtime-order smoke path drift');
if (registry.evidence?.consumer_runtime_order_smoke_runner !== consumerRuntimeOrderSmoke.runner) fail('registry consumer runtime-order smoke runner drift');
if (evidence.generated_from !== registryPath || evidence.status !== registry.contract_tests.fallback_smoke.status) fail('evidence header drift');
sameSet(evidence.cases.map(c => c.operation), registry.contract_tests.cases.map(c => c.operation), 'evidence/registry cases');
sameSet(evidence.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles, 'fallback profiles');
sameSet(evidence.fallback_smoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes, 'degraded modes');
if (registry.contract_tests.fallback_smoke.provider_source !== providerFallbackSmokePath) fail('registry fallback smoke provider source drift');
if (evidence.fallback_smoke.provider_source !== providerFallbackSmokePath) fail('evidence fallback smoke provider source drift');
if (consumerRuntimeOrderSmoke.generated_from !== registryPath || consumerRuntimeOrderSmoke.status !== 'executable_no_compile') fail('consumer runtime-order smoke header drift');
if (consumerRuntimeOrderSmoke.provider !== 'media' || consumerRuntimeOrderSmoke.role !== 'consumer') fail('consumer runtime-order smoke identity drift');
if (consumerRuntimeOrderSmoke.fallback_smoke.provider_source !== providerFallbackSmokePath) fail('consumer runtime-order smoke provider source drift');
sameSet(consumerRuntimeOrderSmoke.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles, 'consumer runtime-order fallback profiles');
sameSet(consumerRuntimeOrderSmoke.fallback_smoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes, 'consumer runtime-order degraded modes');
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
const artifactTemplate = evidence.consumer_runtime_artifact_template ?? {};
if (artifactTemplate.status !== 'template_only_pending_consumer_runtime') fail('consumer runtime artifact template status drift');
for (const requiredFile of [
  'image-descriptor-in-process.json',
  'provider-unavailable-omit-image-metadata.json',
  'asset-unavailable-keep-existing-seo-image.json',
  'relative-url-proxy-fallback.json',
  'diagnostics-image-quality-before-after.json',
]) {
  if (!(artifactTemplate.required_files ?? []).includes(requiredFile)) fail(`consumer runtime artifact template misses ${requiredFile}`);
  if (!(registry.contract_tests?.consumer_runtime_artifacts?.required_files ?? []).includes(requiredFile)) fail(`registry consumer runtime artifacts miss ${requiredFile}`);
}
for (const field of ['captured_at', 'provider_contract_version', 'consumer_profile', 'operation', 'profile', 'context', 'input', 'result', 'redactions_applied']) {
  if (!(artifactTemplate.required_top_level_fields ?? []).includes(field)) fail(`consumer runtime artifact template misses field ${field}`);
}
for (const field of ['missing_image_alt', 'missing_image_size', 'seo_targets_checked', 'fallback_images_preserved', 'proxied_relative_urls']) {
  if (!(artifactTemplate.counter_fields ?? []).includes(field)) fail(`consumer runtime artifact template misses counter ${field}`);
}
if (!(artifactTemplate.redaction_policy ?? []).some(rule => rule.includes('auth tokens'))) fail('consumer runtime artifact template misses auth token redaction');
if ((evidence.consumer_runtime_drill_matrix ?? []).length < 3) fail('consumer runtime drill matrix is incomplete');
for (const row of evidence.consumer_runtime_drill_matrix) {
  if (!row.case || !row.operation || !row.profile) fail('consumer runtime drill row misses identity fields');
  if ((row.required_evidence ?? []).length < 3) fail(`consumer runtime drill ${row.case} misses required evidence`);
  if ((row.blocks_closeout_if ?? []).length < 2) fail(`consumer runtime drill ${row.case} misses blockers`);
}

const plan = read('crates/rustok-seo/docs/implementation-plan.md');
hasAll(plan, ['- FBA status: `in_progress`', 'seo-fba-registry.json', 'MediaAssetReadPort', 'media asset UUID'], 'local plan');
const central = read('docs/modules/registry.md');
hasAll(central, ['| `seo` |', 'crates/rustok-seo/contracts/seo-fba-registry.json', '`in_progress` | `in_progress`', 'media asset UUIDs'], 'central registry');
const unified = read('docs/research/fluid-backend-architecture-unified-plan.md');
hasAll(unified, ['`seo`', 'MediaAssetReadPort', 'seo-fba-registry.json', 'media asset UUIDs'], 'unified plan');

console.log('[verify-seo-fba] seo FBA media consumer metadata and static evidence are consistent');
