import { readFile } from 'node:fs/promises';

const paths = {
  vocabulary: 'crates/fly-browser/src/lib.rs',
  capability: 'crates/rustok-page-builder/admin/src/capability_access.rs',
  adapter: 'crates/rustok-page-builder/admin/src/ui/browser_adapter.rs',
  assets: 'crates/rustok-page-builder/admin/src/editor/ssr_assets.rs',
  forms: 'crates/rustok-page-builder/admin/src/editor/ssr_forms.rs',
  editorMod: 'crates/rustok-page-builder/admin/src/editor/mod.rs',
  canvas: 'crates/rustok-page-builder/admin/src/editor/modular_canvas.rs',
  lib: 'crates/rustok-page-builder/admin/src/lib.rs',
  browserTests: 'crates/rustok-page-builder/admin/src/ssr_assets_browser_tests.rs',
  localeEn: 'crates/rustok-page-builder/admin/locales/en.json',
  localeRu: 'crates/rustok-page-builder/admin/locales/ru.json',
};

const source = Object.fromEntries(
  await Promise.all(
    Object.entries(paths).map(async ([key, path]) => [key, await readFile(path, 'utf8')]),
  ),
);
const failures = [];
const requireMarker = (key, marker, message) => {
  if (!source[key].includes(marker)) failures.push(message);
};
const rejectMarker = (key, marker, message) => {
  if (source[key].includes(marker)) failures.push(message);
};
const requireMarkers = (key, markers, label) => {
  for (const marker of markers) requireMarker(key, marker, `${label} is missing ${marker}`);
};
const flattenKeys = (value, prefix = '') => Object.entries(value).flatMap(([key, nested]) => {
  const path = prefix ? `${prefix}.${key}` : key;
  return nested && typeof nested === 'object' && !Array.isArray(nested)
    ? flattenKeys(nested, path)
    : [path];
}).sort();

requireMarkers('vocabulary', [
  'pub enum BrowserIntentKind',
  'UpsertAsset',
  'RemoveAsset',
  'SelectAsset',
  'Self::UpsertAsset => "upsert_asset"',
  'Self::RemoveAsset => "remove_asset"',
  'Self::SelectAsset => "select_asset"',
  'intent_kind_names_are_unique_and_round_trip',
], 'typed asset intent vocabulary');
requireMarkers('capability', [
  'BrowserIntentKind::UpsertAsset | BrowserIntentKind::RemoveAsset',
  'BrowserIntentKind::SelectAsset =>',
  'vec![EditorCapability::Assets, EditorCapability::Properties]',
  'selecting_an_asset_requires_asset_and_property_capabilities',
], 'asset capability preflight');
rejectMarker(
  'capability',
  'match envelope.intent.as_str()',
  'asset capabilities must use the typed browser intent vocabulary',
);
requireMarkers('adapter', [
  'data-fly-intent-form',
  'delete payload[number.name]',
  'data-fly-selected-component-input',
], 'SSR form adapter');
requireMarkers('assets', [
  'pub struct SsrAssetUpsertRequest',
  'pub struct SsrAssetRemoveRequest',
  'pub struct SsrAssetApplyRequest',
  'pub fn ssr_asset_upsert_intent(',
  'pub fn ssr_asset_remove_intent(',
  'pub fn ssr_asset_apply_intent(',
  'source_allowed(&descriptor.source, descriptor.kind, &AssetPolicy::default())',
  'providerFuture',
  'data-fly-intent-form="upsert_asset"',
  'data-fly-intent-form="select_asset"',
  'data-fly-intent-form="remove_asset"',
  'data-fly-selected-component-input="true"',
  'disabled=!apply_enabled',
  'asset_upsert_preserves_unknown_fields_and_history',
  'asset_apply_uses_explicit_component_and_provider_reference_patch',
  'asset_remove_uses_normal_history',
  'unsafe_asset_source_is_rejected_before_dispatch',
], 'SSR asset authoring module');
requireMarkers('forms', [
  'use super::ssr_assets::{SsrAssetApplyRequest, SsrAssetRemoveRequest, SsrAssetUpsertRequest};',
  '"upsert_asset" => self.ssr_asset_upsert_intent(',
  '"remove_asset" => self.ssr_asset_remove_intent(',
  '"select_asset" => self.ssr_asset_apply_intent(',
], 'SSR asset form routing');
requireMarkers('editorMod', [
  'mod ssr_assets;',
  'SsrAssetApplyRequest, SsrAssetPanel, SsrAssetRemoveRequest, SsrAssetUpsertRequest,',
], 'SSR asset module graph');
requireMarkers('canvas', [
  'SsrAssetPanel',
  'let ssr_assets_runtime = runtime.clone();',
  '<SsrAssetPanel runtime=ssr_assets_runtime />',
], 'SSR asset canvas mount');
requireMarker(
  'lib',
  'mod ssr_assets_browser_tests;',
  'SSR asset browser regression module is not registered',
);
requireMarkers('browserTests', [
  'BrowserIntentKind::UpsertAsset',
  'BrowserIntentKind::SelectAsset',
  'BrowserIntentKind::RemoveAsset',
  'browser_dispatches_asset_upsert_apply_and_remove_contracts',
  'unsafe_asset_source_is_rejected_by_browser_dispatch',
  'stale_asset_mutation_is_rejected_before_dispatch',
  'providerFuture',
  'data-fly-asset-id',
], 'SSR asset browser regressions');
for (const localeKey of ['localeEn', 'localeRu']) {
  requireMarkers(localeKey, [
    '"ssrAssets"',
    '"title"',
    '"description"',
    '"empty"',
    '"name"',
    '"sourceAttribute"',
  ], `${localeKey} SSR asset messages`);
}
const en = JSON.parse(source.localeEn);
const ru = JSON.parse(source.localeRu);
if (JSON.stringify(flattenKeys(en)) !== JSON.stringify(flattenKeys(ru))) {
  failures.push('Page Builder en/ru locale key parity failed after SSR asset wiring');
}

if (failures.length > 0) {
  console.error('Fly SSR asset verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly SSR assets verified.');
