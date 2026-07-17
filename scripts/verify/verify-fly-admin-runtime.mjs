import './verify-fly-ssr-first.mjs';
import './verify-fly-internal-links.mjs';
import { readFile } from 'node:fs/promises';

const files = {
  flyCodec: 'crates/fly/src/codec.rs',
  flyCommand: 'crates/fly/src/command.rs',
  manifest: 'crates/rustok-page-builder/rustok-module.toml',
  canvasDocument: 'crates/rustok-page-builder/admin/src/editor/canvas_document.rs',
  canvasRuntime: 'crates/rustok-page-builder/admin/src/editor/canvas_runtime.js',
  canvasProtocol: 'crates/rustok-page-builder/admin/src/editor/canvas_protocol.rs',
  facade: 'crates/rustok-page-builder/admin/src/transport/mod.rs',
  controller: 'crates/rustok-page-builder/admin/src/model.rs',
  pagesBuilder: 'crates/rustok-pages/admin/src/builder.rs',
  pagesComposition: 'crates/rustok-pages/admin/src/composition.rs',
  localeEn: 'crates/rustok-page-builder/admin/locales/en.json',
  localeRu: 'crates/rustok-page-builder/admin/locales/ru.json',
};

const source = Object.fromEntries(
  await Promise.all(
    Object.entries(files).map(async ([key, path]) => [key, await readFile(path, 'utf8')]),
  ),
);

const failures = [];
const requireMarker = (key, marker, message) => {
  if (!source[key].includes(marker)) failures.push(message);
};
const forbidMarker = (key, marker, message) => {
  if (source[key].includes(marker)) failures.push(message);
};

for (const marker of [
  'hydrate_page_components_from_frames',
  'canonical_project',
  'synchronize_first_frame',
  'GrapesJsV1Codec::encode_vec',
]) {
  requireMarker('flyCodec', marker, `Fly GrapesJS codec is missing ${marker}`);
}
for (const marker of [
  'GrapesJsV1Codec::encode_vec(document)',
  'pub fn from_bytes(bytes: &[u8])',
]) {
  requireMarker('flyCommand', marker, `Fly canonical hashing is missing ${marker}`);
}
for (const marker of [
  'ui_classification = "admin_only"',
  '[provides.admin_ui]',
  'leptos_crate = "rustok-page-builder-admin"',
  'route_segment = "page-builder"',
  'supported_locales = ["en", "ru"]',
]) {
  requireMarker('manifest', marker, `Page Builder manifest is missing ${marker}`);
}
for (const marker of [
  'Content-Security-Policy',
  'include_str!("canvas_runtime.js")',
  'data-fly-component-id',
  'safe_attribute_name',
  'safe_style',
]) {
  requireMarker('canvasDocument', marker, `instrumented canvas renderer is missing ${marker}`);
}
for (const marker of [
  'getBoundingClientRect',
  'ResizeObserver',
  'geometry_snapshot',
  'focus_requested',
  'hover_requested',
  'zoom: 1',
]) {
  requireMarker('canvasRuntime', marker, `iframe canvas runtime is missing ${marker}`);
}
for (const marker of [
  'CanvasBridgeEnvelope',
  'GeometrySnapshot',
  'decode_canvas_message',
  'last_sequence',
]) {
  requireMarker('canvasProtocol', marker, `canvas protocol is missing ${marker}`);
}
for (const marker of [
  'pub trait PageBuilderAdminFacade: Send + Sync',
  'PageBuilderCapabilityRequest',
  'PageBuilderCapabilityResponse',
]) {
  requireMarker('facade', marker, `admin facade boundary is missing ${marker}`);
}
requireMarker('controller', 'FlyUiStateMachine', 'admin controller must use Fly UI state');
requireMarker('controller', 'FlyEditor', 'admin controller must use Fly engine');
requireMarker('controller', 'PageBuilderCapabilityRequest::Publish', 'controller must emit canonical publish requests');
for (const marker of [
  'impl PageBuilderAdminFacade for PagesBuilderFacade',
  'transport::fetch_page',
  'transport::update_page',
  'REVISION_CONFLICT',
]) {
  requireMarker('pagesBuilder', marker, `Pages builder facade is missing ${marker}`);
}
for (const marker of [
  'PageBuilderAdminHostContext',
  'provide_context',
  'PageBuilderAdmin',
  'PagesBuilderFacade',
]) {
  requireMarker('pagesComposition', marker, `Pages composition is missing ${marker}`);
}
forbidMarker('pagesBuilder', 'graphql_adapter', 'Pages builder facade must not bypass transport facade');
forbidMarker('pagesComposition', 'graphql_adapter', 'Pages composition must not select transport');

const flattenKeys = (value, prefix = '') => Object.entries(value).flatMap(([key, nested]) => {
  const path = prefix ? `${prefix}.${key}` : key;
  return nested && typeof nested === 'object' && !Array.isArray(nested)
    ? flattenKeys(nested, path)
    : [path];
}).sort();
const en = JSON.parse(source.localeEn);
const ru = JSON.parse(source.localeRu);
if (JSON.stringify(flattenKeys(en)) !== JSON.stringify(flattenKeys(ru))) {
  failures.push('Page Builder locale key parity failed');
}

if (failures.length > 0) {
  console.error('Fly admin/browser runtime verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly admin/browser runtime verification passed.');
